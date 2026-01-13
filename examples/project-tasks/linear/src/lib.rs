//! project-tasks/linear integration for Operai Toolbox.

use std::collections::HashMap;

use gql_client::Client as GqlClient;
use operai::{
    Context, JsonSchema, Result, anyhow::anyhow, define_user_credential, ensure, info, init,
    schemars, shutdown, tool,
};
use serde::{Deserialize, Serialize};

mod types;
use types::{
    CreateCommentData, CreateIssueData, GraphQLComment, GraphQLCycle, GraphQLIssue,
    GraphQLIssueState, GraphQLLabel, GraphQLTeam, GraphQLUser, ListCyclesData, SearchIssuesData,
    UpdateIssueData,
};

define_user_credential! {
    LinearCredential("linear") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_GRAPHQL_ENDPOINT: &str = "https://api.linear.app/graphql";

#[init]
async fn setup() -> Result<()> {
    info!("Linear integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Linear integration shutting down");
}

// Public types

#[derive(Debug, Serialize, JsonSchema)]
pub struct Issue {
    pub id: String,
    pub identifier: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub state: IssueState,
    pub priority: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<User>,
    pub team: Team,
    pub labels: Vec<Label>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct IssueState {
    pub id: String,
    pub name: String,
    pub state_type: String,
    pub color: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Team {
    pub id: String,
    pub name: String,
    pub key: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Label {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Comment {
    pub id: String,
    pub body: String,
    pub user: User,
    pub created_at: String,
    pub updated_at: String,
    pub resolves_parent: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Cycle {
    pub id: String,
    pub number: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub starts_at: String,
    pub ends_at: String,
    pub issue_count: u32,
    pub completed_issue_count: u32,
    pub scope: f32,
    pub completed_scope: f32,
    pub progress: f32,
}

// Search Issues

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchIssuesInput {
    pub query: String,
    #[serde(default)]
    pub team_id: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub assignee_id: Option<String>,
    #[serde(default)]
    pub priority: Option<u8>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchIssuesOutput {
    pub issues: Vec<Issue>,
    pub total_count: u32,
    pub has_more: bool,
}

/// # Search Linear Issues
///
/// Searches for issues in Linear using flexible filtering options. Use this
/// tool when you need to find existing issues based on various criteria such as
/// title keywords, team, state, assignee, or priority level.
///
/// This tool performs a text search across issue titles and supports optional
/// filters for:
/// - **Team**: Filter by team ID to scope results to a specific team
/// - **State**: Filter by state name (e.g., "Backlog", "In Progress", "Done")
/// - **Assignee**: Filter by assignee ID to find issues assigned to a specific
///   user
/// - **Priority**: Filter by priority level (0-4, where 0 is no priority and 4
///   is urgent)
///
/// The query parameter searches issue titles using a case-insensitive contains
/// match. Results are paginated with a default limit of 50 issues (maximum
/// 100).
///
/// **When to use this tool:**
/// - User asks to find, search, or lookup issues in Linear
/// - User wants to see issues assigned to them or others
/// - User needs to filter issues by status, team, or priority
/// - User is looking for specific issues by title or keyword
///
/// **Output:** Returns a list of matching issues with full details including
/// identifier, title, description, state, assignee, team, labels, and
/// timestamps.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - project-management
/// - linear
/// - issues
///
/// # Errors
///
/// Returns an error if:
/// - The provided query is empty or contains only whitespace
/// - No Linear credentials are configured in the context
/// - The configured access token is empty
/// - The GraphQL endpoint is unreachable or returns a non-success status
/// - The GraphQL query fails validation or execution (returned via GraphQL
///   errors)
/// - The response data is missing or malformed
/// - The number of returned issues cannot be converted to u32
#[tool]
pub async fn search_issues(ctx: Context, input: SearchIssuesInput) -> Result<SearchIssuesOutput> {
    ensure!(!input.query.trim().is_empty(), "query must not be empty");
    let limit = input.limit.unwrap_or(50).min(100);

    let client = LinearClient::from_ctx(&ctx)?;

    let mut filter_parts = vec![format!(
        "title: {{ contains: \"{}\" }}",
        escape_graphql_string(&input.query)
    )];

    if let Some(team_id) = &input.team_id {
        filter_parts.push(format!(
            "team: {{ id: {{ eq: \"{}\" }} }}",
            escape_graphql_string(team_id)
        ));
    }
    if let Some(state) = &input.state {
        filter_parts.push(format!(
            "state: {{ name: {{ eqIgnoreCase: \"{}\" }} }}",
            escape_graphql_string(state)
        ));
    }
    if let Some(assignee_id) = &input.assignee_id {
        filter_parts.push(format!(
            "assignee: {{ id: {{ eq: \"{}\" }} }}",
            escape_graphql_string(assignee_id)
        ));
    }
    if let Some(priority) = input.priority {
        filter_parts.push(format!("priority: {{ eq: {priority} }}"));
    }

    let filter = if filter_parts.is_empty() {
        String::new()
    } else {
        format!("filter: {{ {} }}", filter_parts.join(", "))
    };

    let query = format!(
        r"
        query {{
            issues({filter} first: {limit}) {{
                nodes {{
                    id
                    identifier
                    title
                    description
                    priority
                    createdAt
                    updatedAt
                    state {{
                        id
                        name
                        type
                        color
                    }}
                    assignee {{
                        id
                        name
                        email
                    }}
                    team {{
                        id
                        name
                        key
                    }}
                    labels {{
                        nodes {{
                            id
                            name
                            color
                        }}
                    }}
                }}
                pageInfo {{
                    hasNextPage
                }}
            }}
        }}
        "
    );

    let data: SearchIssuesData = client.execute_graphql(&query).await?;

    let issues: Vec<Issue> = data.issues.nodes.into_iter().map(map_issue).collect();
    let count = u32::try_from(issues.len())?;

    Ok(SearchIssuesOutput {
        total_count: count,
        has_more: data.issues.page_info.has_next_page,
        issues,
    })
}

// Create Issue

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateIssueInput {
    pub title: String,
    pub team_id: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub priority: Option<u8>,
    #[serde(default)]
    pub assignee_id: Option<String>,
    #[serde(default)]
    pub state_id: Option<String>,
    #[serde(default)]
    pub label_ids: Option<Vec<String>>,
    #[serde(default)]
    pub cycle_id: Option<String>,
    #[serde(default)]
    pub estimate: Option<f32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateIssueOutput {
    pub issue: Issue,
    pub success: bool,
}

/// # Create Linear Issue
///
/// Creates a new issue in Linear with the provided properties. Use this tool
/// when a user wants to create a new task, bug report, feature request, or any
/// other type of issue in Linear.
///
/// This tool creates an issue with the following configurable properties:
/// - **Title** (required): The issue title/name
/// - **Team ID** (required): The ID of the team where the issue will be created
/// - **Description** (optional): Detailed description of the issue (supports
///   markdown)
/// - **Priority** (optional): Priority level 0-4 (0=no priority, 1=urgent,
///   2=high, 3=medium, 4=low)
/// - **Assignee ID** (optional): User ID to assign the issue to
/// - **State ID** (optional): Initial state ID (e.g., "Backlog", "Todo"). If
///   not provided, uses team's default state
/// - **Label IDs** (optional): List of label IDs to categorize the issue
/// - **Cycle ID** (optional): ID of the cycle/sprint to add the issue to
/// - **Estimate** (optional): Issue estimate in the team's configured units
///
/// **When to use this tool:**
/// - User asks to create, make, or add a new issue in Linear
/// - User wants to report a bug or request a feature
/// - User needs to track a new task or work item
/// - User wants to add an item to a specific sprint or cycle
///
/// **Output:** Returns the created issue with all its properties including the
/// auto-generated identifier (e.g., "ENG-123").
///
/// **Note:** You typically need to obtain the `team_id`, `assignee_id`,
/// `state_id`, and other IDs from other tools or by searching first.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - project-management
/// - linear
/// - issues
///
/// # Errors
///
/// Returns an error if:
/// - The provided title is empty or contains only whitespace
/// - The provided `team_id` is empty or contains only whitespace
/// - No Linear credentials are configured in the context
/// - The configured `access_token` is empty
/// - The GraphQL endpoint is unreachable or returns a non-success status
/// - The GraphQL mutation fails validation or execution (returned via GraphQL
///   errors)
/// - The response data is missing or malformed (e.g., no issue in response)
#[tool]
pub async fn create_issue(ctx: Context, input: CreateIssueInput) -> Result<CreateIssueOutput> {
    ensure!(!input.title.trim().is_empty(), "title must not be empty");
    ensure!(
        !input.team_id.trim().is_empty(),
        "team_id must not be empty"
    );

    let client = LinearClient::from_ctx(&ctx)?;

    let mut input_fields = vec![
        format!("title: \"{}\"", escape_graphql_string(&input.title)),
        format!("teamId: \"{}\"", escape_graphql_string(&input.team_id)),
    ];

    if let Some(desc) = &input.description {
        input_fields.push(format!("description: \"{}\"", escape_graphql_string(desc)));
    }
    if let Some(priority) = input.priority {
        input_fields.push(format!("priority: {priority}"));
    }
    if let Some(assignee_id) = &input.assignee_id {
        input_fields.push(format!(
            "assigneeId: \"{}\"",
            escape_graphql_string(assignee_id)
        ));
    }
    if let Some(state_id) = &input.state_id {
        input_fields.push(format!("stateId: \"{}\"", escape_graphql_string(state_id)));
    }
    if let Some(label_ids) = &input.label_ids {
        let ids = label_ids
            .iter()
            .map(|id| format!("\"{}\"", escape_graphql_string(id)))
            .collect::<Vec<_>>()
            .join(", ");
        input_fields.push(format!("labelIds: [{ids}]"));
    }
    if let Some(cycle_id) = &input.cycle_id {
        input_fields.push(format!("cycleId: \"{}\"", escape_graphql_string(cycle_id)));
    }
    if let Some(estimate) = input.estimate {
        input_fields.push(format!("estimate: {estimate}"));
    }

    let query = format!(
        r"
        mutation {{
            issueCreate(input: {{ {} }}) {{
                success
                issue {{
                    id
                    identifier
                    title
                    description
                    priority
                    createdAt
                    updatedAt
                    state {{
                        id
                        name
                        type
                        color
                    }}
                    assignee {{
                        id
                        name
                        email
                    }}
                    team {{
                        id
                        name
                        key
                    }}
                    labels {{
                        nodes {{
                            id
                            name
                            color
                        }}
                    }}
                }}
            }}
        }}
        ",
        input_fields.join(", ")
    );

    let data: CreateIssueData = client.execute_graphql(&query).await?;
    let payload = data.issue_create;
    let issue = payload
        .issue
        .ok_or_else(|| anyhow!("No issue in response"))?;

    Ok(CreateIssueOutput {
        issue: map_issue(issue),
        success: payload.success,
    })
}

// Update State

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateStateInput {
    pub issue_id: String,
    pub state_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateStateOutput {
    pub issue: Issue,
    pub success: bool,
}

/// # Update Linear Issue State
///
/// Updates the state (status) of an existing Linear issue. Use this tool when a
/// user wants to change the status of an issue, such as moving it from
/// "Backlog" to "In Progress" or marking it as "Done".
///
/// This tool transitions an issue to a different workflow state. Common state
/// transitions include:
/// - Moving from "Backlog" or "Todo" to "In Progress"
/// - Marking an issue as "Done", "Completed", or "Closed"
/// - Moving to "In Review", "Blocked", or other custom states
///
/// **When to use this tool:**
/// - User asks to start, begin, or work on an issue (move to "In Progress")
/// - User wants to complete, finish, or resolve an issue (move to "Done")
/// - User asks to change the status, state, or stage of an issue
/// - User mentions moving an issue through a workflow
///
/// **Note:** You need the `issue_id` (obtained from search or issue details)
/// and the `state_id`. State names/IDs vary by team workflow configuration, so
/// you may need to search or list available states first.
///
/// **Output:** Returns the updated issue with its new state and other current
/// properties.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - project-management
/// - linear
/// - issues
///
/// # Errors
///
/// Returns an error if:
/// - The provided `issue_id` is empty or contains only whitespace
/// - The provided `state_id` is empty or contains only whitespace
/// - No Linear credentials are configured in the context
/// - The configured `access_token` is empty
/// - The GraphQL endpoint is unreachable or returns a non-success status
/// - The GraphQL mutation fails validation or execution (returned via GraphQL
///   errors)
/// - The response data is missing or malformed (e.g., no issue in response)
#[tool]
pub async fn update_state(ctx: Context, input: UpdateStateInput) -> Result<UpdateStateOutput> {
    ensure!(
        !input.issue_id.trim().is_empty(),
        "issue_id must not be empty"
    );
    ensure!(
        !input.state_id.trim().is_empty(),
        "state_id must not be empty"
    );

    let client = LinearClient::from_ctx(&ctx)?;

    let query = format!(
        r#"
        mutation {{
            issueUpdate(
                id: "{}",
                input: {{ stateId: "{}" }}
            ) {{
                success
                issue {{
                    id
                    identifier
                    title
                    description
                    priority
                    createdAt
                    updatedAt
                    state {{
                        id
                        name
                        type
                        color
                    }}
                    assignee {{
                        id
                        name
                        email
                    }}
                    team {{
                        id
                        name
                        key
                    }}
                    labels {{
                        nodes {{
                            id
                            name
                            color
                        }}
                    }}
                }}
            }}
        }}
        "#,
        escape_graphql_string(&input.issue_id),
        escape_graphql_string(&input.state_id)
    );

    let data: UpdateIssueData = client.execute_graphql(&query).await?;
    let payload = data.issue_update;
    let issue = payload
        .issue
        .ok_or_else(|| anyhow!("No issue in response"))?;

    Ok(UpdateStateOutput {
        issue: map_issue(issue),
        success: payload.success,
    })
}

// Add Comment

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddCommentInput {
    pub issue_id: String,
    pub body: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AddCommentOutput {
    pub comment: Comment,
    pub issue_id: String,
    pub success: bool,
}

/// # Add Linear Issue Comment
///
/// Adds a comment to an existing Linear issue. Use this tool when a user wants
/// to add a note, question, feedback, or any other comment to an issue
/// discussion.
///
/// This tool appends a new comment to the issue's comment thread. Comments are
/// used for:
/// - Providing updates or progress reports
/// - Asking questions or clarifying requirements
/// - Sharing feedback or suggestions
/// - Collaborating with team members on an issue
/// - Documenting decisions or discussions
///
/// **When to use this tool:**
/// - User asks to comment on, add a note to, or reply to an issue
/// - User wants to provide an update or status on an issue
/// - User asks a question about an issue or needs clarification
/// - User wants to share feedback or thoughts on an issue
///
/// **Note:** The comment body supports markdown formatting. You need the
/// `issue_id` (obtained from search or issue details) to add a comment.
///
/// **Output:** Returns the created comment with its ID, body, author
/// information, and timestamps.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - project-management
/// - linear
/// - issues
///
/// # Errors
///
/// Returns an error if:
/// - The provided `issue_id` is empty or contains only whitespace
/// - The provided body is empty or contains only whitespace
/// - No Linear credentials are configured in the context
/// - The configured `access_token` is empty
/// - The GraphQL endpoint is unreachable or returns a non-success status
/// - The GraphQL mutation fails validation or execution (returned via GraphQL
///   errors)
/// - The response data is missing or malformed (e.g., no comment in response)
#[tool]
pub async fn add_comment(ctx: Context, input: AddCommentInput) -> Result<AddCommentOutput> {
    ensure!(
        !input.issue_id.trim().is_empty(),
        "issue_id must not be empty"
    );
    ensure!(!input.body.trim().is_empty(), "body must not be empty");

    let client = LinearClient::from_ctx(&ctx)?;

    let query = format!(
        r#"
        mutation {{
            commentCreate(input: {{
                issueId: "{}",
                body: "{}"
            }}) {{
                success
                comment {{
                    id
                    body
                    createdAt
                    updatedAt
                    resolvesParent
                    user {{
                        id
                        name
                        email
                    }}
                }}
            }}
        }}
        "#,
        escape_graphql_string(&input.issue_id),
        escape_graphql_string(&input.body)
    );

    let data: CreateCommentData = client.execute_graphql(&query).await?;
    let payload = data.comment_create;
    let comment = payload
        .comment
        .ok_or_else(|| anyhow!("No comment in response"))?;

    Ok(AddCommentOutput {
        comment: map_comment(comment),
        issue_id: input.issue_id,
        success: payload.success,
    })
}

// List Cycles

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListCyclesInput {
    pub team_id: String,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListCyclesOutput {
    pub cycles: Vec<Cycle>,
    pub team: Team,
    pub total_count: u32,
}

/// # List Linear Team Cycles
///
/// Lists cycles (sprints) for a Linear team. Use this tool when a user wants to
/// see all cycles/sprints for a team, including active, upcoming, and past
/// cycles with their progress and metrics.
///
/// This tool returns cycles with detailed information including:
/// - Cycle number and name (e.g., "Sprint 1", "Sprint 2")
/// - Start and end dates
/// - Progress metrics (percentage complete)
/// - Issue counts (total and completed)
/// - Scope metrics (estimated work and completed work)
///
/// **When to use this tool:**
/// - User asks to list, show, or view cycles/sprints for a team
/// - User wants to see the current or active sprint
/// - User needs to check sprint progress or status
/// - User wants to know which issues are in a particular cycle
/// - User is planning work for upcoming cycles
///
/// **Output:** Returns a list of cycles with progress metrics, issue counts,
/// and team information. Results are limited to 10 cycles by default (maximum
/// 50).
///
/// **Note:** You need the `team_id` to list cycles. Cycles are team-specific in
/// Linear, so you must specify which team's cycles to retrieve.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - project-management
/// - linear
/// - cycles
/// - sprints
///
/// # Errors
///
/// Returns an error if:
/// - The provided `team_id` is empty or contains only whitespace
/// - No Linear credentials are configured in the context
/// - The configured `access_token` is empty
/// - The GraphQL endpoint is unreachable or returns a non-success status
/// - The GraphQL query fails validation or execution (returned via GraphQL
///   errors)
/// - The response data is missing or malformed
/// - The number of returned cycles cannot be converted to u32
#[tool]
pub async fn list_cycles(ctx: Context, input: ListCyclesInput) -> Result<ListCyclesOutput> {
    ensure!(
        !input.team_id.trim().is_empty(),
        "team_id must not be empty"
    );
    let limit = input.limit.unwrap_or(10).min(50);

    let client = LinearClient::from_ctx(&ctx)?;

    let query = format!(
        r#"
        query {{
            cycles(filter: {{ team: {{ id: {{ eq: "{}" }} }} }}, first: {}) {{
                nodes {{
                    id
                    number
                    name
                    description
                    startsAt
                    endsAt
                    progress
                    scopeHistory
                    completedScopeHistory
                    issues {{
                        count
                    }}
                    completedIssues {{
                        count
                    }}
                }}
            }}
            team(id: "{}") {{
                id
                name
                key
            }}
        }}
        "#,
        escape_graphql_string(&input.team_id),
        limit,
        escape_graphql_string(&input.team_id)
    );

    let data: ListCyclesData = client.execute_graphql(&query).await?;

    let cycles: Vec<Cycle> = data.cycles.nodes.into_iter().map(map_cycle).collect();
    let count = u32::try_from(cycles.len())?;

    Ok(ListCyclesOutput {
        total_count: count,
        cycles,
        team: map_team(data.team),
    })
}

// GraphQL Client

struct LinearClient {
    client: GqlClient,
}

impl std::fmt::Debug for LinearClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinearClient").finish_non_exhaustive()
    }
}

impl LinearClient {
    /// Creates a new `LinearClient` from the provided context.
    ///
    /// Extracts Linear credentials (`access_token` and optional endpoint) from
    /// the context and initializes an HTTP client for making GraphQL
    /// requests.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No Linear credentials are configured in the context
    /// - The configured `access_token` is empty or contains only whitespace
    /// - The configured endpoint is empty or contains only whitespace
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = LinearCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let endpoint = cred.endpoint.as_deref().unwrap_or(DEFAULT_GRAPHQL_ENDPOINT);
        ensure!(!endpoint.trim().is_empty(), "endpoint must not be empty");

        let mut headers = HashMap::new();
        headers.insert("authorization", format!("Bearer {}", cred.access_token));

        Ok(Self {
            client: GqlClient::new_with_headers(endpoint.trim(), headers),
        })
    }

    /// Executes a GraphQL request against the Linear API.
    ///
    /// Sends a POST request with the provided GraphQL query
    /// to the configured Linear endpoint, using bearer token authentication.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails (network errors, timeout, etc.)
    /// - The GraphQL query returns errors
    /// - The response body cannot be parsed as JSON
    async fn execute_graphql<T: for<'de> Deserialize<'de>>(&self, query: &str) -> Result<T> {
        self.client
            .query::<T>(query)
            .await
            .map_err(|e| anyhow!("GraphQL error: {e}"))?
            .ok_or_else(|| anyhow!("No data in GraphQL response"))
    }
}

// Mapping functions

fn map_issue(issue: GraphQLIssue) -> Issue {
    Issue {
        id: issue.id,
        identifier: issue.identifier,
        title: issue.title,
        description: issue.description,
        priority: issue.priority,
        created_at: issue.created_at,
        updated_at: issue.updated_at,
        state: map_state(issue.state),
        assignee: issue.assignee.map(map_user),
        team: map_team(issue.team),
        labels: issue.labels.nodes.into_iter().map(map_label).collect(),
    }
}

fn map_state(state: GraphQLIssueState) -> IssueState {
    IssueState {
        id: state.id,
        name: state.name,
        state_type: state.state_type,
        color: state.color,
    }
}

fn map_user(user: GraphQLUser) -> User {
    User {
        id: user.id,
        name: user.name,
        email: user.email,
    }
}

fn map_team(team: GraphQLTeam) -> Team {
    Team {
        id: team.id,
        name: team.name,
        key: team.key,
    }
}

fn map_label(label: GraphQLLabel) -> Label {
    Label {
        id: label.id,
        name: label.name,
        color: label.color,
    }
}

fn map_comment(comment: GraphQLComment) -> Comment {
    Comment {
        id: comment.id,
        body: comment.body,
        user: map_user(comment.user),
        created_at: comment.created_at,
        updated_at: comment.updated_at,
        resolves_parent: comment.resolves_parent,
    }
}

fn map_cycle(cycle: GraphQLCycle) -> Cycle {
    let scope = cycle.scope_history.last().copied().unwrap_or(0.0);
    let completed_scope = cycle.completed_scope_history.last().copied().unwrap_or(0.0);

    Cycle {
        id: cycle.id,
        number: cycle.number,
        name: cycle.name,
        description: cycle.description,
        starts_at: cycle.starts_at,
        ends_at: cycle.ends_at,
        issue_count: cycle.issues.count,
        completed_issue_count: cycle.completed_issues.count,
        scope,
        completed_scope,
        progress: cycle.progress,
    }
}

fn escape_graphql_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_string_contains, header, method, path},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut linear_values = HashMap::new();
        linear_values.insert("access_token".to_string(), "test-token".to_string());
        linear_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("linear", linear_values)
    }

    #[test]
    fn test_escape_graphql_string_escapes_quotes() {
        assert_eq!(
            escape_graphql_string(r#"hello "world""#),
            r#"hello \"world\""#
        );
    }

    #[test]
    fn test_escape_graphql_string_escapes_newlines() {
        assert_eq!(escape_graphql_string("hello\nworld"), "hello\\nworld");
    }

    #[tokio::test]
    async fn test_search_issues_empty_query_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = search_issues(
            ctx,
            SearchIssuesInput {
                query: "   ".to_string(),
                team_id: None,
                state: None,
                assignee_id: None,
                priority: None,
                limit: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("query must not be empty")
        );
    }

    #[tokio::test]
    async fn test_search_issues_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains("query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "issues": {
                        "nodes": [{
                            "id": "issue-1",
                            "identifier": "ENG-123",
                            "title": "Test issue",
                            "description": "Description",
                            "priority": 2,
                            "createdAt": "2024-01-15T10:00:00Z",
                            "updatedAt": "2024-01-15T11:00:00Z",
                            "state": {
                                "id": "state-1",
                                "name": "In Progress",
                                "type": "started",
                                "color": "#f2c94c"
                            },
                            "assignee": {
                                "id": "user-1",
                                "name": "John Doe",
                                "email": "john@example.com"
                            },
                            "team": {
                                "id": "team-1",
                                "name": "Engineering",
                                "key": "ENG"
                            },
                            "labels": {
                                "nodes": []
                            }
                        }],
                        "pageInfo": {
                            "hasNextPage": false,
                            "hasPreviousPage": false
                        }
                    }
                }
            })))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = search_issues(
            ctx,
            SearchIssuesInput {
                query: "test".to_string(),
                team_id: None,
                state: None,
                assignee_id: None,
                priority: None,
                limit: Some(10),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.issues.len(), 1);
        assert_eq!(output.issues[0].identifier, "ENG-123");
        assert_eq!(output.issues[0].title, "Test issue");
        assert!(!output.has_more);
    }

    #[tokio::test]
    async fn test_create_issue_empty_title_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = create_issue(
            ctx,
            CreateIssueInput {
                title: "   ".to_string(),
                team_id: "team-1".to_string(),
                description: None,
                priority: None,
                assignee_id: None,
                state_id: None,
                label_ids: None,
                cycle_id: None,
                estimate: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("title must not be empty")
        );
    }

    #[tokio::test]
    async fn test_create_issue_empty_team_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = create_issue(
            ctx,
            CreateIssueInput {
                title: "Test Issue".to_string(),
                team_id: "   ".to_string(),
                description: None,
                priority: None,
                assignee_id: None,
                state_id: None,
                label_ids: None,
                cycle_id: None,
                estimate: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("team_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_create_issue_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "issueCreate": {
                        "success": true,
                        "issue": {
                            "id": "issue-new",
                            "identifier": "ENG-456",
                            "title": "New Issue",
                            "description": "Test description",
                            "priority": 2,
                            "createdAt": "2024-01-20T10:00:00Z",
                            "updatedAt": "2024-01-20T10:00:00Z",
                            "state": {
                                "id": "state-1",
                                "name": "Backlog",
                                "type": "backlog",
                                "color": "#e5e7eb"
                            },
                            "assignee": null,
                            "team": {
                                "id": "team-1",
                                "name": "Engineering",
                                "key": "ENG"
                            },
                            "labels": {
                                "nodes": []
                            }
                        }
                    }
                }
            })))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = create_issue(
            ctx,
            CreateIssueInput {
                title: "New Issue".to_string(),
                team_id: "team-1".to_string(),
                description: Some("Test description".to_string()),
                priority: Some(2),
                assignee_id: None,
                state_id: None,
                label_ids: None,
                cycle_id: None,
                estimate: None,
            },
        )
        .await
        .unwrap();

        assert!(output.success);
        assert_eq!(output.issue.identifier, "ENG-456");
        assert_eq!(output.issue.title, "New Issue");
    }

    #[tokio::test]
    async fn test_update_state_empty_issue_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = update_state(
            ctx,
            UpdateStateInput {
                issue_id: "   ".to_string(),
                state_id: "state-1".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("issue_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_update_state_empty_state_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = update_state(
            ctx,
            UpdateStateInput {
                issue_id: "issue-1".to_string(),
                state_id: "   ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("state_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_update_state_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "issueUpdate": {
                        "success": true,
                        "issue": {
                            "id": "issue-1",
                            "identifier": "ENG-123",
                            "title": "Test Issue",
                            "description": "Description",
                            "priority": 2,
                            "createdAt": "2024-01-15T10:00:00Z",
                            "updatedAt": "2024-01-20T11:00:00Z",
                            "state": {
                                "id": "state-in-progress",
                                "name": "In Progress",
                                "type": "started",
                                "color": "#f2c94c"
                            },
                            "assignee": null,
                            "team": {
                                "id": "team-1",
                                "name": "Engineering",
                                "key": "ENG"
                            },
                            "labels": {
                                "nodes": []
                            }
                        }
                    }
                }
            })))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = update_state(
            ctx,
            UpdateStateInput {
                issue_id: "issue-1".to_string(),
                state_id: "state-in-progress".to_string(),
            },
        )
        .await
        .unwrap();

        assert!(output.success);
        assert_eq!(output.issue.state.name, "In Progress");
        assert_eq!(output.issue.state.id, "state-in-progress");
    }

    #[tokio::test]
    async fn test_add_comment_empty_issue_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = add_comment(
            ctx,
            AddCommentInput {
                issue_id: "   ".to_string(),
                body: "This is a comment".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("issue_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_add_comment_empty_body_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = add_comment(
            ctx,
            AddCommentInput {
                issue_id: "issue-1".to_string(),
                body: "   ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("body must not be empty")
        );
    }

    #[tokio::test]
    async fn test_add_comment_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "commentCreate": {
                        "success": true,
                        "comment": {
                            "id": "comment-1",
                            "body": "This is a comment",
                            "createdAt": "2024-01-20T12:00:00Z",
                            "updatedAt": "2024-01-20T12:00:00Z",
                            "resolvesParent": false,
                            "user": {
                                "id": "user-1",
                                "name": "John Doe",
                                "email": "john@example.com"
                            }
                        }
                    }
                }
            })))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = add_comment(
            ctx,
            AddCommentInput {
                issue_id: "issue-1".to_string(),
                body: "This is a comment".to_string(),
            },
        )
        .await
        .unwrap();

        assert!(output.success);
        assert_eq!(output.comment.body, "This is a comment");
        assert_eq!(output.comment.user.name, "John Doe");
        assert_eq!(output.issue_id, "issue-1");
    }

    #[tokio::test]
    async fn test_list_cycles_empty_team_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = list_cycles(
            ctx,
            ListCyclesInput {
                team_id: "   ".to_string(),
                limit: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("team_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_list_cycles_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "cycles": {
                        "nodes": [
                            {
                                "id": "cycle-1",
                                "number": 1,
                                "name": "Sprint 1",
                                "description": "First sprint",
                                "startsAt": "2024-01-01T00:00:00Z",
                                "endsAt": "2024-01-14T23:59:59Z",
                                "progress": 0.75,
                                "scopeHistory": [10.0, 15.0],
                                "completedScopeHistory": [5.0, 10.0],
                                "issues": {
                                    "count": 5
                                },
                                "completedIssues": {
                                    "count": 3
                                }
                            }
                        ]
                    },
                    "team": {
                        "id": "team-1",
                        "name": "Engineering",
                        "key": "ENG"
                    }
                }
            })))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = list_cycles(
            ctx,
            ListCyclesInput {
                team_id: "team-1".to_string(),
                limit: Some(10),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.cycles.len(), 1);
        assert_eq!(output.cycles[0].number, 1);
        assert_eq!(output.cycles[0].name, Some("Sprint 1".to_string()));
        assert_eq!(output.cycles[0].issue_count, 5);
        assert_eq!(output.cycles[0].completed_issue_count, 3);
        assert_eq!(output.team.name, "Engineering");
        assert_eq!(output.total_count, 1);
    }

    #[tokio::test]
    async fn test_linear_client_from_ctx_empty_access_token_returns_error() {
        let mut linear_values = HashMap::new();
        linear_values.insert("access_token".to_string(), "   ".to_string());
        linear_values.insert(
            "endpoint".to_string(),
            "https://api.example.com".to_string(),
        );

        let ctx = Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("linear", linear_values);

        let result = LinearClient::from_ctx(&ctx);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("access_token must not be empty")
        );
    }

    #[tokio::test]
    async fn test_graphql_error_handling() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": null,
                "errors": [
                    {
                        "message": "Validation error"
                    }
                ]
            })))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let result = search_issues(
            ctx,
            SearchIssuesInput {
                query: "test".to_string(),
                team_id: None,
                state: None,
                assignee_id: None,
                priority: None,
                limit: Some(10),
            },
        )
        .await;

        assert!(result.is_err());
        // gql_client returns "GraphQL error: ..." for GraphQL errors
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("GraphQL error") || err_msg.contains("Validation error"),
            "Expected GraphQL error message, got: {err_msg}"
        );
    }

    #[tokio::test]
    async fn test_http_error_handling() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let result = search_issues(
            ctx,
            SearchIssuesInput {
                query: "test".to_string(),
                team_id: None,
                state: None,
                assignee_id: None,
                priority: None,
                limit: Some(10),
            },
        )
        .await;

        assert!(result.is_err());
        // gql_client returns its own HTTP error format
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("GraphQL error")
                || err_msg.contains("401")
                || err_msg.contains("Unauthorized"),
            "Expected HTTP error message, got: {err_msg}"
        );
    }
}
