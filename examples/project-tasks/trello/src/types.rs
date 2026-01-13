// Internal request types for Trello API calls

#![allow(
    dead_code,
    reason = "These types are used for serde serialization/deserialization but may not all be \
              directly referenced in the codebase"
)]

use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct CreateCardRequest {
    pub name: String,
    #[serde(rename = "idList")]
    pub id_list: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pos: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(rename = "idLabels")]
    pub id_labels: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(rename = "idMembers")]
    pub id_members: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct UpdateCardRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "idList")]
    pub id_list: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "idBoard")]
    pub id_board: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pos: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AddCommentRequest {
    pub text: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreateCheckItemRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pos: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "idMember")]
    pub id_member: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
}
