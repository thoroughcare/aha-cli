//! Shared helpers for write / edit commands: body resolution, TTY-aware
//! confirmation, and dry-run preview. Every mutating command flows through
//! the same gate so the safety contract (no surprise writes from pipes /
//! agent shells) holds everywhere.

use std::io::{IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

/// One of three mutually-exclusive sources for a long-form text body.
/// Mirrors `gh issue create --body / --body-file / --editor`.
#[derive(Debug, Clone)]
pub enum BodySource {
    Inline(String),
    File(PathBuf),
    /// Open `$EDITOR` on a tempfile. The optional string pre-fills the
    /// buffer (used on edit-style commands so users see the existing body).
    Editor {
        prefill: Option<String>,
    },
}

impl BodySource {
    /// Build a `BodySource` from a mutually-exclusive trio of CLI options.
    /// `clap`'s `conflicts_with_all` enforces the mutex at parse time; this
    /// helper picks the right variant or returns `None` when nothing was
    /// supplied so callers can substitute a default.
    pub fn from_flags(
        inline: Option<String>,
        file: Option<PathBuf>,
        editor: bool,
        editor_prefill: Option<String>,
    ) -> Option<Self> {
        if let Some(s) = inline {
            Some(BodySource::Inline(s))
        } else if let Some(p) = file {
            Some(BodySource::File(p))
        } else if editor {
            Some(BodySource::Editor {
                prefill: editor_prefill,
            })
        } else {
            None
        }
    }

    /// Resolve to the final body string. Returns an error rather than an
    /// empty string when the editor flow produces no content — matches
    /// `git commit`'s empty-message rule.
    pub fn resolve(self) -> Result<String> {
        match self {
            BodySource::Inline(s) => Ok(s),
            BodySource::File(path) => {
                if path.as_os_str() == "-" {
                    let mut buf = String::new();
                    std::io::stdin()
                        .read_to_string(&mut buf)
                        .context("reading body from stdin")?;
                    Ok(buf)
                } else {
                    std::fs::read_to_string(&path)
                        .with_context(|| format!("reading body from {}", path.display()))
                }
            }
            BodySource::Editor { prefill } => run_editor(prefill.as_deref()),
        }
    }
}

/// Spawn `$EDITOR` on a tempfile, optionally pre-filled, and return the
/// final contents. Bails if the result is empty after trim (mirrors
/// `git commit`'s empty-message rule) or if there's no TTY for the editor
/// to attach to.
fn run_editor(prefill: Option<&str>) -> Result<String> {
    if !std::io::stdin().is_terminal() {
        anyhow::bail!(
            "--editor needs an interactive terminal — pass --body / --body-file on non-TTY shells"
        );
    }

    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());

    let tmp = tempfile::Builder::new()
        .prefix("aha-edit-")
        .suffix(".md")
        .tempfile()
        .context("creating tempfile for editor")?;
    let path = tmp.path().to_path_buf();
    if let Some(text) = prefill {
        std::fs::write(&path, text)
            .with_context(|| format!("writing prefill to {}", path.display()))?;
    }

    // Split the editor command on whitespace so things like `code --wait`
    // work without requiring a shell.
    let mut parts = editor.split_whitespace();
    let program = parts.next().context("EDITOR variable is empty")?;
    let extra_args: Vec<&str> = parts.collect();
    let status = Command::new(program)
        .args(&extra_args)
        .arg(&path)
        .status()
        .with_context(|| format!("launching editor `{editor}`"))?;
    if !status.success() {
        anyhow::bail!("editor `{editor}` exited with {status}");
    }

    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("reading edited file {}", path.display()))?;
    if contents.trim().is_empty() {
        anyhow::bail!("aborting: empty body");
    }
    Ok(contents)
}

/// Confirmation / dry-run gate. Every write command should call this
/// before sending bytes to the API.
///
/// Returns:
/// - `Ok(Confirm::Proceed)` if the user explicitly accepted or `--yes`
/// - `Ok(Confirm::DryRun)` if `--dry-run` was set
/// - `Err` if the prompt was declined or no TTY is available and `--yes`
///   was not passed
pub fn confirm(opts: &ConfirmOpts<'_>) -> Result<Confirm> {
    if opts.dry_run {
        println!("dry-run: {}", opts.preview);
        return Ok(Confirm::DryRun);
    }
    if opts.yes {
        return Ok(Confirm::Proceed);
    }

    let stdout_tty = std::io::stdout().is_terminal();
    let stdin_tty = std::io::stdin().is_terminal();
    if !(stdout_tty || stdin_tty) {
        anyhow::bail!(
            "{summary}: refusing to write from a non-TTY shell without --yes. \
             Re-run with --yes to confirm, or --dry-run to preview.",
            summary = opts.summary
        );
    }

    let mut stderr = std::io::stderr();
    write!(stderr, "{} [y/N] ", opts.summary).ok();
    stderr.flush().ok();
    let mut buf = String::new();
    std::io::stdin()
        .read_line(&mut buf)
        .context("reading confirmation")?;
    let answer = buf.trim().to_ascii_lowercase();
    if answer == "y" || answer == "yes" {
        Ok(Confirm::Proceed)
    } else {
        anyhow::bail!("aborted");
    }
}

#[derive(Debug)]
pub struct ConfirmOpts<'a> {
    pub summary: &'a str,
    /// What to print when `--dry-run` is set. Typically `"<METHOD> <path>"`
    /// followed by a pretty-printed JSON body.
    pub preview: &'a str,
    pub dry_run: bool,
    pub yes: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Confirm {
    Proceed,
    DryRun,
}

/// Convenience: render a method + path + body for `--dry-run` preview.
pub fn dry_run_preview<B: serde::Serialize>(method: &str, path: &str, body: &B) -> String {
    let json = serde_json::to_string_pretty(body)
        .unwrap_or_else(|e| format!("(failed to render body: {e})"));
    format!("{method} {path}\n{json}")
}
