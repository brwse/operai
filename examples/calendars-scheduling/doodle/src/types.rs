//! Type definitions for the Doodle API.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Participant {
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PollOption {
    pub option_id: String,
    pub text: String,
    #[serde(default)]
    pub start_time: Option<String>,
    #[serde(default)]
    pub end_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Vote {
    pub participant_name: String,
    pub option_id: String,
    #[serde(rename = "type")]
    pub vote_type: VoteType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum VoteType {
    Yes,
    No,
    IfNeedBe,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Poll {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    pub options: Vec<PollOption>,
    #[serde(default)]
    pub votes: Vec<Vote>,
    #[serde(default)]
    pub is_closed: bool,
    #[serde(default)]
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PollSummary {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub option_count: usize,
    pub vote_count: usize,
    #[serde(default)]
    pub is_closed: bool,
}
