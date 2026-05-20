use anyhow::Result;

use crate::cli::{RequirementCommentArgs, RequirementEditArgs};
use crate::client::models::Requirement;
use crate::client::resources::RequirementUpdate;
use crate::client::AhaClient;
use crate::cmd::write::{confirm, dry_run_preview, BodySource, Confirm, ConfirmOpts};
use crate::output::{render_one, OutputFormat};

use super::status_label;

pub async fn show(client: &AhaClient, id: &str, format: OutputFormat) -> Result<()> {
    let r = client.get_requirement(id).await?;
    render_one(format, &kv_for(&r), &r)
}

pub async fn edit(
    client: &AhaClient,
    args: &RequirementEditArgs,
    format: OutputFormat,
) -> Result<()> {
    // Editor default on edit: pre-fill with existing description so users
    // don't accidentally clobber a long acceptance-criteria block.
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
        let existing = client.get_requirement(&args.id).await?;
        let prefill = existing.description.as_ref().map(|d| d.body.clone());
        BodySource::Editor { prefill }.resolve().map(Some)?
    } else {
        None
    };

    let body = RequirementUpdate {
        name: args.name.as_deref(),
        description: description.as_deref(),
        workflow_status: args.status.as_deref(),
        assigned_to_user: args.assignee.as_deref(),
    };

    if body.name.is_none()
        && body.description.is_none()
        && body.workflow_status.is_none()
        && body.assigned_to_user.is_none()
    {
        anyhow::bail!("nothing to update — pass at least one of --name / --description / --status / --assignee");
    }

    let preview = dry_run_preview(
        "PUT",
        &format!("/requirements/{}", args.id),
        &serde_json::json!({ "requirement": &body }),
    );
    let opts = ConfirmOpts {
        summary: &format!("update requirement {}?", args.id),
        preview: &preview,
        dry_run: args.dry_run,
        yes: args.yes,
    };
    if confirm(&opts)? == Confirm::DryRun {
        return Ok(());
    }

    let updated = client.update_requirement(&args.id, &body).await?;
    eprintln!(
        "Updated requirement {} (id={})",
        updated.reference_num, updated.id
    );
    render_one(format, &kv_for(&updated), &updated)
}

pub async fn comment(
    client: &AhaClient,
    args: &RequirementCommentArgs,
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
        &format!("/requirements/{}/comments", args.id),
        &serde_json::json!({ "comment": { "body": &body } }),
    );
    let opts = ConfirmOpts {
        summary: &format!("comment on requirement {}?", args.id),
        preview: &preview,
        dry_run: args.dry_run,
        yes: args.yes,
    };
    if confirm(&opts)? == Confirm::DryRun {
        return Ok(());
    }

    let created = client.create_requirement_comment(&args.id, &body).await?;
    eprintln!(
        "Posted comment (id={}) on requirement {}",
        created.id, args.id
    );
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
        ],
        &created,
    )
}

fn kv_for(r: &Requirement) -> Vec<(&'static str, String)> {
    vec![
        ("id", r.id.clone()),
        ("reference_num", r.reference_num.clone()),
        ("name", r.name.clone()),
        ("status", status_label(&r.workflow_status)),
    ]
}
