//! Aha! API response models.
//!
//! All IDs are typed as `String` because Aha! returns 19-digit snowflake
//! IDs that exceed `i53`. Parsing them as numbers would silently truncate.
//! Optional fields use `#[serde(default)]` so additive API changes don't
//! break us. Enums sourced from API strings have an `Unknown(String)`
//! catch-all to absorb new values.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// Loose reference to a related entity (release, epic, etc.) embedded in
/// list responses. Aha! sometimes returns just `{id, reference_num, name}`
/// here rather than the full record.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct EntityRef {
    pub id: String,
    #[serde(default)]
    pub reference_num: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct User {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct WorkflowStatus {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub complete: bool,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Product {
    pub id: String,
    #[serde(default)]
    pub reference_prefix: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Release {
    pub id: String,
    #[serde(default)]
    pub reference_num: String,
    pub name: String,
    #[serde(default)]
    pub release_date: Option<NaiveDate>,
    #[serde(default)]
    pub released: bool,
    #[serde(default)]
    pub parking_lot: bool,
    #[serde(default)]
    pub product_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Epic {
    pub id: String,
    #[serde(default)]
    pub reference_num: String,
    pub name: String,
    #[serde(default)]
    pub workflow_status: Option<WorkflowStatus>,
    #[serde(default)]
    pub release: Option<EntityRef>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Feature {
    pub id: String,
    #[serde(default)]
    pub reference_num: String,
    pub name: String,
    #[serde(default)]
    pub workflow_status: Option<WorkflowStatus>,
    #[serde(default)]
    pub assigned_to_user: Option<User>,
    #[serde(default)]
    pub release: Option<EntityRef>,
    #[serde(default)]
    pub epic: Option<EntityRef>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub description: Option<Description>,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Requirement {
    pub id: String,
    #[serde(default)]
    pub reference_num: String,
    pub name: String,
    #[serde(default)]
    pub workflow_status: Option<WorkflowStatus>,
    #[serde(default)]
    pub description: Option<Description>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Todo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub due_date: Option<NaiveDate>,
    #[serde(default)]
    pub assigned_to_users: Vec<User>,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    /// Free-text body. Only populated by the `/tasks/:id` show endpoint;
    /// the list endpoint returns lean todos without it.
    #[serde(default)]
    pub body: Option<String>,
    /// Files / images attached to the todo. Same caveat as `body`: only
    /// the show endpoint surfaces these.
    #[serde(default)]
    pub attachments: Vec<Attachment>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Comment {
    pub id: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    /// Files / images attached to the comment.
    #[serde(default)]
    pub attachments: Vec<Attachment>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Attachment {
    pub id: String,
    #[serde(default)]
    pub file_name: String,
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub file_size: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Idea {
    pub id: String,
    #[serde(default)]
    pub reference_num: String,
    pub name: String,
    #[serde(default)]
    pub workflow_status: Option<WorkflowStatus>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Description {
    #[serde(default)]
    pub body: String,
}

/// Aha! pagination metadata, returned alongside every list response.
#[derive(Debug, Clone, Deserialize, Default, Serialize)]
pub struct Pagination {
    #[serde(default)]
    pub current_page: u32,
    #[serde(default)]
    pub total_pages: u32,
    #[serde(default)]
    pub total_records: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snowflake_id_round_trips_as_string() {
        let json = r#"{"id":"7626760672407598886","name":"X"}"#;
        let p: Product = serde_json::from_str(json).unwrap();
        assert_eq!(p.id, "7626760672407598886");
    }

    #[test]
    fn missing_optional_fields_default() {
        let json = r#"{"id":"1","name":"X"}"#;
        let f: Feature = serde_json::from_str(json).unwrap();
        assert!(f.workflow_status.is_none());
        assert!(f.tags.is_empty());
    }

    #[test]
    fn unknown_top_level_fields_ignored() {
        // Forward-compat: Aha! adds a new field, we don't break.
        let json = r#"{"id":"1","name":"X","new_field_in_v1":{"nested":true},"other":[1,2,3]}"#;
        let p: Product = serde_json::from_str(json).unwrap();
        assert_eq!(p.name, "X");
    }
}
