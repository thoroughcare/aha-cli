use anyhow::Result;

use crate::cli::{OnType, TodoCreateArgs, TodoEditArgs, TodoStatusArg};
use crate::client::models::Todo;
use crate::client::resources::{TaskableType, TodoCreate, TodoStatus, TodoUpdate};
use crate::client::AhaClient;
use crate::cmd::write::{confirm, dry_run_preview, BodySource, Confirm, ConfirmOpts};
use crate::output::{render_list, render_one, OutputFormat};

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
            let max_key = "assignees".len();
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
            if let Some(url) = t.url.as_deref().filter(|u| !u.is_empty()) {
                println!("{:<width$}  {}", "url", url, width = max_key);
            }

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

// ---------- Write surface ----------

pub async fn create(client: &AhaClient, args: &TodoCreateArgs, format: OutputFormat) -> Result<()> {
    let taskable_type = match args.on_type {
        Some(t) => map_on_type(t),
        None => infer_taskable_type(&args.on).ok_or_else(|| {
            anyhow::anyhow!(
                "could not infer parent type from --on={} — pass --on-type {{feature|requirement|release|epic}}",
                args.on
            )
        })?,
    };

    let body_text =
        BodySource::from_flags(args.body.clone(), args.body_file.clone(), args.editor, None)
            .map(|src| src.resolve())
            .transpose()?
            .unwrap_or_default();

    let assignees = if args.assignee.is_empty() {
        None
    } else {
        Some(args.assignee.clone())
    };

    let body = TodoCreate {
        name: &args.name,
        body: &body_text,
        taskable_type: Some(taskable_type),
        taskable_id: &args.on,
        due_date: args.due.as_deref(),
        assigned_to_users: assignees,
    };

    let preview = dry_run_preview("POST", "/tasks", &serde_json::json!({ "task": &body }));
    let opts = ConfirmOpts {
        summary: &format!(
            "create to-do '{}' on {} {}?",
            args.name, taskable_type, args.on
        ),
        preview: &preview,
        dry_run: args.dry_run,
        yes: args.yes,
    };
    if confirm(&opts)? == Confirm::DryRun {
        return Ok(());
    }

    let todo = client.create_todo(&body).await?;
    eprintln!(
        "Created to-do (id={}) on {} {}",
        todo.id, taskable_type, args.on
    );
    print_todo_detail(&todo, format)
}

pub async fn edit(client: &AhaClient, args: &TodoEditArgs, format: OutputFormat) -> Result<()> {
    let body_text = if args.body.is_some() || args.body_file.is_some() {
        BodySource::from_flags(args.body.clone(), args.body_file.clone(), false, None)
            .map(|src| src.resolve())
            .transpose()?
    } else if args.editor {
        let existing = client.get_todo(&args.id).await?;
        BodySource::Editor {
            prefill: existing.body.clone(),
        }
        .resolve()
        .map(Some)?
    } else {
        None
    };

    let assignees = if args.assignee.is_empty() {
        None
    } else {
        Some(args.assignee.clone())
    };

    let body = TodoUpdate {
        name: args.name.as_deref(),
        body: body_text.as_deref(),
        status: args.status.map(map_status),
        due_date: args.due.as_deref(),
        assigned_to_users: assignees,
    };

    if body.name.is_none()
        && body.body.is_none()
        && body.status.is_none()
        && body.due_date.is_none()
        && body.assigned_to_users.is_none()
    {
        anyhow::bail!("nothing to update — pass at least one of --name / --body / --status / --due / --assignee");
    }

    let preview = dry_run_preview(
        "PUT",
        &format!("/tasks/{}", args.id),
        &serde_json::json!({ "task": &body }),
    );
    let opts = ConfirmOpts {
        summary: &format!("update to-do {}?", args.id),
        preview: &preview,
        dry_run: args.dry_run,
        yes: args.yes,
    };
    if confirm(&opts)? == Confirm::DryRun {
        return Ok(());
    }

    let todo = client.update_todo(&args.id, &body).await?;
    eprintln!("Updated to-do (id={})", todo.id);
    print_todo_detail(&todo, format)
}

pub async fn set_status(
    client: &AhaClient,
    id: &str,
    status: TodoStatus,
    dry_run: bool,
    yes: bool,
    format: OutputFormat,
) -> Result<()> {
    let body = TodoUpdate {
        name: None,
        body: None,
        status: Some(status),
        due_date: None,
        assigned_to_users: None,
    };
    let label = match status {
        TodoStatus::Completed => "complete",
        TodoStatus::Pending => "re-open",
    };
    let preview = dry_run_preview(
        "PUT",
        &format!("/tasks/{id}"),
        &serde_json::json!({ "task": &body }),
    );
    let opts = ConfirmOpts {
        summary: &format!("{label} to-do {id}?"),
        preview: &preview,
        dry_run,
        yes,
    };
    if confirm(&opts)? == Confirm::DryRun {
        return Ok(());
    }
    let _ = client.update_todo(id, &body).await?;

    // Empirically (probed 2026-05-21 via examples/probe_task_status.rs),
    // Aha!'s public API accepts the PUT with 200 OK but silently does not
    // persist task status changes. Re-GET to verify; if the state didn't
    // move, surface a clear error so users don't think the to-do is done
    // when it isn't. Same applies for `reopen` flipping back to pending.
    let verified = client.get_todo(id).await?;
    let expected = match status {
        TodoStatus::Completed => "complete",
        TodoStatus::Pending => "pending",
    };
    let actual = verified.status.as_deref().unwrap_or("(none)");
    if actual != expected {
        anyhow::bail!(
            "Aha! accepted the PUT but the to-do's status is still `{actual}` (expected `{expected}`). \
             This is a known Aha! API limitation: `PUT /tasks/<id>` silently no-ops `status`. \
             Use the Aha! web UI to flip the to-do."
        );
    }
    eprintln!(
        "To-do {} now {}",
        id,
        match status {
            TodoStatus::Completed => "completed",
            TodoStatus::Pending => "pending",
        }
    );
    print_todo_detail(&verified, format)
}

fn print_todo_detail(t: &Todo, format: OutputFormat) -> Result<()> {
    let mut kv: Vec<(&str, String)> = vec![
        ("id", t.id.clone()),
        ("name", t.name.clone()),
        ("status", t.status.clone().unwrap_or_else(|| "—".into())),
        (
            "due_date",
            t.due_date
                .map(|d| d.to_string())
                .unwrap_or_else(|| "—".into()),
        ),
    ];
    if let Some(url) = t.url.as_deref().filter(|u| !u.is_empty()) {
        kv.push(("url", url.to_string()));
    }
    render_one(format, &kv, t)
}

fn map_on_type(t: OnType) -> TaskableType {
    match t {
        OnType::Feature => TaskableType::Feature,
        OnType::Requirement => TaskableType::Requirement,
        OnType::Release => TaskableType::Release,
        OnType::Epic => TaskableType::Epic,
    }
}

fn map_status(s: TodoStatusArg) -> TodoStatus {
    match s {
        TodoStatusArg::Pending => TodoStatus::Pending,
        TodoStatusArg::Completed => TodoStatus::Completed,
    }
}

/// Reference-prefix heuristic. Aha! encodes parent type in the reference
/// itself: `TC-1234` (feature), `TC-R-12` (release), `TC-E-42` (epic),
/// `TC-1234-5` (requirement). Numeric ids and anything else return `None`
/// so the caller can demand `--on-type` explicitly.
pub(super) fn infer_taskable_type(reference: &str) -> Option<TaskableType> {
    let trimmed = reference.trim();
    let parts: Vec<&str> = trimmed.split('-').collect();
    match parts.as_slice() {
        // PREFIX-R-N → release
        [_, "R" | "r", n] if n.chars().all(|c| c.is_ascii_digit()) => Some(TaskableType::Release),
        // PREFIX-E-N → epic
        [_, "E" | "e", n] if n.chars().all(|c| c.is_ascii_digit()) => Some(TaskableType::Epic),
        // PREFIX-N-M → requirement
        [_, a, b]
            if a.chars().all(|c| c.is_ascii_digit()) && b.chars().all(|c| c.is_ascii_digit()) =>
        {
            Some(TaskableType::Requirement)
        }
        // PREFIX-N → feature
        [_, n] if n.chars().all(|c| c.is_ascii_digit()) => Some(TaskableType::Feature),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_feature_from_reference() {
        assert_eq!(infer_taskable_type("TC-1234"), Some(TaskableType::Feature));
    }

    #[test]
    fn infer_requirement_from_reference() {
        assert_eq!(
            infer_taskable_type("TC-1234-5"),
            Some(TaskableType::Requirement)
        );
    }

    #[test]
    fn infer_release_from_reference() {
        assert_eq!(infer_taskable_type("TC-R-12"), Some(TaskableType::Release));
    }

    #[test]
    fn infer_epic_from_reference() {
        assert_eq!(infer_taskable_type("TC-E-42"), Some(TaskableType::Epic));
    }

    #[test]
    fn infer_rejects_bare_numeric_id() {
        assert_eq!(infer_taskable_type("7626760672407598886"), None);
    }
}
