pub mod attachments;
pub mod backlog;
pub mod epics;
pub mod features;
pub mod ideas;
pub mod products;
pub mod releases;
pub mod requirements;
pub mod todos;

use serde::Serialize;
use tabled::Tabled;

use crate::client::models::*;

// ---------- Tabled row projections ----------

#[derive(Tabled, Serialize)]
pub struct ProductRow {
    #[tabled(rename = "ID")]
    pub id: String,
    #[tabled(rename = "PREFIX")]
    pub prefix: String,
    #[tabled(rename = "NAME")]
    pub name: String,
}

impl From<&Product> for ProductRow {
    fn from(p: &Product) -> Self {
        Self {
            id: p.id.clone(),
            prefix: p.reference_prefix.clone(),
            name: p.name.clone(),
        }
    }
}

#[derive(Tabled, Serialize)]
pub struct ReleaseRow {
    #[tabled(rename = "REF")]
    pub reference_num: String,
    #[tabled(rename = "NAME")]
    pub name: String,
    #[tabled(rename = "DATE")]
    pub release_date: String,
    #[tabled(rename = "STATUS")]
    pub status: String,
}

impl From<&Release> for ReleaseRow {
    fn from(r: &Release) -> Self {
        let status = if r.released {
            "released"
        } else if r.parking_lot {
            "parking-lot"
        } else {
            "planned"
        };
        Self {
            reference_num: r.reference_num.clone(),
            name: r.name.clone(),
            release_date: r
                .release_date
                .map(|d| d.to_string())
                .unwrap_or_else(|| "—".into()),
            status: status.to_string(),
        }
    }
}

#[derive(Tabled, Serialize)]
pub struct EpicRow {
    #[tabled(rename = "REF")]
    pub reference_num: String,
    #[tabled(rename = "NAME")]
    pub name: String,
    #[tabled(rename = "STATUS")]
    pub status: String,
    #[tabled(rename = "RELEASE")]
    pub release: String,
}

impl From<&Epic> for EpicRow {
    fn from(e: &Epic) -> Self {
        Self {
            reference_num: e.reference_num.clone(),
            name: e.name.clone(),
            status: status_label(&e.workflow_status),
            release: e
                .release
                .as_ref()
                .map(|r| r.reference_num.clone())
                .unwrap_or_else(|| "—".into()),
        }
    }
}

#[derive(Tabled, Serialize)]
pub struct FeatureRow {
    #[tabled(rename = "REF")]
    pub reference_num: String,
    #[tabled(rename = "NAME")]
    pub name: String,
    #[tabled(rename = "STATUS")]
    pub status: String,
    #[tabled(rename = "ASSIGNEE")]
    pub assignee: String,
    #[tabled(rename = "RELEASE")]
    pub release: String,
}

impl From<&Feature> for FeatureRow {
    fn from(f: &Feature) -> Self {
        Self {
            reference_num: f.reference_num.clone(),
            name: f.name.clone(),
            status: status_label(&f.workflow_status),
            assignee: f
                .assigned_to_user
                .as_ref()
                .and_then(|u| u.email.clone().or(Some(u.name.clone())))
                .unwrap_or_else(|| "—".into()),
            release: f
                .release
                .as_ref()
                .map(|r| r.reference_num.clone())
                .unwrap_or_else(|| "—".into()),
        }
    }
}

#[derive(Tabled, Serialize)]
pub struct TodoRow {
    #[tabled(rename = "ID")]
    pub id: String,
    #[tabled(rename = "NAME")]
    pub name: String,
    #[tabled(rename = "STATUS")]
    pub status: String,
    #[tabled(rename = "DUE")]
    pub due: String,
}

impl From<&Todo> for TodoRow {
    fn from(t: &Todo) -> Self {
        Self {
            id: t.id.clone(),
            name: t.name.clone(),
            status: t.status.clone().unwrap_or_else(|| "—".into()),
            due: t
                .due_date
                .map(|d| d.to_string())
                .unwrap_or_else(|| "—".into()),
        }
    }
}

#[derive(Tabled, Serialize)]
pub struct IdeaRow {
    #[tabled(rename = "REF")]
    pub reference_num: String,
    #[tabled(rename = "NAME")]
    pub name: String,
    #[tabled(rename = "STATUS")]
    pub status: String,
}

impl From<&Idea> for IdeaRow {
    fn from(i: &Idea) -> Self {
        Self {
            reference_num: i.reference_num.clone(),
            name: i.name.clone(),
            status: status_label(&i.workflow_status),
        }
    }
}

pub(crate) fn status_label(status: &Option<WorkflowStatus>) -> String {
    status
        .as_ref()
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "—".into())
}
