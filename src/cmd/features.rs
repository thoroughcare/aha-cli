use anyhow::Result;

use crate::cli::{FeatureCommentArgs, FeatureCreateArgs, FeatureEditArgs};
use crate::client::models::Feature;
use crate::client::resources::{FeatureCreate, FeatureFilters, FeatureUpdate};
use crate::client::AhaClient;
use crate::cmd::write::{confirm, dry_run_preview, BodySource, Confirm, ConfirmOpts};
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

// ---------- Write surface ----------

pub async fn create(
    client: &AhaClient,
    args: &FeatureCreateArgs,
    format: OutputFormat,
) -> Result<()> {
    let description = BodySource::from_flags(
        args.description.clone(),
        args.description_file.clone(),
        args.editor,
        None,
    )
    .map(|src| src.resolve())
    .transpose()?;

    let tags = args.tags.as_deref().map(normalize_tag_csv);

    let body = FeatureCreate {
        name: &args.name,
        description: description.as_deref(),
        tags,
        assigned_to_user: args.assignee.as_deref(),
        workflow_status: args.status.as_deref(),
    };

    let preview = dry_run_preview(
        "POST",
        &format!("/products/{}/features", args.product),
        &serde_json::json!({ "feature": &body }),
    );
    let opts = ConfirmOpts {
        summary: &format!(
            "create feature '{}' in product {}?",
            args.name, args.product
        ),
        preview: &preview,
        dry_run: args.dry_run,
        yes: args.yes,
    };
    if confirm(&opts)? == Confirm::DryRun {
        return Ok(());
    }

    let feature = client.create_feature(&args.product, &body).await?;
    announce_created(&feature);
    print_feature_detail(&feature, format)
}

pub async fn edit(client: &AhaClient, args: &FeatureEditArgs, format: OutputFormat) -> Result<()> {
    // Tag merge: when --add-tag / --remove-tag are present, fetch the
    // existing feature to compute the union/difference, then send a full
    // replace. The API only supports replacement on `tags`.
    let merged_tags: Option<String> = if !args.add_tag.is_empty() || !args.remove_tag.is_empty() {
        let existing = client.get_feature(&args.id).await?;
        Some(merge_tags(&existing.tags, &args.add_tag, &args.remove_tag))
    } else {
        args.tags.as_deref().map(normalize_tag_csv)
    };

    // Editor default on edit: pre-fill with the existing description if no
    // explicit body flag was supplied.
    let description = if args.description.is_some() || args.description_file.is_some() {
        BodySource::from_flags(
            args.description.clone(),
            args.description_file.clone(),
            false,
            None,
        )
        .map(|src| src.resolve())
        .transpose()?
    } else if args.editor {
        let existing = client.get_feature(&args.id).await?;
        let prefill = existing.description.as_ref().map(|d| d.body.clone());
        BodySource::Editor { prefill }.resolve().map(Some)?
    } else {
        None
    };

    let body = FeatureUpdate {
        name: args.name.as_deref(),
        description: description.as_deref(),
        tags: merged_tags,
        assigned_to_user: args.assignee.as_deref(),
        workflow_status: args.status.as_deref(),
    };

    if is_feature_update_empty(&body) {
        anyhow::bail!("nothing to update — pass at least one of --name / --description / --tags / --assignee / --status");
    }

    let preview = dry_run_preview(
        "PUT",
        &format!("/features/{}", args.id),
        &serde_json::json!({ "feature": &body }),
    );
    let opts = ConfirmOpts {
        summary: &format!("update feature {}?", args.id),
        preview: &preview,
        dry_run: args.dry_run,
        yes: args.yes,
    };
    if confirm(&opts)? == Confirm::DryRun {
        return Ok(());
    }

    let feature = client.update_feature(&args.id, &body).await?;
    eprintln!(
        "Updated feature {} (id={})",
        feature.reference_num, feature.id
    );
    print_feature_detail(&feature, format)
}

pub async fn comment(
    client: &AhaClient,
    args: &FeatureCommentArgs,
    format: OutputFormat,
) -> Result<()> {
    let source =
        BodySource::from_flags(args.body.clone(), args.body_file.clone(), args.editor, None)
            .ok_or_else(|| anyhow::anyhow!("one of --body / --body-file / --editor is required"))?;
    let body = source.resolve()?;
    if body.trim().is_empty() {
        anyhow::bail!("aborting: empty body");
    }

    let preview = dry_run_preview(
        "POST",
        &format!("/features/{}/comments", args.id),
        &serde_json::json!({ "comment": { "body": &body } }),
    );
    let opts = ConfirmOpts {
        summary: &format!("comment on feature {}?", args.id),
        preview: &preview,
        dry_run: args.dry_run,
        yes: args.yes,
    };
    if confirm(&opts)? == Confirm::DryRun {
        return Ok(());
    }

    let created = client.create_feature_comment(&args.id, &body).await?;
    eprintln!("Posted comment (id={}) on feature {}", created.id, args.id);
    render_one(
        format,
        &[
            ("id", created.id.clone()),
            (
                "author",
                created
                    .user
                    .as_ref()
                    .map(|u| u.email.clone().unwrap_or_else(|| u.name.clone()))
                    .unwrap_or_else(|| "—".into()),
            ),
            (
                "created_at",
                created
                    .created_at
                    .map(|t| t.to_rfc3339())
                    .unwrap_or_else(|| "—".into()),
            ),
        ],
        &created,
    )
}

fn announce_created(f: &Feature) {
    eprintln!(
        "Created feature {} (id={})",
        if f.reference_num.is_empty() {
            "(no reference yet)".to_string()
        } else {
            f.reference_num.clone()
        },
        f.id,
    );
}

fn print_feature_detail(f: &Feature, format: OutputFormat) -> Result<()> {
    let kv: Vec<(&str, String)> = vec![
        ("id", f.id.clone()),
        ("reference_num", f.reference_num.clone()),
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
            "tags",
            if f.tags.is_empty() {
                "—".into()
            } else {
                f.tags.join(", ")
            },
        ),
    ];
    render_one(format, &kv, f)
}

/// Trim whitespace around each comma-separated tag. Aha! accepts the comma
/// form verbatim, but users naturally write `"a, b, c"` and we shouldn't
/// silently send `"a"`, `" b"`, `" c"` as three distinct tags.
pub(super) fn normalize_tag_csv(s: &str) -> String {
    s.split(',')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(",")
}

/// Compute the post-edit tag set client-side: start from `existing`, add
/// `adds`, drop `removes`. Preserves the order of the existing tags so a
/// dry-run preview is stable.
pub(super) fn merge_tags(existing: &[String], adds: &[String], removes: &[String]) -> String {
    let mut set: Vec<String> = existing.to_vec();
    for tag in removes {
        set.retain(|t| t != tag);
    }
    for tag in adds {
        if !set.iter().any(|t| t == tag) {
            set.push(tag.clone());
        }
    }
    set.join(",")
}

fn is_feature_update_empty(b: &FeatureUpdate<'_>) -> bool {
    b.name.is_none()
        && b.description.is_none()
        && b.tags.is_none()
        && b.assigned_to_user.is_none()
        && b.workflow_status.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| (*x).to_string()).collect()
    }

    #[test]
    fn merge_tags_adds_new_and_preserves_existing_order() {
        let out = merge_tags(&v(&["alpha", "beta"]), &v(&["gamma"]), &[]);
        assert_eq!(out, "alpha,beta,gamma");
    }

    #[test]
    fn merge_tags_removes_then_adds() {
        let out = merge_tags(
            &v(&["alpha", "beta", "gamma"]),
            &v(&["delta"]),
            &v(&["beta"]),
        );
        assert_eq!(out, "alpha,gamma,delta");
    }

    #[test]
    fn merge_tags_skips_duplicate_adds() {
        let out = merge_tags(&v(&["alpha"]), &v(&["alpha", "beta"]), &[]);
        assert_eq!(out, "alpha,beta");
    }

    #[test]
    fn normalize_tag_csv_trims_whitespace() {
        assert_eq!(normalize_tag_csv("a, b ,c"), "a,b,c");
    }

    #[test]
    fn normalize_tag_csv_drops_empty_entries() {
        assert_eq!(normalize_tag_csv("a,,b,"), "a,b");
    }
}
