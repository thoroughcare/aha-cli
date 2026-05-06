use std::io::IsTerminal;

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
