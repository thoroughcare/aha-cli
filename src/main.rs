use std::process::ExitCode;

use aha_cli::cli;

#[tokio::main]
async fn main() -> ExitCode {
    cli::run().await
}
