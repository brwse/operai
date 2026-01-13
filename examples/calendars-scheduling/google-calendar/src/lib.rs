//! calendars-scheduling/google-calendar integration for Operai Toolbox.

use operai::{
    Context, JsonSchema, Result, bail, define_user_credential, ensure, info, init, schemars,
    shutdown, tool,
};
use reqwest::Url;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

const DEFAULT_ENDPOINT: &str = "https://www.googleapis.com/calendar/v3";

define_user_credential! {
    GoogleCalendarCredential("google_calendar") {
        /// OAuth2 access token to call the Google Calendar API.
        access_token: String,
        /// Optional API base URL (default: https://www.googleapis.com/calendar/v3).
        #[optional]
        endpoint: Option<String>,
    }
}

#[init]
async fn setup() -> Result<()> {
    info!("Google Calendar integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Google Calendar integration shutting down");
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct EventTime {
    /// RFC3339 timestamp (e.g. "2026-01-11T15:00:00Z") for timed events.
    #[serde(default)]
    pub date_time: Option<String>,
    /// All-day date in YYYY-MM-DD format (e.g. "2026-01-11").
    #[serde(default)]
    pub date: Option<String>,
    /// IANA time zone name (e.g. "`America/Los_Angeles`").
    #[serde(default)]
    pub time_zone: Option<String>,
}

impl EventTime {
    fn validate(&self, field_name: &str) -> Result<()> {
        let has_date_time = self
            .date_time
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty());
        let has_date = self
            .date
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty());

        ensure!(
            has_date_time ^ has_date,
            "{field_name} must have exactly one of `date_time` or `date`"
        );

        if let Some(tz) = &self.time_zone {
            ensure!(
                !tz.trim().is_empty(),
                "{field_name}.time_zone must not be empty when provided"
            );
        }

        Ok(())
    }

    fn into_api(self) -> ApiEventDateTime {
        ApiEventDateTime {
            date_time: self.date_time,
            date: self.date,
            time_zone: self.time_zone,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct EventAttendee {
    /// Attendee email address.
    #[serde(default)]
    pub email: Option<String>,
    /// Optional display name.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Optional response status (e.g. "accepted", "declined", "needsAction").
    #[serde(default)]
    pub response_status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct CalendarEvent {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub html_link: Option<String>,
    #[serde(default)]
    pub start: Option<EventTime>,
    #[serde(default)]
    pub end: Option<EventTime>,
    #[serde(default)]
    pub attendees: Option<Vec<EventAttendee>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiEventListResponse {
    #[serde(default)]
    items: Vec<ApiEvent>,
    next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiEvent {
    id: Option<String>,
    status: Option<String>,
    summary: Option<String>,
    description: Option<String>,
    location: Option<String>,
    html_link: Option<String>,
    start: Option<ApiEventDateTime>,
    end: Option<ApiEventDateTime>,
    attendees: Option<Vec<ApiEventAttendee>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiEventAttendee {
    email: Option<String>,
    display_name: Option<String>,
    response_status: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiEventDateTime {
    #[serde(skip_serializing_if = "Option::is_none")]
    date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    time_zone: Option<String>,
}

impl From<ApiEvent> for CalendarEvent {
    fn from(value: ApiEvent) -> Self {
        Self {
            id: value.id,
            status: value.status,
            summary: value.summary,
            description: value.description,
            location: value.location,
            html_link: value.html_link,
            start: value.start.map(|start| EventTime {
                date_time: start.date_time,
                date: start.date,
                time_zone: start.time_zone,
            }),
            end: value.end.map(|end| EventTime {
                date_time: end.date_time,
                date: end.date,
                time_zone: end.time_zone,
            }),
            attendees: value.attendees.map(|attendees| {
                attendees
                    .into_iter()
                    .map(|attendee| EventAttendee {
                        email: attendee.email,
                        display_name: attendee.display_name,
                        response_status: attendee.response_status,
                    })
                    .collect()
            }),
        }
    }
}

fn validate_send_updates(send_updates: &str) -> Result<()> {
    ensure!(
        matches!(send_updates, "all" | "externalOnly" | "none"),
        "send_updates must be one of: all, externalOnly, none"
    );
    Ok(())
}

fn validate_non_empty(field_name: &str, value: &str) -> Result<()> {
    ensure!(!value.trim().is_empty(), "{field_name} must not be empty");
    Ok(())
}

#[derive(Debug, Clone)]
struct GoogleCalendarApi {
    http: reqwest::Client,
    base_url: Url,
    access_token: String,
}

impl GoogleCalendarApi {
    fn new(cred: GoogleCalendarCredential) -> Result<Self> {
        validate_non_empty("credential.access_token", &cred.access_token)?;

        let base = cred.endpoint.as_deref().unwrap_or(DEFAULT_ENDPOINT);
        let base_url = Url::parse(base)?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            access_token: cred.access_token,
        })
    }

    fn url_with_path_segments(&self, segments: &[&str]) -> Result<Url> {
        let mut url = self.base_url.clone();
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|()| operai::anyhow::anyhow!("credential.endpoint must be a base URL"))?;
            for segment in segments {
                path.push(segment);
            }
        }
        Ok(url)
    }

    fn events_collection_url(&self, calendar_id: &str) -> Result<Url> {
        self.url_with_path_segments(&["calendars", calendar_id, "events"])
    }

    fn event_url(&self, calendar_id: &str, event_id: &str) -> Result<Url> {
        self.url_with_path_segments(&["calendars", calendar_id, "events", event_id])
    }

    fn free_busy_url(&self) -> Result<Url> {
        self.url_with_path_segments(&["freeBusy"])
    }

    fn with_auth(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        request.bearer_auth(&self.access_token)
    }
}

async fn send_json<T: DeserializeOwned>(
    request: reqwest::RequestBuilder,
    operation: &str,
) -> Result<T> {
    let response = request.send().await?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!("{operation} failed with status {status}: {body}");
    }
    Ok(response.json::<T>().await?)
}

async fn send_no_content(request: reqwest::RequestBuilder, operation: &str) -> Result<()> {
    let response = request.send().await?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!("{operation} failed with status {status}: {body}");
    }
    Ok(())
}

/// Input for the `list_events` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListEventsInput {
    /// Calendar ID (use "primary" for the authenticated user's primary
    /// calendar).
    pub calendar_id: String,
    /// RFC3339 lower bound (inclusive), e.g. "2026-01-11T00:00:00Z".
    pub time_min: String,
    /// RFC3339 upper bound (exclusive), e.g. "2026-01-18T00:00:00Z".
    pub time_max: String,
    /// Optional free-text search query.
    #[serde(default)]
    pub q: Option<String>,
    /// Optional page token from a previous response.
    #[serde(default)]
    pub page_token: Option<String>,
    /// Optional max results (1-2500).
    #[serde(default)]
    pub max_results: Option<u32>,
    /// Expand recurring events into instances (recommended).
    #[serde(default = "default_true")]
    pub single_events: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListEventsOutput {
    pub events: Vec<CalendarEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

fn default_true() -> bool {
    true
}

/// # List Google Calendar Events
///
/// Retrieves events from a Google Calendar within a specified time range.
///
/// Use this tool when a user wants to:
/// - View their upcoming calendar events
/// - Check what events are scheduled between specific dates/times
/// - Search for events matching a specific query string
/// - Browse calendar events with pagination for large date ranges
///
/// The tool supports both the authenticated user's primary calendar (use
/// "primary" as `calendar_id`) and any other calendars they have access to by
/// calendar ID.
///
/// By default, recurring events are expanded into individual instances for
/// easier processing. Time-based queries use RFC3339 format (e.g.,
/// "2026-01-11T00:00:00Z").
///
/// # Errors
///
/// Returns an error if:
/// - Required fields (`calendar_id`, `time_min`, `time_max`) are empty
/// - `max_results` is outside the valid range (1-2500)
/// - The credential is missing or invalid
/// - The API request fails (network error, authentication failure, etc.)
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - google-calendar
/// - calendar
/// - events
#[tool]
pub async fn list_events(ctx: Context, input: ListEventsInput) -> Result<ListEventsOutput> {
    validate_non_empty("calendar_id", &input.calendar_id)?;
    validate_non_empty("time_min", &input.time_min)?;
    validate_non_empty("time_max", &input.time_max)?;

    let cred = GoogleCalendarCredential::get(&ctx)?;
    let api = GoogleCalendarApi::new(cred)?;

    let url = api.events_collection_url(&input.calendar_id)?;
    let order_by = if input.single_events {
        "startTime"
    } else {
        "updated"
    };
    let mut params: Vec<(&str, String)> = vec![
        ("timeMin", input.time_min),
        ("timeMax", input.time_max),
        ("singleEvents", input.single_events.to_string()),
        ("orderBy", order_by.to_string()),
    ];

    if let Some(q) = input.q {
        validate_non_empty("q", &q)?;
        params.push(("q", q));
    }
    if let Some(page_token) = input.page_token {
        validate_non_empty("page_token", &page_token)?;
        params.push(("pageToken", page_token));
    }
    if let Some(max_results) = input.max_results {
        ensure!(
            (1..=2500).contains(&max_results),
            "max_results must be in the range 1..=2500"
        );
        params.push(("maxResults", max_results.to_string()));
    }

    let request = api.with_auth(api.http.get(url)).query(&params);
    let response: ApiEventListResponse = send_json(request, "list events").await?;

    Ok(ListEventsOutput {
        events: response.items.into_iter().map(Into::into).collect(),
        next_page_token: response.next_page_token,
    })
}

/// Input for the `create_event` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateEventInput {
    /// Calendar ID (use "primary" for the authenticated user's primary
    /// calendar).
    pub calendar_id: String,
    /// Event summary/title.
    pub summary: String,
    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,
    /// Optional location.
    #[serde(default)]
    pub location: Option<String>,
    /// Event start time/date.
    pub start: EventTime,
    /// Event end time/date.
    pub end: EventTime,
    /// Attendee email addresses.
    #[serde(default)]
    pub attendees: Option<Vec<String>>,
    /// Whether to send updates to attendees: all, externalOnly, none.
    #[serde(default)]
    pub send_updates: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateEventOutput {
    pub event: CalendarEvent,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiEventWrite {
    summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<String>,
    start: ApiEventDateTime,
    end: ApiEventDateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    attendees: Option<Vec<ApiEventAttendeeWrite>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiEventAttendeeWrite {
    email: String,
}

/// # Create Google Calendar Event
///
/// Creates a new event on a Google Calendar.
///
/// Use this tool when a user wants to:
/// - Schedule a new meeting or appointment
/// - Add an event to their calendar (primary or shared calendar)
/// - Create all-day events or time-specific events
/// - Invite attendees to an event
///
/// Events can be created as:
/// - Timed events: Provide `date_time` (RFC3339 timestamp) for start/end
/// - All-day events: Provide `date` (YYYY-MM-DD format) for start/end
///
/// The tool supports inviting attendees via email and controlling whether
/// notifications are sent. When attendees are specified, you can choose to send
/// updates to all, only external participants, or none.
///
/// # Errors
///
/// Returns an error if:
/// - Required fields (`calendar_id`, `summary`) are empty
/// - `start` or `end` time validation fails
/// - `start` and `end` use different time types (one `date_time`, one `date`)
/// - `send_updates` is not one of: all, externalOnly, none
/// - Any attendee email is empty
/// - The credential is missing or invalid
/// - The API request fails (network error, authentication failure, etc.)
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - google-calendar
/// - calendar
/// - events
#[tool]
pub async fn create_event(ctx: Context, input: CreateEventInput) -> Result<CreateEventOutput> {
    validate_non_empty("calendar_id", &input.calendar_id)?;
    validate_non_empty("summary", &input.summary)?;

    if let Some(description) = &input.description {
        validate_non_empty("description", description)?;
    }
    if let Some(location) = &input.location {
        validate_non_empty("location", location)?;
    }

    input.start.validate("start")?;
    input.end.validate("end")?;
    ensure!(
        input.start.date.is_some() == input.end.date.is_some(),
        "start and end must both use `date_time` or both use `date`"
    );

    if let Some(send_updates) = &input.send_updates {
        validate_non_empty("send_updates", send_updates)?;
        validate_send_updates(send_updates)?;
    }

    let attendees = input.attendees.map(|emails| {
        emails
            .into_iter()
            .map(|email| {
                validate_non_empty("attendees[]", &email)?;
                Ok(ApiEventAttendeeWrite { email })
            })
            .collect::<Result<Vec<_>>>()
    });

    let body = ApiEventWrite {
        summary: input.summary,
        description: input.description,
        location: input.location,
        start: input.start.into_api(),
        end: input.end.into_api(),
        attendees: attendees.transpose()?,
    };

    let cred = GoogleCalendarCredential::get(&ctx)?;
    let api = GoogleCalendarApi::new(cred)?;

    let url = api.events_collection_url(&input.calendar_id)?;
    let mut request = api.with_auth(api.http.post(url)).json(&body);
    if let Some(send_updates) = input.send_updates {
        request = request.query(&[("sendUpdates", send_updates)]);
    }

    let created: ApiEvent = send_json(request, "create event").await?;
    Ok(CreateEventOutput {
        event: created.into(),
    })
}

/// Input for the `update_event` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateEventInput {
    /// Calendar ID (use "primary" for the authenticated user's primary
    /// calendar).
    pub calendar_id: String,
    /// Event ID.
    pub event_id: String,
    /// Updated event summary/title.
    #[serde(default)]
    pub summary: Option<String>,
    /// Updated description.
    #[serde(default)]
    pub description: Option<String>,
    /// Updated location.
    #[serde(default)]
    pub location: Option<String>,
    /// Updated start time/date.
    #[serde(default)]
    pub start: Option<EventTime>,
    /// Updated end time/date.
    #[serde(default)]
    pub end: Option<EventTime>,
    /// Updated attendee email addresses (replaces attendee list when provided).
    #[serde(default)]
    pub attendees: Option<Vec<String>>,
    /// Whether to send updates to attendees: all, externalOnly, none.
    #[serde(default)]
    pub send_updates: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateEventOutput {
    pub event: CalendarEvent,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiEventPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    start: Option<ApiEventDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<ApiEventDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attendees: Option<Vec<ApiEventAttendeeWrite>>,
}

/// # Update Google Calendar Event
///
/// Modifies an existing event on a Google Calendar.
///
/// Use this tool when a user wants to:
/// - Change an event's time, title, description, or location
/// - Add or remove attendees from an existing event
/// - Reschedule a meeting to a different date/time
/// - Update any other event details
///
/// This tool performs a partial update—only fields that are explicitly provided
/// will be changed. For example, providing only `summary` will update just the
/// title while preserving all other event properties.
///
/// When updating attendees, the entire attendee list is replaced with the new
/// list. To add attendees without removing existing ones, you must first
/// retrieve the current event, then provide the complete updated list.
///
/// The `send_updates` parameter controls whether attendees receive
/// notifications about the changes.
///
/// # Errors
///
/// Returns an error if:
/// - Required fields (`calendar_id`, `event_id`) are empty
/// - No update fields are provided (at least one must be set)
/// - `start` or `end` time validation fails (when provided)
/// - `start` and `end` use different time types (when both provided)
/// - `send_updates` is not one of: all, externalOnly, none
/// - Any attendee email is empty
/// - The credential is missing or invalid
/// - The API request fails (network error, authentication failure, etc.)
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - google-calendar
/// - calendar
/// - events
#[tool]
pub async fn update_event(ctx: Context, input: UpdateEventInput) -> Result<UpdateEventOutput> {
    validate_non_empty("calendar_id", &input.calendar_id)?;
    validate_non_empty("event_id", &input.event_id)?;

    if let Some(summary) = &input.summary {
        validate_non_empty("summary", summary)?;
    }
    if let Some(description) = &input.description {
        validate_non_empty("description", description)?;
    }
    if let Some(location) = &input.location {
        validate_non_empty("location", location)?;
    }

    if let Some(start) = &input.start {
        start.validate("start")?;
    }
    if let Some(end) = &input.end {
        end.validate("end")?;
    }
    if let (Some(start), Some(end)) = (&input.start, &input.end) {
        ensure!(
            start.date.is_some() == end.date.is_some(),
            "start and end must both use `date_time` or both use `date`"
        );
    }

    if let Some(send_updates) = &input.send_updates {
        validate_non_empty("send_updates", send_updates)?;
        validate_send_updates(send_updates)?;
    }

    let has_any_update = input.summary.is_some()
        || input.description.is_some()
        || input.location.is_some()
        || input.start.is_some()
        || input.end.is_some()
        || input.attendees.is_some();
    ensure!(
        has_any_update,
        "at least one of summary, description, location, start, end, attendees must be provided"
    );

    let attendees = input.attendees.map(|emails| {
        emails
            .into_iter()
            .map(|email| {
                validate_non_empty("attendees[]", &email)?;
                Ok(ApiEventAttendeeWrite { email })
            })
            .collect::<Result<Vec<_>>>()
    });

    let patch = ApiEventPatch {
        summary: input.summary,
        description: input.description,
        location: input.location,
        start: input.start.map(EventTime::into_api),
        end: input.end.map(EventTime::into_api),
        attendees: attendees.transpose()?,
    };

    let cred = GoogleCalendarCredential::get(&ctx)?;
    let api = GoogleCalendarApi::new(cred)?;

    let url = api.event_url(&input.calendar_id, &input.event_id)?;
    let mut request = api.with_auth(api.http.patch(url)).json(&patch);
    if let Some(send_updates) = input.send_updates {
        request = request.query(&[("sendUpdates", send_updates)]);
    }

    let updated: ApiEvent = send_json(request, "update event").await?;
    Ok(UpdateEventOutput {
        event: updated.into(),
    })
}

/// Input for the cancel tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CancelInput {
    /// Calendar ID (use "primary" for the authenticated user's primary
    /// calendar).
    pub calendar_id: String,
    /// Event ID.
    pub event_id: String,
    /// Whether to send updates to attendees: all, externalOnly, none.
    #[serde(default)]
    pub send_updates: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CancelOutput {
    pub cancelled: bool,
}

/// # Cancel Google Calendar Event
///
/// Cancels (deletes) an event on a Google Calendar.
///
/// Use this tool when a user wants to:
/// - Remove an event from their calendar
/// - Cancel a meeting they organized
/// - Delete an event that is no longer needed
///
/// This operation permanently removes the event from the calendar. If the event
/// has attendees, you can control whether they receive cancellation
/// notifications via the `send_updates` parameter.
///
/// Note: This is a destructive operation that cannot be undone. For recurring
/// events, this deletes the entire series, not just a single instance.
///
/// # Errors
///
/// Returns an error if:
/// - Required fields (`calendar_id`, `event_id`) are empty
/// - `send_updates` is not one of: all, externalOnly, none
/// - The credential is missing or invalid
/// - The API request fails (network error, authentication failure, event not
///   found, etc.)
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - google-calendar
/// - calendar
/// - events
#[tool]
pub async fn cancel(ctx: Context, input: CancelInput) -> Result<CancelOutput> {
    validate_non_empty("calendar_id", &input.calendar_id)?;
    validate_non_empty("event_id", &input.event_id)?;

    if let Some(send_updates) = &input.send_updates {
        validate_non_empty("send_updates", send_updates)?;
        validate_send_updates(send_updates)?;
    }

    let cred = GoogleCalendarCredential::get(&ctx)?;
    let api = GoogleCalendarApi::new(cred)?;

    let url = api.event_url(&input.calendar_id, &input.event_id)?;
    let mut request = api.with_auth(api.http.delete(url));
    if let Some(send_updates) = input.send_updates {
        request = request.query(&[("sendUpdates", send_updates)]);
    }

    send_no_content(request, "cancel event").await?;
    Ok(CancelOutput { cancelled: true })
}

/// Input for the `free_busy` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FreeBusyInput {
    /// RFC3339 lower bound (inclusive), e.g. "2026-01-11T00:00:00Z".
    pub time_min: String,
    /// RFC3339 upper bound (exclusive), e.g. "2026-01-18T00:00:00Z".
    pub time_max: String,
    /// Calendar IDs to query.
    pub calendar_ids: Vec<String>,
    /// Optional IANA time zone name for the response.
    #[serde(default)]
    pub time_zone: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FreeBusyOutput {
    pub calendars: Vec<FreeBusyCalendar>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FreeBusyCalendar {
    pub calendar_id: String,
    pub busy: Vec<BusyInterval>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BusyInterval {
    pub start: String,
    pub end: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiFreeBusyRequest {
    time_min: String,
    time_max: String,
    items: Vec<ApiFreeBusyItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    time_zone: Option<String>,
}

#[derive(Debug, Serialize)]
struct ApiFreeBusyItem {
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiFreeBusyResponse {
    #[serde(default)]
    calendars: std::collections::HashMap<String, ApiFreeBusyCalendar>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiFreeBusyCalendar {
    #[serde(default)]
    busy: Vec<ApiBusyInterval>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiBusyInterval {
    start: String,
    end: String,
}

/// # Query Google Calendar Free/Busy
///
/// Queries free/busy availability for one or more Google Calendars.
///
/// Use this tool when a user wants to:
/// - Find available time slots for scheduling meetings
/// - Check when multiple people are free at the same time
/// - Determine calendar availability without accessing event details
/// - Coordinate meeting times across multiple calendars
///
/// This tool only returns busy time intervals (when events are scheduled),
/// not the actual event details. It's ideal for finding mutually available
/// time slots without accessing private calendar information.
///
/// You can query multiple calendars at once, including the user's primary
/// calendar ("primary") and other calendars by ID (email addresses for other
/// users). The response returns intervals when each calendar is busy, allowing
/// you to identify gaps where all participants are available.
///
/// Time ranges are specified in RFC3339 format (e.g., "2026-01-11T00:00:00Z").
///
/// # Errors
///
/// Returns an error if:
/// - Required fields (`time_min`, `time_max`) are empty
/// - `calendar_ids` is empty or contains any empty strings
/// - `time_zone` is empty when provided
/// - The credential is missing or invalid
/// - The API request fails (network error, authentication failure, etc.)
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - google-calendar
/// - calendar
/// - free-busy
#[tool]
pub async fn free_busy(ctx: Context, input: FreeBusyInput) -> Result<FreeBusyOutput> {
    validate_non_empty("time_min", &input.time_min)?;
    validate_non_empty("time_max", &input.time_max)?;
    ensure!(
        !input.calendar_ids.is_empty(),
        "calendar_ids must include at least one calendar id"
    );

    for calendar_id in &input.calendar_ids {
        validate_non_empty("calendar_ids[]", calendar_id)?;
    }
    if let Some(time_zone) = &input.time_zone {
        validate_non_empty("time_zone", time_zone)?;
    }

    let body = ApiFreeBusyRequest {
        time_min: input.time_min,
        time_max: input.time_max,
        items: input
            .calendar_ids
            .into_iter()
            .map(|id| ApiFreeBusyItem { id })
            .collect(),
        time_zone: input.time_zone,
    };

    let cred = GoogleCalendarCredential::get(&ctx)?;
    let api = GoogleCalendarApi::new(cred)?;

    let url = api.free_busy_url()?;
    let request = api.with_auth(api.http.post(url)).json(&body);
    let response: ApiFreeBusyResponse = send_json(request, "free/busy").await?;

    let mut calendars: Vec<FreeBusyCalendar> = response
        .calendars
        .into_iter()
        .map(|(calendar_id, cal)| FreeBusyCalendar {
            calendar_id,
            busy: cal
                .busy
                .into_iter()
                .map(|b| BusyInterval {
                    start: b.start,
                    end: b.end,
                })
                .collect(),
        })
        .collect();
    calendars.sort_by(|a, b| a.calendar_id.cmp(&b.calendar_id));

    Ok(FreeBusyOutput { calendars })
}

operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_json, header, method, path, query_param},
    };

    use super::*;

    fn test_context(endpoint: &str) -> Context {
        let mut fields = HashMap::new();
        fields.insert("access_token".to_string(), "test-token".to_string());
        fields.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-1", "sess-1", "user-1")
            .with_user_credential("google_calendar", fields)
    }

    // ==========================================================================
    // Validation tests
    // ==========================================================================

    #[test]
    fn test_event_time_with_datetime_is_valid() {
        let time = EventTime {
            date_time: Some("2026-01-11T15:00:00Z".to_string()),
            date: None,
            time_zone: Some("America/Los_Angeles".to_string()),
        };
        assert!(time.validate("start").is_ok());
    }

    #[test]
    fn test_event_time_with_date_is_valid() {
        let time = EventTime {
            date_time: None,
            date: Some("2026-01-11".to_string()),
            time_zone: None,
        };
        assert!(time.validate("end").is_ok());
    }

    #[test]
    fn test_event_time_with_both_datetime_and_date_is_invalid() {
        let time = EventTime {
            date_time: Some("2026-01-11T15:00:00Z".to_string()),
            date: Some("2026-01-11".to_string()),
            time_zone: None,
        };
        let err = time.validate("start").unwrap_err();
        assert!(err.to_string().contains("exactly one of"));
    }

    #[test]
    fn test_event_time_with_neither_datetime_nor_date_is_invalid() {
        let time = EventTime {
            date_time: None,
            date: None,
            time_zone: None,
        };
        let err = time.validate("start").unwrap_err();
        assert!(err.to_string().contains("exactly one of"));
    }

    #[test]
    fn test_event_time_with_empty_datetime_is_invalid() {
        let time = EventTime {
            date_time: Some("   ".to_string()),
            date: None,
            time_zone: None,
        };
        let err = time.validate("start").unwrap_err();
        assert!(err.to_string().contains("exactly one of"));
    }

    #[test]
    fn test_event_time_with_empty_timezone_is_invalid() {
        let time = EventTime {
            date_time: Some("2026-01-11T15:00:00Z".to_string()),
            date: None,
            time_zone: Some("  ".to_string()),
        };
        let err = time.validate("start").unwrap_err();
        assert!(err.to_string().contains("time_zone must not be empty"));
    }

    #[test]
    fn test_validate_send_updates_accepts_valid_values() {
        assert!(validate_send_updates("all").is_ok());
        assert!(validate_send_updates("externalOnly").is_ok());
        assert!(validate_send_updates("none").is_ok());
    }

    #[test]
    fn test_validate_send_updates_rejects_invalid_value() {
        let err = validate_send_updates("invalid").unwrap_err();
        assert!(err.to_string().contains("must be one of"));
    }

    #[test]
    fn test_validate_non_empty_accepts_non_empty_string() {
        assert!(validate_non_empty("field", "value").is_ok());
    }

    #[test]
    fn test_validate_non_empty_rejects_empty_string() {
        let err = validate_non_empty("calendar_id", "").unwrap_err();
        assert!(err.to_string().contains("must not be empty"));
    }

    #[test]
    fn test_validate_non_empty_rejects_whitespace_only_string() {
        let err = validate_non_empty("calendar_id", "   ").unwrap_err();
        assert!(err.to_string().contains("must not be empty"));
    }

    // ==========================================================================
    // Serialization roundtrip tests
    // ==========================================================================

    #[test]
    fn test_event_time_serialization_roundtrip() {
        let time = EventTime {
            date_time: Some("2026-01-11T15:00:00Z".to_string()),
            date: None,
            time_zone: Some("America/Los_Angeles".to_string()),
        };

        let json = serde_json::to_string(&time).unwrap();
        let parsed: EventTime = serde_json::from_str(&json).unwrap();

        assert_eq!(time.date_time, parsed.date_time);
        assert_eq!(time.date, parsed.date);
        assert_eq!(time.time_zone, parsed.time_zone);
    }

    #[test]
    fn test_event_attendee_serialization_roundtrip() {
        let attendee = EventAttendee {
            email: Some("test@example.com".to_string()),
            display_name: Some("Test User".to_string()),
            response_status: Some("accepted".to_string()),
        };

        let json = serde_json::to_string(&attendee).unwrap();
        let parsed: EventAttendee = serde_json::from_str(&json).unwrap();

        assert_eq!(attendee.email, parsed.email);
        assert_eq!(attendee.display_name, parsed.display_name);
        assert_eq!(attendee.response_status, parsed.response_status);
    }

    #[test]
    fn test_calendar_event_serialization_roundtrip() {
        let event = CalendarEvent {
            id: Some("evt1".to_string()),
            status: Some("confirmed".to_string()),
            summary: Some("Meeting".to_string()),
            description: Some("Team sync".to_string()),
            location: Some("Room 101".to_string()),
            html_link: Some("https://calendar.google.com/event?eid=xxx".to_string()),
            start: Some(EventTime {
                date_time: Some("2026-01-11T15:00:00Z".to_string()),
                date: None,
                time_zone: None,
            }),
            end: Some(EventTime {
                date_time: Some("2026-01-11T16:00:00Z".to_string()),
                date: None,
                time_zone: None,
            }),
            attendees: Some(vec![EventAttendee {
                email: Some("user@example.com".to_string()),
                display_name: None,
                response_status: None,
            }]),
        };

        let json = serde_json::to_string(&event).unwrap();
        let parsed: CalendarEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(event.id, parsed.id);
        assert_eq!(event.summary, parsed.summary);
        assert_eq!(event.attendees.as_ref().map(Vec::len), Some(1));
    }

    #[test]
    fn test_calendar_event_deserializes_with_missing_optional_fields() {
        let json = r#"{"id": "evt1"}"#;
        let event: CalendarEvent = serde_json::from_str(json).unwrap();

        assert_eq!(event.id, Some("evt1".to_string()));
        assert!(event.status.is_none());
        assert!(event.summary.is_none());
        assert!(event.start.is_none());
        assert!(event.attendees.is_none());
    }

    // ==========================================================================
    // From trait implementation tests
    // ==========================================================================

    #[test]
    fn test_calendar_event_from_api_event_converts_all_fields() {
        let api_event = ApiEvent {
            id: Some("evt1".to_string()),
            status: Some("confirmed".to_string()),
            summary: Some("Meeting".to_string()),
            description: Some("Description".to_string()),
            location: Some("Location".to_string()),
            html_link: Some("https://link".to_string()),
            start: Some(ApiEventDateTime {
                date_time: Some("2026-01-11T15:00:00Z".to_string()),
                date: None,
                time_zone: Some("UTC".to_string()),
            }),
            end: Some(ApiEventDateTime {
                date_time: Some("2026-01-11T16:00:00Z".to_string()),
                date: None,
                time_zone: None,
            }),
            attendees: Some(vec![ApiEventAttendee {
                email: Some("user@example.com".to_string()),
                display_name: Some("User".to_string()),
                response_status: Some("accepted".to_string()),
            }]),
        };

        let event: CalendarEvent = api_event.into();

        assert_eq!(event.id, Some("evt1".to_string()));
        assert_eq!(event.status, Some("confirmed".to_string()));
        assert_eq!(event.summary, Some("Meeting".to_string()));
        assert_eq!(event.description, Some("Description".to_string()));
        assert_eq!(event.location, Some("Location".to_string()));
        assert_eq!(event.html_link, Some("https://link".to_string()));

        let start = event.start.unwrap();
        assert_eq!(start.date_time, Some("2026-01-11T15:00:00Z".to_string()));
        assert_eq!(start.time_zone, Some("UTC".to_string()));

        let attendees = event.attendees.unwrap();
        assert_eq!(attendees.len(), 1);
        assert_eq!(attendees[0].email, Some("user@example.com".to_string()));
        assert_eq!(attendees[0].display_name, Some("User".to_string()));
    }

    #[test]
    fn test_calendar_event_from_api_event_handles_none_fields() {
        let api_event = ApiEvent {
            id: None,
            status: None,
            summary: None,
            description: None,
            location: None,
            html_link: None,
            start: None,
            end: None,
            attendees: None,
        };

        let event: CalendarEvent = api_event.into();

        assert!(event.id.is_none());
        assert!(event.start.is_none());
        assert!(event.attendees.is_none());
    }

    #[test]
    fn test_calendar_event_from_api_event_handles_empty_attendees() {
        let api_event = ApiEvent {
            id: Some("evt1".to_string()),
            status: None,
            summary: None,
            description: None,
            location: None,
            html_link: None,
            start: None,
            end: None,
            attendees: Some(vec![]),
        };

        let event: CalendarEvent = api_event.into();

        assert_eq!(event.attendees.as_ref().map(Vec::len), Some(0));
    }

    // ==========================================================================
    // API integration tests
    // ==========================================================================

    #[tokio::test]
    async fn test_list_events_returns_events_with_pagination() {
        let server = MockServer::start().await;
        let endpoint = format!("{}/calendar/v3", server.uri());

        Mock::given(method("GET"))
            .and(path("/calendar/v3/calendars/primary/events"))
            .and(query_param("timeMin", "2026-01-11T00:00:00Z"))
            .and(query_param("timeMax", "2026-01-12T00:00:00Z"))
            .and(query_param("singleEvents", "true"))
            .and(query_param("orderBy", "startTime"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [
                    {
                        "id": "evt1",
                        "status": "confirmed",
                        "summary": "Standup",
                        "start": { "dateTime": "2026-01-11T15:00:00Z" },
                        "end": { "dateTime": "2026-01-11T15:15:00Z" }
                    }
                ],
                "nextPageToken": "next"
            })))
            .mount(&server)
            .await;

        let ctx = test_context(&endpoint);
        let output = list_events(
            ctx,
            ListEventsInput {
                calendar_id: "primary".to_string(),
                time_min: "2026-01-11T00:00:00Z".to_string(),
                time_max: "2026-01-12T00:00:00Z".to_string(),
                q: None,
                page_token: None,
                max_results: None,
                single_events: true,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.events.len(), 1);
        assert_eq!(output.next_page_token.as_deref(), Some("next"));
        assert_eq!(output.events[0].id.as_deref(), Some("evt1"));
        assert_eq!(output.events[0].summary.as_deref(), Some("Standup"));
    }

    #[tokio::test]
    async fn test_list_events_returns_empty_list_when_no_events() {
        let server = MockServer::start().await;
        let endpoint = format!("{}/calendar/v3", server.uri());

        Mock::given(method("GET"))
            .and(path("/calendar/v3/calendars/primary/events"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": []
            })))
            .mount(&server)
            .await;

        let ctx = test_context(&endpoint);
        let output = list_events(
            ctx,
            ListEventsInput {
                calendar_id: "primary".to_string(),
                time_min: "2026-01-11T00:00:00Z".to_string(),
                time_max: "2026-01-12T00:00:00Z".to_string(),
                q: None,
                page_token: None,
                max_results: None,
                single_events: true,
            },
        )
        .await
        .unwrap();

        assert!(output.events.is_empty());
        assert!(output.next_page_token.is_none());
    }

    #[tokio::test]
    async fn test_list_events_with_empty_calendar_id_returns_error() {
        let server = MockServer::start().await;
        let endpoint = format!("{}/calendar/v3", server.uri());

        let ctx = test_context(&endpoint);
        let result = list_events(
            ctx,
            ListEventsInput {
                calendar_id: String::new(),
                time_min: "2026-01-11T00:00:00Z".to_string(),
                time_max: "2026-01-12T00:00:00Z".to_string(),
                q: None,
                page_token: None,
                max_results: None,
                single_events: true,
            },
        )
        .await;

        let err = result.unwrap_err().to_string();
        assert!(err.contains("calendar_id") && err.contains("must not be empty"));
    }

    #[tokio::test]
    async fn test_list_events_with_invalid_max_results_returns_error() {
        let server = MockServer::start().await;
        let endpoint = format!("{}/calendar/v3", server.uri());

        let ctx = test_context(&endpoint);
        let result = list_events(
            ctx,
            ListEventsInput {
                calendar_id: "primary".to_string(),
                time_min: "2026-01-11T00:00:00Z".to_string(),
                time_max: "2026-01-12T00:00:00Z".to_string(),
                q: None,
                page_token: None,
                max_results: Some(3000), // exceeds 2500 limit
                single_events: true,
            },
        )
        .await;

        let err = result.unwrap_err().to_string();
        assert!(err.contains("max_results"));
    }

    #[tokio::test]
    async fn test_list_events_returns_error_on_unauthorized() {
        let server = MockServer::start().await;
        let endpoint = format!("{}/calendar/v3", server.uri());

        Mock::given(method("GET"))
            .and(path("/calendar/v3/calendars/primary/events"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "error": { "message": "Unauthorized" }
            })))
            .mount(&server)
            .await;

        let ctx = test_context(&endpoint);
        let result = list_events(
            ctx,
            ListEventsInput {
                calendar_id: "primary".to_string(),
                time_min: "2026-01-11T00:00:00Z".to_string(),
                time_max: "2026-01-12T00:00:00Z".to_string(),
                q: None,
                page_token: None,
                max_results: None,
                single_events: true,
            },
        )
        .await;

        let err = result.unwrap_err().to_string();
        assert!(err.contains("401"), "{err}");
    }

    #[tokio::test]
    async fn test_create_event_returns_created_event() {
        let server = MockServer::start().await;
        let endpoint = format!("{}/calendar/v3", server.uri());

        Mock::given(method("POST"))
            .and(path("/calendar/v3/calendars/primary/events"))
            .and(query_param("sendUpdates", "all"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_json(json!({
                "summary": "Interview",
                "description": "Candidate interview",
                "start": { "dateTime": "2026-01-11T16:00:00Z" },
                "end": { "dateTime": "2026-01-11T17:00:00Z" },
                "attendees": [{ "email": "a@example.com" }]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "evt2",
                "status": "confirmed",
                "summary": "Interview",
                "start": { "dateTime": "2026-01-11T16:00:00Z" },
                "end": { "dateTime": "2026-01-11T17:00:00Z" }
            })))
            .mount(&server)
            .await;

        let ctx = test_context(&endpoint);
        let output = create_event(
            ctx,
            CreateEventInput {
                calendar_id: "primary".to_string(),
                summary: "Interview".to_string(),
                description: Some("Candidate interview".to_string()),
                location: None,
                start: EventTime {
                    date_time: Some("2026-01-11T16:00:00Z".to_string()),
                    date: None,
                    time_zone: None,
                },
                end: EventTime {
                    date_time: Some("2026-01-11T17:00:00Z".to_string()),
                    date: None,
                    time_zone: None,
                },
                attendees: Some(vec!["a@example.com".to_string()]),
                send_updates: Some("all".to_string()),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.event.id.as_deref(), Some("evt2"));
        assert_eq!(output.event.summary.as_deref(), Some("Interview"));
    }

    #[tokio::test]
    async fn test_create_event_with_empty_summary_returns_error() {
        let server = MockServer::start().await;
        let endpoint = format!("{}/calendar/v3", server.uri());

        let ctx = test_context(&endpoint);
        let result = create_event(
            ctx,
            CreateEventInput {
                calendar_id: "primary".to_string(),
                summary: String::new(),
                description: None,
                location: None,
                start: EventTime {
                    date_time: Some("2026-01-11T16:00:00Z".to_string()),
                    date: None,
                    time_zone: None,
                },
                end: EventTime {
                    date_time: Some("2026-01-11T17:00:00Z".to_string()),
                    date: None,
                    time_zone: None,
                },
                attendees: None,
                send_updates: None,
            },
        )
        .await;

        let err = result.unwrap_err().to_string();
        assert!(err.contains("summary") && err.contains("must not be empty"));
    }

    #[tokio::test]
    async fn test_create_event_with_mismatched_time_types_returns_error() {
        let server = MockServer::start().await;
        let endpoint = format!("{}/calendar/v3", server.uri());

        let ctx = test_context(&endpoint);
        let result = create_event(
            ctx,
            CreateEventInput {
                calendar_id: "primary".to_string(),
                summary: "Event".to_string(),
                description: None,
                location: None,
                start: EventTime {
                    date_time: Some("2026-01-11T16:00:00Z".to_string()),
                    date: None,
                    time_zone: None,
                },
                end: EventTime {
                    date_time: None,
                    date: Some("2026-01-11".to_string()),
                    time_zone: None,
                },
                attendees: None,
                send_updates: None,
            },
        )
        .await;

        let err = result.unwrap_err().to_string();
        assert!(err.contains("start and end must both use"));
    }

    #[tokio::test]
    async fn test_create_event_returns_error_on_server_error() {
        let server = MockServer::start().await;
        let endpoint = format!("{}/calendar/v3", server.uri());

        Mock::given(method("POST"))
            .and(path("/calendar/v3/calendars/primary/events"))
            .respond_with(ResponseTemplate::new(500).set_body_json(json!({
                "error": { "message": "Internal error" }
            })))
            .mount(&server)
            .await;

        let ctx = test_context(&endpoint);
        let result = create_event(
            ctx,
            CreateEventInput {
                calendar_id: "primary".to_string(),
                summary: "Interview".to_string(),
                description: None,
                location: None,
                start: EventTime {
                    date_time: Some("2026-01-11T16:00:00Z".to_string()),
                    date: None,
                    time_zone: None,
                },
                end: EventTime {
                    date_time: Some("2026-01-11T17:00:00Z".to_string()),
                    date: None,
                    time_zone: None,
                },
                attendees: None,
                send_updates: None,
            },
        )
        .await;

        let err = result.unwrap_err().to_string();
        assert!(err.contains("500"), "{err}");
    }

    #[tokio::test]
    async fn test_update_event_returns_updated_event() {
        let server = MockServer::start().await;
        let endpoint = format!("{}/calendar/v3", server.uri());

        Mock::given(method("PATCH"))
            .and(path("/calendar/v3/calendars/primary/events/evt3"))
            .and(query_param("sendUpdates", "none"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_json(json!({
                "summary": "Updated title"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "evt3",
                "status": "confirmed",
                "summary": "Updated title",
                "start": { "dateTime": "2026-01-11T10:00:00Z" },
                "end": { "dateTime": "2026-01-11T10:30:00Z" }
            })))
            .mount(&server)
            .await;

        let ctx = test_context(&endpoint);
        let output = update_event(
            ctx,
            UpdateEventInput {
                calendar_id: "primary".to_string(),
                event_id: "evt3".to_string(),
                summary: Some("Updated title".to_string()),
                description: None,
                location: None,
                start: None,
                end: None,
                attendees: None,
                send_updates: Some("none".to_string()),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.event.id.as_deref(), Some("evt3"));
        assert_eq!(output.event.summary.as_deref(), Some("Updated title"));
    }

    #[tokio::test]
    async fn test_update_event_with_no_updates_returns_error() {
        let server = MockServer::start().await;
        let endpoint = format!("{}/calendar/v3", server.uri());

        let ctx = test_context(&endpoint);
        let result = update_event(
            ctx,
            UpdateEventInput {
                calendar_id: "primary".to_string(),
                event_id: "evt1".to_string(),
                summary: None,
                description: None,
                location: None,
                start: None,
                end: None,
                attendees: None,
                send_updates: None,
            },
        )
        .await;

        let err = result.unwrap_err().to_string();
        assert!(err.contains("at least one of"));
    }

    #[tokio::test]
    async fn test_update_event_returns_error_on_not_found() {
        let server = MockServer::start().await;
        let endpoint = format!("{}/calendar/v3", server.uri());

        Mock::given(method("PATCH"))
            .and(path("/calendar/v3/calendars/primary/events/evt404"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({
                "error": { "message": "Not found" }
            })))
            .mount(&server)
            .await;

        let ctx = test_context(&endpoint);
        let result = update_event(
            ctx,
            UpdateEventInput {
                calendar_id: "primary".to_string(),
                event_id: "evt404".to_string(),
                summary: Some("Updated title".to_string()),
                description: None,
                location: None,
                start: None,
                end: None,
                attendees: None,
                send_updates: None,
            },
        )
        .await;

        let err = result.unwrap_err().to_string();
        assert!(err.contains("404"), "{err}");
    }

    #[tokio::test]
    async fn test_cancel_returns_success() {
        let server = MockServer::start().await;
        let endpoint = format!("{}/calendar/v3", server.uri());

        Mock::given(method("DELETE"))
            .and(path("/calendar/v3/calendars/primary/events/evt4"))
            .and(query_param("sendUpdates", "none"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let ctx = test_context(&endpoint);
        let output = cancel(
            ctx,
            CancelInput {
                calendar_id: "primary".to_string(),
                event_id: "evt4".to_string(),
                send_updates: Some("none".to_string()),
            },
        )
        .await
        .unwrap();

        assert!(output.cancelled);
    }

    #[tokio::test]
    async fn test_cancel_returns_error_on_not_found() {
        let server = MockServer::start().await;
        let endpoint = format!("{}/calendar/v3", server.uri());

        Mock::given(method("DELETE"))
            .and(path("/calendar/v3/calendars/primary/events/evt404"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({
                "error": { "message": "Not found" }
            })))
            .mount(&server)
            .await;

        let ctx = test_context(&endpoint);
        let result = cancel(
            ctx,
            CancelInput {
                calendar_id: "primary".to_string(),
                event_id: "evt404".to_string(),
                send_updates: None,
            },
        )
        .await;

        let err = result.unwrap_err().to_string();
        assert!(err.contains("404"), "{err}");
    }

    #[tokio::test]
    async fn test_free_busy_returns_busy_intervals() {
        let server = MockServer::start().await;
        let endpoint = format!("{}/calendar/v3", server.uri());

        Mock::given(method("POST"))
            .and(path("/calendar/v3/freeBusy"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_json(json!({
                "timeMin": "2026-01-11T00:00:00Z",
                "timeMax": "2026-01-12T00:00:00Z",
                "items": [
                    { "id": "primary" },
                    { "id": "other@example.com" }
                ]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "calendars": {
                    "primary": { "busy": [ { "start": "2026-01-11T15:00:00Z", "end": "2026-01-11T15:15:00Z" } ] },
                    "other@example.com": { "busy": [] }
                }
            })))
            .mount(&server)
            .await;

        let ctx = test_context(&endpoint);
        let output = free_busy(
            ctx,
            FreeBusyInput {
                time_min: "2026-01-11T00:00:00Z".to_string(),
                time_max: "2026-01-12T00:00:00Z".to_string(),
                calendar_ids: vec!["primary".to_string(), "other@example.com".to_string()],
                time_zone: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.calendars.len(), 2);
        assert_eq!(output.calendars[0].calendar_id, "other@example.com");
        assert_eq!(output.calendars[1].calendar_id, "primary");
        assert_eq!(output.calendars[1].busy.len(), 1);
    }

    #[tokio::test]
    async fn test_free_busy_with_empty_calendar_ids_returns_error() {
        let server = MockServer::start().await;
        let endpoint = format!("{}/calendar/v3", server.uri());

        let ctx = test_context(&endpoint);
        let result = free_busy(
            ctx,
            FreeBusyInput {
                time_min: "2026-01-11T00:00:00Z".to_string(),
                time_max: "2026-01-12T00:00:00Z".to_string(),
                calendar_ids: vec![],
                time_zone: None,
            },
        )
        .await;

        let err = result.unwrap_err().to_string();
        assert!(err.contains("calendar_ids must include at least one"));
    }

    #[tokio::test]
    async fn test_free_busy_returns_error_on_unauthorized() {
        let server = MockServer::start().await;
        let endpoint = format!("{}/calendar/v3", server.uri());

        Mock::given(method("POST"))
            .and(path("/calendar/v3/freeBusy"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "error": { "message": "Unauthorized" }
            })))
            .mount(&server)
            .await;

        let ctx = test_context(&endpoint);
        let result = free_busy(
            ctx,
            FreeBusyInput {
                time_min: "2026-01-11T00:00:00Z".to_string(),
                time_max: "2026-01-12T00:00:00Z".to_string(),
                calendar_ids: vec!["primary".to_string()],
                time_zone: None,
            },
        )
        .await;

        let err = result.unwrap_err().to_string();
        assert!(err.contains("401"), "{err}");
    }
}
