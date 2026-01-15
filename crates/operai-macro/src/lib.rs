// Copyright Operai LLC

//! # Operai Procedural Macros
//!
//! This crate provides procedural macros for defining Operai tools and
//! credentials. It handles code generation for tool registration, lifecycle
//! hooks, and credential management.
//!
//! ## Main Macros
//!
//! - `#[tool]` - Attribute macro for defining tool handlers
//! - `#[init]` - Attribute macro for initialization functions
//! - `#[shutdown]` - Attribute macro for shutdown/cleanup functions
//! - `define_system_credential!` - Macro for defining system-level credentials
//! - `define_user_credential!` - Macro for defining user-level credentials
//!
//! ## Tool Definition
//!
//! Tools are defined using the `#[tool]` attribute with doc comments providing
//! metadata:
//!
//! ```ignore
//! /// # Greet User
//! ///
//! /// Greets a user by name with a friendly message.
//! ///
//! /// ## Capabilities
//! /// - read
//! ///
//! /// ## Tags
//! /// - utility
//! /// - greeting
//! #[tool]
//! async fn greet(ctx: Context, input: GreetInput) -> Result<GreetOutput, Error> {
//!     // ...
//! }
//! ```
//!
//! The macro generates:
//! - A wrapper function for JSON serialization/deserialization
//! - Schema generation for input/output types
//! - Registration with the global tool inventory
//!
//! ## Lifecycle Hooks
//!
//! Init and shutdown functions can be defined to handle setup and cleanup:
//!
//! ```ignore
//! #[init]
//! async fn setup() -> Result<(), Error> {
//!     // Initialize resources
//! }
//!
//! #[shutdown]
//! fn cleanup() {
//!     // Release resources
//! }
//! ```
//!
//! ## Credential Definitions
//!
//! Credentials are defined using the credential macros:
//!
//! ```ignore
//! define_system_credential!(ApiKey("api_key") {
//!     /// API key for authentication
//!     key: String,
//!     /// Optional endpoint override
//!     #[optional]
//!     endpoint: Option<String>,
//! });
//! ```

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Attribute, Error, Expr, FnArg, Ident, ItemFn, Lit, Meta, Result, Type, parse_macro_input,
    spanned::Spanned,
};

/// Metadata extracted from a function's doc comments.
///
/// Used by the `#[tool]` macro to populate tool metadata from structured
/// documentation.
struct DocCommentMetadata {
    /// Tool description (extracted from content after H1 heading)
    description: Option<String>,
    /// Custom tool ID (from H1 heading suffix: `(ID: custom_id)`)
    id: Option<String>,
    /// Display name (from H1 heading)
    name: Option<String>,
    /// List of capabilities (from ## Capabilities section)
    capabilities: Vec<String>,
    /// List of tags (from ## Tags section)
    tags: Vec<String>,
}

/// Extracts tool metadata from a function's doc comments.
///
/// Parses structured doc comments following this format:
/// - `# Tool Name (ID: custom_id)` - H1 heading with optional ID override
/// - Description text following H1
/// - `## Capabilities` section with list items
/// - `## Tags` section with list items
///
/// Returns `None` if no doc comments are present.
fn extract_doc_metadata(attrs: &[Attribute]) -> Option<DocCommentMetadata> {
    let doc_lines: Vec<String> = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .filter_map(|attr| {
            if let Meta::NameValue(nv) = &attr.meta
                && let Expr::Lit(lit) = &nv.value
                && let Lit::Str(s) = &lit.lit
            {
                return Some(s.value().trim().to_string());
            }
            None
        })
        .collect();

    if doc_lines.is_empty() {
        return None;
    }

    let mut metadata = DocCommentMetadata {
        description: None,
        id: None,
        name: None,
        capabilities: Vec::new(),
        tags: Vec::new(),
    };

    let mut current_section: Option<String> = None;
    let mut description_lines = Vec::new();
    let mut h1_parsed = false;

    for line in doc_lines {
        // Check for H1 heading: # Title or # Title (ID: custom-id)
        if let Some(rest) = line.strip_prefix("# ") {
            // If we already parsed the Name/ID H1, a subsequent H1 (like "# Errors")
            // signals the end of the metadata section.
            if h1_parsed {
                break;
            }

            let h1_content = rest.trim();
            h1_parsed = true;

            // Check for (ID: custom-id) suffix
            if let Some(pos) = h1_content.find("(ID:") {
                let name_part = h1_content[..pos].trim().to_string();
                let id_part = h1_content[pos + 4..] // Skip "(ID:"
                    .trim()
                    .strip_suffix(')')
                    .unwrap_or("")
                    .trim()
                    .to_string();

                metadata.name = Some(name_part);
                if !id_part.is_empty() {
                    metadata.id = Some(id_part);
                }
            } else {
                metadata.name = Some(h1_content.to_string());
            }
            continue;
        }

        // Check for H2 heading: ## Section Name
        if let Some(rest) = line.strip_prefix("## ") {
            current_section = Some(rest.trim().to_string());
            continue;
        }

        // Check for list item: - value
        if let Some(rest) = line.strip_prefix('-') {
            let value = rest.trim().to_string();
            let mut handled = false;
            match current_section.as_deref() {
                Some("Capabilities") => {
                    metadata.capabilities.push(value);
                    handled = true;
                }
                Some("Tags") => {
                    metadata.tags.push(value);
                    handled = true;
                }
                _ => {}
            }
            if handled {
                continue;
            }
        }

        // Accumulate description (after H1, before any H2)
        if h1_parsed && current_section.is_none() && !line.is_empty() {
            description_lines.push(line);
        }
    }

    metadata.description = if description_lines.is_empty() {
        None
    } else {
        Some(description_lines.join("\n\n"))
    };

    Some(metadata)
}

/// Attribute macro for defining tool handler functions.
///
/// This macro generates the necessary boilerplate for tool registration and
/// invocation. The annotated function must:
/// - Be `async`
/// - Take exactly 2 parameters: `(ctx: Context, input: Input)`
/// - Return `Result<Output, Error>`
///
/// Tool metadata is extracted from structured doc comments:
/// - `# Tool Name (ID: custom_id)` - H1 heading with optional ID override
/// - Description text after H1
/// - `## Capabilities` section with `-` list items
/// - `## Tags` section with `-` list items
///
/// # Errors
///
/// Returns a compile error if:
/// - Attributes are provided (use doc comments instead)
/// - Doc comments are missing or malformed
/// - Function signature requirements aren't met
#[proc_macro_attribute]
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Reject any attributes - doc comments only
    if !attr.is_empty() {
        return Error::new(
            proc_macro2::Span::call_site(),
            "#[tool] no longer accepts attributes. Use doc comments instead:\n/// # Tool \
             Name\n/// Description here.\n/// ## Capabilities\n/// - read\n#[tool]\nasync fn \
             my_tool(...)",
        )
        .to_compile_error()
        .into();
    }

    let func = parse_macro_input!(item as ItemFn);

    match expand_tool(&func) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// Expands a `#[tool]` attribute into generated code.
///
/// This function performs validation and code generation for tool functions:
/// 1. Extracts and validates metadata from doc comments
/// 2. Validates function signature (async, 2 args, Result return)
/// 3. Generates a wrapper function for JSON serialization
/// 4. Submits the tool to the global inventory
///
/// # Errors
///
/// Returns an error if:
/// - Doc metadata is missing required fields
/// - Function is not async
/// - Function doesn't have exactly 2 parameters
/// - Return type is not `Result<T, E>`
fn expand_tool(func: &ItemFn) -> Result<proc_macro2::TokenStream> {
    let func_name = &func.sig.ident;

    let mut metadata = extract_doc_metadata(&func.attrs).ok_or_else(|| {
        Error::new(
            func.sig.ident.span(),
            "tool must have doc comments used for metadata",
        )
    })?;

    let func_name_str = func_name.to_string();

    // If the extracted ID matches the function name, remove it (treat as default)
    if let Some(id) = &metadata.id
        && id == &func_name_str
    {
        metadata.id = None;
    }

    let tool_id = metadata.id.unwrap_or_else(|| func_name_str.clone());

    // Require that the first header (tool name) is present
    let display_name = metadata.name.ok_or_else(|| {
        Error::new(
            func.sig.ident.span(),
            "missing tool name in doc comments (must be the first H1 heading)",
        )
    })?;

    let description = metadata.description.ok_or_else(|| {
        Error::new(
            func.sig.ident.span(),
            "missing tool description in doc comments (must follow H1 heading)",
        )
    })?;

    let capabilities = &metadata.capabilities;
    let tags = &metadata.tags;
    let sig = &func.sig;

    if sig.asyncness.is_none() {
        return Err(Error::new(sig.fn_token.span, "tool handler must be async"));
    }

    let args: Vec<_> = sig.inputs.iter().collect();
    if args.len() != 2 {
        return Err(Error::new(
            sig.inputs.span(),
            "tool handler must have exactly 2 arguments: (ctx: Context, input: Input)",
        ));
    }

    let input_type = match &args[1] {
        FnArg::Typed(pat_type) => &pat_type.ty,
        FnArg::Receiver(_) => {
            return Err(Error::new(
                args[1].span(),
                "expected typed argument for input",
            ));
        }
    };

    let output_type = match &sig.output {
        syn::ReturnType::Type(_, ty) => extract_result_ok_type(ty)?,
        syn::ReturnType::Default => {
            return Err(Error::new(
                sig.output.span(),
                "tool handler must return Result<Output, ...>",
            ));
        }
    };

    let wrapper_ident = format_ident!("__operai_wrapper_{}", func_name);

    let expanded = quote! {
        #func

        #[doc(hidden)]
        pub fn #wrapper_ident(
            ctx: ::operai::Context,
            input_json: ::std::vec::Vec<u8>,
        ) -> ::std::pin::Pin<::std::boxed::Box<dyn ::std::future::Future<Output = ::operai::__private::anyhow::Result<::std::vec::Vec<u8>>> + ::std::marker::Send + 'static>> {
            ::std::boxed::Box::pin(async move {
                let input: #input_type = ::operai::__private::serde_json::from_slice(&input_json)?;
                let output = #func_name(ctx, input).await?;
                let output_json = ::operai::__private::serde_json::to_vec(&output)?;
                Ok(output_json)
            })
        }

        // Sealed token prevents external construction of ToolEntry
        ::operai::__private::inventory::submit! {
            ::operai::__private::ToolEntry {
                id: #tool_id,
                name: #display_name,
                description: #description,
                capabilities: &[#(#capabilities),*],
                tags: &[#(#tags),*],
                input_schema_fn: || {
                    let schema = ::operai::__private::schemars::schema_for!(#input_type);
                    ::operai::__private::serde_json::to_string(&schema)
                        .expect("schema serialization should never fail")
                },
                output_schema_fn: || {
                    let schema = ::operai::__private::schemars::schema_for!(#output_type);
                    ::operai::__private::serde_json::to_string(&schema)
                        .expect("schema serialization should never fail")
                },
                handler: #wrapper_ident,
                __sealed: ::operai::__private::sealed(),
            }
        }
    };

    Ok(expanded)
}

/// Attribute macro for defining initialization functions.
///
/// Annotates an async function that runs once when the tool library is loaded.
/// The function must:
/// - Be `async`
/// - Take no parameters
///
/// The macro generates a wrapper function and submits it to the init inventory.
#[proc_macro_attribute]
pub fn init(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);

    match expand_init(&func) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// Expands a `#[init]` attribute into generated code.
///
/// Validates that the function is async and takes no parameters,
/// then generates a wrapper and submits it to the init inventory.
///
/// # Errors
///
/// Returns an error if:
/// - Function is not async
/// - Function takes parameters
fn expand_init(func: &ItemFn) -> Result<proc_macro2::TokenStream> {
    let func_name = &func.sig.ident;
    let func_name_str = func_name.to_string();
    let sig = &func.sig;

    if sig.asyncness.is_none() {
        return Err(Error::new(sig.fn_token.span, "init function must be async"));
    }

    if !sig.inputs.is_empty() {
        return Err(Error::new(
            sig.inputs.span(),
            "init function must have no parameters",
        ));
    }

    let wrapper_ident = format_ident!("__brwse_init_wrapper_{}", func_name);

    let expanded = quote! {
        #func

        #[doc(hidden)]
        pub fn #wrapper_ident(
        ) -> ::std::pin::Pin<::std::boxed::Box<dyn ::std::future::Future<Output = ::operai::__private::anyhow::Result<()>> + ::std::marker::Send + 'static>> {
            ::std::boxed::Box::pin(async move {
                #func_name().await
            })
        }

        ::operai::__private::inventory::submit! {
            ::operai::__private::InitEntry {
                name: #func_name_str,
                handler: #wrapper_ident,
                __sealed: ::operai::__private::sealed(),
            }
        }
    };

    Ok(expanded)
}

/// Attribute macro for defining shutdown/cleanup functions.
///
/// Annotates a synchronous function that runs when the tool library is
/// unloaded. The function must:
/// - Be synchronous (not `async`)
/// - Take no parameters
///
/// The macro submits the function to the shutdown inventory.
#[proc_macro_attribute]
pub fn shutdown(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);

    match expand_shutdown(&func) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// Expands a `#[shutdown]` attribute into generated code.
///
/// Validates that the function is synchronous and takes no parameters,
/// then submits it to the shutdown inventory.
///
/// # Errors
///
/// Returns an error if:
/// - Function is async
/// - Function takes parameters
fn expand_shutdown(func: &ItemFn) -> Result<proc_macro2::TokenStream> {
    let func_name = &func.sig.ident;
    let func_name_str = func_name.to_string();
    let sig = &func.sig;

    if sig.asyncness.is_some() {
        return Err(Error::new(
            sig.fn_token.span,
            "shutdown function must be synchronous (not async)",
        ));
    }

    if !sig.inputs.is_empty() {
        return Err(Error::new(
            sig.inputs.span(),
            "shutdown function must have no parameters",
        ));
    }

    let expanded = quote! {
        #func

        ::operai::__private::inventory::submit! {
            ::operai::__private::ShutdownEntry {
                name: #func_name_str,
                handler: #func_name,
                __sealed: ::operai::__private::sealed(),
            }
        }
    };

    Ok(expanded)
}

/// Extracts the `T` from a `Result<T, E>` type.
///
/// Used to determine the output type of tool functions from their return type.
///
/// # Errors
///
/// Returns an error if:
/// - Type is not a `Result`
/// - `Result` has no type arguments
/// - First type argument is not a type
fn extract_result_ok_type(ty: &Type) -> Result<&Type> {
    match ty {
        Type::Path(type_path) => {
            let segment = type_path
                .path
                .segments
                .last()
                .ok_or_else(|| Error::new(ty.span(), "expected Result<T, E> return type"))?;

            if segment.ident != "Result" {
                return Err(Error::new(ty.span(), "expected Result<T, E> return type"));
            }

            match &segment.arguments {
                syn::PathArguments::AngleBracketed(args) => {
                    let first_arg = args
                        .args
                        .first()
                        .ok_or_else(|| Error::new(ty.span(), "Result must have type arguments"))?;

                    match first_arg {
                        syn::GenericArgument::Type(t) => Ok(t),
                        _ => Err(Error::new(ty.span(), "expected type argument")),
                    }
                }
                _ => Err(Error::new(
                    ty.span(),
                    "expected Result<T, E> with type arguments",
                )),
            }
        }
        _ => Err(Error::new(ty.span(), "expected Result<T, E> return type")),
    }
}

/// Defines a system-level credential type.
///
/// Syntax:
/// ```ignore
/// define_system_credential!(StructName("credential_name") {
///     /// Field description
///     field_name: Type,
///     /// Optional field description
///     #[optional]
///     optional_field: Option<Type>,
/// });
/// ```
///
/// This macro generates:
/// - A struct with the provided fields
/// - A `get(ctx: &Context)` method for retrieving the credential
/// - Registration with the credential inventory
#[proc_macro]
pub fn define_system_credential(input: TokenStream) -> TokenStream {
    let cred = parse_macro_input!(input as CredentialDef);
    expand_credential(&cred, CredentialKind::System).into()
}

/// Defines a user-level credential type.
///
/// Syntax is identical to `define_system_credential!`, but the credential
/// is retrieved from the user credential store instead of the system store.
#[proc_macro]
pub fn define_user_credential(input: TokenStream) -> TokenStream {
    let cred = parse_macro_input!(input as CredentialDef);
    expand_credential(&cred, CredentialKind::User).into()
}

/// Indicates whether a credential is system or user-scoped.
#[derive(Clone, Copy)]
enum CredentialKind {
    System,
    User,
}

/// Parsed credential definition from the macro input.
struct CredentialDef {
    /// Name of the generated struct
    struct_name: Ident,
    /// Credential identifier (used for lookup via Context)
    cred_name: String,
    /// Fields in the credential struct
    fields: Vec<CredentialField>,
}

/// A single field in a credential definition.
struct CredentialField {
    /// Field name
    name: Ident,
    /// Field type
    ty: Type,
    /// Whether the field is marked with `#[optional]`
    optional: bool,
    /// Description from doc comments
    description: Option<String>,
}

impl syn::parse::Parse for CredentialDef {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        // Parse: StructName("cred_name") { fields... }
        let struct_name: Ident = input.parse()?;

        let content;
        syn::parenthesized!(content in input);
        let cred_name: syn::LitStr = content.parse()?;

        let fields_content;
        syn::braced!(fields_content in input);

        let mut fields = Vec::new();
        while !fields_content.is_empty() {
            let attrs: Vec<Attribute> = fields_content.call(Attribute::parse_outer)?;
            let mut optional = false;
            let mut description = None;

            for attr in attrs {
                if attr.path().is_ident("optional") {
                    optional = true;
                } else if attr.path().is_ident("doc")
                    && let Meta::NameValue(nv) = &attr.meta
                    && let Expr::Lit(lit) = &nv.value
                    && let Lit::Str(s) = &lit.lit
                {
                    description = Some(s.value().trim().to_string());
                }
            }

            let name: Ident = fields_content.parse()?;
            fields_content.parse::<syn::Token![:]>()?;
            let ty: Type = fields_content.parse()?;

            if fields_content.peek(syn::Token![,]) {
                fields_content.parse::<syn::Token![,]>()?;
            }

            fields.push(CredentialField {
                name,
                ty,
                optional,
                description,
            });
        }

        Ok(Self {
            struct_name,
            cred_name: cred_name.value(),
            fields,
        })
    }
}

/// Expands a credential definition into generated code.
///
/// Generates:
/// - A struct with the provided fields
/// - An impl with a `get(ctx: &Context)` method
/// - Schema functions for validation
/// - Inventory submission for registration
fn expand_credential(def: &CredentialDef, kind: CredentialKind) -> proc_macro2::TokenStream {
    let struct_name = &def.struct_name;
    let cred_name = &def.cred_name;

    let field_names: Vec<_> = def.fields.iter().map(|f| &f.name).collect();
    let field_types: Vec<_> = def.fields.iter().map(|f| &f.ty).collect();

    let field_schema_entries: Vec<_> = def
        .fields
        .iter()
        .map(|f| {
            let name_str = f.name.to_string();
            let required = !f.optional;
            let desc = f.description.as_deref().unwrap_or("");
            quote! {
                (#name_str, ::operai::__private::CredentialFieldSchema {
                    description: #desc,
                    required: #required,
                })
            }
        })
        .collect();

    let getter_fn = match kind {
        CredentialKind::System => quote! { system_credential },
        CredentialKind::User => quote! { user_credential },
    };

    let description = match kind {
        CredentialKind::System => format!("System credential: {cred_name}"),
        CredentialKind::User => format!("User credential: {cred_name}"),
    };

    quote! {
        #[derive(Debug, Clone, ::operai::__private::serde::Deserialize)]
        pub struct #struct_name {
            #(pub #field_names: #field_types,)*
        }

        impl #struct_name {
            pub fn get(ctx: &::operai::Context) -> ::std::result::Result<Self, ::operai::CredentialError> {
                ctx.#getter_fn(#cred_name)
            }
        }

        // Sealed token prevents external construction of CredentialEntry
        ::operai::__private::inventory::submit! {
            ::operai::__private::CredentialEntry {
                name: #cred_name,
                description: #description,
                fields: &[#(#field_schema_entries,)*],
                __sealed: ::operai::__private::sealed(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::*;

    /// Removes all whitespace for easier content comparison in tests.
    fn strip_whitespace(input: &str) -> String {
        input.chars().filter(|c| !c.is_whitespace()).collect()
    }

    /// Helper to parse a function from tokens for testing.
    fn parse_item_fn(tokens: proc_macro2::TokenStream) -> ItemFn {
        syn::parse2(tokens).expect("failed to parse ItemFn")
    }

    /// Helper to parse a credential definition from tokens for testing.
    fn parse_credential_def(tokens: proc_macro2::TokenStream) -> CredentialDef {
        syn::parse2(tokens).expect("failed to parse CredentialDef")
    }

    #[test]
    fn test_extract_doc_metadata_full() {
        let input = quote! {
            /// # Custom Name (ID: custom_id)
            ///
            /// Tool description here.
            ///
            /// More details about the tool.
            ///
            /// ## Capabilities
            /// - read
            /// - write
            ///
            /// ## Tags
            /// - example
            /// - test
        };

        let _item: ItemFn = syn::parse2(quote! { fn f() {} }).unwrap(); // Just to get Attribute type easily? No wait, extract_doc_metadata takes &[Attribute].
        // But I need to parse the doc comments into Attributes.
        // Let's attach them to a function.
        let input_fn = quote! {
            #input
            fn f() {}
        };
        let item: ItemFn = syn::parse2(input_fn).unwrap();
        let metadata = extract_doc_metadata(&item.attrs).unwrap();

        assert_eq!(metadata.name.as_deref(), Some("Custom Name"));
        assert_eq!(metadata.id.as_deref(), Some("custom_id"));
        assert_eq!(
            metadata.description.as_deref(),
            Some("Tool description here.\n\nMore details about the tool.")
        );
        assert_eq!(metadata.capabilities, vec!["read", "write"]);
        assert_eq!(metadata.tags, vec!["example", "test"]);
    }

    #[test]
    fn test_extract_doc_metadata_without_id() {
        let input_fn = quote! {
            /// # My Tool
            ///
            /// Tool description.
            ///
            /// ## Capabilities
            /// - read
            fn f() {}
        };

        let item: ItemFn = syn::parse2(input_fn).unwrap();
        let metadata = extract_doc_metadata(&item.attrs).unwrap();

        assert_eq!(metadata.name.as_deref(), Some("My Tool"));
        assert!(metadata.id.is_none()); // Will default to function name later
        assert_eq!(metadata.description.as_deref(), Some("Tool description."));
        assert_eq!(metadata.capabilities, vec!["read"]);
    }

    #[test]
    fn test_expand_tool_with_missing_description_returns_error() {
        // Arrange
        let func = parse_item_fn(quote!(
            // No doc comments
            async fn greet(ctx: Context, input: Input) -> Result<Output, Error> {}
        ));

        // Act
        let err = expand_tool(&func).expect_err("expected description error");

        // Assert
        // The error might be "tool must have doc comments used for metadata" or
        // "missing tool description..." In this case `extract_doc_metadata`
        // returns None, so "tool must have doc comments..."
        assert_eq!(
            err.to_string(),
            "tool must have doc comments used for metadata"
        );
    }

    #[test]
    fn test_expand_tool_requires_async_function() {
        // Arrange
        let func = parse_item_fn(quote!(
            /// # Greet
            /// Description.
            fn greet(ctx: Context, input: Input) -> Result<Output, Error> {}
        ));

        // Act
        let err = expand_tool(&func).expect_err("expected asyncness error");

        // Assert
        assert_eq!(err.to_string(), "tool handler must be async");
    }

    #[test]
    fn test_expand_tool_requires_exactly_two_arguments() {
        // Arrange
        let func = parse_item_fn(quote!(
            /// # Greet
            /// Description.
            async fn greet(ctx: Context) -> Result<Output, Error> {}
        ));

        // Act
        let err = expand_tool(&func).expect_err("expected argument-count error");

        // Assert
        assert_eq!(
            err.to_string(),
            "tool handler must have exactly 2 arguments: (ctx: Context, input: Input)"
        );
    }

    #[test]
    fn test_expand_tool_requires_result_return_type() {
        // Arrange
        let func = parse_item_fn(quote!(
            /// # Greet
            /// Description.
            async fn greet(ctx: Context, input: Input) -> Output {}
        ));

        // Act
        let err = expand_tool(&func).expect_err("expected Result return type error");

        // Assert
        assert_eq!(err.to_string(), "expected Result<T, E> return type");
    }

    #[test]
    fn test_expand_tool_happy_path_includes_defaults_and_metadata() {
        // Arrange
        let func = parse_item_fn(quote!(
            /// # Greet Tool
            ///
            /// This is a description.
            ///
            /// ## Capabilities
            /// - read
            ///
            /// ## Tags
            /// - tag1
            /// - tag2
            async fn greet(ctx: Context, input: Input) -> Result<Output, Error> {}
        ));

        // Act
        let expanded = expand_tool(&func).expect("expected expansion to succeed");

        // Assert
        let expanded = strip_whitespace(&expanded.to_string());
        assert!(expanded.contains("pubfn__operai_wrapper_greet"));
        assert!(expanded.contains("ToolEntry"));
        assert!(expanded.contains("id:\"greet\"")); // Defaulted from function name
        assert!(expanded.contains("name:\"GreetTool\""));
        assert!(expanded.contains("description:\"Thisisadescription.\""));
        assert!(expanded.contains("capabilities:&[\"read\"]"));
        assert!(expanded.contains("tags:&[\"tag1\",\"tag2\"]"));
    }

    #[test]
    fn test_expand_init_requires_async_function() {
        // Arrange
        let func = parse_item_fn(quote!(
            fn setup() -> Result<(), Error> {}
        ));

        // Act
        let err = expand_init(&func).expect_err("expected asyncness error");

        // Assert
        assert_eq!(err.to_string(), "init function must be async");
    }

    #[test]
    fn test_expand_init_requires_no_parameters() {
        // Arrange
        let func = parse_item_fn(quote!(
            async fn setup(ctx: Context) -> Result<(), Error> {}
        ));

        // Act
        let err = expand_init(&func).expect_err("expected parameter error");

        // Assert
        assert_eq!(err.to_string(), "init function must have no parameters");
    }

    #[test]
    fn test_expand_init_happy_path_registers_init_entry_and_wrapper() {
        // Arrange
        let func = parse_item_fn(quote!(
            async fn setup() -> Result<(), Error> {}
        ));

        // Act
        let expanded = expand_init(&func).expect("expected init expansion to succeed");

        // Assert
        let expanded = strip_whitespace(&expanded.to_string());
        assert!(expanded.contains("pubfn__brwse_init_wrapper_setup"));
        assert!(expanded.contains("InitEntry"));
        assert!(expanded.contains("name:\"setup\""));
    }

    #[test]
    fn test_expand_shutdown_rejects_async_functions() {
        // Arrange
        let func = parse_item_fn(quote!(
            async fn cleanup() {}
        ));

        // Act
        let err = expand_shutdown(&func).expect_err("expected async rejection");

        // Assert
        assert_eq!(
            err.to_string(),
            "shutdown function must be synchronous (not async)"
        );
    }

    #[test]
    fn test_expand_shutdown_requires_no_parameters() {
        // Arrange
        let func = parse_item_fn(quote!(
            fn cleanup(ctx: Context) {}
        ));

        // Act
        let err = expand_shutdown(&func).expect_err("expected parameter error");

        // Assert
        assert_eq!(err.to_string(), "shutdown function must have no parameters");
    }

    #[test]
    fn test_expand_shutdown_happy_path_registers_shutdown_entry() {
        // Arrange
        let func = parse_item_fn(quote!(
            fn cleanup() {}
        ));

        // Act
        let expanded = expand_shutdown(&func).expect("expected shutdown expansion to succeed");

        // Assert
        let expanded = strip_whitespace(&expanded.to_string());
        assert!(expanded.contains("ShutdownEntry"));
        assert!(expanded.contains("name:\"cleanup\""));
        assert!(expanded.contains("handler:cleanup"));
    }

    #[test]
    fn test_credential_def_parses_optional_and_doc_attributes() {
        // Arrange
        let tokens = quote!(
            ApiCredential("api") {
                /// API key used for auth.
                api_key: String,
                /// Optional endpoint override.
                #[optional]
                endpoint: Option<String>,
            }
        );

        // Act
        let def = parse_credential_def(tokens);

        // Assert
        assert_eq!(def.struct_name.to_string(), "ApiCredential");
        assert_eq!(def.cred_name, "api");
        assert_eq!(def.fields.len(), 2);

        let api_key = &def.fields[0];
        assert_eq!(api_key.name.to_string(), "api_key");
        assert!(!api_key.optional);
        assert_eq!(
            api_key.description.as_deref(),
            Some("API key used for auth.")
        );

        let endpoint = &def.fields[1];
        assert_eq!(endpoint.name.to_string(), "endpoint");
        assert!(endpoint.optional);
        assert_eq!(
            endpoint.description.as_deref(),
            Some("Optional endpoint override.")
        );
    }

    #[test]
    fn test_expand_system_credential_includes_entry_and_system_getter() {
        // Arrange
        let def = parse_credential_def(quote!(
            ApiCredential("api") {
                /// API key used for auth.
                api_key: String,
                /// Optional endpoint override.
                #[optional]
                endpoint: Option<String>,
            }
        ));

        // Act
        let expanded = expand_credential(&def, CredentialKind::System);

        // Assert
        let expanded = strip_whitespace(&expanded.to_string());
        assert!(expanded.contains("CredentialEntry"));
        assert!(expanded.contains("name:\"api\""));
        assert!(expanded.contains("Systemcredential:api"));
        assert!(expanded.contains("ctx.system_credential(\"api\")"));
        assert!(expanded.contains("required:true"));
        assert!(expanded.contains("required:false"));
        assert!(expanded.contains("description:\"APIkeyusedforauth.\""));
        assert!(expanded.contains("description:\"Optionalendpointoverride.\""));
    }

    #[test]
    fn test_expand_user_credential_includes_entry_and_user_getter() {
        // Arrange
        let def = parse_credential_def(quote!(
            UserApiKey("user_api") {
                token: String,
            }
        ));

        // Act
        let expanded = expand_credential(&def, CredentialKind::User);

        // Assert
        let expanded = strip_whitespace(&expanded.to_string());
        assert!(expanded.contains("CredentialEntry"));
        assert!(expanded.contains("name:\"user_api\""));
        assert!(expanded.contains("Usercredential:user_api"));
        assert!(expanded.contains("ctx.user_credential(\"user_api\")"));
        assert!(expanded.contains("required:true"));
    }

    #[test]
    fn test_expand_tool_with_explicit_id_and_name_uses_overrides() {
        // Arrange
        let func = parse_item_fn(quote!(
            /// # Custom Name (ID: custom_id)
            /// desc
            async fn greet(ctx: Context, input: Input) -> Result<Output, Error> {}
        ));

        // Act
        let expanded = expand_tool(&func).expect("expected expansion to succeed");

        // Assert
        let expanded = strip_whitespace(&expanded.to_string());
        assert!(expanded.contains("id:\"custom_id\""));
        assert!(expanded.contains("name:\"CustomName\""));
        // The wrapper still uses the function name
        assert!(expanded.contains("pubfn__operai_wrapper_greet"));
    }

    #[test]
    fn test_expand_tool_rejects_no_return_type() {
        // Arrange
        let func = parse_item_fn(quote!(
            /// # Greet
            /// desc
            async fn greet(ctx: Context, input: Input) {}
        ));

        // Act
        let err = expand_tool(&func).expect_err("expected return type error");

        // Assert
        assert_eq!(
            err.to_string(),
            "tool handler must return Result<Output, ...>"
        );
    }

    #[test]
    fn test_expand_tool_rejects_too_many_arguments() {
        // Arrange
        let func = parse_item_fn(quote!(
            /// # Greet
            /// desc
            async fn greet(ctx: Context, input: Input, extra: Extra) -> Result<Output, Error> {}
        ));

        // Act
        let err = expand_tool(&func).expect_err("expected argument-count error");

        // Assert
        assert_eq!(
            err.to_string(),
            "tool handler must have exactly 2 arguments: (ctx: Context, input: Input)"
        );
    }

    #[test]
    fn test_extract_result_ok_type_rejects_non_result_type() {
        // Arrange
        let ty: Type = syn::parse2(quote!(Option<T>)).expect("failed to parse type");

        // Act
        let err = extract_result_ok_type(&ty).expect_err("expected Result type error");

        // Assert
        assert_eq!(err.to_string(), "expected Result<T, E> return type");
    }

    #[test]
    fn test_extract_result_ok_type_rejects_result_without_type_arguments() {
        // Arrange
        let ty: Type = syn::parse2(quote!(Result)).expect("failed to parse type");

        // Act
        let err = extract_result_ok_type(&ty).expect_err("expected type arguments error");

        // Assert
        assert_eq!(err.to_string(), "expected Result<T, E> with type arguments");
    }

    #[test]
    fn test_credential_with_no_fields_produces_empty_struct() {
        // Arrange
        let def = parse_credential_def(quote!(
            EmptyCredential("empty") {}
        ));

        // Act
        let expanded = expand_credential(&def, CredentialKind::System);

        // Assert
        let expanded = strip_whitespace(&expanded.to_string());
        assert!(expanded.contains("structEmptyCredential{}"));
        assert!(expanded.contains("fields:&[]"));
    }

    #[test]
    fn test_tool_attrs_with_only_description() {
        // Arrange
        let func = parse_item_fn(quote!(
            /// # Tool
            /// minimal tool
            async fn f() {}
        ));

        // Act
        let metadata = extract_doc_metadata(&func.attrs).unwrap();

        // Assert
        assert_eq!(metadata.description.as_deref(), Some("minimal tool"));
        assert!(metadata.capabilities.is_empty());
        assert!(metadata.tags.is_empty());
    }

    #[test]
    fn test_credential_field_without_description() {
        // Arrange
        let def = parse_credential_def(quote!(
            SimpleCredential("simple") {
                token: String,
            }
        ));

        // Act & Assert
        assert_eq!(def.fields.len(), 1);
        assert!(def.fields[0].description.is_none());
        assert!(!def.fields[0].optional);
    }

    #[test]
    fn test_extract_result_ok_type_extracts_type_from_valid_result() {
        // Arrange
        let ty: Type = syn::parse2(quote!(Result<Output, Error>)).expect("failed to parse type");

        // Act
        let ok_type = extract_result_ok_type(&ty).expect("expected Ok type extraction to succeed");

        // Assert
        let ok_type_str = quote!(#ok_type).to_string();
        assert_eq!(ok_type_str, "Output");
    }
}
