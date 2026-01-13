//! calendars-scheduling/outlook-calendar integration for Operai Toolbox.

#![warn(clippy::missing_errors_doc, clippy::missing_panics_doc, dead_code)]

mod types;

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};
pub use types::*;

define_user_credential! {
    OutlookCalendarCredential("outlook_calendar") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_GRAPH_ENDPOINT: &str = "https://graph.microsoft.com/v1.0";

#[init]
async fn setup() -> Result<()> {
    info!("Outlook Calendar integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Outlook Calendar integration shutting down");
}

// ===== List Events =====

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListEventsInput {
    /// Start date-time filter (ISO 8601).
    #[serde(default)]
    pub start: Option<String>,
    /// End date-time filter (ISO 8601).
    #[serde(default)]
    pub end: Option<String>,
    /// Maximum number of results (1-1000). Defaults to 50.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListEventsOutput {
    pub events: Vec<Event>,
}

/// # List Outlook Calendar Events
///
/// Retrieves calendar events from the authenticated user's Outlook Calendar
/// using the Microsoft Graph API.
///
/// Use this tool when a user wants to:
/// - View their upcoming calendar events
/// - Check what events occur within a specific date/time range
/// - Retrieve event details for display or analysis
///
/// The results can be filtered by date range and limited to a specific number
/// of events. Returns comprehensive event information including subject,
/// location, attendees, organizer, online meeting details, and web links.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - calendar
/// - outlook
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The `limit` parameter is not between 1 and 1000
/// - User credentials are missing or invalid (no access token configured)
/// - The Microsoft Graph API request fails due to network or authentication
///   issues
/// - The API response cannot be parsed as expected JSON format
#[tool]
pub async fn list_events(ctx: Context, input: ListEventsInput) -> Result<ListEventsOutput> {
    let limit = input.limit.unwrap_or(50);
    ensure!(
        (1..=1000).contains(&limit),
        "limit must be between 1 and 1000"
    );

    let client = GraphClient::from_ctx(&ctx)?;
    let mut query = vec![
        ("$top", limit.to_string()),
        (
            "$select",
            "id,subject,body,start,end,location,attendees,organizer,isAllDay,showAs,sensitivity,\
             isOnlineMeeting,onlineMeetingUrl,webLink"
                .to_string(),
        ),
    ];

    if input.start.is_some() || input.end.is_some() {
        let mut filter_parts = Vec::new();
        if let Some(start) = &input.start {
            filter_parts.push(format!("start/dateTime ge '{start}'"));
        }
        if let Some(end) = &input.end {
            filter_parts.push(format!("end/dateTime le '{end}'"));
        }
        query.push(("$filter", filter_parts.join(" and ")));
    }

    let response: GraphListResponse<Event> = client
        .get_json(
            client.url_with_segments(&["me", "calendar", "events"])?,
            &query,
            &[],
        )
        .await?;

    Ok(ListEventsOutput {
        events: response.value,
    })
}

// ===== Create Event =====

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateEventInput {
    /// Event subject/title.
    pub subject: String,
    /// Event body content.
    #[serde(default)]
    pub body: Option<String>,
    /// Body content type ("text" or "html"). Defaults to "text".
    #[serde(default)]
    pub body_content_type: Option<BodyContentType>,
    /// Start date-time (ISO 8601).
    pub start: String,
    /// Start time zone (e.g., "UTC", "Pacific Standard Time"). Defaults to
    /// "UTC".
    #[serde(default)]
    pub start_time_zone: Option<String>,
    /// End date-time (ISO 8601).
    pub end: String,
    /// End time zone (e.g., "UTC", "Pacific Standard Time"). Defaults to "UTC".
    #[serde(default)]
    pub end_time_zone: Option<String>,
    /// Location display name.
    #[serde(default)]
    pub location: Option<String>,
    /// Attendees to invite (email addresses).
    #[serde(default)]
    pub attendees: Vec<String>,
    /// Whether this is an all-day event.
    #[serde(default)]
    pub is_all_day: Option<bool>,
    /// How the event should be shown (free, tentative, busy, etc.).
    #[serde(default)]
    pub show_as: Option<EventShowAs>,
    /// Whether to create as an online meeting.
    #[serde(default)]
    pub is_online_meeting: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateEventOutput {
    pub event: Event,
}

/// # Create Outlook Calendar Event
///
/// Creates a new calendar event in the authenticated user's Outlook Calendar
/// using the Microsoft Graph API.
///
/// Use this tool when a user wants to:
/// - Schedule a new meeting or appointment
/// - Block time on their calendar
/// - Invite others to an event
/// - Create an all-day event or timed event
/// - Set up an online meeting (Teams/Zoom integration)
///
/// This tool supports creating events with optional attendees, locations,
/// online meeting links, and availability status (free, busy, tentative, etc.).
/// All-day events and time zone specification are also supported.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - calendar
/// - outlook
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The `subject` parameter is empty or contains only whitespace
/// - The `start` or `end` date-time parameters are empty
/// - User credentials are missing or invalid (no access token configured)
/// - The Microsoft Graph API request fails due to network or authentication
///   issues
/// - The API response cannot be parsed as expected JSON format
#[tool]
pub async fn create_event(ctx: Context, input: CreateEventInput) -> Result<CreateEventOutput> {
    ensure!(
        !input.subject.trim().is_empty(),
        "subject must not be empty"
    );
    ensure!(!input.start.trim().is_empty(), "start must not be empty");
    ensure!(!input.end.trim().is_empty(), "end must not be empty");

    let client = GraphClient::from_ctx(&ctx)?;

    let body_content_type = input.body_content_type.unwrap_or(BodyContentType::Text);
    let start_time_zone = input.start_time_zone.unwrap_or_else(|| "UTC".to_string());
    let end_time_zone = input.end_time_zone.unwrap_or_else(|| "UTC".to_string());

    let request = GraphCreateEventRequest {
        subject: input.subject,
        body: input.body.map(|content| GraphItemBody {
            content_type: body_content_type,
            content,
        }),
        start: GraphDateTimeTimeZone {
            date_time: input.start,
            time_zone: start_time_zone,
        },
        end: GraphDateTimeTimeZone {
            date_time: input.end,
            time_zone: end_time_zone,
        },
        location: input.location.map(|display_name| GraphLocation {
            display_name: Some(display_name),
            location_uri: None,
        }),
        attendees: input
            .attendees
            .into_iter()
            .map(|email| GraphAttendee {
                email_address: GraphEmailAddress {
                    address: email,
                    name: None,
                },
                attendee_type: AttendeeType::Required,
            })
            .collect(),
        is_all_day: input.is_all_day,
        show_as: input.show_as,
        is_online_meeting: input.is_online_meeting,
    };

    let event: Event = client
        .post_json(
            client.url_with_segments(&["me", "calendar", "events"])?,
            &request,
            &[],
        )
        .await?;

    Ok(CreateEventOutput { event })
}

// ===== Update Event =====

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEventInput {
    /// Event ID to update.
    pub event_id: String,
    /// New subject/title.
    #[serde(default)]
    pub subject: Option<String>,
    /// New body content.
    #[serde(default)]
    pub body: Option<String>,
    /// Body content type.
    #[serde(default)]
    pub body_content_type: Option<BodyContentType>,
    /// New start date-time (ISO 8601).
    #[serde(default)]
    pub start: Option<String>,
    /// Start time zone.
    #[serde(default)]
    pub start_time_zone: Option<String>,
    /// New end date-time (ISO 8601).
    #[serde(default)]
    pub end: Option<String>,
    /// End time zone.
    #[serde(default)]
    pub end_time_zone: Option<String>,
    /// New location.
    #[serde(default)]
    pub location: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateEventOutput {
    pub event: Event,
}

/// # Update Outlook Calendar Event
///
/// Updates an existing calendar event in the authenticated user's Outlook
/// Calendar using the Microsoft Graph API.
///
/// Use this tool when a user wants to:
/// - Modify an existing event's details (subject, time, location, etc.)
/// - Reschedule a meeting to a different time
/// - Change the location or add online meeting details
/// - Update the event description or body content
///
/// Only the fields that are provided (non-None) will be updated. All other
/// event properties remain unchanged. The event ID must be known beforehand
/// (typically from a previous `list_events` call).
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - calendar
/// - outlook
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The `event_id` parameter is empty or contains only whitespace
/// - User credentials are missing or invalid (no access token configured)
/// - The Microsoft Graph API request fails due to network or authentication
///   issues
/// - The API response cannot be parsed as expected JSON format
#[tool]
pub async fn update_event(ctx: Context, input: UpdateEventInput) -> Result<UpdateEventOutput> {
    ensure!(
        !input.event_id.trim().is_empty(),
        "event_id must not be empty"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    let mut request = GraphUpdateEventRequest {
        subject: input.subject,
        body: None,
        start: None,
        end: None,
        location: None,
    };

    if let Some(body_content) = input.body {
        let content_type = input.body_content_type.unwrap_or(BodyContentType::Text);
        request.body = Some(GraphItemBody {
            content_type,
            content: body_content,
        });
    }

    if let Some(start_dt) = input.start {
        let tz = input.start_time_zone.unwrap_or_else(|| "UTC".to_string());
        request.start = Some(GraphDateTimeTimeZone {
            date_time: start_dt,
            time_zone: tz,
        });
    }

    if let Some(end_dt) = input.end {
        let tz = input.end_time_zone.unwrap_or_else(|| "UTC".to_string());
        request.end = Some(GraphDateTimeTimeZone {
            date_time: end_dt,
            time_zone: tz,
        });
    }

    if let Some(loc) = input.location {
        request.location = Some(GraphLocation {
            display_name: Some(loc),
            location_uri: None,
        });
    }

    let event: Event = client
        .patch_json(
            client.url_with_segments(&["me", "calendar", "events", input.event_id.as_str()])?,
            &request,
            &[],
        )
        .await?;

    Ok(UpdateEventOutput { event })
}

// ===== Cancel Event =====

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CancelEventInput {
    /// Event ID to cancel.
    pub event_id: String,
    /// Optional cancellation comment.
    #[serde(default)]
    pub comment: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CancelEventOutput {
    pub cancelled: bool,
}

/// # Cancel Outlook Calendar Event
///
/// Cancels a calendar event in the authenticated user's Outlook Calendar using
/// the Microsoft Graph API.
///
/// Use this tool when a user wants to:
/// - Cancel an existing meeting or appointment
/// - Remove an event from their calendar
/// - Send a cancellation notification to attendees
///
/// This tool sends cancellation emails to all attendees if the event had
/// invitees. An optional comment can be included to explain the reason for
/// cancellation. The event is moved to the deleted items folder and removed
/// from the calendar view.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - calendar
/// - outlook
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The `event_id` parameter is empty or contains only whitespace
/// - User credentials are missing or invalid (no access token configured)
/// - The Microsoft Graph API request fails due to network or authentication
///   issues
#[tool]
pub async fn cancel_event(ctx: Context, input: CancelEventInput) -> Result<CancelEventOutput> {
    ensure!(
        !input.event_id.trim().is_empty(),
        "event_id must not be empty"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    let request = GraphCancelEventRequest {
        comment: input.comment,
    };

    client
        .post_empty(
            client.url_with_segments(&[
                "me",
                "calendar",
                "events",
                input.event_id.as_str(),
                "cancel",
            ])?,
            &request,
            &[],
        )
        .await?;

    Ok(CancelEventOutput { cancelled: true })
}

// ===== Get Free/Busy Schedule =====

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetFreeBusyInput {
    /// Email addresses to query.
    pub schedules: Vec<String>,
    /// Start time (ISO 8601).
    pub start_time: String,
    /// End time (ISO 8601).
    pub end_time: String,
    /// Time zone. Defaults to "UTC".
    #[serde(default)]
    pub time_zone: Option<String>,
    /// Availability view interval in minutes. Defaults to 30.
    #[serde(default)]
    pub availability_view_interval: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetFreeBusyOutput {
    pub schedules: Vec<ScheduleInformation>,
}

/// # Get Outlook Calendar Free/Busy Schedule
///
/// Retrieves free/busy schedule information for one or more users using the
/// Microsoft Graph API.
///
/// Use this tool when a user wants to:
/// - Find available meeting times for themselves or colleagues
/// - Check when someone is free or busy before scheduling
/// - Coordinate meeting times across multiple attendees
/// - Avoid scheduling conflicts when proposing meeting times
///
/// This tool queries the availability of specified email addresses within a
/// given time window. Returns detailed schedule information including
/// availability view (a string representing free/busy status at intervals) and
/// individual schedule items with conflict details. Useful for meeting
/// scheduling and calendar coordination workflows.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - calendar
/// - outlook
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The `schedules` parameter is empty (must contain at least one email
///   address)
/// - The `start_time` or `end_time` parameters are empty or contain only
///   whitespace
/// - User credentials are missing or invalid (no access token configured)
/// - The Microsoft Graph API request fails due to network or authentication
///   issues
/// - The API response cannot be parsed as expected JSON format
#[tool]
pub async fn get_free_busy(ctx: Context, input: GetFreeBusyInput) -> Result<GetFreeBusyOutput> {
    ensure!(
        !input.schedules.is_empty(),
        "schedules must contain at least one email address"
    );
    ensure!(
        !input.start_time.trim().is_empty(),
        "start_time must not be empty"
    );
    ensure!(
        !input.end_time.trim().is_empty(),
        "end_time must not be empty"
    );

    let client = GraphClient::from_ctx(&ctx)?;
    let time_zone = input.time_zone.unwrap_or_else(|| "UTC".to_string());
    let availability_view_interval = input.availability_view_interval.unwrap_or(30);

    let request = GraphGetScheduleRequest {
        schedules: input.schedules,
        start_time: GraphDateTimeTimeZone {
            date_time: input.start_time,
            time_zone: time_zone.clone(),
        },
        end_time: GraphDateTimeTimeZone {
            date_time: input.end_time,
            time_zone,
        },
        availability_view_interval,
    };

    let response: GraphGetScheduleResponse = client
        .post_json(
            client.url_with_segments(&["me", "calendar", "getSchedule"])?,
            &request,
            &[],
        )
        .await?;

    Ok(GetFreeBusyOutput {
        schedules: response.value,
    })
}

// ===== Internal Graph API types =====

#[derive(Debug, Deserialize)]
struct GraphListResponse<T> {
    value: Vec<T>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphEmailAddress {
    address: String,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphAttendee {
    email_address: GraphEmailAddress,
    #[serde(rename = "type")]
    attendee_type: AttendeeType,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphLocation {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    location_uri: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphDateTimeTimeZone {
    date_time: String,
    time_zone: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphItemBody {
    content_type: BodyContentType,
    content: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphCreateEventRequest {
    subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<GraphItemBody>,
    start: GraphDateTimeTimeZone,
    end: GraphDateTimeTimeZone,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<GraphLocation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    attendees: Vec<GraphAttendee>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_all_day: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    show_as: Option<EventShowAs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_online_meeting: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphUpdateEventRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<GraphItemBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    start: Option<GraphDateTimeTimeZone>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<GraphDateTimeTimeZone>,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<GraphLocation>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphCancelEventRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    comment: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphGetScheduleRequest {
    schedules: Vec<String>,
    start_time: GraphDateTimeTimeZone,
    end_time: GraphDateTimeTimeZone,
    availability_view_interval: u32,
}

#[derive(Debug, Deserialize)]
struct GraphGetScheduleResponse {
    value: Vec<ScheduleInformation>,
}

// ===== GraphClient =====

#[derive(Debug, Clone)]
struct GraphClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl GraphClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = OutlookCalendarCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_GRAPH_ENDPOINT))?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            access_token: cred.access_token,
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

    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        query: &[(&str, String)],
        extra_headers: &[(&str, &str)],
    ) -> Result<T> {
        let mut request = self.http.get(url).query(query);
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }

        let response = self.send_request(request).await?;
        Ok(response.json::<T>().await?)
    }

    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
        extra_headers: &[(&str, &str)],
    ) -> Result<TRes> {
        let mut request = self.http.post(url).json(body);
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }

        let response = self.send_request(request).await?;
        Ok(response.json::<TRes>().await?)
    }

    async fn post_empty<TReq: Serialize>(
        &self,
        url: reqwest::Url,
        body: &TReq,
        extra_headers: &[(&str, &str)],
    ) -> Result<()> {
        let mut request = self.http.post(url).json(body);
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }

        self.send_request(request).await?;
        Ok(())
    }

    async fn patch_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
        extra_headers: &[(&str, &str)],
    ) -> Result<TRes> {
        let mut request = self.http.patch(url).json(body);
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }

        let response = self.send_request(request).await?;
        Ok(response.json::<TRes>().await?)
    }

    async fn send_request(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let response = request
            .bearer_auth(&self.access_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "Microsoft Graph request failed ({status}): {body}"
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
        matchers::{body_string_contains, header, method, path, query_param},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut outlook_values = HashMap::new();
        outlook_values.insert("access_token".to_string(), "test-token".to_string());
        outlook_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("outlook_calendar", outlook_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        format!("{}/v1.0", server.uri())
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_body_content_type_serialization_roundtrip() {
        for variant in [BodyContentType::Text, BodyContentType::Html] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: BodyContentType = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_event_show_as_serialization_roundtrip() {
        for variant in [
            EventShowAs::Free,
            EventShowAs::Tentative,
            EventShowAs::Busy,
            EventShowAs::Oof,
            EventShowAs::WorkingElsewhere,
            EventShowAs::Unknown,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: EventShowAs = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_attendee_type_serialization_roundtrip() {
        for variant in [
            AttendeeType::Required,
            AttendeeType::Optional,
            AttendeeType::Resource,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: AttendeeType = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://graph.microsoft.com/").unwrap();
        assert_eq!(result, "https://graph.microsoft.com");
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
    async fn test_list_events_limit_zero_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_events(
            ctx,
            ListEventsInput {
                start: None,
                end: None,
                limit: Some(0),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("limit must be between 1 and 1000")
        );
    }

    #[tokio::test]
    async fn test_list_events_limit_exceeds_max_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_events(
            ctx,
            ListEventsInput {
                start: None,
                end: None,
                limit: Some(1001),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("limit must be between 1 and 1000")
        );
    }

    #[tokio::test]
    async fn test_create_event_empty_subject_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = create_event(
            ctx,
            CreateEventInput {
                subject: "  ".to_string(),
                body: None,
                body_content_type: None,
                start: "2024-01-01T10:00:00".to_string(),
                start_time_zone: None,
                end: "2024-01-01T11:00:00".to_string(),
                end_time_zone: None,
                location: None,
                attendees: vec![],
                is_all_day: None,
                show_as: None,
                is_online_meeting: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("subject must not be empty")
        );
    }

    #[tokio::test]
    async fn test_update_event_empty_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = update_event(
            ctx,
            UpdateEventInput {
                event_id: "  ".to_string(),
                subject: Some("New Subject".to_string()),
                body: None,
                body_content_type: None,
                start: None,
                start_time_zone: None,
                end: None,
                end_time_zone: None,
                location: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("event_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_cancel_event_empty_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = cancel_event(
            ctx,
            CancelEventInput {
                event_id: "  ".to_string(),
                comment: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("event_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_get_free_busy_empty_schedules_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = get_free_busy(
            ctx,
            GetFreeBusyInput {
                schedules: vec![],
                start_time: "2024-01-01T00:00:00".to_string(),
                end_time: "2024-01-01T23:59:59".to_string(),
                time_zone: None,
                availability_view_interval: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("schedules must contain at least one email address")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_list_events_success_returns_events() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "value": [
            {
              "id": "event-1",
              "subject": "Team Meeting",
              "start": { "dateTime": "2024-01-01T10:00:00", "timeZone": "UTC" },
              "end": { "dateTime": "2024-01-01T11:00:00", "timeZone": "UTC" },
              "location": { "displayName": "Conference Room A" },
              "attendees": [],
              "isAllDay": false
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1.0/me/calendar/events"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("$top", "50"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_events(
            ctx,
            ListEventsInput {
                start: None,
                end: None,
                limit: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.events.len(), 1);
        assert_eq!(output.events[0].id, "event-1");
        assert_eq!(output.events[0].subject.as_deref(), Some("Team Meeting"));
    }

    #[tokio::test]
    async fn test_create_event_success_returns_event() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "event-new",
          "subject": "New Meeting",
          "start": { "dateTime": "2024-01-01T14:00:00", "timeZone": "UTC" },
          "end": { "dateTime": "2024-01-01T15:00:00", "timeZone": "UTC" }
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/v1.0/me/calendar/events"))
            .and(body_string_contains("\"subject\":\"New Meeting\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = create_event(
            ctx,
            CreateEventInput {
                subject: "New Meeting".to_string(),
                body: None,
                body_content_type: None,
                start: "2024-01-01T14:00:00".to_string(),
                start_time_zone: None,
                end: "2024-01-01T15:00:00".to_string(),
                end_time_zone: None,
                location: None,
                attendees: vec![],
                is_all_day: None,
                show_as: None,
                is_online_meeting: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.event.id, "event-new");
        assert_eq!(output.event.subject.as_deref(), Some("New Meeting"));
    }

    #[tokio::test]
    async fn test_update_event_success_returns_updated_event() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "event-123",
          "subject": "Updated Meeting",
          "start": { "dateTime": "2024-01-01T16:00:00", "timeZone": "UTC" },
          "end": { "dateTime": "2024-01-01T17:00:00", "timeZone": "UTC" }
        }
        "#;

        Mock::given(method("PATCH"))
            .and(path("/v1.0/me/calendar/events/event-123"))
            .and(body_string_contains("\"subject\":\"Updated Meeting\""))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = update_event(
            ctx,
            UpdateEventInput {
                event_id: "event-123".to_string(),
                subject: Some("Updated Meeting".to_string()),
                body: None,
                body_content_type: None,
                start: None,
                start_time_zone: None,
                end: None,
                end_time_zone: None,
                location: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.event.id, "event-123");
        assert_eq!(output.event.subject.as_deref(), Some("Updated Meeting"));
    }

    #[tokio::test]
    async fn test_cancel_event_success_returns_cancelled() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("POST"))
            .and(path("/v1.0/me/calendar/events/event-123/cancel"))
            .respond_with(ResponseTemplate::new(202))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = cancel_event(
            ctx,
            CancelEventInput {
                event_id: "event-123".to_string(),
                comment: Some("Meeting no longer needed".to_string()),
            },
        )
        .await
        .unwrap();

        assert!(output.cancelled);
    }

    #[tokio::test]
    async fn test_get_free_busy_success_returns_schedules() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "value": [
            {
              "scheduleId": "user@example.com",
              "availabilityView": "0000002222",
              "scheduleItems": [
                {
                  "status": "busy",
                  "start": { "dateTime": "2024-01-01T14:00:00", "timeZone": "UTC" },
                  "end": { "dateTime": "2024-01-01T15:00:00", "timeZone": "UTC" }
                }
              ]
            }
          ]
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/v1.0/me/calendar/getSchedule"))
            .and(body_string_contains("\"schedules\":[\"user@example.com\"]"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = get_free_busy(
            ctx,
            GetFreeBusyInput {
                schedules: vec!["user@example.com".to_string()],
                start_time: "2024-01-01T00:00:00".to_string(),
                end_time: "2024-01-01T23:59:59".to_string(),
                time_zone: None,
                availability_view_interval: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.schedules.len(), 1);
        assert_eq!(output.schedules[0].schedule_id, "user@example.com");
        assert_eq!(
            output.schedules[0].availability_view.as_deref(),
            Some("0000002222")
        );
    }

    #[tokio::test]
    async fn test_list_events_error_response_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path("/v1.0/me/calendar/events"))
            .respond_with(ResponseTemplate::new(401).set_body_raw(
                r#"{ "error": { "code": "InvalidAuthenticationToken", "message": "Bad token" } }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = list_events(
            ctx,
            ListEventsInput {
                start: None,
                end: None,
                limit: None,
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("401"));
    }
}
