use anyhow::Result;

use crate::client::AhaClient;
use crate::output::{render_list, render_one, OutputFormat};

use super::TodoRow;

pub async fn list(client: &AhaClient, feature: Option<&str>, format: OutputFormat) -> Result<()> {
    let todos = client.list_todos(feature).await?;
    let rows: Vec<TodoRow> = todos.iter().map(TodoRow::from).collect();
    render_list(format, &rows, &todos)
}

pub async fn show(client: &AhaClient, id: &str, format: OutputFormat) -> Result<()> {
    let t = client.get_todo(id).await?;
    let kv = vec![
        ("id", t.id.clone()),
        ("name", t.name.clone()),
        ("status", t.status.clone().unwrap_or_else(|| "—".into())),
        (
            "due_date",
            t.due_date
                .map(|d| d.to_string())
                .unwrap_or_else(|| "—".into()),
        ),
        (
            "assignees",
            if t.assigned_to_users.is_empty() {
                "—".into()
            } else {
                t.assigned_to_users
                    .iter()
                    .map(|u| u.email.clone().unwrap_or_else(|| u.name.clone()))
                    .collect::<Vec<_>>()
                    .join(", ")
            },
        ),
    ];
    render_one(format, &kv, &t)
}
