//! `aha attachments download <id>` — fetch an attachment by id, stream
//! the bytes to a file (default: `<file_name>` in CWD) or stdout.
//!
//! Live behavior against `tcare.aha.io` is mixed: some attachments (e.g.
//! comment images) download cleanly via the bearer-derived flow, others
//! return HTTP 500 / `/access_denied` for reasons that aren't documented.
//! On failure the Aha error body is bubbled up so the user can decide
//! whether to retry, paste the `download_url` into a browser, or move on.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tokio::io::AsyncWriteExt;

use crate::client::models::Attachment;
use crate::client::AhaClient;
use crate::output::OutputFormat;

pub enum Output {
    /// Resolve from the server-provided `file_name`. Refuses to overwrite
    /// without `force`.
    Default { force: bool },
    /// Explicit filesystem path. Refuses to overwrite without `force`.
    Path { path: PathBuf, force: bool },
    /// Write to stdout (`-o -`).
    Stdout,
}

pub async fn download(
    client: &AhaClient,
    id: &str,
    output: Output,
    format: OutputFormat,
) -> Result<()> {
    match output {
        Output::Stdout => {
            let mut out = tokio::io::stdout();
            let meta = client.download_attachment(id, &mut out).await?;
            out.flush().await.ok();
            // For JSON/YAML mode, surface the metadata on stderr so it
            // doesn't corrupt the binary on stdout.
            if matches!(format, OutputFormat::Json | OutputFormat::Yaml) {
                eprintln!("{}", summary(&meta, format)?);
            }
        }
        Output::Default { force } => {
            let meta = client.get_attachment(id).await?;
            let path = PathBuf::from(if meta.file_name.is_empty() {
                format!("aha-attachment-{id}")
            } else {
                meta.file_name.clone()
            });
            write_to_path(client, id, &path, force, format).await?;
        }
        Output::Path { path, force } => {
            write_to_path(client, id, &path, force, format).await?;
        }
    }
    Ok(())
}

async fn write_to_path(
    client: &AhaClient,
    id: &str,
    path: &Path,
    force: bool,
    format: OutputFormat,
) -> Result<()> {
    if path.exists() && !force {
        anyhow::bail!(
            "{} already exists. Pass --force to overwrite.",
            path.display()
        );
    }
    let mut file = tokio::fs::File::create(path)
        .await
        .with_context(|| format!("creating {}", path.display()))?;
    let meta = client.download_attachment(id, &mut file).await?;
    file.flush().await.ok();
    drop(file);

    match format {
        OutputFormat::Table => {
            println!(
                "Wrote {} ({}) to {}",
                meta.file_name,
                human_size(meta.file_size),
                path.display()
            );
        }
        _ => println!("{}", summary(&meta, format)?),
    }
    Ok(())
}

fn summary(meta: &Attachment, format: OutputFormat) -> Result<String> {
    match format {
        OutputFormat::Yaml => Ok(serde_yaml::to_string(meta)?),
        _ => Ok(serde_json::to_string_pretty(meta)?),
    }
}

fn human_size(bytes: Option<u64>) -> String {
    let Some(b) = bytes else {
        return "unknown size".into();
    };
    const UNITS: &[(&str, u64)] = &[
        ("GB", 1024 * 1024 * 1024),
        ("MB", 1024 * 1024),
        ("KB", 1024),
    ];
    for (suffix, divisor) in UNITS {
        if b >= *divisor {
            return format!("{:.1} {suffix}", b as f64 / *divisor as f64);
        }
    }
    format!("{b} B")
}
