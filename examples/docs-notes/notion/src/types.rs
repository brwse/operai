//! Type definitions for Notion API responses and requests.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ObjectType {
    Page,
    Database,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Parent {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RichTextContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub plain_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PageSummary {
    pub object: String,
    pub id: String,
    pub created_time: String,
    pub last_edited_time: String,
    #[serde(default)]
    pub archived: bool,
    pub parent: Parent,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PageDetail {
    pub object: String,
    pub id: String,
    pub created_time: String,
    pub last_edited_time: String,
    #[serde(default)]
    pub archived: bool,
    pub parent: Parent,
    #[serde(default)]
    pub properties: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DatabaseSummary {
    pub object: String,
    pub id: String,
    pub created_time: String,
    pub last_edited_time: String,
    #[serde(default)]
    pub archived: bool,
    pub title: Vec<RichTextContent>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- ObjectType serialization tests ---

    #[test]
    fn test_object_type_page_serializes_to_lowercase() {
        let page = ObjectType::Page;
        let json = serde_json::to_string(&page).unwrap();
        assert_eq!(json, "\"page\"");
    }

    #[test]
    fn test_object_type_database_serializes_to_lowercase() {
        let db = ObjectType::Database;
        let json = serde_json::to_string(&db).unwrap();
        assert_eq!(json, "\"database\"");
    }

    #[test]
    fn test_object_type_page_deserializes_from_lowercase() {
        let json = "\"page\"";
        let parsed: ObjectType = serde_json::from_str(json).unwrap();
        assert_eq!(parsed, ObjectType::Page);
    }

    #[test]
    fn test_object_type_database_deserializes_from_lowercase() {
        let json = "\"database\"";
        let parsed: ObjectType = serde_json::from_str(json).unwrap();
        assert_eq!(parsed, ObjectType::Database);
    }

    // --- Parent serialization tests ---

    #[test]
    fn test_parent_with_workspace_serializes_correctly() {
        let parent = Parent {
            type_: "workspace".to_string(),
            database_id: None,
            page_id: None,
            workspace: Some(true),
        };

        let json = serde_json::to_value(&parent).unwrap();
        let obj = json.as_object().unwrap();

        assert_eq!(obj.get("type").unwrap().as_str().unwrap(), "workspace");
        assert!(obj.get("workspace").unwrap().as_bool().unwrap());
        assert!(!obj.contains_key("database_id"));
        assert!(!obj.contains_key("page_id"));
    }

    #[test]
    fn test_parent_with_page_id_serializes_correctly() {
        let parent = Parent {
            type_: "page_id".to_string(),
            database_id: None,
            page_id: Some("page-123".to_string()),
            workspace: None,
        };

        let json = serde_json::to_value(&parent).unwrap();
        let obj = json.as_object().unwrap();

        assert_eq!(obj.get("type").unwrap().as_str().unwrap(), "page_id");
        assert_eq!(obj.get("page_id").unwrap().as_str().unwrap(), "page-123");
        assert!(!obj.contains_key("database_id"));
        assert!(!obj.contains_key("workspace"));
    }

    #[test]
    fn test_parent_with_database_id_serializes_correctly() {
        let parent = Parent {
            type_: "database_id".to_string(),
            database_id: Some("db-123".to_string()),
            page_id: None,
            workspace: None,
        };

        let json = serde_json::to_value(&parent).unwrap();
        let obj = json.as_object().unwrap();

        assert_eq!(obj.get("type").unwrap().as_str().unwrap(), "database_id");
        assert_eq!(obj.get("database_id").unwrap().as_str().unwrap(), "db-123");
        assert!(!obj.contains_key("page_id"));
        assert!(!obj.contains_key("workspace"));
    }

    #[test]
    fn test_parent_optional_fields_not_serialized_when_none() {
        let parent = Parent {
            type_: "workspace".to_string(),
            database_id: None,
            page_id: None,
            workspace: Some(true),
        };

        let json = serde_json::to_value(&parent).unwrap();
        let obj = json.as_object().unwrap();

        // Ensure only non-None fields are present
        assert_eq!(obj.len(), 2); // "type" and "workspace" only
        assert!(obj.contains_key("type"));
        assert!(obj.contains_key("workspace"));
    }

    // --- RichTextContent serialization tests ---

    #[test]
    fn test_rich_text_content_serializes_correctly() {
        let content = RichTextContent {
            content_type: "text".to_string(),
            plain_text: "Hello, world!".to_string(),
        };

        let json = serde_json::to_value(&content).unwrap();
        let obj = json.as_object().unwrap();

        assert_eq!(obj.get("type").unwrap().as_str().unwrap(), "text");
        assert_eq!(
            obj.get("plain_text").unwrap().as_str().unwrap(),
            "Hello, world!"
        );
    }

    // --- PageSummary serialization roundtrip tests ---

    #[test]
    fn test_page_summary_serialization_roundtrip() {
        let page = PageSummary {
            object: "page".to_string(),
            id: "page-123".to_string(),
            created_time: "2024-01-01T00:00:00.000Z".to_string(),
            last_edited_time: "2024-01-02T00:00:00.000Z".to_string(),
            archived: false,
            parent: Parent {
                type_: "workspace".to_string(),
                database_id: None,
                page_id: None,
                workspace: Some(true),
            },
        };

        let json = serde_json::to_string(&page).unwrap();
        let parsed: PageSummary = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.object, page.object);
        assert_eq!(parsed.id, page.id);
        assert_eq!(parsed.created_time, page.created_time);
        assert_eq!(parsed.last_edited_time, page.last_edited_time);
        assert_eq!(parsed.archived, page.archived);
        assert_eq!(parsed.parent.type_, page.parent.type_);
    }

    // --- DatabaseSummary serialization roundtrip tests ---

    #[test]
    fn test_database_summary_serialization_roundtrip() {
        let db = DatabaseSummary {
            object: "database".to_string(),
            id: "db-123".to_string(),
            created_time: "2024-01-01T00:00:00.000Z".to_string(),
            last_edited_time: "2024-01-02T00:00:00.000Z".to_string(),
            archived: false,
            title: vec![RichTextContent {
                content_type: "text".to_string(),
                plain_text: "My Database".to_string(),
            }],
        };

        let json = serde_json::to_string(&db).unwrap();
        let parsed: DatabaseSummary = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.object, db.object);
        assert_eq!(parsed.id, db.id);
        assert_eq!(parsed.created_time, db.created_time);
        assert_eq!(parsed.last_edited_time, db.last_edited_time);
        assert_eq!(parsed.archived, db.archived);
        assert_eq!(parsed.title.len(), db.title.len());
        assert_eq!(parsed.title[0].plain_text, db.title[0].plain_text);
    }

    // --- PageDetail serialization tests ---

    #[test]
    fn test_page_detail_with_empty_properties_serializes_correctly() {
        let page = PageDetail {
            object: "page".to_string(),
            id: "page-123".to_string(),
            created_time: "2024-01-01T00:00:00.000Z".to_string(),
            last_edited_time: "2024-01-02T00:00:00.000Z".to_string(),
            archived: false,
            parent: Parent {
                type_: "workspace".to_string(),
                database_id: None,
                page_id: None,
                workspace: Some(true),
            },
            properties: serde_json::json!({}),
        };

        let json = serde_json::to_value(&page).unwrap();
        let obj = json.as_object().unwrap();

        assert!(obj.contains_key("properties"));
        assert_eq!(obj.get("properties").unwrap().as_object().unwrap().len(), 0);
    }
}
