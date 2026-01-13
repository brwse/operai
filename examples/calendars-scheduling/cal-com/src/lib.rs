//! calendars-scheduling/cal-com integration for Operai Toolbox.

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};

mod types;
use types::{
    ApiListResponse, ApiResponse, BookingStatus, BookingSummary, EventType, Pagination, Schedule,
};

define_user_credential! {
    CalComCredential("cal_com") {
        api_key: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_API_ENDPOINT: &str = "https://api.cal.com/v2";

#[init]
async fn setup() -> Result<()> {
    info!("Cal.com integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Cal.com integration shutting down");
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListBookingsInput {
    /// Filter by booking status
    #[serde(default)]
    pub status: Option<BookingStatus>,
    /// Limit number of results (1-100). Defaults to 20.
    #[serde(default)]
    pub limit: Option<u32>,
    /// Page number for pagination. Defaults to 1.
    #[serde(default)]
    pub page: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListBookingsOutput {
    pub bookings: Vec<BookingSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<Pagination>,
}

/// # List Cal.com Bookings
///
/// Retrieves bookings from the Cal.com scheduling API with support for
/// filtering by status and pagination.
///
/// Use this tool when a user wants to:
/// - View all upcoming or past bookings
/// - Check booking statuses (accepted, pending, cancelled, rejected)
/// - Retrieve a specific page of booking results
/// - Get booking summaries with event type details
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - calendar
/// - scheduling
/// - cal.com
///
/// # Input Details
///
/// - `status`: Optional filter to retrieve only bookings with a specific status
///   (e.g., "accepted", "pending", "cancelled", "rejected")
/// - `limit`: Number of results per page (1-100, defaults to 20)
/// - `page`: Page number for pagination (defaults to 1, must be at least 1)
///
/// # Output
///
/// Returns a list of booking summaries including:
/// - Booking ID, UID, start/end times, and duration
/// - Booking status
/// - Associated event type information
/// - Pagination metadata (total items, current page, total pages, has next
///   page)
///
/// # Errors
///
/// Returns an error if:
/// - The limit is not between 1 and 100
/// - The page is less than 1
/// - Cal.com credentials are missing or invalid
/// - The Cal.com API request fails
/// - The response cannot be parsed
#[tool]
pub async fn list_bookings(ctx: Context, input: ListBookingsInput) -> Result<ListBookingsOutput> {
    let limit = input.limit.unwrap_or(20);
    ensure!(
        (1..=100).contains(&limit),
        "limit must be between 1 and 100"
    );

    let page = input.page.unwrap_or(1);
    ensure!(page >= 1, "page must be at least 1");

    let client = CalComClient::from_ctx(&ctx)?;

    let mut query = vec![("limit", limit.to_string()), ("page", page.to_string())];

    if let Some(status) = input.status {
        let status_str = match status {
            BookingStatus::Accepted => "accepted",
            BookingStatus::Pending => "pending",
            BookingStatus::Cancelled => "cancelled",
            BookingStatus::Rejected => "rejected",
        };
        query.push(("status", status_str.to_string()));
    }

    let response: ApiListResponse<BookingSummary> = client
        .get_json(client.url_with_path("/bookings")?, &query)
        .await?;

    Ok(ListBookingsOutput {
        bookings: response.data,
        pagination: response.pagination,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CancelBookingInput {
    /// Booking UID to cancel
    pub booking_uid: String,
    /// Cancellation reason
    pub cancellation_reason: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CancelBookingOutput {
    pub cancelled: bool,
    pub booking_uid: String,
}

/// # Cancel Cal.com Booking
///
/// Cancels an existing booking in Cal.com by providing the booking UID and a
/// cancellation reason.
///
/// Use this tool when a user wants to:
/// - Cancel a scheduled meeting or appointment
/// - Remove an upcoming booking from their calendar
/// - Notify that a previously scheduled event will no longer take place
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - calendar
/// - scheduling
/// - cal.com
///
/// # Input Details
///
/// - `booking_uid`: The unique identifier of the booking to cancel (must not be
///   empty or whitespace only)
/// - `cancellation_reason`: A text explanation for why the booking is being
///   cancelled (must not be empty or whitespace only)
///
/// # Output
///
/// Returns confirmation including:
/// - `cancelled`: Boolean indicating successful cancellation
/// - `booking_uid`: The UID of the cancelled booking
///
/// # Errors
///
/// Returns an error if:
/// - The booking UID is empty or contains only whitespace
/// - The cancellation reason is empty or contains only whitespace
/// - Cal.com credentials are missing or invalid
/// - The Cal.com API request fails
/// - The booking does not exist or has already been cancelled
#[tool]
pub async fn cancel_booking(
    ctx: Context,
    input: CancelBookingInput,
) -> Result<CancelBookingOutput> {
    ensure!(
        !input.booking_uid.trim().is_empty(),
        "booking_uid must not be empty"
    );
    ensure!(
        !input.cancellation_reason.trim().is_empty(),
        "cancellation_reason must not be empty"
    );

    let client = CalComClient::from_ctx(&ctx)?;

    let request = CalComCancelRequest {
        cancellation_reason: input.cancellation_reason,
    };

    client
        .post_empty(
            client.url_with_path(&format!("/bookings/{}/cancel", input.booking_uid))?,
            &request,
        )
        .await?;

    Ok(CancelBookingOutput {
        cancelled: true,
        booking_uid: input.booking_uid,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RescheduleBookingInput {
    /// Booking UID to reschedule
    pub booking_uid: String,
    /// New start time (ISO 8601 format)
    pub start: String,
    /// Reschedule reason
    #[serde(default)]
    pub reschedule_reason: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RescheduleBookingOutput {
    pub rescheduled: bool,
    pub booking_uid: String,
    pub new_start: String,
}

/// # Reschedule Cal.com Booking
///
/// Reschedules an existing Cal.com booking to a new start time, optionally
/// providing a reason for the change.
///
/// Use this tool when a user wants to:
/// - Move a booking to a different date or time
/// - Change the scheduled time of a meeting or appointment
/// - Accommodate scheduling conflicts or requested time changes
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - calendar
/// - scheduling
/// - cal.com
///
/// # Input Details
///
/// - `booking_uid`: The unique identifier of the booking to reschedule (must
///   not be empty or whitespace only)
/// - `start`: The new start time in ISO 8601 format (e.g.,
///   "2024-01-20T14:00:00Z", must not be empty or whitespace only)
/// - `reschedule_reason`: Optional text explaining why the booking is being
///   rescheduled
///
/// # Output
///
/// Returns confirmation including:
/// - `rescheduled`: Boolean indicating successful rescheduling
/// - `booking_uid`: The UID of the rescheduled booking
/// - `new_start`: The new start time for the booking
///
/// # Errors
///
/// Returns an error if:
/// - The booking UID is empty or contains only whitespace
/// - The start time is empty or contains only whitespace
/// - Cal.com credentials are missing or invalid
/// - The Cal.com API request fails
/// - The new time slot is not available
/// - The booking does not exist or cannot be rescheduled
#[tool]
pub async fn reschedule_booking(
    ctx: Context,
    input: RescheduleBookingInput,
) -> Result<RescheduleBookingOutput> {
    ensure!(
        !input.booking_uid.trim().is_empty(),
        "booking_uid must not be empty"
    );
    ensure!(
        !input.start.trim().is_empty(),
        "start time must not be empty"
    );

    let client = CalComClient::from_ctx(&ctx)?;

    let request = CalComRescheduleRequest {
        start: input.start.clone(),
        rescheduling_reason: input.reschedule_reason,
    };

    client
        .post_empty(
            client.url_with_path(&format!("/bookings/{}/reschedule", input.booking_uid))?,
            &request,
        )
        .await?;

    Ok(RescheduleBookingOutput {
        rescheduled: true,
        booking_uid: input.booking_uid,
        new_start: input.start,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetAvailabilityInput {
    /// Schedule name
    pub name: String,
    /// Time zone (e.g., "`America/New_York`")
    pub time_zone: String,
    /// Availability schedule entries
    pub schedule: Vec<AvailabilitySchedule>,
    /// Set as default schedule
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AvailabilitySchedule {
    /// Days of the week (e.g., `Monday`, `Tuesday`)
    pub days: Vec<String>,
    /// Start time (HH:MM format, e.g., "09:00")
    pub start_time: String,
    /// End time (HH:MM format, e.g., "17:00")
    pub end_time: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SetAvailabilityOutput {
    pub schedule_id: i64,
    pub name: String,
}

/// # Set Cal.com Availability
///
/// Creates or updates an availability schedule in Cal.com, defining when the
/// user is available for bookings.
///
/// Use this tool when a user wants to:
/// - Configure their available hours for booking meetings
/// - Set up recurring availability patterns (e.g., Monday-Friday 9am-5pm)
/// - Create custom schedules for different time zones
/// - Establish default availability for their account
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - calendar
/// - scheduling
/// - cal.com
/// - availability
///
/// # Input Details
///
/// - `name`: Display name for the schedule (must not be empty or whitespace
///   only)
/// - `time_zone`: IANA time zone identifier (e.g., "`America/New_York`",
///   "Europe/London", must not be empty or whitespace only)
/// - `schedule`: Array of availability entries, where each entry specifies:
///   - `days`: Array of day names (e.g., `["Monday", "Tuesday", "Friday"]`)
///   - `start_time`: Start time in HH:MM format (e.g., "09:00")
///   - `end_time`: End time in HH:MM format (e.g., "17:00")
/// - `is_default`: Whether to set this as the default schedule for the account
///   (defaults to false)
///
/// # Output
///
/// Returns confirmation including:
/// - `schedule_id`: The ID of the created/updated schedule
/// - `name`: The name of the schedule
///
/// # Errors
///
/// Returns an error if:
/// - The schedule name is empty or contains only whitespace
/// - The time zone is empty or contains only whitespace
/// - The schedule contains no entries (at least one availability entry is
///   required)
/// - Any schedule entry has empty days array, empty start time, or empty end
///   time
/// - Cal.com credentials are missing or invalid
/// - The Cal.com API request fails
/// - The response cannot be parsed
/// - The time zone is not a valid IANA identifier
#[tool]
pub async fn set_availability(
    ctx: Context,
    input: SetAvailabilityInput,
) -> Result<SetAvailabilityOutput> {
    ensure!(
        !input.name.trim().is_empty(),
        "schedule name must not be empty"
    );
    ensure!(
        !input.time_zone.trim().is_empty(),
        "time_zone must not be empty"
    );
    ensure!(
        !input.schedule.is_empty(),
        "schedule must contain at least one entry"
    );

    for entry in &input.schedule {
        ensure!(
            !entry.days.is_empty(),
            "each schedule entry must have at least one day"
        );
        ensure!(
            !entry.start_time.trim().is_empty(),
            "start_time must not be empty"
        );
        ensure!(
            !entry.end_time.trim().is_empty(),
            "end_time must not be empty"
        );
    }

    let client = CalComClient::from_ctx(&ctx)?;

    let request = CalComScheduleRequest {
        name: input.name.clone(),
        time_zone: input.time_zone,
        is_default: input.is_default,
        availability: input.schedule,
    };

    let response: ApiResponse<Schedule> = client
        .post_json(client.url_with_path("/schedules")?, &request)
        .await?;

    Ok(SetAvailabilityOutput {
        schedule_id: response.data.id,
        name: input.name,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateBookingLinkInput {
    /// Event type ID to create link for
    pub event_type_id: i64,
    /// Optional custom link identifier
    #[serde(default)]
    pub link: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateBookingLinkOutput {
    pub booking_url: String,
    pub event_type_id: i64,
}

/// # Create Cal.com Booking Link
///
/// Generates a shareable booking URL for a specific Cal.com event type,
/// allowing others to schedule time using that event.
///
/// Use this tool when a user wants to:
/// - Share a booking link for a specific meeting type (e.g., "30-minute
///   meeting")
/// - Create a URL that others can use to book time with them
/// - Generate custom booking links with specific identifiers
/// - Retrieve the public URL for an existing event type
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - calendar
/// - scheduling
/// - cal.com
///
/// # Input Details
///
/// - `event_type_id`: The numeric ID of the event type to generate a link for
///   (must be a positive integer)
/// - `link`: Optional custom link identifier. If provided, creates a custom URL
///   path (e.g., "my-custom-link" produces `<https://cal.com/my-custom-link>`).
///   If omitted, uses the event type's default slug.
///
/// # Output
///
/// Returns the booking link information:
/// - `booking_url`: The complete URL that can be shared for bookings (e.g., `<https://cal.com/30min>`
///   or `<https://cal.com/my-custom-link>`)
/// - `event_type_id`: The ID of the event type the link was generated for
///
/// # Errors
///
/// Returns an error if:
/// - The event type ID is not positive (zero or negative)
/// - Cal.com credentials are missing or invalid
/// - The Cal.com API request fails
/// - The event type cannot be found (invalid event type ID)
/// - The response cannot be parsed
#[tool]
pub async fn create_booking_link(
    ctx: Context,
    input: CreateBookingLinkInput,
) -> Result<CreateBookingLinkOutput> {
    ensure!(input.event_type_id > 0, "event_type_id must be positive");

    let client = CalComClient::from_ctx(&ctx)?;

    let response: ApiResponse<EventType> = client
        .get_json(
            client.url_with_path(&format!("/event-types/{}", input.event_type_id))?,
            &[],
        )
        .await?;

    let base_url = client
        .base_url
        .trim_end_matches("/v2")
        .trim_end_matches("/api");

    let booking_url = if let Some(custom_link) = input.link {
        format!("{base_url}/{custom_link}")
    } else {
        format!("{base_url}/{}", response.data.slug)
    };

    Ok(CreateBookingLinkOutput {
        booking_url,
        event_type_id: input.event_type_id,
    })
}

// Internal request types
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CalComCancelRequest {
    cancellation_reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CalComRescheduleRequest {
    start: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    rescheduling_reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CalComScheduleRequest {
    name: String,
    time_zone: String,
    is_default: bool,
    availability: Vec<AvailabilitySchedule>,
}

#[derive(Debug, Clone)]
struct CalComClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl CalComClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = CalComCredential::get(ctx)?;
        ensure!(!cred.api_key.trim().is_empty(), "api_key must not be empty");

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_API_ENDPOINT))?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            api_key: cred.api_key,
        })
    }

    fn url_with_path(&self, path: &str) -> Result<reqwest::Url> {
        let url_string = format!("{}{}", self.base_url, path);
        Ok(reqwest::Url::parse(&url_string)?)
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        query: &[(&str, String)],
    ) -> Result<T> {
        let request = self.http.get(url).query(query);
        let response = self.send_request(request).await?;
        Ok(response.json::<T>().await?)
    }

    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
    ) -> Result<TRes> {
        let request = self.http.post(url).json(body);
        let response = self.send_request(request).await?;
        Ok(response.json::<TRes>().await?)
    }

    async fn post_empty<TReq: Serialize>(&self, url: reqwest::Url, body: &TReq) -> Result<()> {
        let request = self.http.post(url).json(body);
        self.send_request(request).await?;
        Ok(())
    }

    async fn send_request(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let response = request
            .bearer_auth(&self.api_key)
            .header(reqwest::header::ACCEPT, "application/json")
            .header("cal-api-version", "2024-08-13")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "Cal.com API request failed ({status}): {body}"
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
        let mut cal_com_values = HashMap::new();
        cal_com_values.insert("api_key".to_string(), "test-api-key".to_string());
        cal_com_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("cal_com", cal_com_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        format!("{}/v2", server.uri())
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_booking_status_serialization_roundtrip() {
        for variant in [
            BookingStatus::Accepted,
            BookingStatus::Pending,
            BookingStatus::Cancelled,
            BookingStatus::Rejected,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: BookingStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_availability_schedule_serialization_roundtrip() {
        let schedule = AvailabilitySchedule {
            days: vec!["Monday".to_string(), "Tuesday".to_string()],
            start_time: "09:00".to_string(),
            end_time: "17:00".to_string(),
        };
        let json = serde_json::to_string(&schedule).unwrap();
        let parsed: AvailabilitySchedule = serde_json::from_str(&json).unwrap();
        assert_eq!(schedule.days, parsed.days);
        assert_eq!(schedule.start_time, parsed.start_time);
        assert_eq!(schedule.end_time, parsed.end_time);
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://api.cal.com/v2/").unwrap();
        assert_eq!(result, "https://api.cal.com/v2");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  https://api.cal.com/v2  ").unwrap();
        assert_eq!(result, "https://api.cal.com/v2");
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
    async fn test_list_bookings_limit_zero_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_bookings(
            ctx,
            ListBookingsInput {
                status: None,
                limit: Some(0),
                page: None,
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
    async fn test_list_bookings_limit_exceeds_max_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_bookings(
            ctx,
            ListBookingsInput {
                status: None,
                limit: Some(101),
                page: None,
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
    async fn test_list_bookings_page_zero_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_bookings(
            ctx,
            ListBookingsInput {
                status: None,
                limit: None,
                page: Some(0),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("page must be at least 1")
        );
    }

    #[tokio::test]
    async fn test_cancel_booking_empty_uid_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = cancel_booking(
            ctx,
            CancelBookingInput {
                booking_uid: "  ".to_string(),
                cancellation_reason: "Test".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("booking_uid must not be empty")
        );
    }

    #[tokio::test]
    async fn test_cancel_booking_empty_reason_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = cancel_booking(
            ctx,
            CancelBookingInput {
                booking_uid: "booking-uid-123".to_string(),
                cancellation_reason: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("cancellation_reason must not be empty")
        );
    }

    #[tokio::test]
    async fn test_reschedule_booking_empty_start_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = reschedule_booking(
            ctx,
            RescheduleBookingInput {
                booking_uid: "booking-uid-123".to_string(),
                start: "  ".to_string(),
                reschedule_reason: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("start time must not be empty")
        );
    }

    #[tokio::test]
    async fn test_reschedule_booking_empty_uid_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = reschedule_booking(
            ctx,
            RescheduleBookingInput {
                booking_uid: "  ".to_string(),
                start: "2024-01-20T14:00:00Z".to_string(),
                reschedule_reason: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("booking_uid must not be empty")
        );
    }

    #[tokio::test]
    async fn test_set_availability_empty_name_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = set_availability(
            ctx,
            SetAvailabilityInput {
                name: "  ".to_string(),
                time_zone: "America/New_York".to_string(),
                schedule: vec![],
                is_default: false,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("schedule name must not be empty")
        );
    }

    #[tokio::test]
    async fn test_set_availability_empty_schedule_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = set_availability(
            ctx,
            SetAvailabilityInput {
                name: "Work Hours".to_string(),
                time_zone: "America/New_York".to_string(),
                schedule: vec![],
                is_default: false,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("schedule must contain at least one entry")
        );
    }

    #[tokio::test]
    async fn test_create_booking_link_negative_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = create_booking_link(
            ctx,
            CreateBookingLinkInput {
                event_type_id: -1,
                link: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("event_type_id must be positive")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_list_bookings_success_returns_bookings() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "status": "success",
          "data": [
            {
              "id": 1,
              "uid": "booking-uid-1",
              "start": "2024-01-15T10:00:00Z",
              "end": "2024-01-15T11:00:00Z",
              "duration": 60,
              "status": "accepted",
              "eventType": {
                "id": 100,
                "slug": "30min"
              }
            }
          ],
          "pagination": {
            "totalItems": 1,
            "currentPage": 1,
            "totalPages": 1,
            "hasNextPage": false
          }
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v2/bookings"))
            .and(header("authorization", "Bearer test-api-key"))
            .and(query_param("limit", "20"))
            .and(query_param("page", "1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_bookings(
            ctx,
            ListBookingsInput {
                status: None,
                limit: None,
                page: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.bookings.len(), 1);
        assert_eq!(output.bookings[0].id, 1);
        assert_eq!(output.bookings[0].uid, "booking-uid-1");
        assert_eq!(output.bookings[0].status, BookingStatus::Accepted);
        assert!(output.pagination.is_some());
    }

    #[tokio::test]
    async fn test_cancel_booking_success_returns_cancelled() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("POST"))
            .and(path("/v2/bookings/booking-uid-123/cancel"))
            .and(header("authorization", "Bearer test-api-key"))
            .and(body_string_contains(
                "\"cancellationReason\":\"No longer needed\"",
            ))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = cancel_booking(
            ctx,
            CancelBookingInput {
                booking_uid: "booking-uid-123".to_string(),
                cancellation_reason: "No longer needed".to_string(),
            },
        )
        .await
        .unwrap();

        assert!(output.cancelled);
        assert_eq!(output.booking_uid, "booking-uid-123");
    }

    #[tokio::test]
    async fn test_reschedule_booking_success_returns_rescheduled() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("POST"))
            .and(path("/v2/bookings/booking-uid-123/reschedule"))
            .and(body_string_contains("\"start\":\"2024-01-20T14:00:00Z\""))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = reschedule_booking(
            ctx,
            RescheduleBookingInput {
                booking_uid: "booking-uid-123".to_string(),
                start: "2024-01-20T14:00:00Z".to_string(),
                reschedule_reason: Some("Conflict resolved".to_string()),
            },
        )
        .await
        .unwrap();

        assert!(output.rescheduled);
        assert_eq!(output.booking_uid, "booking-uid-123");
        assert_eq!(output.new_start, "2024-01-20T14:00:00Z");
    }

    #[tokio::test]
    async fn test_set_availability_success_returns_schedule_id() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "status": "success",
          "data": {
            "id": 456,
            "name": "Work Hours",
            "timeZone": "America/New_York",
            "isDefault": true,
            "availability": []
          }
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/v2/schedules"))
            .and(body_string_contains("\"name\":\"Work Hours\""))
            .and(body_string_contains("\"timeZone\":\"America/New_York\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = set_availability(
            ctx,
            SetAvailabilityInput {
                name: "Work Hours".to_string(),
                time_zone: "America/New_York".to_string(),
                schedule: vec![AvailabilitySchedule {
                    days: vec!["Monday".to_string(), "Friday".to_string()],
                    start_time: "09:00".to_string(),
                    end_time: "17:00".to_string(),
                }],
                is_default: true,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.schedule_id, 456);
        assert_eq!(output.name, "Work Hours");
    }

    #[tokio::test]
    async fn test_create_booking_link_success_returns_url() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "status": "success",
          "data": {
            "id": 100,
            "title": "30 Minute Meeting",
            "slug": "30min",
            "lengthInMinutes": 30,
            "locations": []
          }
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v2/event-types/100"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = create_booking_link(
            ctx,
            CreateBookingLinkInput {
                event_type_id: 100,
                link: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.event_type_id, 100);
        assert!(output.booking_url.contains("30min"));
    }

    #[tokio::test]
    async fn test_list_bookings_api_error_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path("/v2/bookings"))
            .respond_with(ResponseTemplate::new(401).set_body_raw(
                r#"{ "status": "error", "error": { "code": "UNAUTHORIZED", "message": "Invalid API key" } }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = list_bookings(
            ctx,
            ListBookingsInput {
                status: None,
                limit: None,
                page: None,
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("401"));
    }
}
