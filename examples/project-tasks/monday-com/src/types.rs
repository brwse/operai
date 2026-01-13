//! Type definitions for Monday.com API responses and requests.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// Represents a Monday.com board.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Board {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
}

/// Reference to a board.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BoardRef {
    pub id: String,
    pub name: String,
}

/// Represents an item (task/row) in a board.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Item {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub board: Option<BoardRef>,
}

/// Represents an update (comment).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Update {
    pub id: String,
    pub body: String,
}
