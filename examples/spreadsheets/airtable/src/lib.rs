//! spreadsheets/airtable integration for Operai Toolbox.
use operai::{
    Context, JsonSchema, Result, define_system_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};

mod types;
use types::{
    Attachment, Base, BaseSchemaResponse, CreateRecordRequest, ListBasesResponse,
    ListRecordsResponse, Record, RecordResponse, Table, UpdateRecordRequest,
};

define_system_credential! {
    AirtableCredential("airtable") {
        api_key: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_API_ENDPOINT: &str = "https://api.airtable.com/v0";

#[init]
async fn setup() -> Result<()> {
    info!("Airtable integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Airtable integration shutting down");
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListBasesInput {
    /// Maximum number of bases to return (1-100). Defaults to 100.
    #[serde(default)]
    pub limit: Option<u32>,
    /// Offset token for pagination.
    #[serde(default)]
    pub offset: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListBasesOutput {
    pub bases: Vec<Base>,
    #[serde(default)]
    pub offset: Option<String>,
}

/// # List Airtable Bases
///
/// Retrieves all Airtable bases accessible to the authenticated user. Use this
/// tool when a user wants to explore their available Airtable workspaces or
/// discover the base IDs needed for other operations.
///
/// This tool returns base metadata including the base ID (required for
/// subsequent operations), base name, and permission level. The results can be
/// paginated using the limit and offset parameters.
///
/// Use this tool first when you need to:
/// - Discover what bases are available to the user
/// - Get base IDs for use with other Airtable tools
/// - Browse the user's Airtable workspace structure
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - airtable
/// - bases
///
/// # Errors
///
/// Returns an error if:
/// - The `limit` is not between 1 and 100
/// - The `offset` string is empty or contains only whitespace
/// - Airtable credentials are not configured or are invalid
/// - The API request fails (network error, authentication failure, etc.)
/// - The response cannot be parsed
#[tool]
pub async fn list_bases(ctx: Context, input: ListBasesInput) -> Result<ListBasesOutput> {
    let limit = input.limit.unwrap_or(100);
    ensure!(
        (1..=100).contains(&limit),
        "limit must be between 1 and 100"
    );

    let client = AirtableClient::from_ctx(&ctx)?;

    let mut query = vec![("pageSize".to_string(), limit.to_string())];
    if let Some(offset) = input.offset {
        ensure!(!offset.trim().is_empty(), "offset must not be empty");
        query.push(("offset".to_string(), offset));
    }

    let response: ListBasesResponse = client
        .get_json(client.meta_url_with_segments(&["bases"])?, &query)
        .await?;

    Ok(ListBasesOutput {
        bases: response
            .bases
            .into_iter()
            .map(|b| Base {
                id: b.id,
                name: b.name,
                permission_level: b.permission_level,
            })
            .collect(),
        offset: response.offset,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListTablesInput {
    /// Base ID (starts with "app").
    pub base_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListTablesOutput {
    pub tables: Vec<Table>,
}

/// # List Airtable Tables
///
/// Retrieves all tables within a specific Airtable base. Use this tool when a
/// user wants to explore the structure of a base and discover the table IDs or
/// names needed for record operations.
///
/// This tool returns table metadata including the table ID (required for record
/// operations), table name, optional description, and the primary field ID. You
/// must have a valid base ID (obtained from `list_bases`) before using this
/// tool.
///
/// Use this tool when you need to:
/// - Discover what tables exist within a specific base
/// - Get table IDs or names for use with record operations
/// - Understand the structure and schema of a base
/// - Find the primary field ID for a table
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - airtable
/// - tables
///
/// # Errors
///
/// Returns an error if:
/// - The `base_id` is empty or contains only whitespace
/// - The `base_id` does not start with "app"
/// - Airtable credentials are not configured or are invalid
/// - The API request fails (network error, authentication failure, etc.)
/// - The response cannot be parsed
#[tool]
pub async fn list_tables(ctx: Context, input: ListTablesInput) -> Result<ListTablesOutput> {
    ensure!(
        !input.base_id.trim().is_empty(),
        "base_id must not be empty"
    );
    ensure!(
        input.base_id.starts_with("app"),
        "base_id must start with 'app'"
    );

    let client = AirtableClient::from_ctx(&ctx)?;

    let response: BaseSchemaResponse = client
        .get_json(
            client.meta_url_with_segments(&["bases", &input.base_id, "tables"])?,
            &[],
        )
        .await?;

    Ok(ListTablesOutput {
        tables: response
            .tables
            .into_iter()
            .map(|t| Table {
                id: t.id,
                name: t.name,
                description: t.description,
                primary_field_id: t.primary_field_id,
            })
            .collect(),
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchRecordsInput {
    /// Base ID (starts with "app").
    pub base_id: String,
    /// Table ID or table name.
    pub table_id_or_name: String,
    /// Optional Airtable formula to filter records.
    #[serde(default)]
    pub filter_by_formula: Option<String>,
    /// Maximum number of records to return (1-100). Defaults to 100.
    #[serde(default)]
    pub max_records: Option<u32>,
    /// Field names to sort by (prefix with "-" for descending).
    #[serde(default)]
    pub sort: Vec<String>,
    /// View name or ID to use.
    #[serde(default)]
    pub view: Option<String>,
    /// Offset token for pagination.
    #[serde(default)]
    pub offset: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchRecordsOutput {
    pub records: Vec<Record>,
    #[serde(default)]
    pub offset: Option<String>,
}

/// # Search Airtable Records
///
/// Searches for and retrieves records from an Airtable table with powerful
/// filtering, sorting, and pagination capabilities. Use this tool when a user
/// wants to find specific records, browse table contents, or retrieve data
/// based on custom criteria.
///
/// This tool supports advanced querying through Airtable formulas, multiple
/// sort fields, view-based filtering, and pagination. It returns the complete
/// record data including field values and metadata.
///
/// Use this tool when you need to:
/// - Retrieve all records or a subset from a table
/// - Filter records using Airtable formulas (e.g., "NOT({Status}='Done')")
/// - Sort results by one or more fields (prefix with "-" for descending)
/// - Paginate through large datasets using offset tokens
/// - Query records within a specific view
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - airtable
/// - records
/// - search
///
/// # Errors
///
/// Returns an error if:
/// - The `base_id` is empty or does not start with "app"
/// - The `table_id_or_name` is empty or contains only whitespace
/// - The `max_records` value is not between 1 and 100
/// - The `filter_by_formula` string is empty or contains only whitespace
/// - The `view` or `offset` strings are empty or contain only whitespace
/// - Any sort field is empty or contains only whitespace
/// - Airtable credentials are not configured or are invalid
/// - The API request fails (network error, authentication failure, etc.)
/// - The response cannot be parsed
#[tool]
pub async fn search_records(
    ctx: Context,
    input: SearchRecordsInput,
) -> Result<SearchRecordsOutput> {
    ensure!(
        !input.base_id.trim().is_empty(),
        "base_id must not be empty"
    );
    ensure!(
        input.base_id.starts_with("app"),
        "base_id must start with 'app'"
    );
    ensure!(
        !input.table_id_or_name.trim().is_empty(),
        "table_id_or_name must not be empty"
    );

    let max_records = input.max_records.unwrap_or(100);
    ensure!(
        (1..=100).contains(&max_records),
        "max_records must be between 1 and 100"
    );

    let client = AirtableClient::from_ctx(&ctx)?;

    let mut query = vec![("maxRecords".to_string(), max_records.to_string())];

    if let Some(formula) = input.filter_by_formula {
        ensure!(
            !formula.trim().is_empty(),
            "filter_by_formula must not be empty"
        );
        query.push(("filterByFormula".to_string(), formula));
    }

    if let Some(view) = input.view {
        ensure!(!view.trim().is_empty(), "view must not be empty");
        query.push(("view".to_string(), view));
    }

    if let Some(offset) = input.offset {
        ensure!(!offset.trim().is_empty(), "offset must not be empty");
        query.push(("offset".to_string(), offset));
    }

    for (i, sort_field) in input.sort.iter().enumerate() {
        ensure!(
            !sort_field.trim().is_empty(),
            "sort field must not be empty"
        );
        let (field, direction) = if let Some(stripped) = sort_field.strip_prefix('-') {
            (stripped, "desc")
        } else {
            (sort_field.as_str(), "asc")
        };
        query.push((format!("sort[{i}][field]"), field.to_string()));
        query.push((format!("sort[{i}][direction]"), direction.to_string()));
    }

    let response: ListRecordsResponse = client
        .get_json(
            client.url_with_segments(&[&input.base_id, &input.table_id_or_name])?,
            &query,
        )
        .await?;

    Ok(SearchRecordsOutput {
        records: response
            .records
            .into_iter()
            .map(|r| Record {
                id: r.id,
                fields: r.fields,
                created_time: r.created_time,
            })
            .collect(),
        offset: response.offset,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateRecordInput {
    /// Base ID (starts with "app").
    pub base_id: String,
    /// Table ID or table name.
    pub table_id_or_name: String,
    /// Field values for the new record.
    pub fields: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateRecordOutput {
    pub record: Record,
}

/// # Create Airtable Record
///
/// Creates a new record in an Airtable table with the specified field values.
/// Use this tool when a user wants to add new data to an Airtable table, such
/// as creating new entries for tasks, contacts, projects, or any other record
/// type.
///
/// This tool accepts a map of field names to field values (JSON-compatible
/// types like strings, numbers, booleans, arrays, or objects). The fields must
/// match the table's schema - required fields must be provided and field types
/// must be compatible. Returns the created record including its auto-generated
/// ID and timestamp.
///
/// Use this tool when you need to:
/// - Add new entries to a table (e.g., new tasks, contacts, items)
/// - Create records with user-provided data
/// - Populate Airtable tables from external sources
/// - Initialize new data rows in a spreadsheet-like interface
///
/// Note: Field values must match the table's schema. Referencing related
/// records may require their record IDs. Required fields must be provided or
/// the operation will fail.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - airtable
/// - records
/// - create
///
/// # Errors
///
/// Returns an error if:
/// - The `base_id` is empty or does not start with "app"
/// - The `table_id_or_name` is empty or contains only whitespace
/// - The `fields` map is empty
/// - Airtable credentials are not configured or are invalid
/// - The API request fails (network error, authentication failure, etc.)
/// - The response cannot be parsed
#[tool]
pub async fn create_record(ctx: Context, input: CreateRecordInput) -> Result<CreateRecordOutput> {
    ensure!(
        !input.base_id.trim().is_empty(),
        "base_id must not be empty"
    );
    ensure!(
        input.base_id.starts_with("app"),
        "base_id must start with 'app'"
    );
    ensure!(
        !input.table_id_or_name.trim().is_empty(),
        "table_id_or_name must not be empty"
    );
    ensure!(!input.fields.is_empty(), "fields must not be empty");

    let client = AirtableClient::from_ctx(&ctx)?;

    let request = CreateRecordRequest {
        fields: input.fields,
    };

    let response: RecordResponse = client
        .post_json(
            client.url_with_segments(&[&input.base_id, &input.table_id_or_name])?,
            &request,
        )
        .await?;

    Ok(CreateRecordOutput {
        record: Record {
            id: response.id,
            fields: response.fields,
            created_time: response.created_time,
        },
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateRecordInput {
    /// Base ID (starts with "app").
    pub base_id: String,
    /// Table ID or table name.
    pub table_id_or_name: String,
    /// Record ID (starts with "rec").
    pub record_id: String,
    /// Field values to update.
    pub fields: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateRecordOutput {
    pub record: Record,
}

/// # Update Airtable Record
///
/// Updates an existing record in an Airtable table with new field values. Use
/// this tool when a user wants to modify existing data, change status fields,
/// update information, or make corrections to records that have already been
/// created.
///
/// This tool performs a partial update - only the fields specified in the
/// request will be modified. You must provide the record ID (starts with
/// "rec"), which can be obtained from a previous `search_records` or
/// `create_record` call. Returns the updated record with all current field
/// values.
///
/// Use this tool when you need to:
/// - Modify existing record data (e.g., change task status, update contact
///   info)
/// - Correct errors or outdated information in records
/// - Increment or modify field values
/// - Update timestamps, statuses, or other tracking fields
///
/// Note: This is a partial update operation. Fields not specified will remain
/// unchanged. The record ID must be valid and the record must exist. Field
/// values must match the table's schema.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - airtable
/// - records
/// - update
///
/// # Errors
///
/// Returns an error if:
/// - The `base_id` is empty or does not start with "app"
/// - The `table_id_or_name` is empty or contains only whitespace
/// - The `record_id` is empty or does not start with "rec"
/// - The `fields` map is empty
/// - Airtable credentials are not configured or are invalid
/// - The API request fails (network error, authentication failure, etc.)
/// - The response cannot be parsed
#[tool]
pub async fn update_record(ctx: Context, input: UpdateRecordInput) -> Result<UpdateRecordOutput> {
    ensure!(
        !input.base_id.trim().is_empty(),
        "base_id must not be empty"
    );
    ensure!(
        input.base_id.starts_with("app"),
        "base_id must start with 'app'"
    );
    ensure!(
        !input.table_id_or_name.trim().is_empty(),
        "table_id_or_name must not be empty"
    );
    ensure!(
        !input.record_id.trim().is_empty(),
        "record_id must not be empty"
    );
    ensure!(
        input.record_id.starts_with("rec"),
        "record_id must start with 'rec'"
    );
    ensure!(!input.fields.is_empty(), "fields must not be empty");

    let client = AirtableClient::from_ctx(&ctx)?;

    let request = UpdateRecordRequest {
        fields: input.fields,
    };

    let response: RecordResponse = client
        .patch_json(
            client.url_with_segments(&[
                &input.base_id,
                &input.table_id_or_name,
                &input.record_id,
            ])?,
            &request,
        )
        .await?;

    Ok(UpdateRecordOutput {
        record: Record {
            id: response.id,
            fields: response.fields,
            created_time: response.created_time,
        },
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AttachFileInput {
    /// Base ID (starts with "app").
    pub base_id: String,
    /// Table ID or table name.
    pub table_id_or_name: String,
    /// Record ID (starts with "rec").
    pub record_id: String,
    /// Field name or ID of the attachment field.
    pub field_name: String,
    /// Attachments to add (public URLs that Airtable will download and store).
    pub attachments: Vec<Attachment>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AttachFileOutput {
    pub record: Record,
}

/// # Attach File to Airtable Record
///
/// Attaches one or more files to an attachment field in an Airtable record by
/// providing public URLs. Use this tool when a user wants to add file
/// attachments to records, such as documents, images, PDFs, or other files
/// hosted at publicly accessible URLs.
///
/// This tool works by providing Airtable with public URLs; Airtable will
/// download and store the files from those URLs. Multiple files can be attached
/// in a single operation. The target field must be an attachment field type in
/// the table's schema. Returns the updated record with the new attachments
/// included.
///
/// Use this tool when you need to:
/// - Attach documents, images, or other files to records
/// - Add multiple files to an attachment field at once
/// - Link external file resources to Airtable records
/// - Update record attachments with new files
///
/// Note: URLs must be publicly accessible for Airtable to download them. The
/// field must be configured as an attachment field in the table schema. This
/// operation adds to existing attachments rather than replacing them (use
/// `update_record` with an empty array to clear attachments).
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - airtable
/// - records
/// - attachments
///
/// # Errors
///
/// Returns an error if:
/// - The `base_id` is empty or does not start with "app"
/// - The `table_id_or_name` is empty or contains only whitespace
/// - The `record_id` is empty or does not start with "rec"
/// - The `field_name` is empty or contains only whitespace
/// - The `attachments` array is empty
/// - Any attachment URL is empty or contains only whitespace
/// - Any attachment filename is empty or contains only whitespace
/// - Airtable credentials are not configured or are invalid
/// - The API request fails (network error, authentication failure, etc.)
/// - The response cannot be parsed
#[tool]
pub async fn attach_file(ctx: Context, input: AttachFileInput) -> Result<AttachFileOutput> {
    ensure!(
        !input.base_id.trim().is_empty(),
        "base_id must not be empty"
    );
    ensure!(
        input.base_id.starts_with("app"),
        "base_id must start with 'app'"
    );
    ensure!(
        !input.table_id_or_name.trim().is_empty(),
        "table_id_or_name must not be empty"
    );
    ensure!(
        !input.record_id.trim().is_empty(),
        "record_id must not be empty"
    );
    ensure!(
        input.record_id.starts_with("rec"),
        "record_id must start with 'rec'"
    );
    ensure!(
        !input.field_name.trim().is_empty(),
        "field_name must not be empty"
    );
    ensure!(
        !input.attachments.is_empty(),
        "attachments must not be empty"
    );

    for attachment in &input.attachments {
        ensure!(
            !attachment.url.trim().is_empty(),
            "attachment url must not be empty"
        );
        if let Some(filename) = &attachment.filename {
            ensure!(
                !filename.trim().is_empty(),
                "attachment filename must not be empty if provided"
            );
        }
    }

    let client = AirtableClient::from_ctx(&ctx)?;

    // Airtable attachments work by providing an array of attachment objects with
    // URLs. Airtable will download the file from the URL and store it.
    let mut fields = std::collections::HashMap::new();
    fields.insert(
        input.field_name.clone(),
        serde_json::to_value(&input.attachments)?,
    );

    let request = UpdateRecordRequest { fields };

    let response: RecordResponse = client
        .patch_json(
            client.url_with_segments(&[
                &input.base_id,
                &input.table_id_or_name,
                &input.record_id,
            ])?,
            &request,
        )
        .await?;

    Ok(AttachFileOutput {
        record: Record {
            id: response.id,
            fields: response.fields,
            created_time: response.created_time,
        },
    })
}

#[derive(Debug, Clone)]
struct AirtableClient {
    http: reqwest::Client,
    base_url: String,
    meta_base_url: String,
    api_key: String,
}

impl AirtableClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = AirtableCredential::get(ctx)?;
        ensure!(!cred.api_key.trim().is_empty(), "api_key must not be empty");

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_API_ENDPOINT))?;
        let meta_base_url = format!("{base_url}/meta");

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            meta_base_url,
            api_key: cred.api_key,
        })
    }

    fn url_with_segments(&self, segments: &[&str]) -> Result<reqwest::Url> {
        let mut url = reqwest::Url::parse(&self.base_url)?;
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|()| operai::anyhow::anyhow!("base_url must be an absolute URL"))?;
            for segment in segments {
                path.push(segment);
            }
        }
        Ok(url)
    }

    fn meta_url_with_segments(&self, segments: &[&str]) -> Result<reqwest::Url> {
        let mut url = reqwest::Url::parse(&self.meta_base_url)?;
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|()| operai::anyhow::anyhow!("meta_base_url must be an absolute URL"))?;
            for segment in segments {
                path.push(segment);
            }
        }
        Ok(url)
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        query: &[(String, String)],
    ) -> Result<T> {
        let response: reqwest::Response =
            self.send_request(self.http.get(url).query(query)).await?;
        Ok(response.json::<T>().await?)
    }

    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
    ) -> Result<TRes> {
        let response: reqwest::Response = self.send_request(self.http.post(url).json(body)).await?;
        Ok(response.json::<TRes>().await?)
    }

    async fn patch_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
    ) -> Result<TRes> {
        let response: reqwest::Response =
            self.send_request(self.http.patch(url).json(body)).await?;
        Ok(response.json::<TRes>().await?)
    }

    async fn send_request(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let response: reqwest::Response = request
            .bearer_auth(&self.api_key)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else {
            let body: String = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "Airtable API request failed ({status}): {body}"
            ))
        }
    }
}

fn normalize_base_url(endpoint: &str) -> Result<String> {
    let trimmed = endpoint.trim();
    ensure!(!trimmed.is_empty(), "endpoint must not be empty");
    Ok(trimmed.trim_end_matches('/').to_string())
}

operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_string_contains, header, method, path, query_param},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut airtable_values = HashMap::new();
        airtable_values.insert("api_key".to_string(), "test-api-key".to_string());
        airtable_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_system_credential("airtable", airtable_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        format!("{}/v0", server.uri())
    }

    #[test]
    fn test_base_serialization_roundtrip() {
        let base = Base {
            id: "app123".to_string(),
            name: "Test Base".to_string(),
            permission_level: Some("owner".to_string()),
        };
        let json = serde_json::to_string(&base).unwrap();
        let parsed: Base = serde_json::from_str(&json).unwrap();
        assert_eq!(base.id, parsed.id);
        assert_eq!(base.name, parsed.name);
        assert_eq!(base.permission_level, parsed.permission_level);
    }

    #[tokio::test]
    async fn test_list_bases_success_returns_bases() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "bases": [
            {
              "id": "app123",
              "name": "Test Base",
              "permissionLevel": "owner"
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v0/meta/bases"))
            .and(header("authorization", "Bearer test-api-key"))
            .and(query_param("pageSize", "10"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_bases(
            ctx,
            ListBasesInput {
                limit: Some(10),
                offset: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.bases.len(), 1);
        assert_eq!(output.bases[0].id, "app123");
        assert_eq!(output.bases[0].name, "Test Base");
    }

    #[test]
    fn test_attachment_serialization_roundtrip() {
        let attachment = Attachment {
            url: "https://example.com/file.pdf".to_string(),
            filename: Some("file.pdf".to_string()),
        };
        let json = serde_json::to_string(&attachment).unwrap();
        let parsed: Attachment = serde_json::from_str(&json).unwrap();
        assert_eq!(attachment.url, parsed.url);
        assert_eq!(attachment.filename, parsed.filename);
    }

    #[test]
    fn test_attachment_without_filename_serialization() {
        let attachment = Attachment {
            url: "https://example.com/file.pdf".to_string(),
            filename: None,
        };
        let json = serde_json::to_string(&attachment).unwrap();
        assert!(!json.contains("filename"));
        let parsed: Attachment = serde_json::from_str(&json).unwrap();
        assert_eq!(attachment.url, parsed.url);
        assert!(parsed.filename.is_none());
    }

    #[tokio::test]
    async fn test_attach_file_empty_base_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = attach_file(
            ctx,
            AttachFileInput {
                base_id: "  ".to_string(),
                table_id_or_name: "tbl123".to_string(),
                record_id: "rec123".to_string(),
                field_name: "Attachments".to_string(),
                attachments: vec![Attachment {
                    url: "https://example.com/file.pdf".to_string(),
                    filename: None,
                }],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("base_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_attach_file_invalid_base_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = attach_file(
            ctx,
            AttachFileInput {
                base_id: "invalid".to_string(),
                table_id_or_name: "tbl123".to_string(),
                record_id: "rec123".to_string(),
                field_name: "Attachments".to_string(),
                attachments: vec![Attachment {
                    url: "https://example.com/file.pdf".to_string(),
                    filename: None,
                }],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("base_id must start with 'app'")
        );
    }

    #[tokio::test]
    async fn test_attach_file_empty_record_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = attach_file(
            ctx,
            AttachFileInput {
                base_id: "app123".to_string(),
                table_id_or_name: "tbl123".to_string(),
                record_id: "  ".to_string(),
                field_name: "Attachments".to_string(),
                attachments: vec![Attachment {
                    url: "https://example.com/file.pdf".to_string(),
                    filename: None,
                }],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("record_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_attach_file_invalid_record_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = attach_file(
            ctx,
            AttachFileInput {
                base_id: "app123".to_string(),
                table_id_or_name: "tbl123".to_string(),
                record_id: "invalid".to_string(),
                field_name: "Attachments".to_string(),
                attachments: vec![Attachment {
                    url: "https://example.com/file.pdf".to_string(),
                    filename: None,
                }],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("record_id must start with 'rec'")
        );
    }

    #[tokio::test]
    async fn test_attach_file_empty_field_name_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = attach_file(
            ctx,
            AttachFileInput {
                base_id: "app123".to_string(),
                table_id_or_name: "tbl123".to_string(),
                record_id: "rec123".to_string(),
                field_name: "  ".to_string(),
                attachments: vec![Attachment {
                    url: "https://example.com/file.pdf".to_string(),
                    filename: None,
                }],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("field_name must not be empty")
        );
    }

    #[tokio::test]
    async fn test_attach_file_empty_attachments_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = attach_file(
            ctx,
            AttachFileInput {
                base_id: "app123".to_string(),
                table_id_or_name: "tbl123".to_string(),
                record_id: "rec123".to_string(),
                field_name: "Attachments".to_string(),
                attachments: vec![],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("attachments must not be empty")
        );
    }

    #[tokio::test]
    async fn test_attach_file_empty_attachment_url_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = attach_file(
            ctx,
            AttachFileInput {
                base_id: "app123".to_string(),
                table_id_or_name: "tbl123".to_string(),
                record_id: "rec123".to_string(),
                field_name: "Attachments".to_string(),
                attachments: vec![Attachment {
                    url: "  ".to_string(),
                    filename: None,
                }],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("attachment url must not be empty")
        );
    }

    #[tokio::test]
    async fn test_attach_file_empty_filename_if_provided_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = attach_file(
            ctx,
            AttachFileInput {
                base_id: "app123".to_string(),
                table_id_or_name: "tbl123".to_string(),
                record_id: "rec123".to_string(),
                field_name: "Attachments".to_string(),
                attachments: vec![Attachment {
                    url: "https://example.com/file.pdf".to_string(),
                    filename: Some("  ".to_string()),
                }],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("attachment filename must not be empty")
        );
    }

    #[tokio::test]
    async fn test_attach_file_success_returns_updated_record() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "rec123",
          "fields": {
            "Attachments": [
              {
                "url": "https://example.com/file.pdf",
                "filename": "file.pdf"
              }
            ]
          },
          "createdTime": "2024-01-01T00:00:00.000Z"
        }
        "#;

        Mock::given(method("PATCH"))
            .and(path("/v0/app123/tbl123/rec123"))
            .and(header("authorization", "Bearer test-api-key"))
            .and(body_string_contains("https://example.com/file.pdf"))
            .and(body_string_contains("file.pdf"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = attach_file(
            ctx,
            AttachFileInput {
                base_id: "app123".to_string(),
                table_id_or_name: "tbl123".to_string(),
                record_id: "rec123".to_string(),
                field_name: "Attachments".to_string(),
                attachments: vec![Attachment {
                    url: "https://example.com/file.pdf".to_string(),
                    filename: Some("file.pdf".to_string()),
                }],
            },
        )
        .await
        .unwrap();

        assert_eq!(output.record.id, "rec123");
        assert!(output.record.fields.contains_key("Attachments"));
    }

    #[tokio::test]
    async fn test_attach_file_multiple_attachments_success() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "rec123",
          "fields": {
            "Attachments": [
              {
                "url": "https://example.com/file1.pdf",
                "filename": "file1.pdf"
              },
              {
                "url": "https://example.com/file2.pdf",
                "filename": "file2.pdf"
              }
            ]
          },
          "createdTime": "2024-01-01T00:00:00.000Z"
        }
        "#;

        Mock::given(method("PATCH"))
            .and(path("/v0/app123/tbl123/rec123"))
            .and(header("authorization", "Bearer test-api-key"))
            .and(body_string_contains("file1.pdf"))
            .and(body_string_contains("file2.pdf"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = attach_file(
            ctx,
            AttachFileInput {
                base_id: "app123".to_string(),
                table_id_or_name: "tbl123".to_string(),
                record_id: "rec123".to_string(),
                field_name: "Attachments".to_string(),
                attachments: vec![
                    Attachment {
                        url: "https://example.com/file1.pdf".to_string(),
                        filename: Some("file1.pdf".to_string()),
                    },
                    Attachment {
                        url: "https://example.com/file2.pdf".to_string(),
                        filename: Some("file2.pdf".to_string()),
                    },
                ],
            },
        )
        .await
        .unwrap();

        assert_eq!(output.record.id, "rec123");
    }

    #[tokio::test]
    async fn test_attach_file_api_error_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("PATCH"))
            .and(path("/v0/app123/tbl123/rec123"))
            .respond_with(ResponseTemplate::new(404).set_body_raw(
                r#"{ "error": { "type": "MODEL_ID_NOT_FOUND", "message": "Record not found" } }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = attach_file(
            ctx,
            AttachFileInput {
                base_id: "app123".to_string(),
                table_id_or_name: "tbl123".to_string(),
                record_id: "rec123".to_string(),
                field_name: "Attachments".to_string(),
                attachments: vec![Attachment {
                    url: "https://example.com/file.pdf".to_string(),
                    filename: None,
                }],
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("404"));
    }

    // ===== list_tables tests =====

    #[tokio::test]
    async fn test_list_tables_success_returns_tables() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "tables": [
            {
              "id": "tbl123",
              "name": "My Table",
              "description": "A test table",
              "primaryFieldId": "fld456"
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v0/meta/bases/app123/tables"))
            .and(header("authorization", "Bearer test-api-key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_tables(
            ctx,
            ListTablesInput {
                base_id: "app123".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.tables.len(), 1);
        assert_eq!(output.tables[0].id, "tbl123");
        assert_eq!(output.tables[0].name, "My Table");
        assert_eq!(
            output.tables[0].description,
            Some("A test table".to_string())
        );
        assert_eq!(
            output.tables[0].primary_field_id,
            Some("fld456".to_string())
        );
    }

    #[tokio::test]
    async fn test_list_tables_empty_base_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_tables(
            ctx,
            ListTablesInput {
                base_id: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("base_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_list_tables_invalid_base_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_tables(
            ctx,
            ListTablesInput {
                base_id: "invalid".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("base_id must start with 'app'")
        );
    }

    #[tokio::test]
    async fn test_list_tables_api_error_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path("/v0/meta/bases/app123/tables"))
            .respond_with(ResponseTemplate::new(404).set_body_raw(
                r#"{ "error": { "type": "BASE_NOT_FOUND", "message": "Base not found" } }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = list_tables(
            ctx,
            ListTablesInput {
                base_id: "app123".to_string(),
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("404"));
    }

    // ===== search_records tests =====

    #[tokio::test]
    async fn test_search_records_success_returns_records() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "records": [
            {
              "id": "rec123",
              "fields": {
                "Name": "John Doe",
                "Email": "john@example.com"
              },
              "createdTime": "2024-01-01T00:00:00.000Z"
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v0/app123/tbl123"))
            .and(header("authorization", "Bearer test-api-key"))
            .and(query_param("maxRecords", "10"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = search_records(
            ctx,
            SearchRecordsInput {
                base_id: "app123".to_string(),
                table_id_or_name: "tbl123".to_string(),
                filter_by_formula: None,
                max_records: Some(10),
                sort: vec![],
                view: None,
                offset: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.records.len(), 1);
        assert_eq!(output.records[0].id, "rec123");
        assert_eq!(
            output.records[0].fields.get("Name").unwrap(),
            &serde_json::json!("John Doe")
        );
    }

    #[tokio::test]
    async fn test_search_records_with_formula_and_sort() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"{ "records": [] }"#;

        Mock::given(method("GET"))
            .and(path("/v0/app123/tbl123"))
            .and(header("authorization", "Bearer test-api-key"))
            .and(query_param("filterByFormula", "NOT({Status}='Done')"))
            .and(query_param("sort[0][field]", "Priority"))
            .and(query_param("sort[0][direction]", "desc"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = search_records(
            ctx,
            SearchRecordsInput {
                base_id: "app123".to_string(),
                table_id_or_name: "tbl123".to_string(),
                filter_by_formula: Some("NOT({Status}='Done')".to_string()),
                max_records: None,
                sort: vec!["-Priority".to_string()],
                view: None,
                offset: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.records.len(), 0);
    }

    #[tokio::test]
    async fn test_search_records_empty_base_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = search_records(
            ctx,
            SearchRecordsInput {
                base_id: "  ".to_string(),
                table_id_or_name: "tbl123".to_string(),
                filter_by_formula: None,
                max_records: None,
                sort: vec![],
                view: None,
                offset: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("base_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_search_records_invalid_max_records_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = search_records(
            ctx,
            SearchRecordsInput {
                base_id: "app123".to_string(),
                table_id_or_name: "tbl123".to_string(),
                filter_by_formula: None,
                max_records: Some(101),
                sort: vec![],
                view: None,
                offset: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("max_records must be between 1 and 100")
        );
    }

    #[tokio::test]
    async fn test_search_records_empty_formula_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = search_records(
            ctx,
            SearchRecordsInput {
                base_id: "app123".to_string(),
                table_id_or_name: "tbl123".to_string(),
                filter_by_formula: Some("  ".to_string()),
                max_records: None,
                sort: vec![],
                view: None,
                offset: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("filter_by_formula must not be empty")
        );
    }

    // ===== create_record tests =====

    #[tokio::test]
    async fn test_create_record_success_returns_record() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "rec123",
          "fields": {
            "Name": "Jane Doe",
            "Status": "Active"
          },
          "createdTime": "2024-01-01T00:00:00.000Z"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/v0/app123/tbl123"))
            .and(header("authorization", "Bearer test-api-key"))
            .and(body_string_contains("Jane Doe"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let mut fields = HashMap::new();
        fields.insert("Name".to_string(), serde_json::json!("Jane Doe"));
        fields.insert("Status".to_string(), serde_json::json!("Active"));

        let ctx = test_ctx(&endpoint);
        let output = create_record(
            ctx,
            CreateRecordInput {
                base_id: "app123".to_string(),
                table_id_or_name: "tbl123".to_string(),
                fields,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.record.id, "rec123");
        assert_eq!(
            output.record.fields.get("Name").unwrap(),
            &serde_json::json!("Jane Doe")
        );
    }

    #[tokio::test]
    async fn test_create_record_empty_fields_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = create_record(
            ctx,
            CreateRecordInput {
                base_id: "app123".to_string(),
                table_id_or_name: "tbl123".to_string(),
                fields: HashMap::new(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("fields must not be empty")
        );
    }

    #[tokio::test]
    async fn test_create_record_invalid_base_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let mut fields = HashMap::new();
        fields.insert("Name".to_string(), serde_json::json!("Test"));

        let result = create_record(
            ctx,
            CreateRecordInput {
                base_id: "invalid".to_string(),
                table_id_or_name: "tbl123".to_string(),
                fields,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("base_id must start with 'app'")
        );
    }

    #[tokio::test]
    async fn test_create_record_api_error_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("POST"))
            .and(path("/v0/app123/tbl123"))
            .respond_with(ResponseTemplate::new(422).set_body_raw(
                r#"{ "error": { "type": "INVALID_REQUEST_UNKNOWN", "message": "Invalid field" } }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let mut fields = HashMap::new();
        fields.insert("InvalidField".to_string(), serde_json::json!("value"));

        let ctx = test_ctx(&endpoint);
        let result = create_record(
            ctx,
            CreateRecordInput {
                base_id: "app123".to_string(),
                table_id_or_name: "tbl123".to_string(),
                fields,
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("422"));
    }

    // ===== update_record tests =====

    #[tokio::test]
    async fn test_update_record_success_returns_updated_record() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "rec123",
          "fields": {
            "Name": "Updated Name",
            "Status": "Complete"
          },
          "createdTime": "2024-01-01T00:00:00.000Z"
        }
        "#;

        Mock::given(method("PATCH"))
            .and(path("/v0/app123/tbl123/rec123"))
            .and(header("authorization", "Bearer test-api-key"))
            .and(body_string_contains("Updated Name"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let mut fields = HashMap::new();
        fields.insert("Name".to_string(), serde_json::json!("Updated Name"));
        fields.insert("Status".to_string(), serde_json::json!("Complete"));

        let ctx = test_ctx(&endpoint);
        let output = update_record(
            ctx,
            UpdateRecordInput {
                base_id: "app123".to_string(),
                table_id_or_name: "tbl123".to_string(),
                record_id: "rec123".to_string(),
                fields,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.record.id, "rec123");
        assert_eq!(
            output.record.fields.get("Name").unwrap(),
            &serde_json::json!("Updated Name")
        );
    }

    #[tokio::test]
    async fn test_update_record_invalid_record_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let mut fields = HashMap::new();
        fields.insert("Name".to_string(), serde_json::json!("Test"));

        let result = update_record(
            ctx,
            UpdateRecordInput {
                base_id: "app123".to_string(),
                table_id_or_name: "tbl123".to_string(),
                record_id: "invalid".to_string(),
                fields,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("record_id must start with 'rec'")
        );
    }

    #[tokio::test]
    async fn test_update_record_empty_fields_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = update_record(
            ctx,
            UpdateRecordInput {
                base_id: "app123".to_string(),
                table_id_or_name: "tbl123".to_string(),
                record_id: "rec123".to_string(),
                fields: HashMap::new(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("fields must not be empty")
        );
    }

    #[tokio::test]
    async fn test_update_record_api_error_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("PATCH"))
            .and(path("/v0/app123/tbl123/rec999"))
            .respond_with(ResponseTemplate::new(404).set_body_raw(
                r#"{ "error": { "type": "MODEL_ID_NOT_FOUND", "message": "Record not found" } }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let mut fields = HashMap::new();
        fields.insert("Name".to_string(), serde_json::json!("Test"));

        let ctx = test_ctx(&endpoint);
        let result = update_record(
            ctx,
            UpdateRecordInput {
                base_id: "app123".to_string(),
                table_id_or_name: "tbl123".to_string(),
                record_id: "rec999".to_string(),
                fields,
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("404"));
    }

    // ===== list_bases additional tests =====

    #[tokio::test]
    async fn test_list_bases_with_offset_returns_pagination_token() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "bases": [
            {
              "id": "app123",
              "name": "Test Base",
              "permissionLevel": "owner"
            }
          ],
          "offset": "itrz2MwVk3OFp8ZY4/eyJwdWIiOiJvYmplY3RfYXBpX2Jhc2VfcmVhZCIsInYiOjEsImMiOiJjb20uYWlydGFibGUuYXBpOmIzMDU0ZWM1LWU4ZGQtND"
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v0/meta/bases"))
            .and(header("authorization", "Bearer test-api-key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_bases(
            ctx,
            ListBasesInput {
                limit: None,
                offset: None,
            },
        )
        .await
        .unwrap();

        assert!(output.offset.is_some());
        assert!(output.offset.as_ref().unwrap().starts_with("itrz"));
    }

    #[tokio::test]
    async fn test_list_bases_invalid_limit_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_bases(
            ctx,
            ListBasesInput {
                limit: Some(101),
                offset: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("limit must be between 1 and 100")
        );
    }

    #[tokio::test]
    async fn test_list_bases_empty_offset_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_bases(
            ctx,
            ListBasesInput {
                limit: None,
                offset: Some("  ".to_string()),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("offset must not be empty")
        );
    }

    #[tokio::test]
    async fn test_list_bases_api_error_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path("/v0/meta/bases"))
            .respond_with(ResponseTemplate::new(401).set_body_raw(
                r#"{ "error": { "type": "INVALID_AUTHORIZATION", "message": "Invalid token" } }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = list_bases(
            ctx,
            ListBasesInput {
                limit: None,
                offset: None,
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("401"));
    }
}
