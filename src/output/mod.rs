use std::io::IsTerminal;

use serde::Serialize;
use tabled::settings::object::Rows;
use tabled::settings::{Modify, Style, Width};
use tabled::{Table, Tabled};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Table,
    Json,
    Yaml,
}

impl OutputFormat {
    /// Pick a default based on whether stdout is attached to a terminal.
    /// Mirrors `gh`: humans see a table, pipes/agents see JSON.
    pub fn auto() -> Self {
        if std::io::stdout().is_terminal() {
            Self::Table
        } else {
            Self::Json
        }
    }
}

/// Render a list of items per the requested output format. `rows` is the
/// table-friendly projection (one row per item); `data` is the structured
/// projection used for JSON/YAML.
pub fn render_list<R, D>(format: OutputFormat, rows: &[R], data: &D) -> anyhow::Result<()>
where
    R: Tabled,
    D: Serialize,
{
    match format {
        OutputFormat::Table => {
            if rows.is_empty() {
                println!("(no results)");
                return Ok(());
            }
            let mut table = Table::new(rows);
            table
                .with(Style::sharp())
                .with(Modify::new(Rows::new(..)).with(Width::wrap(80).keep_words(true)));
            println!("{table}");
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(data)?);
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(data)?);
        }
    }
    Ok(())
}

/// Render a single item as either a kv-detail table or structured output.
pub fn render_one<D>(
    format: OutputFormat,
    kv_pairs: &[(&str, String)],
    data: &D,
) -> anyhow::Result<()>
where
    D: Serialize,
{
    match format {
        OutputFormat::Table => {
            let max_key_len = kv_pairs.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
            for (k, v) in kv_pairs {
                println!("{k:<width$}  {v}", width = max_key_len);
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(data)?);
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(data)?);
        }
    }
    Ok(())
}
