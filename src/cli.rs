use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::output::OutputFormat;

/// Command-line client for the Aha! API.
#[derive(Debug, Parser)]
#[command(name = "aha", version, about, long_about = None)]
pub struct Cli {
    /// Override the Aha! subdomain (e.g. `tcare`). Falls back to `AHA_COMPANY`
    /// env var or the entry stored by `aha auth login`.
    #[arg(long, global = true, env = "AHA_COMPANY")]
    pub subdomain: Option<String>,

    /// Override the Aha! API token. Rarely needed; prefer `aha auth login`.
    #[arg(long, global = true, env = "AHA_TOKEN", hide_env_values = true)]
    pub token: Option<String>,

    /// Force JSON output (default when stdout is not a TTY).
    #[arg(long, global = true, conflicts_with_all = ["yaml", "no_json"])]
    pub json: bool,

    /// Force human-readable tables (default when stdout is a TTY).
    #[arg(long, global = true, conflicts_with_all = ["json", "yaml"])]
    pub no_json: bool,

    /// Force YAML output.
    #[arg(long, global = true, conflicts_with_all = ["json", "no_json"])]
    pub yaml: bool,

    /// Disable color output (also honors NO_COLOR env var).
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Increase log verbosity. -v info, -vv debug, -vvv trace.
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    /// Resolve the user's requested output format, accounting for TTY detection.
    pub fn resolved_format(&self) -> OutputFormat {
        if self.json {
            OutputFormat::Json
        } else if self.yaml {
            OutputFormat::Yaml
        } else if self.no_json {
            OutputFormat::Table
        } else {
            OutputFormat::auto()
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Authenticate to Aha! and manage credentials.
    #[command(subcommand)]
    Auth(AuthCommand),
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Verify stored credentials are valid.
    Check,
}

pub async fn run() -> ExitCode {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    let result: anyhow::Result<()> = match &cli.command {
        Command::Auth(AuthCommand::Check) => {
            // Phase 0: stub. Implemented in Phase 0.5.
            eprintln!("auth check: not implemented yet");
            Ok(())
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::from(1)
        }
    }
}

fn init_tracing(verbose: u8) {
    use tracing_subscriber::{fmt, EnvFilter};
    let default_level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("aha_cli={default_level}")));
    let _ = fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}
