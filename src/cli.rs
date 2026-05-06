use std::io::{IsTerminal, Read};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};

use crate::auth::{self, netrc, Credentials, Overrides};
use crate::client::AhaClient;
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

    fn overrides(&self) -> Overrides {
        Overrides {
            subdomain: self.subdomain.clone(),
            token: self.token.clone(),
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
    /// Save credentials. Currently supports `--with-token`; the browser-based
    /// OAuth flow is wired in once the OAuth app is registered.
    Login(LoginArgs),
    /// Verify stored credentials are valid.
    Check,
    /// Print the authenticated user.
    Whoami,
    /// Remove stored credentials for a subdomain.
    Logout(LogoutArgs),
}

#[derive(Debug, Args)]
pub struct LoginArgs {
    /// Aha! subdomain (e.g. `tcare`). If omitted, falls back to `--subdomain`
    /// / `AHA_COMPANY`.
    #[arg(long)]
    pub subdomain: Option<String>,

    /// Read a personal API token from stdin and save it. Avoids putting the
    /// token on the command line. Required until the browser OAuth flow ships.
    #[arg(long)]
    pub with_token: bool,
}

#[derive(Debug, Args)]
pub struct LogoutArgs {
    /// Aha! subdomain to forget. If omitted, removes the active credentials
    /// resolved from `--subdomain` / `AHA_COMPANY` / netrc.
    #[arg(long)]
    pub subdomain: Option<String>,
}

pub async fn run() -> ExitCode {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    let result: Result<()> = match &cli.command {
        Command::Auth(cmd) => dispatch_auth(&cli, cmd).await,
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::from(1)
        }
    }
}

async fn dispatch_auth(cli: &Cli, cmd: &AuthCommand) -> Result<()> {
    match cmd {
        AuthCommand::Login(args) => auth_login(cli, args).await,
        AuthCommand::Check => auth_check(cli).await,
        AuthCommand::Whoami => auth_whoami(cli).await,
        AuthCommand::Logout(args) => auth_logout(cli, args),
    }
}

async fn auth_login(cli: &Cli, args: &LoginArgs) -> Result<()> {
    if !args.with_token {
        anyhow::bail!(
            "browser OAuth login is not wired up yet. \
             For now, generate a personal API token at \
             https://<subdomain>.aha.io/settings/personal/developer and run \
             `aha auth login --with-token --subdomain <name>`."
        );
    }

    let subdomain = args
        .subdomain
        .clone()
        .or_else(|| cli.subdomain.clone())
        .context("--subdomain (or AHA_COMPANY) is required")?;

    let token = read_token_from_stdin().context("reading token from stdin")?;
    if token.is_empty() {
        anyhow::bail!("empty token — pipe the token in: `printf '%s' \"$TOKEN\" | aha auth login --with-token --subdomain {subdomain}`");
    }

    // Verify the token before persisting.
    let creds = Credentials {
        subdomain: subdomain.clone(),
        token: token.clone(),
    };
    let client = AhaClient::new(&creds)?;
    let me = client
        .me()
        .await
        .context("verifying token against Aha! API")?;

    let path = netrc::default_path()?;
    netrc::upsert(
        &path,
        &netrc::Entry {
            host: creds.host(),
            login: "oauth".to_string(),
            password: token,
        },
    )?;

    println!("Saved credentials for {} as {}", me.email, creds.host());
    Ok(())
}

async fn auth_check(cli: &Cli) -> Result<()> {
    let creds = auth::resolve(&cli.overrides())?;
    let client = AhaClient::new(&creds)?;
    let me = client.me().await?;
    println!("OK — authenticated as {} <{}>", me.name, me.email);
    Ok(())
}

async fn auth_whoami(cli: &Cli) -> Result<()> {
    let creds = auth::resolve(&cli.overrides())?;
    let client = AhaClient::new(&creds)?;
    let me = client.me().await?;
    match cli.resolved_format() {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "id": me.id, "name": me.name, "email": me.email, "subdomain": creds.subdomain,
                }))?
            );
        }
        OutputFormat::Yaml => {
            println!(
                "id: {}\nname: {}\nemail: {}\nsubdomain: {}",
                me.id, me.name, me.email, creds.subdomain
            );
        }
        OutputFormat::Table => {
            println!("id        {}", me.id);
            println!("name      {}", me.name);
            println!("email     {}", me.email);
            println!("subdomain {}", creds.subdomain);
        }
    }
    Ok(())
}

fn auth_logout(cli: &Cli, args: &LogoutArgs) -> Result<()> {
    let subdomain = match args.subdomain.clone().or_else(|| cli.subdomain.clone()) {
        Some(s) => s,
        None => match auth::resolve(&cli.overrides()) {
            Ok(creds) => creds.subdomain,
            Err(_) => {
                println!("No credentials found — nothing to remove.");
                return Ok(());
            }
        },
    };
    let host = format!("{subdomain}.aha.io");
    netrc::remove(&netrc::default_path()?, &host)?;
    println!("Removed credentials for {host}");
    Ok(())
}

fn read_token_from_stdin() -> Result<String> {
    let mut stdin = std::io::stdin();
    if stdin.is_terminal() {
        anyhow::bail!("--with-token expects the token on stdin (e.g. `printf '%s' \"$TOKEN\" | aha auth login --with-token`)");
    }
    let mut buf = String::new();
    stdin.read_to_string(&mut buf)?;
    Ok(buf.trim().to_string())
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
