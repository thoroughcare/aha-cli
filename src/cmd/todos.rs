use anyhow::Result;

use crate::client::AhaClient;
use crate::output::{render_list, OutputFormat};

use super::TodoRow;

pub async fn list(client: &AhaClient, feature: Option<&str>, format: OutputFormat) -> Result<()> {
    let todos = client.list_todos(feature).await?;
    let rows: Vec<TodoRow> = todos.iter().map(TodoRow::from).collect();
    render_list(format, &rows, &todos)
}

pub async fn show(client: &AhaClient, id: &str, format: OutputFormat) -> Result<()> {
    let deep = client.todo_show(id).await?;
    match format {
        OutputFormat::Json => {
            let payload = serde_json::json!({
                "todo": deep.todo,
                "comments": deep.comments,
            });
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
        OutputFormat::Yaml => {
            let payload = serde_json::json!({
                "todo": deep.todo,
                "comments": deep.comments,
            });
            println!("{}", serde_yaml::to_string(&payload)?);
        }
        OutputFormat::Table => {
            let t = &deep.todo;
            let max_key = "due_date".len();
            println!("{:<width$}  {}", "id", t.id, width = max_key);
            println!("{:<width$}  {}", "name", t.name, width = max_key);
            println!(
                "{:<width$}  {}",
                "status",
                t.status.clone().unwrap_or_else(|| "—".into()),
                width = max_key
            );
            println!(
                "{:<width$}  {}",
                "due_date",
                t.due_date
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| "—".into()),
                width = max_key
            );
            let assignees = if t.assigned_to_users.is_empty() {
                "—".into()
            } else {
                t.assigned_to_users
                    .iter()
                    .map(|u| u.email.clone().unwrap_or_else(|| u.name.clone()))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            println!("{:<width$}  {}", "assignees", assignees, width = max_key);

            if let Some(body) = t.body.as_ref().filter(|b| !b.is_empty()) {
                println!("\nbody:\n{body}");
            }

            if !t.attachments.is_empty() {
                println!("\nattachments:");
                for a in &t.attachments {
                    print_attachment(a);
                }
            }

            if !deep.comments.is_empty() {
                println!("\ncomments: {} entries", deep.comments.len());
                for c in &deep.comments {
                    let author = c
                        .user
                        .as_ref()
                        .map(|u| u.email.clone().unwrap_or_else(|| u.name.clone()))
                        .unwrap_or_else(|| "—".into());
                    let when = c
                        .created_at
                        .map(|t| t.to_rfc3339())
                        .unwrap_or_else(|| "—".into());
                    println!("  - [{when}] {author}");
                    if !c.attachments.is_empty() {
                        for a in &c.attachments {
                            print_attachment_indented(a, "      ");
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn print_attachment(a: &crate::client::models::Attachment) {
    print_attachment_indented(a, "  ");
}

fn print_attachment_indented(a: &crate::client::models::Attachment, indent: &str) {
    let size = a
        .file_size
        .map(|b| format!(" ({b} bytes)"))
        .unwrap_or_default();
    let ct = a
        .content_type
        .as_deref()
        .map(|c| format!(", {c}"))
        .unwrap_or_default();
    println!("{indent}- {} [id={}{ct}{size}]", a.file_name, a.id);
    if let Some(url) = a.download_url.as_deref() {
        println!("{indent}  download: {url}");
    }
}
