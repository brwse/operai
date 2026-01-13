//! Type definitions for Google Docs API

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

// ===== Public API Types (exposed to users) =====

/// Document structure with metadata and content
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Document {
    #[serde(rename = "documentId")]
    pub document_id: String,
    pub title: String,
    #[serde(default)]
    pub body: Option<Body>,
    #[serde(rename = "revisionId", default)]
    pub revision_id: Option<String>,
}

/// Document body containing structural elements
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Body {
    pub content: Vec<StructuralElement>,
}

/// A structural element within the document
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StructuralElement {
    #[serde(default)]
    pub start_index: Option<i32>,
    #[serde(default)]
    pub end_index: Option<i32>,
    #[serde(default)]
    pub paragraph: Option<Paragraph>,
}

/// A paragraph element
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Paragraph {
    pub elements: Vec<ParagraphElement>,
}

/// An element within a paragraph
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ParagraphElement {
    #[serde(default)]
    pub start_index: Option<i32>,
    #[serde(default)]
    pub end_index: Option<i32>,
    #[serde(default)]
    pub text_run: Option<TextRun>,
}

/// A text run with content and style
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TextRun {
    pub content: String,
}

/// Location within a document (used for inserting text)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    /// The zero-based index in the document
    pub index: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segment_id: Option<String>,
}

/// Range within a document (used for deleting/updating)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Range {
    pub start_index: i32,
    pub end_index: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segment_id: Option<String>,
}

// ===== Internal Google Docs API Types =====

/// Request wrapper for batchUpdate
#[derive(Debug, Serialize)]
pub(crate) struct BatchUpdateRequest {
    pub requests: Vec<Request>,
}

/// Response from batchUpdate
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BatchUpdateResponse {
    pub document_id: String,
    #[serde(default)]
    pub revision_id: String,
}

/// Response from creating a comment
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentResponse {
    pub id: String,
}

/// Individual request in a batch
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Request {
    InsertText(InsertTextRequest),
    DeleteContentRange(DeleteContentRangeRequest),
}

/// Insert text at a specific location
#[derive(Debug, Serialize)]
pub(crate) struct InsertTextRequest {
    pub text: String,
    pub location: Location,
}

/// Delete content within a range
#[derive(Debug, Serialize)]
pub(crate) struct DeleteContentRangeRequest {
    pub range: Range,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_location_serialization_roundtrip() {
        let location = Location {
            index: 42,
            segment_id: None,
        };
        let json = serde_json::to_string(&location).unwrap();
        let parsed: Location = serde_json::from_str(&json).unwrap();
        assert_eq!(location.index, parsed.index);
    }

    #[test]
    fn test_range_serialization_roundtrip() {
        let range = Range {
            start_index: 10,
            end_index: 50,
            segment_id: None,
        };
        let json = serde_json::to_string(&range).unwrap();
        let parsed: Range = serde_json::from_str(&json).unwrap();
        assert_eq!(range.start_index, parsed.start_index);
        assert_eq!(range.end_index, parsed.end_index);
    }

    #[test]
    fn test_document_deserialization() {
        let json = r#"{
            "documentId": "doc-123",
            "title": "Test Document",
            "revisionId": "rev-456"
        }"#;
        let doc: Document = serde_json::from_str(json).unwrap();
        assert_eq!(doc.document_id, "doc-123");
        assert_eq!(doc.title, "Test Document");
        assert_eq!(doc.revision_id.as_deref(), Some("rev-456"));
    }

    #[test]
    fn test_insert_text_request_serialization() {
        let request = Request::InsertText(InsertTextRequest {
            text: "Hello".to_string(),
            location: Location {
                index: 1,
                segment_id: None,
            },
        });
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("insertText"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_batch_update_request_serialization() {
        let batch = BatchUpdateRequest {
            requests: vec![Request::InsertText(InsertTextRequest {
                text: "Test".to_string(),
                location: Location {
                    index: 1,
                    segment_id: None,
                },
            })],
        };
        let json = serde_json::to_string(&batch).unwrap();
        assert!(json.contains("requests"));
        assert!(json.contains("insertText"));
    }

    #[test]
    fn test_batch_update_response_deserialization() {
        let json = r#"{
            "documentId": "doc-123"
        }"#;
        let response: BatchUpdateResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.document_id, "doc-123");
    }
}
