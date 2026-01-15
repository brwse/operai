//! Tool registry and runtime for dynamically loaded tool libraries.
//!
//! This module provides the core infrastructure for managing tool lifecycles,
//! including dynamic loading from shared libraries, invocation, and semantic
//! search via embeddings.
//!
//! # Architecture
//!
//! The tool system is built around three main types:
//!
//! - [`ToolRegistry`]: Central registry managing all loaded tools
//! - [`ToolHandle`]: Runtime handle for invoking a specific tool
//! - [`ToolInfo`]: Immutable metadata describing a tool's capabilities
//!
//! # Tool Loading
//!
//! Tools are loaded from dynamic libraries (`.so`, `.dylib`, `.dll`) that
//! implement the Operai ABI defined in `operai_abi`. Each library can export
//! multiple tools, identified by qualified IDs like `crate-name.tool-name`.
//!
//! # Thread Safety
//!
//! ## Loading Phase (Not Thread-Safe)
//!
//! During tool loading, the registry requires exclusive mutable access:
//! - [`ToolRegistry::load_library`] takes `&mut self` and cannot be called
//!   concurrently
//! - Load all tools before wrapping the registry in [`Arc`] for concurrent
//!   access
//!
//! ## Execution Phase (Thread-Safe)
//!
//! Once tools are loaded and the registry is wrapped in [`Arc`]:
//! - [`ToolRegistry::get`], [`ToolRegistry::list`], and
//!   [`ToolRegistry::search`] can be called concurrently
//! - Tool handles are wrapped in [`Arc`] for safe sharing across threads
//! - Tool invocations via [`ToolHandle::call`] may occur concurrently on
//!   different threads
//! - The in-flight request counter uses atomic operations for thread-safe
//!   tracking
//!
//! # Semantic Search
//!
//! Tools can include embeddings for semantic search. The registry provides
//! [`ToolRegistry::search`] to find tools by cosine similarity between
//! embeddings.

use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use abi_stable::std_types::{RSlice, RStr};
use async_ffi::FfiFuture;
use operai_abi::{
    CallArgs, CallContext, CallResult, InitArgs, RuntimeContext, TOOL_ABI_VERSION, ToolModuleRef,
    ToolResult,
};
use rkyv::rancor::BoxedError;
use tracing::{info, instrument};

use crate::loader::{LoadError, ToolLibrary};

/// Errors that can occur during tool registry operations.
///
/// This enum represents failures during tool loading, registration, or
/// invocation. It uses `#[non_exhaustive]` to allow adding new error variants
/// without breaking existing code.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RegistryError {
    /// A tool with the given qualified ID was not found in the registry.
    #[error("tool not found: {0}")]
    NotFound(String),

    /// Failed to load a tool library from disk.
    #[error("failed to load library: {0}")]
    LoadError(#[from] LoadError),

    /// Attempted to register a tool with a qualified ID that already exists.
    ///
    /// Each tool must have a unique qualified ID (format:
    /// `crate-name.tool-name`).
    #[error("duplicate tool ID: {0}")]
    DuplicateId(String),

    /// Tool invocation failed at runtime.
    #[error("tool invocation failed: {0}")]
    InvocationError(String),
}

/// Immutable metadata describing a tool's interface and capabilities.
///
/// `ToolInfo` provides a complete description of a tool, including its schemas,
/// capabilities, and optional semantic embedding for search. This information
/// is extracted from the tool's ABI descriptor during registration.
///
/// # Fields
///
/// - `qualified_id`: Full identifier including crate name (e.g.,
///   `crate-name.tool-name`)
/// - `tool_id`: Tool identifier within its crate (e.g., `tool-name`)
/// - `crate_name`: Name of the crate/library providing this tool
/// - `crate_version`: Version of the crate/library
/// - `display_name`: Human-readable name for the tool
/// - `description`: Detailed description of what the tool does
/// - `input_schema`: JSON Schema describing valid inputs
/// - `output_schema`: JSON Schema describing output format
/// - `credential_schema`: Optional schema for required credentials
/// - `capabilities`: List of capability identifiers (e.g., `["stream",
///   "async"]`)
/// - `tags`: Optional tags for categorization and search
/// - `embedding`: Optional vector embedding for semantic search
#[derive(Debug, Clone)]
pub struct ToolInfo {
    /// Full qualified identifier (format: `crate-name.tool-name`)
    pub qualified_id: String,
    /// Tool identifier within its crate
    pub tool_id: String,
    /// Name of the crate/library providing this tool
    pub crate_name: String,
    /// Version of the crate/library
    pub crate_version: String,
    /// Human-readable display name
    pub display_name: String,
    /// Detailed description of the tool's purpose
    pub description: String,
    /// JSON Schema for input validation
    pub input_schema: String,
    /// JSON Schema describing output format
    pub output_schema: String,
    /// Optional JSON Schema for required credentials
    pub credential_schema: Option<String>,
    /// List of capability identifiers
    pub capabilities: Vec<String>,
    /// Optional tags for categorization
    pub tags: Vec<String>,
    /// Optional vector embedding for semantic search
    pub embedding: Option<Vec<f32>>,
}

/// Runtime handle for invoking a specific tool.
///
/// `ToolHandle` provides methods to invoke tools and query their metadata.
/// Handles are typically accessed via [`Arc`] to enable concurrent use across
/// multiple threads.
///
/// # Invocation
///
/// Use [`ToolHandle::call`] to invoke the tool with serialized input bytes.
/// The call is asynchronous and returns an FFI-compatible future.
pub struct ToolHandle {
    /// Tool metadata and interface description
    info: ToolInfo,
    /// Reference to the loaded tool module
    module: ToolModuleRef,
    /// Serialized system credentials (rkyv-encoded)
    pub system_credentials: Vec<u8>,
    /// Tool identifier (unqualified)
    tool_id: String,
}

impl ToolHandle {
    /// Returns a reference to this tool's metadata.
    #[must_use]
    pub fn info(&self) -> &ToolInfo {
        &self.info
    }

    /// Invokes the tool with the provided input.
    ///
    /// This method is instrumented with tracing and logs the tool's qualified
    /// ID.
    ///
    /// # Arguments
    ///
    /// * `context` - Call context including session, user, and credential
    ///   information
    /// * `input` - Serialized input bytes (typically JSON)
    ///
    /// # Returns
    ///
    /// An FFI-compatible future that resolves to the tool's output or error.
    #[instrument(skip(self, context, input), fields(tool_id = %self.info.qualified_id))]
    pub fn call(&self, context: CallContext<'_>, input: RSlice<'_, u8>) -> FfiFuture<CallResult> {
        let args = CallArgs::new(context, RStr::from_str(&self.tool_id), input);
        self.module.call()(args)
    }
}

/// Central registry for managing loaded tools and their lifecycles.
///
/// The `ToolRegistry` is responsible for:
///
/// - Loading tool libraries from dynamic libraries
/// - Registering tools and validating ABI compatibility
/// - Providing access to tools via qualified IDs
/// - Tracking in-flight requests for graceful shutdown
/// - Semantic search via tool embeddings
///
/// # Usage Pattern
///
/// The registry has two phases of operation:
///
/// ```ignore
/// use operai_core::ToolRegistry;
/// use std::sync::Arc;
///
/// // Phase 1: Loading (requires mutable access)
/// let mut registry = ToolRegistry::new();
/// # let ctx = operai_abi::RuntimeContext::new();
/// registry.load_library("path/to/tool.so", Some("checksum"), None, &ctx).await?;
///
/// // Phase 2: Concurrent access (wrap in Arc)
/// let registry = Arc::new(registry);
/// # let policy_store = std::sync::Arc::new(operai_core::PolicyStore::new(std::sync::Arc::new(
/// #     operai_core::InMemoryPolicySessionStore::new()
/// # )));
/// # use operai_runtime::LocalRuntime;
/// let runtime = LocalRuntime::new(registry.clone(), policy_store);
///
/// // Now multiple threads can safely query and invoke tools
/// let tool = registry.get("crate-name.tool-name").unwrap();
/// ```
///
/// # Thread Safety
///
/// **Loading phase**: `load_library` requires `&mut self` and is not
/// thread-safe.
///
/// **Execution phase**: Once wrapped in `Arc`, query operations (`get`, `list`,
/// `search`) are thread-safe and can be called concurrently. Tool handles use
/// interior `Arc` wrapping for safe concurrent invocation.
///
/// # Example
///
/// ```no_run
/// # use operai_core::ToolRegistry;
/// # use operai_abi::RuntimeContext;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut registry = ToolRegistry::new();
/// let runtime_ctx = RuntimeContext::default();
///
/// // Load a tool library
/// registry
///     .load_library(
///         "./path/to/tool.so",
///         Some("sha256-checksum"),
///         None,
///         &runtime_ctx,
///     )
///     .await?;
///
/// // Get a tool handle
/// let tool = registry
///     .get("crate-name.tool-name")
///     .expect("tool not found");
///
/// // List all tools
/// for info in registry.list() {
///     println!("{}: {}", info.display_name, info.description);
/// }
/// # Ok(())
/// # }
/// ```
pub struct ToolRegistry {
    /// Loaded tool libraries (kept for lifetime management)
    libraries: Vec<ToolLibrary>,
    /// Map from qualified ID to tool handle
    tools: HashMap<String, Arc<ToolHandle>>,
    /// Tool embeddings for semantic search (`qualified_id`, embedding)
    embeddings: Vec<(String, Vec<f32>)>,
    /// Counter for tracking in-flight requests
    inflight: AtomicU64,
}

/// RAII guard for tracking in-flight tool requests.
///
/// When created, this guard increments the registry's in-flight counter.
/// When dropped, it automatically decrements the counter. This is useful for
/// ensuring accurate request tracking even when errors occur.
///
/// # Example
///
/// ```no_run
/// # use operai_core::ToolRegistry;
/// let registry = ToolRegistry::new();
///
/// {
///     let _guard = registry.start_request_guard();
///     // Do work that might fail or panic
///     // Guard ensures counter is decremented when scope exits
/// } // Guard dropped here, counter decremented
/// ```
#[must_use = "if unused, the in-flight request will be immediately ended"]
pub struct InflightRequestGuard<'a> {
    registry: &'a ToolRegistry,
}

impl Drop for InflightRequestGuard<'_> {
    /// Decrements the registry's in-flight counter when the guard is dropped.
    fn drop(&mut self) {
        self.registry.end_request();
    }
}

impl ToolRegistry {
    /// Creates a new, empty tool registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            libraries: Vec::new(),
            tools: HashMap::new(),
            embeddings: Vec::new(),
            inflight: AtomicU64::new(0),
        }
    }

    /// Loads a tool library from a dynamic library file and registers all its
    /// tools.
    ///
    /// This method validates the library's checksum (if provided), loads it
    /// from disk, initializes the module, and registers all exported tools.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the dynamic library (`.so`, `.dylib`, or `.dll`)
    /// * `checksum` - Optional SHA-256 checksum for validation
    /// * `credentials` - Optional system credentials to pass to tools
    /// * `runtime_ctx` - Runtime context for initialization
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::LoadError`] if:
    /// - The library file cannot be loaded
    /// - The checksum doesn't match
    /// - The library's ABI version is incompatible
    /// - Initialization fails
    ///
    /// Returns [`RegistryError::DuplicateId`] if any tool in the library
    /// has a qualified ID that's already registered.
    pub async fn load_library(
        &mut self,
        path: impl AsRef<std::path::Path>,
        checksum: Option<&str>,
        credentials: Option<&HashMap<String, HashMap<String, String>>>,
        runtime_ctx: &RuntimeContext,
    ) -> Result<(), RegistryError> {
        let library = ToolLibrary::load(&path, checksum)?;
        library.init(runtime_ctx).await?;

        self.register_module_ref(library.module(), credentials)?;
        self.libraries.push(library);

        Ok(())
    }

    /// Registers a pre-loaded tool module reference.
    ///
    /// This is useful for testing or when you have a module reference that
    /// wasn't loaded from a file. The method validates ABI version and
    /// initializes the module.
    ///
    /// # Arguments
    ///
    /// * `module` - FFI-compatible reference to the tool module
    /// * `credentials` - Optional system credentials to pass to tools
    /// * `runtime_ctx` - Runtime context for initialization
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::LoadError`] if:
    /// - The ABI version doesn't match
    /// - Module initialization fails
    ///
    /// Returns [`RegistryError::DuplicateId`] if any tool ID conflicts.
    pub async fn register_module(
        &mut self,
        module: ToolModuleRef,
        credentials: Option<&HashMap<String, HashMap<String, String>>>,
        runtime_ctx: &RuntimeContext,
    ) -> Result<(), RegistryError> {
        let meta = module.meta();
        if meta.abi_version != TOOL_ABI_VERSION {
            return Err(RegistryError::LoadError(LoadError::AbiMismatch {
                expected: TOOL_ABI_VERSION,
                actual: meta.abi_version,
            }));
        }

        let args = InitArgs::new(*runtime_ctx);
        let result = (module.init())(args).await;
        if result != ToolResult::Ok {
            return Err(RegistryError::LoadError(LoadError::InitFailed));
        }

        self.register_module_ref(module, credentials)
    }

    /// Internal method to register tools from a module reference.
    ///
    /// This method extracts tool descriptors from the module, creates tool
    /// handles, and adds them to the registry. It also builds the embedding
    /// index for search.
    ///
    /// # Arguments
    ///
    /// * `module` - FFI-compatible reference to the tool module
    /// * `credentials` - Optional system credentials (rkyv-encoded and stored
    ///   in each handle)
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::DuplicateId`] if any qualified tool ID
    /// conflicts with an already-registered tool.
    fn register_module_ref(
        &mut self,
        module: ToolModuleRef,
        credentials: Option<&HashMap<String, HashMap<String, String>>>,
    ) -> Result<(), RegistryError> {
        let meta = module.meta();
        let crate_name = meta.crate_name.as_str();
        let crate_version = meta.crate_version.as_str();

        for descriptor in module.descriptors_iter() {
            let tool_id = descriptor.id.as_str().to_string();
            let qualified_id = format!("{crate_name}.{tool_id}");

            if self.tools.contains_key(&qualified_id) {
                return Err(RegistryError::DuplicateId(qualified_id));
            }

            let info = ToolInfo {
                qualified_id: qualified_id.clone(),
                tool_id: tool_id.clone(),
                crate_name: crate_name.to_string(),
                crate_version: crate_version.to_string(),
                display_name: descriptor.name.as_str().to_string(),
                description: descriptor.description.as_str().to_string(),
                input_schema: descriptor.input_schema.as_str().to_string(),
                output_schema: descriptor.output_schema.as_str().to_string(),
                credential_schema: descriptor
                    .credential_schema
                    .as_ref()
                    .into_option()
                    .map(|s| s.as_str().to_string()),
                capabilities: descriptor
                    .capabilities
                    .as_slice()
                    .iter()
                    .map(|s| s.as_str().to_string())
                    .collect(),
                tags: descriptor
                    .tags
                    .as_slice()
                    .iter()
                    .map(|s| s.as_str().to_string())
                    .collect(),
                embedding: {
                    let slice = descriptor.embedding.as_slice();
                    if slice.is_empty() {
                        None
                    } else {
                        Some(slice.to_vec())
                    }
                },
            };

            if let Some(ref embedding) = info.embedding {
                self.embeddings
                    .push((qualified_id.clone(), embedding.clone()));
            }

            let system_credentials = if let Some(creds) = credentials {
                rkyv::to_bytes::<BoxedError>(creds)
                    .map_err(|e| {
                        RegistryError::LoadError(LoadError::InvalidPath(format!(
                            "serialization error: {e}",
                        )))
                    })?
                    .into_vec()
            } else {
                rkyv::to_bytes::<BoxedError>(&HashMap::<String, HashMap<String, String>>::new())
                    .expect("failed to serialize empty credentials")
                    .into_vec()
            };

            let handle = ToolHandle {
                info,
                module,
                system_credentials,
                tool_id,
            };

            info!(qualified_id = %qualified_id, "Registered tool");
            self.tools.insert(qualified_id, Arc::new(handle));
        }

        Ok(())
    }

    /// Gets a tool handle by its qualified ID.
    ///
    /// Returns `None` if the tool is not registered.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use operai_core::ToolRegistry;
    /// let registry = ToolRegistry::new();
    ///
    /// if let Some(tool) = registry.get("my-crate.my-tool") {
    ///     println!("Found tool: {}", tool.info().display_name);
    /// }
    /// ```
    #[must_use]
    pub fn get(&self, qualified_id: &str) -> Option<Arc<ToolHandle>> {
        self.tools.get(qualified_id).cloned()
    }

    /// Returns an iterator over all registered tools' metadata.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use operai_core::ToolRegistry;
    /// let registry = ToolRegistry::new();
    ///
    /// for info in registry.list() {
    ///     println!("{}: {}", info.qualified_id, info.description);
    /// }
    /// ```
    pub fn list(&self) -> impl Iterator<Item = &ToolInfo> {
        self.tools.values().map(|h| &h.info)
    }

    /// Returns the number of registered tools.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Returns `true` if no tools are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Searches for tools by semantic similarity using embeddings.
    ///
    /// This method computes cosine similarity between the query embedding and
    /// each tool's embedding, returning the top `limit` results sorted by
    /// descending similarity score.
    ///
    /// # Arguments
    ///
    /// * `query_embedding` - Vector embedding of the query (typically from an
    ///   embedding model)
    /// * `limit` - Maximum number of results to return
    ///
    /// # Returns
    ///
    /// A vector of `(ToolInfo, score)` tuples, where `score` is the cosine
    /// similarity in the range `[-1.0, 1.0]`. Higher values indicate
    /// greater similarity.
    ///
    /// # Notes
    ///
    /// - Tools without embeddings are not included in results
    /// - Returns an empty vector if `query_embedding` is empty
    /// - Results are sorted by descending similarity score
    #[must_use]
    pub fn search(&self, query_embedding: &[f32], limit: usize) -> Vec<(&ToolInfo, f32)> {
        if query_embedding.is_empty() || limit == 0 {
            return Vec::new();
        }

        // Use a min-heap to maintain top-K results in O(n log k) time
        // instead of collecting all and sorting in O(n log n)
        let mut heap: BinaryHeap<Reverse<OrderedScore<'_>>> = BinaryHeap::with_capacity(limit);

        for (id, embedding) in &self.embeddings {
            let score = cosine_similarity(query_embedding, embedding);
            if let Some(handle) = self.tools.get(id) {
                let entry = Reverse(OrderedScore {
                    score,
                    info: &handle.info,
                });

                if heap.len() < limit {
                    heap.push(entry);
                } else if let Some(min) = heap.peek() {
                    if score > min.0.score {
                        heap.pop();
                        heap.push(entry);
                    }
                }
            }
        }

        // Extract results: min-heap with Reverse gives us ascending order,
        // so we collect and reverse to get descending order
        let mut results: Vec<_> = heap
            .into_iter()
            .map(|Reverse(os)| (os.info, os.score))
            .collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    /// Increments the in-flight request counter.
    ///
    /// This should be called when a tool invocation starts. Remember to call
    /// [`Self::end_request`] when the invocation completes, or use
    /// [`Self::start_request_guard`] for automatic cleanup.
    pub fn start_request(&self) {
        self.inflight.fetch_add(1, Ordering::Relaxed);
    }

    /// Creates a guard that tracks an in-flight request.
    ///
    /// The guard increments the counter on creation and automatically
    /// decrements it when dropped. This is safer than manually calling
    /// [`Self::start_request`] and [`Self::end_request`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use operai_core::ToolRegistry;
    /// let registry = ToolRegistry::new();
    /// let _guard = registry.start_request_guard();
    /// // Do work here - counter will be decremented when guard is dropped
    /// ```
    #[must_use = "dropping the guard immediately will end the request"]
    pub fn start_request_guard(&self) -> InflightRequestGuard<'_> {
        self.start_request();
        InflightRequestGuard { registry: self }
    }

    /// Decrements the in-flight request counter.
    ///
    /// Uses saturating subtraction to prevent underflow. If you need to call
    /// this manually, consider using [`Self::start_request_guard`] instead
    /// for RAII-style cleanup.
    pub fn end_request(&self) {
        let _ = self
            .inflight
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                Some(current.saturating_sub(1))
            });
    }

    /// Returns the current number of in-flight requests.
    #[must_use]
    pub fn inflight_count(&self) -> u64 {
        self.inflight.load(Ordering::Relaxed)
    }

    /// Waits for all in-flight requests to complete.
    ///
    /// This method polls the in-flight counter every 10ms until it reaches
    /// zero. It's useful for ensuring all tool invocations have finished
    /// before shutting down the registry.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use operai_core::ToolRegistry;
    /// # #[tokio::main]
    /// # async fn main() {
    /// let registry = ToolRegistry::new();
    /// // ... do work that spawns tool invocations
    /// registry.drain().await; // Wait for all to complete
    /// //
    /// # }
    /// ```
    pub async fn drain(&self) {
        while self.inflight_count() > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }
}

impl Default for ToolRegistry {
    /// Creates a new empty registry, same as [`ToolRegistry::new`].
    fn default() -> Self {
        Self::new()
    }
}

/// Computes cosine similarity between two vectors.
///
/// Returns a value in the range `[-1.0, 1.0]` where:
/// - `1.0` indicates identical direction (maximum similarity)
/// - `0.0` indicates orthogonal (no similarity)
/// - `-1.0` indicates opposite direction (maximum dissimilarity)
///
/// # Arguments
///
/// * `a` - First vector
/// * `b` - Second vector
///
/// # Returns
///
/// Cosine similarity, or `0.0` if vectors have different lengths or are empty.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    // Use iterator-based fold for better compiler auto-vectorization
    let (dot_product, norm_a, norm_b) = a
        .iter()
        .zip(b.iter())
        .fold((0.0_f32, 0.0_f32, 0.0_f32), |(dot, na, nb), (&x, &y)| {
            (dot + x * y, na + x * x, nb + y * y)
        });

    let denominator = norm_a.sqrt() * norm_b.sqrt();
    if denominator == 0.0 {
        0.0
    } else {
        dot_product / denominator
    }
}

/// Helper struct for ordered comparison in BinaryHeap.
///
/// Wraps a score and tool info reference for use in the top-K heap.
/// Comparison is based solely on the score for heap ordering.
#[derive(Debug)]
struct OrderedScore<'a> {
    score: f32,
    info: &'a ToolInfo,
}

impl PartialEq for OrderedScore<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score
    }
}

impl Eq for OrderedScore<'_> {}

impl PartialOrd for OrderedScore<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedScore<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.score
            .partial_cmp(&other.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

#[cfg(test)]
mod tests {
    /// Unit tests for the tool registry and related functionality.
    ///
    /// These tests cover:
    /// - Cosine similarity calculations
    /// - Registry operations (load, register, get, list)
    /// - In-flight request tracking
    /// - Tool invocation
    /// - Semantic search
    /// - Error conditions
    use abi_stable::{
        prefix_type::{PrefixRefTrait, WithMetadata},
        std_types::RVec,
    };
    use operai_abi::{InitArgs, ToolModule, ToolResult};

    use super::*;

    /// Test helper: Creates a mock tool init function that always succeeds.
    extern "C" fn test_tool_init(_args: InitArgs) -> FfiFuture<ToolResult> {
        FfiFuture::new(async { ToolResult::Ok })
    }

    /// Test helper: Creates a mock tool call function that echoes the tool ID
    /// and input.
    extern "C" fn test_tool_call(args: CallArgs<'_>) -> FfiFuture<CallResult> {
        let tool_id = args.tool_id.as_str().to_string();
        let input = args.input.as_slice().to_vec();

        FfiFuture::new(async move {
            let mut output = tool_id.into_bytes();
            output.push(b'|');
            output.extend_from_slice(&input);
            CallResult::ok(RVec::from(output))
        })
    }

    /// Test helper: Creates a mock tool shutdown function.
    extern "C" fn test_tool_shutdown() {}

    /// Test helper: Creates a mock tool module reference for testing.
    fn test_tool_module_ref() -> ToolModuleRef {
        let module = ToolModule {
            meta: operai_abi::ToolMeta::new(
                operai_abi::TOOL_ABI_VERSION,
                RStr::from_str("test-crate"),
                RStr::from_str("0.0.0"),
            ),
            descriptors: RSlice::from_slice(&[]),
            init: test_tool_init,
            call: test_tool_call,
            shutdown: test_tool_shutdown,
        };

        let with_metadata: &'static WithMetadata<ToolModule> =
            Box::leak(Box::new(WithMetadata::new(module)));
        ToolModuleRef::from_prefix_ref(with_metadata.static_as_prefix())
    }

    /// Test helper: Creates a mock `ToolInfo` for testing.
    fn test_tool_info(qualified_id: &str, tool_id: &str, embedding: Option<Vec<f32>>) -> ToolInfo {
        let (crate_name, _) = qualified_id
            .split_once('.')
            .expect("qualified_id must contain '.'");

        ToolInfo {
            qualified_id: qualified_id.to_string(),
            tool_id: tool_id.to_string(),
            crate_name: crate_name.to_string(),
            crate_version: "0.0.0".to_string(),
            display_name: "Test Tool".to_string(),
            description: "Test tool description".to_string(),
            input_schema: "{}".to_string(),
            output_schema: "{}".to_string(),
            credential_schema: None,
            capabilities: Vec::new(),
            tags: Vec::new(),
            embedding,
        }
    }

    /// Test helper: Inserts a test tool into the registry.
    fn insert_test_tool(registry: &mut ToolRegistry, info: ToolInfo) {
        let qualified_id = info.qualified_id.clone();
        if let Some(ref embedding) = info.embedding {
            registry
                .embeddings
                .push((qualified_id.clone(), embedding.clone()));
        }

        let handle = ToolHandle {
            tool_id: info.tool_id.clone(),
            info,
            module: test_tool_module_ref(),
            system_credentials: Vec::new(),
        };

        registry.tools.insert(qualified_id, Arc::new(handle));
    }

    #[test]
    fn test_cosine_similarity_identical_vectors_returns_one() {
        let a = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &a) - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal_vectors_returns_zero() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &b)).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_opposite_vectors_returns_negative_one() {
        let a = vec![1.0, 0.0, 0.0];
        let c = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &c) + 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_mismatched_lengths_returns_zero() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!(cosine_similarity(&a, &b).abs() < f32::EPSILON);
    }

    #[test]
    fn test_cosine_similarity_empty_vectors_returns_zero() {
        let empty: Vec<f32> = vec![];
        assert!(cosine_similarity(&empty, &empty).abs() < f32::EPSILON);
    }

    #[test]
    fn test_cosine_similarity_zero_vector_returns_zero() {
        let a = vec![1.0, 0.0, 0.0];
        let zero = vec![0.0, 0.0, 0.0];
        assert!(cosine_similarity(&a, &zero).abs() < f32::EPSILON);
    }

    #[test]
    fn test_cosine_similarity_both_zero_vectors_returns_zero() {
        let zero = vec![0.0, 0.0, 0.0];
        assert!(cosine_similarity(&zero, &zero).abs() < f32::EPSILON);
    }

    #[test]
    fn test_empty_registry() {
        let registry = ToolRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_inflight_counter_basic() {
        let registry = ToolRegistry::new();
        assert_eq!(registry.inflight_count(), 0);

        registry.start_request();
        assert_eq!(registry.inflight_count(), 1);

        registry.start_request();
        assert_eq!(registry.inflight_count(), 2);

        registry.end_request();
        assert_eq!(registry.inflight_count(), 1);

        registry.end_request();
        assert_eq!(registry.inflight_count(), 0);
    }

    #[test]
    fn test_inflight_counter_concurrent_increments() {
        use std::{sync::Arc, thread};

        let registry = Arc::new(ToolRegistry::new());
        let num_threads = 100;
        let increments_per_thread = 1000;

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let registry = Arc::clone(&registry);
                thread::spawn(move || {
                    for _ in 0..increments_per_thread {
                        registry.start_request();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("thread panicked");
        }

        assert_eq!(
            registry.inflight_count(),
            num_threads * increments_per_thread
        );
    }

    #[test]
    fn test_inflight_counter_concurrent_inc_dec() {
        use std::{sync::Arc, thread};

        let registry = Arc::new(ToolRegistry::new());
        let num_threads = 50;
        let ops_per_thread = 1000;

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let registry = Arc::clone(&registry);
                thread::spawn(move || {
                    for _ in 0..ops_per_thread {
                        registry.start_request();
                        thread::yield_now();
                        registry.end_request();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("thread panicked");
        }

        assert_eq!(registry.inflight_count(), 0);
    }

    #[tokio::test]
    async fn test_drain_empty_registry() {
        let registry = ToolRegistry::new();
        registry.drain().await;
        assert_eq!(registry.inflight_count(), 0);
    }

    #[test]
    fn test_registry_error_display_messages() {
        let not_found = RegistryError::NotFound("hello-world.greet".to_string());
        assert_eq!(not_found.to_string(), "tool not found: hello-world.greet");

        let duplicate = RegistryError::DuplicateId("hello-world.greet".to_string());
        assert_eq!(
            duplicate.to_string(),
            "duplicate tool ID: hello-world.greet"
        );

        let invocation = RegistryError::InvocationError("boom".to_string());
        assert_eq!(invocation.to_string(), "tool invocation failed: boom");

        let load_error: RegistryError = LoadError::InvalidPath("bad-path".to_string()).into();
        assert_eq!(
            load_error.to_string(),
            "failed to load library: invalid path: bad-path"
        );
    }

    #[test]
    fn test_registry_default_returns_empty_registry() {
        let registry = ToolRegistry::default();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_search_empty_query_embedding_returns_empty() {
        let mut registry = ToolRegistry::new();
        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-a", "tool-a", Some(vec![1.0, 0.0])),
        );

        let results = registry.search(&[], 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_registry_search_returns_sorted_results_and_respects_limit() {
        let mut registry = ToolRegistry::new();
        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-a", "tool-a", Some(vec![1.0, 0.0])),
        );
        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-b", "tool-b", Some(vec![0.0, 1.0])),
        );

        let results = registry.search(&[1.0, 0.0], 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0.qualified_id, "test-crate.tool-a");
        assert!((results[0].1 - 1.0).abs() < 0.0001);
        assert_eq!(results[1].0.qualified_id, "test-crate.tool-b");
        assert!((results[1].1).abs() < 0.0001);

        let limited = registry.search(&[1.0, 0.0], 1);
        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0].0.qualified_id, "test-crate.tool-a");
    }

    #[test]
    fn test_registry_search_skips_tools_without_embeddings() {
        let mut registry = ToolRegistry::new();
        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-a", "tool-a", None),
        );

        let results = registry.search(&[1.0, 0.0], 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_registry_search_with_limit_zero_returns_empty() {
        let mut registry = ToolRegistry::new();
        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-a", "tool-a", Some(vec![1.0, 0.0])),
        );

        let results = registry.search(&[1.0, 0.0], 0);
        assert!(results.is_empty());
    }

    #[test]
    fn test_registry_search_empty_registry_returns_empty() {
        let registry = ToolRegistry::new();
        let results = registry.search(&[1.0, 0.0], 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_registry_search_with_nan_embedding_does_not_panic() {
        let mut registry = ToolRegistry::new();
        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-a", "tool-a", Some(vec![1.0, 0.0])),
        );
        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-b", "tool-b", Some(vec![0.0, 1.0])),
        );

        let results = registry.search(&[f32::NAN, 0.0], 10);
        assert_eq!(results.len(), 2);

        let ids: std::collections::HashSet<_> = results
            .into_iter()
            .map(|(info, _)| info.qualified_id.as_str())
            .collect();
        assert_eq!(
            ids,
            std::collections::HashSet::from(["test-crate.tool-a", "test-crate.tool-b"])
        );
    }

    #[tokio::test]
    async fn test_tool_handle_call_passes_tool_id_and_input() {
        let module = test_tool_module_ref();
        let handle = ToolHandle {
            info: test_tool_info("test-crate.greet", "greet", None),
            module,
            system_credentials: Vec::new(),
            tool_id: "greet".to_string(),
        };

        let context = CallContext {
            request_id: RStr::from_str("request"),
            session_id: RStr::from_str("session"),
            user_id: RStr::from_str("user"),
            user_credentials: RSlice::from_slice(&[]),
            system_credentials: RSlice::from_slice(&[]),
        };
        let input = br#"{"hello":"world"}"#;

        let result = handle.call(context, RSlice::from_slice(input)).await;

        assert_eq!(result.result, ToolResult::Ok);
        assert_eq!(result.output.as_slice(), b"greet|{\"hello\":\"world\"}");
    }

    #[tokio::test]
    async fn test_drain_waits_for_inflight_requests_to_complete() {
        let registry = Arc::new(ToolRegistry::new());
        registry.start_request();

        let cloned = Arc::clone(&registry);
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
            cloned.end_request();
        });

        let result =
            tokio::time::timeout(tokio::time::Duration::from_secs(1), registry.drain()).await;
        assert!(result.is_ok());
        assert_eq!(registry.inflight_count(), 0);
    }

    #[test]
    fn test_inflight_request_guard_decrements_on_drop() {
        let registry = ToolRegistry::new();
        assert_eq!(registry.inflight_count(), 0);

        {
            let _guard = registry.start_request_guard();
            assert_eq!(registry.inflight_count(), 1);
        }

        assert_eq!(registry.inflight_count(), 0);
    }

    #[test]
    fn test_inflight_request_guard_multiple_guards() {
        let registry = ToolRegistry::new();

        let guard1 = registry.start_request_guard();
        let guard2 = registry.start_request_guard();
        assert_eq!(registry.inflight_count(), 2);

        drop(guard1);
        assert_eq!(registry.inflight_count(), 1);

        drop(guard2);
        assert_eq!(registry.inflight_count(), 0);
    }

    #[test]
    fn test_end_request_saturates_at_zero() {
        let registry = ToolRegistry::new();
        assert_eq!(registry.inflight_count(), 0);

        registry.end_request();
        assert_eq!(registry.inflight_count(), 0);

        registry.end_request();
        assert_eq!(registry.inflight_count(), 0);
    }

    #[test]
    fn test_registry_get_returns_tool_by_qualified_id() {
        let mut registry = ToolRegistry::new();
        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-a", "tool-a", None),
        );

        let handle = registry.get("test-crate.tool-a");
        assert!(handle.is_some());
        assert_eq!(handle.unwrap().info().qualified_id, "test-crate.tool-a");
    }

    #[test]
    fn test_registry_get_returns_none_for_nonexistent_tool() {
        let registry = ToolRegistry::new();
        assert!(registry.get("nonexistent.tool").is_none());
    }

    #[test]
    fn test_registry_list_returns_all_tool_infos() {
        let mut registry = ToolRegistry::new();
        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-a", "tool-a", None),
        );
        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-b", "tool-b", None),
        );

        let infos: Vec<_> = registry.list().collect();
        assert_eq!(infos.len(), 2);

        let ids: std::collections::HashSet<_> = infos
            .iter()
            .map(|info| info.qualified_id.as_str())
            .collect();
        assert!(ids.contains("test-crate.tool-a"));
        assert!(ids.contains("test-crate.tool-b"));
    }

    #[test]
    fn test_registry_len_and_is_empty_with_tools() {
        let mut registry = ToolRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-a", "tool-a", None),
        );
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);

        insert_test_tool(
            &mut registry,
            test_tool_info("test-crate.tool-b", "tool-b", None),
        );
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn test_tool_handle_info_returns_tool_info() {
        let info = test_tool_info("test-crate.greet", "greet", Some(vec![1.0, 2.0]));
        let handle = ToolHandle {
            info: info.clone(),
            module: test_tool_module_ref(),
            system_credentials: Vec::new(),
            tool_id: "greet".to_string(),
        };

        let returned_info = handle.info();
        assert_eq!(returned_info.qualified_id, "test-crate.greet");
        assert_eq!(returned_info.tool_id, "greet");
        assert_eq!(returned_info.crate_name, "test-crate");
        assert_eq!(returned_info.embedding, Some(vec![1.0, 2.0]));
    }

    #[test]
    fn test_tool_info_clone_creates_independent_copy() {
        let original = test_tool_info("test-crate.greet", "greet", Some(vec![1.0, 2.0]));
        let cloned = original.clone();

        assert_eq!(original.qualified_id, cloned.qualified_id);
        assert_eq!(original.tool_id, cloned.tool_id);
        assert_eq!(original.crate_name, cloned.crate_name);
        assert_eq!(original.embedding, cloned.embedding);
    }

    #[test]
    fn test_registry_list_empty_registry_returns_empty_iterator() {
        let registry = ToolRegistry::new();
        let infos: Vec<_> = registry.list().collect();
        assert!(infos.is_empty());
    }
}
