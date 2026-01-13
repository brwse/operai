//! Type definitions for the Linear API.

use serde::Deserialize;

// Search issues types

#[derive(Debug, Deserialize)]
pub(crate) struct SearchIssuesData {
    pub issues: IssueConnection,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IssueConnection {
    pub nodes: Vec<GraphQLIssue>,
    pub page_info: PageInfo,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageInfo {
    pub has_next_page: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GraphQLIssue {
    pub id: String,
    pub identifier: String,
    pub title: String,
    pub description: Option<String>,
    pub priority: u8,
    pub created_at: String,
    pub updated_at: String,
    pub state: GraphQLIssueState,
    pub assignee: Option<GraphQLUser>,
    pub team: GraphQLTeam,
    pub labels: LabelConnection,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LabelConnection {
    pub nodes: Vec<GraphQLLabel>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GraphQLIssueState {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub state_type: String,
    pub color: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GraphQLUser {
    pub id: String,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GraphQLTeam {
    pub id: String,
    pub name: String,
    pub key: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GraphQLLabel {
    pub id: String,
    pub name: String,
    pub color: String,
}

// Create issue types

#[derive(Debug, Deserialize)]
pub(crate) struct CreateIssueData {
    #[serde(rename = "issueCreate")]
    pub issue_create: IssuePayload,
}

#[derive(Debug, Deserialize)]
pub(crate) struct IssuePayload {
    pub success: bool,
    pub issue: Option<GraphQLIssue>,
}

// Update issue types

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateIssueData {
    #[serde(rename = "issueUpdate")]
    pub issue_update: IssuePayload,
}

// Comment types

#[derive(Debug, Deserialize)]
pub(crate) struct CreateCommentData {
    #[serde(rename = "commentCreate")]
    pub comment_create: CommentPayload,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CommentPayload {
    pub success: bool,
    pub comment: Option<GraphQLComment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GraphQLComment {
    pub id: String,
    pub body: String,
    pub user: GraphQLUser,
    pub created_at: String,
    pub updated_at: String,
    pub resolves_parent: bool,
}

// Cycle types

#[derive(Debug, Deserialize)]
pub(crate) struct ListCyclesData {
    pub cycles: CycleConnection,
    pub team: GraphQLTeam,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CycleConnection {
    pub nodes: Vec<GraphQLCycle>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GraphQLCycle {
    pub id: String,
    pub number: u32,
    pub name: Option<String>,
    pub description: Option<String>,
    pub starts_at: String,
    pub ends_at: String,
    pub issues: IssueCountConnection,
    pub completed_issues: IssueCountConnection,
    pub scope_history: Vec<f32>,
    pub completed_scope_history: Vec<f32>,
    pub progress: f32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IssueCountConnection {
    pub count: u32,
}
