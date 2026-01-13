//! calendars-scheduling/calendly integration for Operai Toolbox.
use std::time::Duration;

use operai::{
    Context, JsonSchema, Result, bail, define_user_credential, ensure, info, init, schemars,
    shutdown, tool,
};
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

define_user_credential! {
    CalendlyCredential("calendly") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_API_ENDPOINT: &str = "https://api.calendly.com";

#[derive(Clone)]
struct CalendlyClient {
    http: reqwest::Client,
    base_url: reqwest::Url,
    api_key: String,
}

impl CalendlyClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = CalendlyCredential::get(ctx)?;

        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(USER_AGENT, HeaderValue::from_static("operai-calendly/0.1"));

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()?;

        let endpoint = cred.endpoint.as_deref().unwrap_or(DEFAULT_API_ENDPOINT);
        let base_url = reqwest::Url::parse(endpoint)?;

        Ok(Self {
            http,
            base_url,
            api_key: cred.access_token,
        })
    }

    fn url(&self, path: &str) -> Result<reqwest::Url> {
        ensure!(
            path.starts_with('/'),
            "internal error: path must start with '/'"
        );
        Ok(self.base_url.join(path)?)
    }

    async fn get_json<T: DeserializeOwned>(&self, url: reqwest::Url) -> Result<T> {
        let response = self.http.get(url).bearer_auth(&self.api_key).send().await?;

        Self::parse_json_response(response).await
    }

    async fn post_json<B: Serialize, T: DeserializeOwned>(
        &self,
        url: reqwest::Url,
        body: &B,
    ) -> Result<T> {
        let response = self
            .http
            .post(url)
            .bearer_auth(&self.api_key)
            .json(body)
            .send()
            .await?;

        Self::parse_json_response(response).await
    }

    async fn parse_json_response<T: DeserializeOwned>(response: reqwest::Response) -> Result<T> {
        let status = response.status();
        let url = response.url().clone();

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            bail!("Calendly API request failed ({status}) for {url}: {body}");
        }

        Ok(response.json::<T>().await?)
    }
}

#[init]
async fn setup() -> Result<()> {
    info!("Calendly integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Calendly integration shutting down");
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListBookingsInput {
    /// Filter bookings for this Calendly user URI.
    ///
    /// Example: `https://api.calendly.com/users/AAAAAAAAAAAAAAAA`
    #[serde(default)]
    pub user_uri: Option<String>,

    /// Filter bookings for this Calendly organization URI.
    ///
    /// Example: `https://api.calendly.com/organizations/AAAAAAAAAAAAAAAA`
    #[serde(default)]
    pub organization_uri: Option<String>,

    /// Earliest event start time to include (ISO 8601).
    #[serde(default)]
    pub min_start_time: Option<String>,

    /// Latest event start time to include (ISO 8601).
    #[serde(default)]
    pub max_start_time: Option<String>,

    /// Limit results returned (Calendly supports pagination).
    #[serde(default)]
    pub count: Option<u32>,

    /// Pagination token or cursor from a previous call.
    #[serde(default)]
    pub page_token: Option<String>,

    /// Event status filter (e.g. `active`, `canceled`).
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct BookingLocation {
    /// Calendly location type (e.g. `physical`, `zoom`, `google_conference`).
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub location_type: Option<String>,

    /// Freeform location string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,

    /// Join URL for virtual meetings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub join_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct Booking {
    /// Calendly scheduled event URI.
    pub uri: String,

    /// Display name of the event type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Scheduled event status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,

    /// Start time (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,

    /// End time (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,

    /// Associated event type URI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_type_uri: Option<String>,

    /// Location details when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<BookingLocation>,

    /// Creation time (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,

    /// Last update time (ISO 8601).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct Pagination {
    /// Number of results in this page (when provided by the API).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,

    /// URL to fetch the next page (when provided by the API).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page: Option<String>,

    /// URL to fetch the previous page (when provided by the API).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_page: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListBookingsOutput {
    pub bookings: Vec<Booking>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<Pagination>,

    pub request_id: String,
}

#[derive(Debug, Deserialize)]
struct ApiPagination {
    #[serde(default)]
    count: Option<u32>,
    #[serde(default)]
    next_page: Option<String>,
    #[serde(default)]
    previous_page: Option<String>,
}

impl From<ApiPagination> for Pagination {
    fn from(value: ApiPagination) -> Self {
        Self {
            count: value.count,
            next_page: value.next_page,
            previous_page: value.previous_page,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ApiCollectionResponse<T> {
    collection: Vec<T>,
    #[serde(default)]
    pagination: Option<ApiPagination>,
}

#[derive(Debug, Deserialize)]
struct ApiLocation {
    #[serde(rename = "type", default)]
    location_type: Option<String>,
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    join_url: Option<String>,
}

impl From<ApiLocation> for BookingLocation {
    fn from(value: ApiLocation) -> Self {
        Self {
            location_type: value.location_type,
            location: value.location,
            join_url: value.join_url,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ApiScheduledEvent {
    uri: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    start_time: Option<String>,
    #[serde(default)]
    end_time: Option<String>,
    #[serde(default)]
    event_type: Option<String>,
    #[serde(default)]
    location: Option<ApiLocation>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
}

impl From<ApiScheduledEvent> for Booking {
    fn from(value: ApiScheduledEvent) -> Self {
        Self {
            uri: value.uri,
            name: value.name,
            status: value.status,
            start_time: value.start_time,
            end_time: value.end_time,
            event_type_uri: value.event_type,
            location: value.location.map(Into::into),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

/// # List Calendly Bookings
///
/// Retrieves scheduled Calendly events (bookings) with support for filtering
/// and pagination. Use this tool when the user wants to view their upcoming or
/// past Calendly appointments, meeting bookings, or scheduled events. Supports
/// filtering by user/organization, date ranges, event status, and pagination
/// for large result sets.
///
/// Key use cases:
/// - Listing upcoming meetings for a specific user
/// - Retrieving bookings within a date range (e.g., "this week's appointments")
/// - Filtering events by status (active, canceled)
/// - Fetching bookings for an entire organization
///
/// Requires either `user_uri` or `organization_uri` to filter results. Supports
/// pagination via `count` and `page_token` for handling large datasets.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - calendar
/// - scheduling
/// - calendly
///
/// # Errors
///
/// Returns an error if:
/// - Neither `user_uri` nor `organization_uri` is provided
/// - `count` is outside the valid range (1-100)
/// - Any provided string field is empty or contains only whitespace
/// - Credentials are not configured or are invalid
/// - The API request fails or returns an error response
/// - The URL construction fails
#[tool]
pub async fn list_bookings(ctx: Context, input: ListBookingsInput) -> Result<ListBookingsOutput> {
    let has_user = input
        .user_uri
        .as_deref()
        .is_some_and(|v| !v.trim().is_empty());
    let has_org = input
        .organization_uri
        .as_deref()
        .is_some_and(|v| !v.trim().is_empty());

    ensure!(
        has_user || has_org,
        "Either user_uri or organization_uri must be provided"
    );

    if let Some(count) = input.count {
        ensure!(
            (1..=100).contains(&count),
            "count must be between 1 and 100"
        );
    }

    for (field, value) in [
        ("min_start_time", input.min_start_time.as_deref()),
        ("max_start_time", input.max_start_time.as_deref()),
        ("page_token", input.page_token.as_deref()),
        ("status", input.status.as_deref()),
    ] {
        if let Some(value) = value {
            ensure!(!value.trim().is_empty(), "{field} must not be empty");
        }
    }

    let client = CalendlyClient::from_ctx(&ctx)?;
    let mut url = client.url("/scheduled_events")?;
    {
        let mut query = url.query_pairs_mut();
        if let Some(user) = input.user_uri.as_deref() {
            query.append_pair("user", user);
        }
        if let Some(org) = input.organization_uri.as_deref() {
            query.append_pair("organization", org);
        }
        if let Some(min_start_time) = input.min_start_time.as_deref() {
            query.append_pair("min_start_time", min_start_time);
        }
        if let Some(max_start_time) = input.max_start_time.as_deref() {
            query.append_pair("max_start_time", max_start_time);
        }
        if let Some(count) = input.count {
            query.append_pair("count", &count.to_string());
        }
        if let Some(page_token) = input.page_token.as_deref() {
            query.append_pair("page_token", page_token);
        }
        if let Some(status) = input.status.as_deref() {
            query.append_pair("status", status);
        }
    }

    let response: ApiCollectionResponse<ApiScheduledEvent> = client.get_json(url).await?;
    Ok(ListBookingsOutput {
        bookings: response.collection.into_iter().map(Into::into).collect(),
        pagination: response.pagination.map(Into::into),
        request_id: ctx.request_id().to_string(),
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateSchedulingLinkInput {
    /// Event type URI to create a scheduling link for.
    ///
    /// Example: `https://api.calendly.com/event_types/AAAAAAAAAAAAAAAA`
    pub event_type_uri: String,

    /// Maximum number of times the scheduling link can be used.
    #[serde(default)]
    pub max_event_count: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateSchedulingLinkOutput {
    /// Scheduling link resource URI.
    pub scheduling_link_uri: String,

    /// URL the end-user can visit to book.
    pub booking_url: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_event_count: Option<u32>,

    pub request_id: String,
}

#[derive(Debug, Serialize)]
struct ApiCreateSchedulingLinkRequest<'a> {
    owner: &'a str,
    owner_type: &'a str,
    max_event_count: u32,
}

#[derive(Debug, Deserialize)]
struct ApiResourceResponse<T> {
    resource: T,
}

#[derive(Debug, Deserialize)]
struct ApiSchedulingLink {
    uri: String,
    booking_url: String,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    owner_type: Option<String>,
    #[serde(default)]
    max_event_count: Option<u32>,
}

/// # Create Calendly Scheduling Link
///
/// Creates a one-time or limited-use scheduling link for a specific Calendly
/// event type. Use this tool when the user wants to generate a shareable
/// booking link that allows others to schedule appointments using a predefined
/// Calendly event type (e.g., "30-minute consultation").
///
/// Key use cases:
/// - Generating a booking link for a specific meeting type
/// - Creating limited-use scheduling links (e.g., "only allow 5 bookings")
/// - Sharing bookable links via email, chat, or other platforms
/// - Automating scheduling link creation for workflows
///
/// The returned `booking_url` is a web URL that can be shared with invitees.
/// The link can be restricted to a maximum number of uses via `max_event_count`
/// (defaults to 1 if not specified). This is useful for controlling how many
/// times a particular scheduling link can be used.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - calendar
/// - scheduling
/// - calendly
///
/// # Errors
///
/// Returns an error if:
/// - `event_type_uri` is empty or contains only whitespace
/// - `max_event_count` is outside the valid range (1-100)
/// - Credentials are not configured or are invalid
/// - The API request fails or returns an error response
/// - The URL construction fails
#[tool]
pub async fn create_scheduling_link(
    ctx: Context,
    input: CreateSchedulingLinkInput,
) -> Result<CreateSchedulingLinkOutput> {
    ensure!(
        !input.event_type_uri.trim().is_empty(),
        "event_type_uri must not be empty"
    );

    let max_event_count = input.max_event_count.unwrap_or(1);
    ensure!(
        (1..=100).contains(&max_event_count),
        "max_event_count must be between 1 and 100"
    );

    let client = CalendlyClient::from_ctx(&ctx)?;
    let url = client.url("/scheduling_links")?;
    let request = ApiCreateSchedulingLinkRequest {
        owner: &input.event_type_uri,
        owner_type: "EventType",
        max_event_count,
    };

    let response: ApiResourceResponse<ApiSchedulingLink> = client.post_json(url, &request).await?;
    Ok(CreateSchedulingLinkOutput {
        scheduling_link_uri: response.resource.uri,
        booking_url: response.resource.booking_url,
        owner: response.resource.owner,
        owner_type: response.resource.owner_type,
        max_event_count: response.resource.max_event_count,
        request_id: ctx.request_id().to_string(),
    })
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CancelRescheduleAction {
    Cancel,
    Reschedule,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CancelRescheduleInput {
    /// Whether to cancel the event in Calendly or generate a rescheduling URL
    /// for the invitee.
    pub action: CancelRescheduleAction,

    /// Scheduled event UUID (preferred for cancellation/rescheduling).
    #[serde(default)]
    pub scheduled_event_uuid: Option<String>,

    /// Scheduled event URI. UUID will be extracted from this when provided.
    #[serde(default)]
    pub scheduled_event_uri: Option<String>,

    /// Invitee UUID (required for `reschedule`).
    #[serde(default)]
    pub invitee_uuid: Option<String>,

    /// Invitee URI (required for `reschedule` if `invitee_uuid` isn't
    /// provided).
    ///
    /// Example: `https://api.calendly.com/scheduled_events/{event_uuid}/invitees/{invitee_uuid}`
    #[serde(default)]
    pub invitee_uri: Option<String>,

    /// Optional cancellation reason.
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct Cancellation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canceled_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduled_event_uri: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CancelRescheduleOutput {
    pub action: CancelRescheduleAction,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancellation: Option<Cancellation>,

    /// Invitee rescheduling URL for `reschedule` action (web URL).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reschedule_url: Option<String>,

    pub request_id: String,
}

#[derive(Debug, Serialize)]
struct ApiCancelRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ApiUriOrObject {
    Uri(String),
    Object { uri: String },
}

impl ApiUriOrObject {
    fn into_uri(self) -> String {
        match self {
            Self::Uri(uri) | Self::Object { uri } => uri,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ApiCancellation {
    #[serde(default)]
    uri: Option<String>,
    #[serde(default)]
    canceled_at: Option<String>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    scheduled_event: Option<ApiUriOrObject>,
}

impl From<ApiCancellation> for Cancellation {
    fn from(value: ApiCancellation) -> Self {
        Self {
            uri: value.uri,
            canceled_at: value.canceled_at,
            reason: value.reason,
            scheduled_event_uri: value.scheduled_event.map(ApiUriOrObject::into_uri),
        }
    }
}

/// # Cancel or Reschedule Calendly Booking
///
/// Cancels a Calendly scheduled event or generates a rescheduling URL for an
/// invitee. Use this tool when the user wants to either cancel an existing
/// booking or help an invitee reschedule their appointment. Supports both
/// cancellation with an optional reason and rescheduling URL generation.
///
/// Key use cases:
/// - Canceling a scheduled event/meeting
/// - Generating a reschedule link for an invitee to pick a new time
/// - Automating cancellation workflows (e.g., with cancellation reason
///   tracking)
/// - Providing self-service rescheduling options to meeting participants
///
/// For cancellation: the event is canceled in Calendly and cancellation details
/// are returned. An optional `reason` can be provided to document why the event
/// was canceled.
///
/// For rescheduling: a `reschedule_url` is returned that can be shared with the
/// invitee, allowing them to select a new time slot. Requires both the
/// scheduled event identifier and the invitee identifier to generate the
/// correct rescheduling link.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - calendar
/// - scheduling
/// - calendly
///
/// # Errors
///
/// Returns an error if:
/// - `reason` is provided but is empty or contains only whitespace
/// - Required UUID fields cannot be resolved from the provided inputs
/// - Credentials are not configured or are invalid
/// - The API request fails or returns an error response
/// - The URL construction fails
/// - The reschedule URL is not present in the invitee response (for reschedule
///   action)
#[tool]
pub async fn cancel_reschedule(
    ctx: Context,
    input: CancelRescheduleInput,
) -> Result<CancelRescheduleOutput> {
    if let Some(reason) = input.reason.as_deref() {
        ensure!(!reason.trim().is_empty(), "reason must not be empty");
    }

    let client = CalendlyClient::from_ctx(&ctx)?;

    match input.action {
        CancelRescheduleAction::Cancel => {
            let event_uuid = resolve_scheduled_event_uuid(
                input.scheduled_event_uuid.as_deref(),
                input.scheduled_event_uri.as_deref(),
                input.invitee_uri.as_deref(),
            )?;
            validate_path_segment("scheduled_event_uuid", &event_uuid)?;

            let url = client.url(&format!("/scheduled_events/{event_uuid}/cancellation"))?;
            let body = ApiCancelRequest {
                reason: input.reason.as_deref(),
            };
            let response: ApiResourceResponse<ApiCancellation> =
                client.post_json(url, &body).await?;

            Ok(CancelRescheduleOutput {
                action: CancelRescheduleAction::Cancel,
                cancellation: Some(response.resource.into()),
                reschedule_url: None,
                request_id: ctx.request_id().to_string(),
            })
        }
        CancelRescheduleAction::Reschedule => {
            let (event_uuid, invitee_uuid) = resolve_scheduled_event_and_invitee(
                input.scheduled_event_uuid.as_deref(),
                input.scheduled_event_uri.as_deref(),
                input.invitee_uuid.as_deref(),
                input.invitee_uri.as_deref(),
            )?;

            validate_path_segment("scheduled_event_uuid", &event_uuid)?;
            validate_path_segment("invitee_uuid", &invitee_uuid)?;

            let url = client.url(&format!(
                "/scheduled_events/{event_uuid}/invitees/{invitee_uuid}"
            ))?;
            let response: ApiResourceResponse<ApiInvitee> = client.get_json(url).await?;

            let reschedule_url = response.resource.reschedule_url;
            ensure!(
                reschedule_url
                    .as_deref()
                    .is_some_and(|v| !v.trim().is_empty()),
                "Calendly invitee did not include a reschedule_url"
            );

            Ok(CancelRescheduleOutput {
                action: CancelRescheduleAction::Reschedule,
                cancellation: None,
                reschedule_url,
                request_id: ctx.request_id().to_string(),
            })
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FetchInviteeInfoInput {
    /// Scheduled event UUID.
    #[serde(default)]
    pub scheduled_event_uuid: Option<String>,

    /// Scheduled event URI (UUID will be extracted from this when provided).
    #[serde(default)]
    pub scheduled_event_uri: Option<String>,

    /// Invitee UUID.
    #[serde(default)]
    pub invitee_uuid: Option<String>,

    /// Invitee URI (UUIDs will be extracted from this when provided).
    #[serde(default)]
    pub invitee_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema, PartialEq, Eq)]
pub struct InviteeInfo {
    pub uri: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel_url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub reschedule_url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FetchInviteeInfoOutput {
    pub invitee: InviteeInfo,
    pub request_id: String,
}

#[derive(Debug, Deserialize)]
struct ApiInvitee {
    uri: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    first_name: Option<String>,
    #[serde(default)]
    last_name: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    cancel_url: Option<String>,
    #[serde(default)]
    reschedule_url: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
}

impl From<ApiInvitee> for InviteeInfo {
    fn from(value: ApiInvitee) -> Self {
        Self {
            uri: value.uri,
            email: value.email,
            name: value.name,
            first_name: value.first_name,
            last_name: value.last_name,
            status: value.status,
            cancel_url: value.cancel_url,
            reschedule_url: value.reschedule_url,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

/// # Fetch Calendly Invitee Information
///
/// Retrieves detailed information about a specific invitee for a Calendly
/// scheduled event. Use this tool when the user wants to look up details about
/// a meeting participant, including their contact information, status, and
/// management URLs (cancel/reschedule links).
///
/// Key use cases:
/// - Viewing invitee contact details (name, email) for a scheduled event
/// - Checking invitee status (active, canceled)
/// - Retrieving cancel/reschedule URLs for an invitee
/// - Validating invitee information before sending follow-up communications
/// - Fetching management links for customer service workflows
///
/// The returned `InviteeInfo` includes comprehensive details about the invitee,
/// including their name, email addresses, current status, and action URLs. The
/// `cancel_url` and `reschedule_url` fields provide direct web links that can
/// be shared with the invitee for self-service management of their booking.
///
/// Requires both the scheduled event identifier and the invitee identifier to
/// fetch the correct invitee information. These can be provided as UUIDs or as
/// full URIs from which UUIDs will be extracted.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - calendar
/// - scheduling
/// - calendly
///
/// # Errors
///
/// Returns an error if:
/// - Required UUID fields cannot be resolved from the provided inputs
/// - Credentials are not configured or are invalid
/// - The API request fails or returns an error response
/// - The URL construction fails
#[tool]
pub async fn fetch_invitee_info(
    ctx: Context,
    input: FetchInviteeInfoInput,
) -> Result<FetchInviteeInfoOutput> {
    let (event_uuid, invitee_uuid) = resolve_scheduled_event_and_invitee(
        input.scheduled_event_uuid.as_deref(),
        input.scheduled_event_uri.as_deref(),
        input.invitee_uuid.as_deref(),
        input.invitee_uri.as_deref(),
    )?;

    validate_path_segment("scheduled_event_uuid", &event_uuid)?;
    validate_path_segment("invitee_uuid", &invitee_uuid)?;

    let client = CalendlyClient::from_ctx(&ctx)?;
    let url = client.url(&format!(
        "/scheduled_events/{event_uuid}/invitees/{invitee_uuid}"
    ))?;
    let response: ApiResourceResponse<ApiInvitee> = client.get_json(url).await?;

    Ok(FetchInviteeInfoOutput {
        invitee: response.resource.into(),
        request_id: ctx.request_id().to_string(),
    })
}

fn validate_path_segment(name: &str, value: &str) -> Result<()> {
    ensure!(!value.trim().is_empty(), "{name} must not be empty");
    ensure!(!value.contains('/'), "{name} must not contain '/'");
    ensure!(!value.contains('?'), "{name} must not contain '?'");
    ensure!(!value.contains('#'), "{name} must not contain '#'");
    Ok(())
}

fn resolve_scheduled_event_uuid(
    scheduled_event_uuid: Option<&str>,
    scheduled_event_uri: Option<&str>,
    invitee_uri: Option<&str>,
) -> Result<String> {
    if let Some(uuid) = scheduled_event_uuid.filter(|v| !v.trim().is_empty()) {
        return Ok(uuid.to_string());
    }

    if let Some(uri) = scheduled_event_uri
        && let Some(uuid) = scheduled_event_uuid_from_uri(uri)
    {
        return Ok(uuid);
    }

    if let Some(uri) = invitee_uri
        && let Some((event_uuid, _invitee_uuid)) = invitee_uuids_from_uri(uri)
    {
        return Ok(event_uuid);
    }

    bail!("scheduled_event_uuid or scheduled_event_uri is required")
}

fn resolve_scheduled_event_and_invitee(
    scheduled_event_uuid: Option<&str>,
    scheduled_event_uri: Option<&str>,
    invitee_uuid: Option<&str>,
    invitee_uri: Option<&str>,
) -> Result<(String, String)> {
    if let Some(invitee_uri) = invitee_uri
        && let Some((event_uuid, invitee_uuid)) = invitee_uuids_from_uri(invitee_uri)
    {
        return Ok((event_uuid, invitee_uuid));
    }

    let event_uuid = resolve_scheduled_event_uuid(scheduled_event_uuid, scheduled_event_uri, None)?;

    let invitee_uuid = invitee_uuid
        .filter(|v| !v.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| operai::anyhow::anyhow!("invitee_uuid or invitee_uri is required"))?;

    Ok((event_uuid, invitee_uuid))
}

fn strip_query_and_fragment(mut uri: &str) -> &str {
    if let Some((head, _)) = uri.split_once('?') {
        uri = head;
    }
    if let Some((head, _)) = uri.split_once('#') {
        uri = head;
    }
    uri
}

fn scheduled_event_uuid_from_uri(uri: &str) -> Option<String> {
    let uri = strip_query_and_fragment(uri);
    let segments: Vec<&str> = uri.split('/').filter(|s| !s.is_empty()).collect();

    segments.windows(2).find_map(|window| match window {
        [scheduled_events, uuid] if *scheduled_events == "scheduled_events" => {
            Some((*uuid).to_string())
        }
        _ => None,
    })
}

fn invitee_uuids_from_uri(uri: &str) -> Option<(String, String)> {
    let uri = strip_query_and_fragment(uri);
    let segments: Vec<&str> = uri.split('/').filter(|s| !s.is_empty()).collect();

    segments.windows(4).find_map(|window| match window {
        [scheduled_events, event_uuid, invitees, invitee_uuid]
            if *scheduled_events == "scheduled_events" && *invitees == "invitees" =>
        {
            Some(((*event_uuid).to_string(), (*invitee_uuid).to_string()))
        }
        _ => None,
    })
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

    fn test_context(server: &MockServer) -> Context {
        let mut calendly_values = HashMap::new();
        calendly_values.insert("access_token".to_string(), "test-token".to_string());
        calendly_values.insert("endpoint".to_string(), server.uri());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("calendly", calendly_values)
    }

    // ========== Serialization Tests ==========

    #[test]
    fn test_booking_serialization_includes_all_fields() {
        let booking = Booking {
            uri: "https://api.calendly.com/scheduled_events/EVT1".to_string(),
            name: Some("Test Meeting".to_string()),
            status: Some("active".to_string()),
            start_time: Some("2025-01-01T00:00:00Z".to_string()),
            end_time: Some("2025-01-01T00:30:00Z".to_string()),
            event_type_uri: Some("https://api.calendly.com/event_types/ET1".to_string()),
            location: Some(BookingLocation {
                location_type: Some("zoom".to_string()),
                location: Some("Zoom".to_string()),
                join_url: Some("https://zoom.example.com/join".to_string()),
            }),
            created_at: Some("2024-12-01T00:00:00Z".to_string()),
            updated_at: Some("2024-12-02T00:00:00Z".to_string()),
        };

        let json = serde_json::to_value(&booking).expect("serialization should succeed");
        let obj = json.as_object().expect("should be an object");

        assert_eq!(
            obj.get("uri").and_then(|v| v.as_str()),
            Some("https://api.calendly.com/scheduled_events/EVT1")
        );
        assert_eq!(
            obj.get("name").and_then(|v| v.as_str()),
            Some("Test Meeting")
        );
        assert_eq!(obj.get("status").and_then(|v| v.as_str()), Some("active"));
        assert!(obj.contains_key("location"));
    }

    #[test]
    fn test_booking_optional_fields_not_serialized_when_none() {
        let booking = Booking {
            uri: "https://api.calendly.com/scheduled_events/EVT1".to_string(),
            name: None,
            status: None,
            start_time: None,
            end_time: None,
            event_type_uri: None,
            location: None,
            created_at: None,
            updated_at: None,
        };

        let json = serde_json::to_value(&booking).expect("serialization should succeed");
        let obj = json.as_object().expect("should be an object");

        assert!(obj.contains_key("uri"));
        assert!(!obj.contains_key("name"));
        assert!(!obj.contains_key("status"));
        assert!(!obj.contains_key("location"));
    }

    #[test]
    fn test_booking_location_type_renamed_to_type_in_json() {
        let location = BookingLocation {
            location_type: Some("zoom".to_string()),
            location: Some("Zoom Meeting".to_string()),
            join_url: Some("https://zoom.example.com".to_string()),
        };

        let json = serde_json::to_value(&location).expect("serialization should succeed");
        let obj = json.as_object().expect("should be an object");

        assert!(obj.contains_key("type"));
        assert!(!obj.contains_key("location_type"));
        assert_eq!(obj.get("type").and_then(|v| v.as_str()), Some("zoom"));
    }

    #[test]
    fn test_pagination_optional_fields_not_serialized_when_none() {
        let pagination = Pagination {
            count: Some(10),
            next_page: None,
            previous_page: None,
        };

        let json = serde_json::to_value(&pagination).expect("serialization should succeed");
        let obj = json.as_object().expect("should be an object");

        assert!(obj.contains_key("count"));
        assert!(!obj.contains_key("next_page"));
        assert!(!obj.contains_key("previous_page"));
    }

    #[test]
    fn test_cancel_reschedule_action_serialization_roundtrip() {
        let cancel = CancelRescheduleAction::Cancel;
        let reschedule = CancelRescheduleAction::Reschedule;

        assert_eq!(serde_json::to_string(&cancel).unwrap(), "\"cancel\"");
        assert_eq!(
            serde_json::to_string(&reschedule).unwrap(),
            "\"reschedule\""
        );

        let parsed_cancel: CancelRescheduleAction =
            serde_json::from_str("\"cancel\"").expect("deserialize cancel");
        let parsed_reschedule: CancelRescheduleAction =
            serde_json::from_str("\"reschedule\"").expect("deserialize reschedule");

        assert_eq!(parsed_cancel, CancelRescheduleAction::Cancel);
        assert_eq!(parsed_reschedule, CancelRescheduleAction::Reschedule);
    }

    #[test]
    fn test_cancellation_optional_fields_not_serialized_when_none() {
        let cancellation = Cancellation {
            uri: None,
            canceled_at: None,
            reason: None,
            scheduled_event_uri: None,
        };

        let json = serde_json::to_value(&cancellation).expect("serialization should succeed");
        let obj = json.as_object().expect("should be an object");

        assert!(obj.is_empty());
    }

    #[test]
    fn test_invitee_info_serialization_includes_all_fields() {
        let invitee = InviteeInfo {
            uri: "https://api.calendly.com/scheduled_events/EV1/invitees/INV1".to_string(),
            email: Some("test@example.com".to_string()),
            name: Some("Test User".to_string()),
            first_name: Some("Test".to_string()),
            last_name: Some("User".to_string()),
            status: Some("active".to_string()),
            cancel_url: Some("https://calendly.com/cancel/abc".to_string()),
            reschedule_url: Some("https://calendly.com/reschedule/abc".to_string()),
            created_at: Some("2024-12-01T00:00:00Z".to_string()),
            updated_at: Some("2024-12-02T00:00:00Z".to_string()),
        };

        let json = serde_json::to_value(&invitee).expect("serialization should succeed");
        let obj = json.as_object().expect("should be an object");

        assert_eq!(
            obj.get("uri").and_then(|v| v.as_str()),
            Some("https://api.calendly.com/scheduled_events/EV1/invitees/INV1")
        );
        assert_eq!(
            obj.get("email").and_then(|v| v.as_str()),
            Some("test@example.com")
        );
        assert_eq!(obj.get("name").and_then(|v| v.as_str()), Some("Test User"));
    }

    // ========== From Trait Implementation Tests ==========

    #[test]
    fn test_api_location_to_booking_location_conversion() {
        let api_location = ApiLocation {
            location_type: Some("zoom".to_string()),
            location: Some("Zoom Meeting".to_string()),
            join_url: Some("https://zoom.example.com/join".to_string()),
        };

        let booking_location: BookingLocation = api_location.into();

        assert_eq!(booking_location.location_type, Some("zoom".to_string()));
        assert_eq!(booking_location.location, Some("Zoom Meeting".to_string()));
        assert_eq!(
            booking_location.join_url,
            Some("https://zoom.example.com/join".to_string())
        );
    }

    #[test]
    fn test_api_pagination_to_pagination_conversion() {
        let api_pagination = ApiPagination {
            count: Some(25),
            next_page: Some("https://api.calendly.com/events?page=3".to_string()),
            previous_page: Some("https://api.calendly.com/events?page=1".to_string()),
        };

        let pagination: Pagination = api_pagination.into();

        assert_eq!(pagination.count, Some(25));
        assert_eq!(
            pagination.next_page,
            Some("https://api.calendly.com/events?page=3".to_string())
        );
        assert_eq!(
            pagination.previous_page,
            Some("https://api.calendly.com/events?page=1".to_string())
        );
    }

    #[test]
    fn test_api_scheduled_event_to_booking_conversion() {
        let api_event = ApiScheduledEvent {
            uri: "https://api.calendly.com/scheduled_events/EVT1".to_string(),
            name: Some("Team Standup".to_string()),
            status: Some("active".to_string()),
            start_time: Some("2025-01-15T09:00:00Z".to_string()),
            end_time: Some("2025-01-15T09:30:00Z".to_string()),
            event_type: Some("https://api.calendly.com/event_types/ET1".to_string()),
            location: Some(ApiLocation {
                location_type: Some("google_conference".to_string()),
                location: None,
                join_url: Some("https://meet.google.com/abc".to_string()),
            }),
            created_at: Some("2025-01-01T00:00:00Z".to_string()),
            updated_at: Some("2025-01-02T00:00:00Z".to_string()),
        };

        let booking: Booking = api_event.into();

        assert_eq!(
            booking.uri,
            "https://api.calendly.com/scheduled_events/EVT1"
        );
        assert_eq!(booking.name, Some("Team Standup".to_string()));
        assert_eq!(booking.status, Some("active".to_string()));
        assert_eq!(
            booking.event_type_uri,
            Some("https://api.calendly.com/event_types/ET1".to_string())
        );
        assert!(booking.location.is_some());
        let location = booking.location.unwrap();
        assert_eq!(
            location.location_type,
            Some("google_conference".to_string())
        );
    }

    #[test]
    fn test_api_invitee_to_invitee_info_conversion() {
        let api_invitee = ApiInvitee {
            uri: "https://api.calendly.com/scheduled_events/EV1/invitees/INV1".to_string(),
            email: Some("alice@example.com".to_string()),
            name: Some("Alice Smith".to_string()),
            first_name: Some("Alice".to_string()),
            last_name: Some("Smith".to_string()),
            status: Some("active".to_string()),
            cancel_url: Some("https://calendly.com/cancel/xyz".to_string()),
            reschedule_url: Some("https://calendly.com/reschedule/xyz".to_string()),
            created_at: Some("2025-01-01T00:00:00Z".to_string()),
            updated_at: Some("2025-01-02T00:00:00Z".to_string()),
        };

        let invitee: InviteeInfo = api_invitee.into();

        assert_eq!(
            invitee.uri,
            "https://api.calendly.com/scheduled_events/EV1/invitees/INV1"
        );
        assert_eq!(invitee.email, Some("alice@example.com".to_string()));
        assert_eq!(invitee.name, Some("Alice Smith".to_string()));
        assert_eq!(invitee.first_name, Some("Alice".to_string()));
        assert_eq!(invitee.last_name, Some("Smith".to_string()));
    }

    // ========== URI Parsing Helper Tests ==========

    #[test]
    fn test_strip_query_and_fragment_removes_query() {
        let uri = "https://api.calendly.com/events?page=2&count=10";
        assert_eq!(
            strip_query_and_fragment(uri),
            "https://api.calendly.com/events"
        );
    }

    #[test]
    fn test_strip_query_and_fragment_removes_fragment() {
        let uri = "https://api.calendly.com/events#section";
        assert_eq!(
            strip_query_and_fragment(uri),
            "https://api.calendly.com/events"
        );
    }

    #[test]
    fn test_strip_query_and_fragment_removes_both() {
        let uri = "https://api.calendly.com/events?page=2#section";
        assert_eq!(
            strip_query_and_fragment(uri),
            "https://api.calendly.com/events"
        );
    }

    #[test]
    fn test_strip_query_and_fragment_preserves_clean_uri() {
        let uri = "https://api.calendly.com/events";
        assert_eq!(strip_query_and_fragment(uri), uri);
    }

    #[test]
    fn test_scheduled_event_uuid_from_uri_extracts_uuid() {
        let uri = "https://api.calendly.com/scheduled_events/ABC123";
        assert_eq!(
            scheduled_event_uuid_from_uri(uri),
            Some("ABC123".to_string())
        );
    }

    #[test]
    fn test_scheduled_event_uuid_from_uri_handles_trailing_slash() {
        let uri = "https://api.calendly.com/scheduled_events/ABC123/";
        assert_eq!(
            scheduled_event_uuid_from_uri(uri),
            Some("ABC123".to_string())
        );
    }

    #[test]
    fn test_scheduled_event_uuid_from_uri_handles_query_params() {
        let uri = "https://api.calendly.com/scheduled_events/ABC123?include=invitees";
        assert_eq!(
            scheduled_event_uuid_from_uri(uri),
            Some("ABC123".to_string())
        );
    }

    #[test]
    fn test_scheduled_event_uuid_from_uri_returns_none_for_invalid() {
        let uri = "https://api.calendly.com/users/USER123";
        assert_eq!(scheduled_event_uuid_from_uri(uri), None);
    }

    #[test]
    fn test_invitee_uuids_from_uri_extracts_both_uuids() {
        let uri = "https://api.calendly.com/scheduled_events/EVT123/invitees/INV456";
        let result = invitee_uuids_from_uri(uri);

        assert_eq!(result, Some(("EVT123".to_string(), "INV456".to_string())));
    }

    #[test]
    fn test_invitee_uuids_from_uri_handles_query_params() {
        let uri = "https://api.calendly.com/scheduled_events/EVT123/invitees/INV456?include=all";
        let result = invitee_uuids_from_uri(uri);

        assert_eq!(result, Some(("EVT123".to_string(), "INV456".to_string())));
    }

    #[test]
    fn test_invitee_uuids_from_uri_returns_none_for_missing_invitees() {
        let uri = "https://api.calendly.com/scheduled_events/EVT123";
        assert_eq!(invitee_uuids_from_uri(uri), None);
    }

    #[test]
    fn test_invitee_uuids_from_uri_returns_none_for_invalid_structure() {
        let uri = "https://api.calendly.com/users/USER123/events/EVT456";
        assert_eq!(invitee_uuids_from_uri(uri), None);
    }

    // ========== Path Validation Tests ==========

    #[test]
    fn test_validate_path_segment_accepts_valid_segment() {
        assert!(validate_path_segment("event_uuid", "ABC123").is_ok());
        assert!(validate_path_segment("event_uuid", "abc-def-123").is_ok());
    }

    #[test]
    fn test_validate_path_segment_rejects_empty() {
        let result = validate_path_segment("event_uuid", "");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("must not be empty")
        );
    }

    #[test]
    fn test_validate_path_segment_rejects_whitespace_only() {
        let result = validate_path_segment("event_uuid", "   ");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("must not be empty")
        );
    }

    #[test]
    fn test_validate_path_segment_rejects_slash() {
        let result = validate_path_segment("event_uuid", "abc/def");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("must not contain '/'")
        );
    }

    #[test]
    fn test_validate_path_segment_rejects_question_mark() {
        let result = validate_path_segment("event_uuid", "abc?def");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("must not contain '?'")
        );
    }

    #[test]
    fn test_validate_path_segment_rejects_hash() {
        let result = validate_path_segment("event_uuid", "abc#def");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("must not contain '#'")
        );
    }

    // ========== Input Validation Edge Cases ==========

    #[tokio::test]
    async fn test_list_bookings_rejects_whitespace_only_user_uri() {
        let result = list_bookings(
            Context::empty(),
            ListBookingsInput {
                user_uri: Some("   ".to_string()),
                organization_uri: None,
                min_start_time: None,
                max_start_time: None,
                count: None,
                page_token: None,
                status: None,
            },
        )
        .await;

        let err = result.expect_err("whitespace-only user_uri should be rejected");
        assert!(
            err.to_string()
                .contains("Either user_uri or organization_uri must be provided")
        );
    }

    #[tokio::test]
    async fn test_list_bookings_rejects_count_zero() {
        let result = list_bookings(
            Context::empty(),
            ListBookingsInput {
                user_uri: Some("https://api.calendly.com/users/USER123".to_string()),
                organization_uri: None,
                min_start_time: None,
                max_start_time: None,
                count: Some(0),
                page_token: None,
                status: None,
            },
        )
        .await;

        let err = result.expect_err("count=0 should be rejected");
        assert!(err.to_string().contains("count must be between 1 and 100"));
    }

    #[tokio::test]
    async fn test_list_bookings_rejects_count_over_100() {
        let result = list_bookings(
            Context::empty(),
            ListBookingsInput {
                user_uri: Some("https://api.calendly.com/users/USER123".to_string()),
                organization_uri: None,
                min_start_time: None,
                max_start_time: None,
                count: Some(101),
                page_token: None,
                status: None,
            },
        )
        .await;

        let err = result.expect_err("count=101 should be rejected");
        assert!(err.to_string().contains("count must be between 1 and 100"));
    }

    #[tokio::test]
    async fn test_list_bookings_rejects_empty_status_filter() {
        let result = list_bookings(
            Context::empty(),
            ListBookingsInput {
                user_uri: Some("https://api.calendly.com/users/USER123".to_string()),
                organization_uri: None,
                min_start_time: None,
                max_start_time: None,
                count: None,
                page_token: None,
                status: Some(String::new()),
            },
        )
        .await;

        let err = result.expect_err("empty status should be rejected");
        assert!(err.to_string().contains("status must not be empty"));
    }

    #[tokio::test]
    async fn test_create_scheduling_link_rejects_whitespace_only_event_type_uri() {
        let result = create_scheduling_link(
            Context::empty(),
            CreateSchedulingLinkInput {
                event_type_uri: "   ".to_string(),
                max_event_count: None,
            },
        )
        .await;

        let err = result.expect_err("whitespace-only event_type_uri should be rejected");
        assert!(err.to_string().contains("event_type_uri must not be empty"));
    }

    #[tokio::test]
    async fn test_create_scheduling_link_rejects_max_event_count_zero() {
        let result = create_scheduling_link(
            Context::empty(),
            CreateSchedulingLinkInput {
                event_type_uri: "https://api.calendly.com/event_types/ET1".to_string(),
                max_event_count: Some(0),
            },
        )
        .await;

        let err = result.expect_err("max_event_count=0 should be rejected");
        assert!(
            err.to_string()
                .contains("max_event_count must be between 1 and 100")
        );
    }

    #[tokio::test]
    async fn test_create_scheduling_link_rejects_max_event_count_over_100() {
        let result = create_scheduling_link(
            Context::empty(),
            CreateSchedulingLinkInput {
                event_type_uri: "https://api.calendly.com/event_types/ET1".to_string(),
                max_event_count: Some(101),
            },
        )
        .await;

        let err = result.expect_err("max_event_count=101 should be rejected");
        assert!(
            err.to_string()
                .contains("max_event_count must be between 1 and 100")
        );
    }

    #[tokio::test]
    async fn test_cancel_reschedule_rejects_empty_reason() {
        let result = cancel_reschedule(
            Context::empty(),
            CancelRescheduleInput {
                action: CancelRescheduleAction::Cancel,
                scheduled_event_uuid: Some("EV1".to_string()),
                scheduled_event_uri: None,
                invitee_uuid: None,
                invitee_uri: None,
                reason: Some(String::new()),
            },
        )
        .await;

        let err = result.expect_err("empty reason should be rejected");
        assert!(err.to_string().contains("reason must not be empty"));
    }

    // ========== Empty Collection Tests ==========

    #[tokio::test]
    async fn test_list_bookings_handles_empty_collection() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/scheduled_events"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "collection": [],
                "pagination": {
                    "count": 0,
                    "next_page": null,
                    "previous_page": null
                }
            })))
            .mount(&server)
            .await;

        let ctx = test_context(&server);
        let output = list_bookings(
            ctx,
            ListBookingsInput {
                user_uri: Some("https://api.calendly.com/users/USER123".to_string()),
                organization_uri: None,
                min_start_time: None,
                max_start_time: None,
                count: None,
                page_token: None,
                status: None,
            },
        )
        .await
        .expect("list_bookings should succeed with empty collection");

        assert!(output.bookings.is_empty());
        assert_eq!(output.pagination.as_ref().and_then(|p| p.count), Some(0));
    }

    #[tokio::test]
    async fn test_list_bookings_returns_collection() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/scheduled_events"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param(
                "user",
                "https://api.calendly.com/users/USER123",
            ))
            .and(query_param("count", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "collection": [
                    {
                        "uri": "https://api.calendly.com/scheduled_events/EVT1",
                        "name": "Intro Call",
                        "status": "active",
                        "start_time": "2025-01-01T00:00:00Z",
                        "end_time": "2025-01-01T00:30:00Z",
                        "event_type": "https://api.calendly.com/event_types/ET1",
                        "location": {
                            "type": "zoom",
                            "location": "Zoom",
                            "join_url": "https://zoom.example.com/join/abc"
                        },
                        "created_at": "2024-12-01T00:00:00Z",
                        "updated_at": "2024-12-02T00:00:00Z"
                    }
                ],
                "pagination": {
                    "count": 1,
                    "next_page": null,
                    "previous_page": null
                }
            })))
            .mount(&server)
            .await;

        let ctx = test_context(&server);
        let output = list_bookings(
            ctx,
            ListBookingsInput {
                user_uri: Some("https://api.calendly.com/users/USER123".to_string()),
                organization_uri: None,
                min_start_time: None,
                max_start_time: None,
                count: Some(2),
                page_token: None,
                status: None,
            },
        )
        .await
        .expect("list_bookings should succeed");

        assert_eq!(output.bookings.len(), 1);
        assert_eq!(
            output.bookings[0],
            Booking {
                uri: "https://api.calendly.com/scheduled_events/EVT1".to_string(),
                name: Some("Intro Call".to_string()),
                status: Some("active".to_string()),
                start_time: Some("2025-01-01T00:00:00Z".to_string()),
                end_time: Some("2025-01-01T00:30:00Z".to_string()),
                event_type_uri: Some("https://api.calendly.com/event_types/ET1".to_string()),
                location: Some(BookingLocation {
                    location_type: Some("zoom".to_string()),
                    location: Some("Zoom".to_string()),
                    join_url: Some("https://zoom.example.com/join/abc".to_string()),
                }),
                created_at: Some("2024-12-01T00:00:00Z".to_string()),
                updated_at: Some("2024-12-02T00:00:00Z".to_string()),
            }
        );
        assert_eq!(output.request_id, "req-123");
    }

    #[tokio::test]
    async fn test_list_bookings_requires_user_or_org() {
        let output = list_bookings(
            Context::empty(),
            ListBookingsInput {
                user_uri: None,
                organization_uri: None,
                min_start_time: None,
                max_start_time: None,
                count: None,
                page_token: None,
                status: None,
            },
        )
        .await;

        let err = output.expect_err("list_bookings should reject missing filters");
        assert!(
            err.to_string()
                .contains("Either user_uri or organization_uri must be provided")
        );
    }

    #[tokio::test]
    async fn test_create_scheduling_link_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/scheduling_links"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_json(json!({
                "owner": "https://api.calendly.com/event_types/ET1",
                "owner_type": "EventType",
                "max_event_count": 1
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "resource": {
                    "uri": "https://api.calendly.com/scheduling_links/SL1",
                    "booking_url": "https://calendly.com/example/intro",
                    "owner": "https://api.calendly.com/event_types/ET1",
                    "owner_type": "EventType",
                    "max_event_count": 1
                }
            })))
            .mount(&server)
            .await;

        let ctx = test_context(&server);
        let output = create_scheduling_link(
            ctx,
            CreateSchedulingLinkInput {
                event_type_uri: "https://api.calendly.com/event_types/ET1".to_string(),
                max_event_count: None,
            },
        )
        .await
        .expect("create_scheduling_link should succeed");

        assert_eq!(
            output.scheduling_link_uri,
            "https://api.calendly.com/scheduling_links/SL1"
        );
        assert_eq!(output.booking_url, "https://calendly.com/example/intro");
        assert_eq!(
            output.owner,
            Some("https://api.calendly.com/event_types/ET1".to_string())
        );
        assert_eq!(output.owner_type, Some("EventType".to_string()));
        assert_eq!(output.max_event_count, Some(1));
        assert_eq!(output.request_id, "req-123");
    }

    #[tokio::test]
    async fn test_create_scheduling_link_rejects_empty_event_type_uri() {
        let output = create_scheduling_link(
            Context::empty(),
            CreateSchedulingLinkInput {
                event_type_uri: String::new(),
                max_event_count: None,
            },
        )
        .await;

        let err = output.expect_err("create_scheduling_link should reject empty event_type_uri");
        assert!(err.to_string().contains("event_type_uri must not be empty"));
    }

    #[tokio::test]
    async fn test_fetch_invitee_info_success() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/scheduled_events/EV1/invitees/INV1"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "resource": {
                    "uri": "https://api.calendly.com/scheduled_events/EV1/invitees/INV1",
                    "email": "alice@example.com",
                    "name": "Alice Example",
                    "first_name": "Alice",
                    "last_name": "Example",
                    "status": "active",
                    "cancel_url": "https://calendly.com/cancel/abc",
                    "reschedule_url": "https://calendly.com/reschedule/abc",
                    "created_at": "2024-12-01T00:00:00Z",
                    "updated_at": "2024-12-02T00:00:00Z"
                }
            })))
            .mount(&server)
            .await;

        let ctx = test_context(&server);
        let output = fetch_invitee_info(
            ctx,
            FetchInviteeInfoInput {
                scheduled_event_uuid: Some("EV1".to_string()),
                scheduled_event_uri: None,
                invitee_uuid: Some("INV1".to_string()),
                invitee_uri: None,
            },
        )
        .await
        .expect("fetch_invitee_info should succeed");

        assert_eq!(
            output.invitee,
            InviteeInfo {
                uri: "https://api.calendly.com/scheduled_events/EV1/invitees/INV1".to_string(),
                email: Some("alice@example.com".to_string()),
                name: Some("Alice Example".to_string()),
                first_name: Some("Alice".to_string()),
                last_name: Some("Example".to_string()),
                status: Some("active".to_string()),
                cancel_url: Some("https://calendly.com/cancel/abc".to_string()),
                reschedule_url: Some("https://calendly.com/reschedule/abc".to_string()),
                created_at: Some("2024-12-01T00:00:00Z".to_string()),
                updated_at: Some("2024-12-02T00:00:00Z".to_string()),
            }
        );
        assert_eq!(output.request_id, "req-123");
    }

    #[tokio::test]
    async fn test_fetch_invitee_info_requires_invitee_identifier() {
        let output = fetch_invitee_info(
            Context::empty(),
            FetchInviteeInfoInput {
                scheduled_event_uuid: Some("EV1".to_string()),
                scheduled_event_uri: None,
                invitee_uuid: None,
                invitee_uri: None,
            },
        )
        .await;

        let err = output.expect_err("fetch_invitee_info should reject missing invitee identifiers");
        assert!(
            err.to_string()
                .contains("invitee_uuid or invitee_uri is required")
        );
    }

    #[tokio::test]
    async fn test_cancel_reschedule_cancel_calls_cancellation_endpoint() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/scheduled_events/EV1/cancellation"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_json(json!({
                "reason": "No longer needed"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "resource": {
                    "uri": "https://api.calendly.com/scheduled_events/EV1/cancellation",
                    "canceled_at": "2025-01-01T00:00:00Z",
                    "reason": "No longer needed",
                    "scheduled_event": "https://api.calendly.com/scheduled_events/EV1"
                }
            })))
            .mount(&server)
            .await;

        let ctx = test_context(&server);
        let output = cancel_reschedule(
            ctx,
            CancelRescheduleInput {
                action: CancelRescheduleAction::Cancel,
                scheduled_event_uuid: Some("EV1".to_string()),
                scheduled_event_uri: None,
                invitee_uuid: None,
                invitee_uri: None,
                reason: Some("No longer needed".to_string()),
            },
        )
        .await
        .expect("cancel_reschedule cancel should succeed");

        assert_eq!(output.action, CancelRescheduleAction::Cancel);
        let cancellation = output
            .cancellation
            .expect("cancellation details should be returned");
        assert_eq!(
            cancellation.uri,
            Some("https://api.calendly.com/scheduled_events/EV1/cancellation".to_string())
        );
        assert_eq!(cancellation.reason, Some("No longer needed".to_string()));
        assert_eq!(
            cancellation.scheduled_event_uri,
            Some("https://api.calendly.com/scheduled_events/EV1".to_string())
        );
        assert_eq!(output.request_id, "req-123");
    }

    #[tokio::test]
    async fn test_cancel_reschedule_reschedule_returns_reschedule_url() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/scheduled_events/EV1/invitees/INV1"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "resource": {
                    "uri": "https://api.calendly.com/scheduled_events/EV1/invitees/INV1",
                    "reschedule_url": "https://calendly.com/reschedule/abc"
                }
            })))
            .mount(&server)
            .await;

        let ctx = test_context(&server);
        let output = cancel_reschedule(
            ctx,
            CancelRescheduleInput {
                action: CancelRescheduleAction::Reschedule,
                scheduled_event_uuid: Some("EV1".to_string()),
                scheduled_event_uri: None,
                invitee_uuid: Some("INV1".to_string()),
                invitee_uri: None,
                reason: None,
            },
        )
        .await
        .expect("cancel_reschedule reschedule should succeed");

        assert_eq!(output.action, CancelRescheduleAction::Reschedule);
        assert_eq!(
            output.reschedule_url,
            Some("https://calendly.com/reschedule/abc".to_string())
        );
        assert_eq!(output.request_id, "req-123");
    }
}
