use anyhow::Result;

use crate::client::resources::FeatureFilters;
use crate::client::AhaClient;
use crate::output::{render_list, render_one, OutputFormat};

use super::{status_label, FeatureRow};

pub async fn list(client: &AhaClient, filters: FeatureFilters, format: OutputFormat) -> Result<()> {
    let features = client.list_features(&filters).await?;
    let rows: Vec<FeatureRow> = features.iter().map(FeatureRow::from).collect();
    render_list(format, &rows, &features)
}

pub async fn show(client: &AhaClient, id: &str, format: OutputFormat) -> Result<()> {
    let deep = client.feature_show(id).await?;
    match format {
        OutputFormat::Json => {
            let payload = serde_json::json!({
                "feature": deep.feature,
                "requirements": deep.requirements,
                "comments": deep.comments,
                "todos": deep.todos.iter().map(|t| serde_json::json!({
                    "todo": t.todo,
                    "comments": t.comments,
                })).collect::<Vec<_>>(),
            });
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
        OutputFormat::Yaml => {
            let payload = serde_json::json!({
                "feature": deep.feature,
                "requirements": deep.requirements,
                "comments": deep.comments,
                "todos": deep.todos.iter().map(|t| serde_json::json!({
                    "todo": t.todo,
                    "comments": t.comments,
                })).collect::<Vec<_>>(),
            });
            println!("{}", serde_yaml::to_string(&payload)?);
        }
        OutputFormat::Table => {
            let f = &deep.feature;
            let kv: Vec<(&str, String)> = vec![
                ("ref", f.reference_num.clone()),
                ("name", f.name.clone()),
                ("status", status_label(&f.workflow_status)),
                (
                    "assignee",
                    f.assigned_to_user
                        .as_ref()
                        .and_then(|u| u.email.clone().or(Some(u.name.clone())))
                        .unwrap_or_else(|| "—".into()),
                ),
                (
                    "release",
                    f.release
                        .as_ref()
                        .map(|r| format!("{} ({})", r.reference_num, r.name))
                        .unwrap_or_else(|| "—".into()),
                ),
                (
                    "epic",
                    f.epic
                        .as_ref()
                        .map(|e| format!("{} ({})", e.reference_num, e.name))
                        .unwrap_or_else(|| "—".into()),
                ),
                (
                    "tags",
                    if f.tags.is_empty() {
                        "—".into()
                    } else {
                        f.tags.join(", ")
                    },
                ),
            ];
            render_one(format, &kv, &deep.feature)?;
            // Re-emit the structured pieces below the kv-detail.
            if !deep.requirements.is_empty() {
                println!("\nrequirements:");
                for r in &deep.requirements {
                    println!(
                        "  {} {}  [{}]",
                        r.reference_num,
                        r.name,
                        status_label(&r.workflow_status)
                    );
                }
            }
            if !deep.comments.is_empty() {
                let attachments_total: usize =
                    deep.comments.iter().map(|c| c.attachments.len()).sum();
                let suffix = if attachments_total > 0 {
                    format!(" ({attachments_total} attachment(s))")
                } else {
                    String::new()
                };
                println!("\ncomments: {} entries{}", deep.comments.len(), suffix);
            }
            if !deep.todos.is_empty() {
                println!("\ntodos:");
                for t in &deep.todos {
                    let status = t.todo.status.clone().unwrap_or_else(|| "—".into());
                    let mut tags = Vec::new();
                    if t.todo
                        .body
                        .as_deref()
                        .map(|b| !b.is_empty())
                        .unwrap_or(false)
                    {
                        tags.push("body".to_string());
                    }
                    if !t.todo.attachments.is_empty() {
                        tags.push(format!("{} attachment(s)", t.todo.attachments.len()));
                    }
                    let comment_attachments: usize =
                        t.comments.iter().map(|c| c.attachments.len()).sum();
                    if !t.comments.is_empty() {
                        let mut s = format!("{} comment(s)", t.comments.len());
                        if comment_attachments > 0 {
                            s.push_str(&format!(", {comment_attachments} attachment(s)"));
                        }
                        tags.push(s);
                    }
                    let suffix = if tags.is_empty() {
                        String::new()
                    } else {
                        format!("  [{}]", tags.join("; "))
                    };
                    println!("  [{}] {}{}", status, t.todo.name, suffix);
                }
            }
        }
    }
    Ok(())
}
