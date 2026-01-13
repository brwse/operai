//! team-chat/zulip integration for Operai Toolbox.
use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};

mod types;
use types::{
    Message, MessagesData, SendMessageData, Stream, StreamsData, TopicsData, ZulipResponse,
    map_message, map_stream,
};

define_user_credential! {
    ZulipCredential("zulip") {
        email: String,
        api_key: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_ZULIP_ENDPOINT: &str = "https://chat.zulip.org/api/v1";

#[init]
async fn setup() -> Result<()> {
    info!("Zulip integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Zulip integration shutting down");
}

// Input/Output types

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListStreamsInput {
    /// Include all public streams. Defaults to true.
    #[serde(default)]
    pub include_public: Option<bool>,
    /// Include subscribed streams only. Defaults to false.
    #[serde(default)]
    pub include_subscribed: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListStreamsOutput {
    pub streams: Vec<Stream>,
}

/// # List Zulip Streams
///
/// Lists and retrieves all available streams (channels) in a Zulip workspace.
///
/// Use this tool when you need to:
/// - Browse or discover available streams in the workspace
/// - Display a list of streams for the user to choose from
/// - Verify a stream exists before performing operations on it
/// - Get stream metadata (name, description, ID, permissions)
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - zulip
/// - team-chat
/// - streams
///
/// # Errors
///
/// Returns an error if:
/// - The user's Zulip credentials are not configured or are invalid
/// - The Zulip API request fails due to network or server issues
/// - The Zulip API returns an error response (e.g., authentication failure)
#[tool]
pub async fn list_streams(ctx: Context, input: ListStreamsInput) -> Result<ListStreamsOutput> {
    let client = ZulipClient::from_ctx(&ctx)?;

    let include_public = input.include_public.unwrap_or(true);
    let query = vec![
        ("include_public", include_public.to_string()),
        ("include_subscribed", input.include_subscribed.to_string()),
    ];

    let response: ZulipResponse<StreamsData> = client.get_json("streams", &query).await?;

    ensure!(
        response.result == "success",
        "Zulip API error: {}",
        response.msg
    );

    let streams = response
        .data
        .map(|d| d.streams.into_iter().map(map_stream).collect())
        .unwrap_or_default();

    Ok(ListStreamsOutput { streams })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SendMessageInput {
    /// Message type: "stream" or "direct"
    #[serde(rename = "type")]
    pub message_type: String,
    /// For stream messages: stream name or ID
    #[serde(default)]
    pub to: Option<String>,
    /// For stream messages: topic name
    #[serde(default)]
    pub topic: Option<String>,
    /// Message content (supports Zulip markdown)
    pub content: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SendMessageOutput {
    pub id: i64,
}

/// # Send Zulip Message
///
/// Sends a message to a Zulip stream or as a direct message.
///
/// Use this tool when the user wants to:
/// - Post a new message to a stream topic
/// - Send a direct message to one or more users
/// - Announce information in a channel
/// - Reply to or continue a conversation
///
/// For stream messages, you must provide both the 'to' (stream name) and
/// 'topic' fields. The content field supports Zulip markdown formatting for
/// rich text.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - zulip
/// - team-chat
/// - messaging
///
/// # Errors
///
/// Returns an error if:
/// - The content is empty or contains only whitespace
/// - The message type is "stream" but the 'to' field (stream name) is missing
/// - The message type is "stream" but the 'topic' field is missing
/// - The user's Zulip credentials are not configured or are invalid
/// - The Zulip API request fails due to network or server issues
/// - The Zulip API returns an error response (e.g., authentication failure,
///   stream not found)
pub async fn send_message(ctx: Context, input: SendMessageInput) -> Result<SendMessageOutput> {
    ensure!(
        !input.content.trim().is_empty(),
        "content must not be empty"
    );

    if input.message_type == "stream" {
        ensure!(
            input.to.is_some(),
            "stream messages require 'to' field (stream name)"
        );
        ensure!(
            input.topic.is_some(),
            "stream messages require 'topic' field"
        );
    }

    let client = ZulipClient::from_ctx(&ctx)?;

    let mut body = serde_json::json!({
        "type": input.message_type,
        "content": input.content,
    });

    if let Some(to) = input.to {
        body["to"] = serde_json::json!(to);
    }

    if let Some(topic) = input.topic {
        body["topic"] = serde_json::json!(topic);
    }

    let response: ZulipResponse<SendMessageData> = client.post_json("messages", &body).await?;

    ensure!(
        response.result == "success",
        "Zulip API error: {}",
        response.msg
    );

    let data = response
        .data
        .ok_or_else(|| operai::anyhow::anyhow!("Missing response data"))?;

    Ok(SendMessageOutput { id: data.id })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadTopicInput {
    /// Stream name or ID
    pub stream: String,
    /// Topic name
    pub topic: String,
    /// Maximum number of messages (1-5000). Defaults to 100.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReadTopicOutput {
    pub messages: Vec<Message>,
}

/// # Read Zulip Topic
///
/// Retrieves messages from a specific topic within a Zulip stream.
///
/// Use this tool when you need to:
/// - Catch up on the conversation history of a topic
/// - Review previous messages in a thread
/// - Search for specific information discussed in a topic
/// - Provide context or summarize a conversation
/// - Check recent activity in a topic
///
/// Messages are returned in reverse chronological order (newest first) based on
/// the limit specified. The limit parameter allows fetching between 1 and 5000
/// messages, defaulting to 100 if not specified.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - zulip
/// - team-chat
/// - messaging
///
/// # Errors
///
/// Returns an error if:
/// - The stream name is empty or contains only whitespace
/// - The topic name is empty or contains only whitespace
/// - The limit is not between 1 and 5000
/// - The user's Zulip credentials are not configured or are invalid
/// - The Zulip API request fails due to network or server issues
/// - The Zulip API returns an error response (e.g., authentication failure,
///   stream not found)
pub async fn read_topic(ctx: Context, input: ReadTopicInput) -> Result<ReadTopicOutput> {
    ensure!(!input.stream.trim().is_empty(), "stream must not be empty");
    ensure!(!input.topic.trim().is_empty(), "topic must not be empty");

    let limit = input.limit.unwrap_or(100);
    ensure!(
        (1..=5000).contains(&limit),
        "limit must be between 1 and 5000"
    );

    let client = ZulipClient::from_ctx(&ctx)?;

    // Build narrow filter for stream + topic
    let narrow = serde_json::json!([
        {"operator": "stream", "operand": input.stream},
        {"operator": "topic", "operand": input.topic},
    ]);

    let query = vec![
        ("anchor", "newest".to_string()),
        ("num_before", limit.to_string()),
        ("num_after", "0".to_string()),
        ("narrow", narrow.to_string()),
    ];

    let response: ZulipResponse<MessagesData> = client.get_json("messages", &query).await?;

    ensure!(
        response.result == "success",
        "Zulip API error: {}",
        response.msg
    );

    let messages = response
        .data
        .map(|d| d.messages.into_iter().map(map_message).collect())
        .unwrap_or_default();

    Ok(ReadTopicOutput { messages })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ResolveTopicInput {
    /// Stream name or ID
    pub stream: String,
    /// Topic name
    pub topic: String,
    /// Propagate change to all messages in topic. Defaults to true.
    #[serde(default)]
    pub propagate_mode: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ResolveTopicOutput {
    pub updated: bool,
    pub new_topic: String,
}

/// # Resolve Zulip Topic
///
/// Marks a Zulip topic as resolved by prepending a checkmark (✔) to the topic
/// name.
///
/// Use this tool when the user wants to:
/// - Mark a conversation or task as completed
/// - Indicate that a topic has been resolved or closed
/// - Organize threads by moving resolved topics to a "done" state
/// - Signal that further discussion on a topic is not needed
///
/// This tool follows Zulip's standard resolution pattern by adding the "✔ "
/// prefix to the topic name. If a topic is already resolved (already has the
/// checkmark prefix), the tool will confirm the existing resolved state.
///
/// Note: This operation modifies the topic name for all messages in the topic
/// by default. Use the `propagate_mode` parameter to control the scope of
/// changes if needed.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - zulip
/// - team-chat
/// - topics
///
/// # Errors
///
/// Returns an error if:
/// - The stream name is empty or contains only whitespace
/// - The topic name is empty or contains only whitespace
/// - The user's Zulip credentials are not configured or are invalid
/// - The Zulip API request fails due to network or server issues
/// - The Zulip API returns an error response (e.g., authentication failure,
///   stream not found, topic not found)
/// - The stream or topic cannot be found in the workspace
pub async fn resolve_topic(ctx: Context, input: ResolveTopicInput) -> Result<ResolveTopicOutput> {
    ensure!(!input.stream.trim().is_empty(), "stream must not be empty");
    ensure!(!input.topic.trim().is_empty(), "topic must not be empty");

    let client = ZulipClient::from_ctx(&ctx)?;

    // Get the stream ID first
    let streams_response: ZulipResponse<StreamsData> = client.get_json("streams", &[]).await?;

    ensure!(
        streams_response.result == "success",
        "Failed to fetch streams"
    );

    let stream_id = streams_response
        .data
        .and_then(|d| {
            d.streams
                .into_iter()
                .find(|s| s.name == input.stream)
                .map(|s| s.id)
        })
        .ok_or_else(|| operai::anyhow::anyhow!("Stream not found: {}", input.stream))?;

    // Get topics in the stream
    let topics_response: ZulipResponse<TopicsData> = client
        .get_json(&format!("streams/{stream_id}/topics"), &[])
        .await?;

    ensure!(
        topics_response.result == "success",
        "Failed to fetch topics"
    );

    let topic = topics_response
        .data
        .and_then(|d| d.topics.into_iter().find(|t| t.name == input.topic))
        .ok_or_else(|| operai::anyhow::anyhow!("Topic not found: {}", input.topic))?;

    // Zulip resolves topics by prepending "✔ " to the topic name
    let new_topic = if input.topic.starts_with("✔ ") {
        input.topic.clone() // Already resolved
    } else {
        format!("✔ {}", input.topic)
    };

    // Update the topic name on the first message
    let body = serde_json::json!({
        "topic": new_topic,
        "propagate_mode": input.propagate_mode.unwrap_or_else(|| "change_all".to_string()),
    });

    let response: ZulipResponse<serde_json::Value> = client
        .patch_json(&format!("messages/{}", topic.max_id), &body)
        .await?;

    ensure!(
        response.result == "success",
        "Zulip API error: {}",
        response.msg
    );

    Ok(ResolveTopicOutput {
        updated: true,
        new_topic,
    })
}

// HTTP Client

#[derive(Debug, Clone)]
struct ZulipClient {
    http: reqwest::Client,
    base_url: String,
    email: String,
    api_key: String,
}

impl ZulipClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = ZulipCredential::get(ctx)?;
        ensure!(!cred.email.trim().is_empty(), "email must not be empty");
        ensure!(!cred.api_key.trim().is_empty(), "api_key must not be empty");

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_ZULIP_ENDPOINT))?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            email: cred.email,
            api_key: cred.api_key,
        })
    }

    fn url_with_path(&self, path: &str) -> Result<reqwest::Url> {
        let url_str = format!("{}/{}", self.base_url, path);
        Ok(reqwest::Url::parse(&url_str)?)
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T> {
        let url = self.url_with_path(path)?;
        let response = self.send_request(self.http.get(url).query(query)).await?;
        Ok(response.json::<T>().await?)
    }

    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &TReq,
    ) -> Result<TRes> {
        let url = self.url_with_path(path)?;
        let response = self.send_request(self.http.post(url).json(body)).await?;
        Ok(response.json::<TRes>().await?)
    }

    async fn patch_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &TReq,
    ) -> Result<TRes> {
        let url = self.url_with_path(path)?;
        let response = self.send_request(self.http.patch(url).json(body)).await?;
        Ok(response.json::<TRes>().await?)
    }

    async fn send_request(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let response = request
            .basic_auth(&self.email, Some(&self.api_key))
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "Zulip API request failed ({status}): {body}"
            ))
        }
    }
}

fn normalize_base_url(endpoint: &str) -> Result<String> {
    let trimmed = endpoint.trim();
    ensure!(!trimmed.is_empty(), "endpoint must not be empty");
    Ok(trimmed.trim_end_matches('/').to_string())
}

// Required for the tool to be dynamically loadable by the toolbox runtime.
operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{basic_auth, method, path, query_param},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut zulip_values = HashMap::new();
        zulip_values.insert("email".to_string(), "bot@example.com".to_string());
        zulip_values.insert("api_key".to_string(), "test-key".to_string());
        zulip_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("zulip", zulip_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        format!("{}/api/v1", server.uri())
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_list_streams_input_deserializes_with_defaults() {
        let json = r"{}";
        let input: ListStreamsInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.include_public, None);
        assert!(!input.include_subscribed);
    }

    #[test]
    fn test_send_message_input_deserializes() {
        let json = r#"{
            "type": "stream",
            "to": "general",
            "topic": "test",
            "content": "Hello"
        }"#;
        let input: SendMessageInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.message_type, "stream");
        assert_eq!(input.to, Some("general".to_string()));
        assert_eq!(input.topic, Some("test".to_string()));
        assert_eq!(input.content, "Hello");
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://chat.zulip.org/api/v1/").unwrap();
        assert_eq!(result, "https://chat.zulip.org/api/v1");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  https://chat.zulip.org/api/v1  ").unwrap();
        assert_eq!(result, "https://chat.zulip.org/api/v1");
    }

    #[test]
    fn test_normalize_base_url_empty_returns_error() {
        let result = normalize_base_url("");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("must not be empty")
        );
    }

    // --- Input validation tests ---

    #[tokio::test]
    async fn test_send_message_empty_content_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = send_message(
            ctx,
            SendMessageInput {
                message_type: "stream".to_string(),
                to: Some("general".to_string()),
                topic: Some("test".to_string()),
                content: "   ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("content must not be empty")
        );
    }

    #[tokio::test]
    async fn test_send_message_stream_without_to_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = send_message(
            ctx,
            SendMessageInput {
                message_type: "stream".to_string(),
                to: None,
                topic: Some("test".to_string()),
                content: "Hello".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_send_message_stream_without_topic_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = send_message(
            ctx,
            SendMessageInput {
                message_type: "stream".to_string(),
                to: Some("general".to_string()),
                topic: None,
                content: "Hello".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_topic_empty_stream_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = read_topic(
            ctx,
            ReadTopicInput {
                stream: "  ".to_string(),
                topic: "test".to_string(),
                limit: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("stream must not be empty")
        );
    }

    #[tokio::test]
    async fn test_read_topic_empty_topic_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = read_topic(
            ctx,
            ReadTopicInput {
                stream: "general".to_string(),
                topic: "  ".to_string(),
                limit: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("topic must not be empty")
        );
    }

    #[tokio::test]
    async fn test_read_topic_limit_too_high_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = read_topic(
            ctx,
            ReadTopicInput {
                stream: "general".to_string(),
                topic: "test".to_string(),
                limit: Some(6000),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("limit must be between 1 and 5000")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_list_streams_success_returns_streams() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "result": "success",
          "msg": "",
          "streams": [
            {
              "stream_id": 1,
              "name": "general",
              "description": "General discussion",
              "is_web_public": false,
              "is_announcement_only": false,
              "stream_post_policy": 1,
              "history_public_to_subscribers": true
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/api/v1/streams"))
            .and(basic_auth("bot@example.com", "test-key"))
            .and(query_param("include_public", "true"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_streams(
            ctx,
            ListStreamsInput {
                include_public: Some(true),
                include_subscribed: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.streams.len(), 1);
        assert_eq!(output.streams[0].id, 1);
        assert_eq!(output.streams[0].name, "general");
    }

    #[tokio::test]
    async fn test_send_message_success_returns_id() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "result": "success",
          "msg": "",
          "id": 42
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/api/v1/messages"))
            .and(basic_auth("bot@example.com", "test-key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = send_message(
            ctx,
            SendMessageInput {
                message_type: "stream".to_string(),
                to: Some("general".to_string()),
                topic: Some("test".to_string()),
                content: "Hello!".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.id, 42);
    }

    #[tokio::test]
    async fn test_read_topic_success_returns_messages() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "result": "success",
          "msg": "",
          "messages": [
            {
              "id": 100,
              "sender_id": 1,
              "sender_full_name": "Alice",
              "sender_email": "alice@example.com",
              "timestamp": 1704067200,
              "content": "Hello!",
              "type": "stream",
              "stream_id": 1,
              "subject": "test"
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/api/v1/messages"))
            .and(basic_auth("bot@example.com", "test-key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = read_topic(
            ctx,
            ReadTopicInput {
                stream: "general".to_string(),
                topic: "test".to_string(),
                limit: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.messages.len(), 1);
        assert_eq!(output.messages[0].id, 100);
        assert_eq!(output.messages[0].sender_full_name, "Alice");
    }

    #[tokio::test]
    async fn test_list_streams_error_response_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path("/api/v1/streams"))
            .respond_with(ResponseTemplate::new(401).set_body_raw(
                r#"{ "result": "error", "msg": "Invalid API key" }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = list_streams(
            ctx,
            ListStreamsInput {
                include_public: Some(true),
                include_subscribed: false,
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("401"));
    }

    // --- resolve_topic tests ---

    #[tokio::test]
    async fn test_resolve_topic_empty_stream_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = resolve_topic(
            ctx,
            ResolveTopicInput {
                stream: "  ".to_string(),
                topic: "test".to_string(),
                propagate_mode: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("stream must not be empty")
        );
    }

    #[tokio::test]
    async fn test_resolve_topic_empty_topic_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = resolve_topic(
            ctx,
            ResolveTopicInput {
                stream: "general".to_string(),
                topic: "  ".to_string(),
                propagate_mode: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("topic must not be empty")
        );
    }

    #[tokio::test]
    async fn test_resolve_topic_success_returns_updated_topic() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        // Mock get streams
        let streams_body = r#"
        {
          "result": "success",
          "msg": "",
          "streams": [
            {
              "stream_id": 123,
              "name": "general",
              "description": "General discussion",
              "is_web_public": false,
              "is_announcement_only": false
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/api/v1/streams"))
            .and(basic_auth("bot@example.com", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(streams_body, "application/json"))
            .mount(&server)
            .await;

        // Mock get topics for stream
        let topics_body = r#"
        {
          "result": "success",
          "msg": "",
          "topics": [
            {
              "name": "Bug fix needed",
              "max_id": 456
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/api/v1/streams/123/topics"))
            .and(basic_auth("bot@example.com", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(topics_body, "application/json"))
            .mount(&server)
            .await;

        // Mock update message
        let update_body = r#"
        {
          "result": "success",
          "msg": ""
        }
        "#;

        Mock::given(method("PATCH"))
            .and(path("/api/v1/messages/456"))
            .and(basic_auth("bot@example.com", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(update_body, "application/json"))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = resolve_topic(
            ctx,
            ResolveTopicInput {
                stream: "general".to_string(),
                topic: "Bug fix needed".to_string(),
                propagate_mode: None,
            },
        )
        .await
        .unwrap();

        assert!(output.updated);
        assert_eq!(output.new_topic, "✔ Bug fix needed");
    }

    #[tokio::test]
    async fn test_resolve_topic_already_resolved_returns_same_topic() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        // Mock get streams
        let streams_body = r#"
        {
          "result": "success",
          "msg": "",
          "streams": [
            {
              "stream_id": 123,
              "name": "general",
              "description": "General discussion",
              "is_web_public": false,
              "is_announcement_only": false
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/api/v1/streams"))
            .and(basic_auth("bot@example.com", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(streams_body, "application/json"))
            .mount(&server)
            .await;

        // Mock get topics for stream
        let topics_body = r#"
        {
          "result": "success",
          "msg": "",
          "topics": [
            {
              "name": "✔ Bug fix needed",
              "max_id": 456
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/api/v1/streams/123/topics"))
            .and(basic_auth("bot@example.com", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(topics_body, "application/json"))
            .mount(&server)
            .await;

        // Mock update message
        let update_body = r#"
        {
          "result": "success",
          "msg": ""
        }
        "#;

        Mock::given(method("PATCH"))
            .and(path("/api/v1/messages/456"))
            .and(basic_auth("bot@example.com", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(update_body, "application/json"))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = resolve_topic(
            ctx,
            ResolveTopicInput {
                stream: "general".to_string(),
                topic: "✔ Bug fix needed".to_string(),
                propagate_mode: None,
            },
        )
        .await
        .unwrap();

        assert!(output.updated);
        assert_eq!(output.new_topic, "✔ Bug fix needed");
    }

    #[tokio::test]
    async fn test_resolve_topic_stream_not_found_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        // Mock get streams with empty list
        let streams_body = r#"
        {
          "result": "success",
          "msg": "",
          "streams": []
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/api/v1/streams"))
            .and(basic_auth("bot@example.com", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(streams_body, "application/json"))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = resolve_topic(
            ctx,
            ResolveTopicInput {
                stream: "nonexistent".to_string(),
                topic: "test".to_string(),
                propagate_mode: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Stream not found"));
    }

    #[tokio::test]
    async fn test_resolve_topic_topic_not_found_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        // Mock get streams
        let streams_body = r#"
        {
          "result": "success",
          "msg": "",
          "streams": [
            {
              "stream_id": 123,
              "name": "general",
              "description": "General discussion",
              "is_web_public": false,
              "is_announcement_only": false
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/api/v1/streams"))
            .and(basic_auth("bot@example.com", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(streams_body, "application/json"))
            .mount(&server)
            .await;

        // Mock get topics with empty list
        let topics_body = r#"
        {
          "result": "success",
          "msg": "",
          "topics": []
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/api/v1/streams/123/topics"))
            .and(basic_auth("bot@example.com", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(topics_body, "application/json"))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = resolve_topic(
            ctx,
            ResolveTopicInput {
                stream: "general".to_string(),
                topic: "nonexistent".to_string(),
                propagate_mode: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Topic not found"));
    }
}
