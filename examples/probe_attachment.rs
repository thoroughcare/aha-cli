//! Dump the raw JSON Aha! returns for an attachment, plus the headers of
//! HEAD requests against the download_url with various URL tweaks.
//!
//! `cargo run --release --example probe_attachment -- <attachment-id>`

use aha_cli::auth::{self, Overrides};
use aha_cli::client::AhaClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let id = std::env::args()
        .nth(1)
        .expect("usage: probe_attachment <attachment-id>");
    let creds = auth::resolve(&Overrides::default())?;
    let client = AhaClient::new(&creds)?;
    let base = std::env::var("AHA_BASE_URL").unwrap_or_else(|_| creds.base_url());

    // Fetch metadata via the existing typed API path, then re-serialize.
    let url = format!("{}/attachments/{id}", base.trim_end_matches('/'));
    println!("---- GET {url} ----");
    let raw: serde_json::Value = client.get_json_raw(&format!("/attachments/{id}")).await?;
    println!("{}", serde_json::to_string_pretty(&raw)?);

    let dl = raw
        .get("attachment")
        .and_then(|a| a.get("download_url"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let Some(dl) = dl else {
        println!("(no download_url in response)");
        return Ok(());
    };

    let probe = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    for tweak in [
        "(as-is)",
        "(no size param)",
        "(size=large)",
        "(size=medium)",
        "(size=thumbnail)",
    ] {
        let url = match tweak {
            "(no size param)" => dl.split('?').next().unwrap_or(&dl).to_string(),
            "(size=large)" => dl.replace("size=original", "size=large"),
            "(size=medium)" => dl.replace("size=original", "size=medium"),
            "(size=thumbnail)" => dl.replace("size=original", "size=thumbnail"),
            _ => dl.clone(),
        };
        let r = probe.get(&url).send().await?;
        let loc = r
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("(none)");
        println!("\nGET {tweak} -> {} (location: {loc})", r.status());
    }
    Ok(())
}
