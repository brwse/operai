//! Asana API client implementation.
//!
//! Provides HTTP client functionality for interacting with the Asana API.

use std::sync::Arc;

use operai::{Context, Result};
use reqwest::{
    Client,
    header::{AUTHORIZATION, HeaderMap},
};

use crate::types::{
    AsanaErrorResponse, AsanaListResponse, AsanaResponse, CreateStoryRequest, CreateTaskRequest,
    UpdateTaskRequest,
};

/// Base URL for the Asana API.
pub const ASANA_API_BASE: &str = "https://app.asana.com/api/1.0";

/// Asana API client.
#[derive(Clone)]
pub struct AsanaClient {
    /// HTTP client for making requests.
    pub(crate) client: Arc<Client>,
}

impl AsanaClient {
    /// Creates a new Asana API client.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created.
    pub fn new(access_token: &str) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            format!("Bearer {access_token}").parse().unwrap(),
        );
        headers.insert("Accept", "application/json".parse().unwrap());

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|e| operai::anyhow::anyhow!("Failed to create HTTP client: {e}"))?;

        Ok(Self {
            client: Arc::new(client),
        })
    }

    /// Lists all projects in a workspace.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The workspace GID is invalid
    /// - Authentication fails
    /// - Network request fails
    /// - API returns an error response
    /// - Response deserialization fails
    pub async fn list_projects(
        &self,
        workspace_gid: &str,
        archived: bool,
        limit: Option<u32>,
    ) -> Result<Vec<crate::types::AsanaApiProject>> {
        let limit = limit.unwrap_or(100);
        let url = format!("{ASANA_API_BASE}/workspaces/{workspace_gid}/projects");

        let response = self
            .client
            .get(&url)
            .query(&[
                ("archived", archived.to_string()),
                ("limit", limit.to_string()),
            ])
            .send()
            .await
            .map_err(|e| operai::anyhow::anyhow!("Failed to send request: {e}"))?;

        response.to_list_result().await
    }

    /// Lists tasks in a project.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The project GID is invalid
    /// - Authentication fails
    /// - Network request fails
    /// - API returns an error response
    /// - Response deserialization fails
    pub async fn list_tasks(
        &self,
        project_gid: &str,
        completed: Option<bool>,
        assignee_gid: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Vec<crate::types::AsanaApiTask>> {
        let limit = limit.unwrap_or(100);
        let url = format!("{ASANA_API_BASE}/projects/{project_gid}/tasks");

        let mut query_params = vec![("limit", limit.to_string())];
        if let Some(completed) = completed {
            query_params.push(("completed", completed.to_string()));
        }
        if let Some(assignee) = assignee_gid {
            query_params.push(("assignee", assignee.to_string()));
        }

        let response = self
            .client
            .get(&url)
            .query(&query_params)
            .send()
            .await
            .map_err(|e| operai::anyhow::anyhow!("Failed to send request: {e}"))?;

        response.to_list_result().await
    }

    /// Creates a new task.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Authentication fails
    /// - Network request fails
    /// - API returns an error response
    /// - Response deserialization fails
    pub async fn create_task(
        &self,
        request: CreateTaskRequest,
    ) -> Result<crate::types::AsanaApiTask> {
        let url = format!("{ASANA_API_BASE}/tasks");

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| operai::anyhow::anyhow!("Failed to send request: {e}"))?;

        response.to_result().await
    }

    /// Updates a task's completion status.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The task GID is invalid
    /// - Authentication fails
    /// - Network request fails
    /// - API returns an error response
    /// - Response deserialization fails
    pub async fn update_task_completed(
        &self,
        task_gid: &str,
        completed: bool,
    ) -> Result<crate::types::AsanaApiTask> {
        let url = format!("{ASANA_API_BASE}/tasks/{task_gid}");

        let request = UpdateTaskRequest {
            data: crate::types::UpdateTaskData {
                completed: Some(completed),
                assignee: None,
            },
        };

        let response = self
            .client
            .put(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| operai::anyhow::anyhow!("Failed to send request: {e}"))?;

        response.to_result().await
    }

    /// Updates a task's assignee.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The task GID is invalid
    /// - Authentication fails
    /// - Network request fails
    /// - API returns an error response
    /// - Response deserialization fails
    pub async fn update_task_assignee(
        &self,
        task_gid: &str,
        assignee_gid: Option<&str>,
    ) -> Result<crate::types::AsanaApiTask> {
        let url = format!("{ASANA_API_BASE}/tasks/{task_gid}");

        let request = UpdateTaskRequest {
            data: crate::types::UpdateTaskData {
                completed: None,
                assignee: assignee_gid.map(std::string::ToString::to_string),
            },
        };

        let response = self
            .client
            .put(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| operai::anyhow::anyhow!("Failed to send request: {e}"))?;

        response.to_result().await
    }

    /// Adds a comment (story) to a task.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The task GID is invalid
    /// - Authentication fails
    /// - Network request fails
    /// - API returns an error response
    /// - Response deserialization fails
    pub async fn create_story(
        &self,
        task_gid: &str,
        request: CreateStoryRequest,
    ) -> Result<crate::types::AsanaApiStory> {
        let url = format!("{ASANA_API_BASE}/tasks/{task_gid}/stories");

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| operai::anyhow::anyhow!("Failed to send request: {e}"))?;

        response.to_result().await
    }
}

/// Helper function to get the Asana credential from the context.
///
/// # Errors
///
/// Returns an error if:
/// - The credential is not configured
/// - The `access_token` is missing
pub async fn get_credential(ctx: &Context) -> Result<(String, Option<String>)> {
    // Use a HashMap to deserialize the credential values
    use std::collections::HashMap;

    let cred: HashMap<String, String> = ctx
        .system_credential("asana")
        .map_err(|e| operai::anyhow::anyhow!("Failed to get credential: {e}"))?;

    let access_token = cred
        .get("access_token")
        .ok_or_else(|| operai::anyhow::anyhow!("Missing access_token in credential"))?
        .clone();

    let workspace_gid = cred
        .get("workspace_gid")
        .map(std::string::String::to_string);

    Ok((access_token, workspace_gid))
}

/// Extension trait to handle Asana API responses.
trait AsanaResponseExt: Sized {
    async fn to_result<T>(self) -> Result<T>
    where
        T: for<'de> serde::Deserialize<'de>;

    async fn to_list_result<T>(self) -> Result<Vec<T>>
    where
        T: for<'de> serde::Deserialize<'de>;
}

impl AsanaResponseExt for reqwest::Response {
    async fn to_result<T>(self) -> Result<T>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        let status = self.status();

        if !status.is_success() {
            let error_body = self
                .bytes()
                .await
                .unwrap_or_else(|_| bytes::Bytes::from_static(b"Unable to read error response"));
            let error_text = String::from_utf8_lossy(&error_body);

            // Try to parse Asana error response
            if let Ok(error_response) = serde_json::from_str::<AsanaErrorResponse>(&error_text)
                && let Some(first_error) = error_response.errors.first()
            {
                return Err(operai::anyhow::anyhow!(
                    "Asana API error: {}",
                    first_error.message
                ));
            }

            return Err(operai::anyhow::anyhow!(
                "Asana API request failed with status {}: {}",
                status.as_u16(),
                error_text
            ));
        }

        self.json::<AsanaResponse<T>>()
            .await
            .map(|r| r.data)
            .map_err(|e| operai::anyhow::anyhow!("Failed to parse response: {e}"))
    }

    async fn to_list_result<T>(self) -> Result<Vec<T>>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        let status = self.status();

        if !status.is_success() {
            let error_body = self
                .bytes()
                .await
                .unwrap_or_else(|_| bytes::Bytes::from_static(b"Unable to read error response"));
            let error_text = String::from_utf8_lossy(&error_body);

            // Try to parse Asana error response
            if let Ok(error_response) = serde_json::from_str::<AsanaErrorResponse>(&error_text)
                && let Some(first_error) = error_response.errors.first()
            {
                return Err(operai::anyhow::anyhow!(
                    "Asana API error: {}",
                    first_error.message
                ));
            }

            return Err(operai::anyhow::anyhow!(
                "Asana API request failed with status {}: {}",
                status.as_u16(),
                error_text
            ));
        }

        self.json::<AsanaListResponse<T>>()
            .await
            .map(|r| r.data)
            .map_err(|e| operai::anyhow::anyhow!("Failed to parse response: {e}"))
    }
}
