//! Probe Aha!'s accepted values for `task.status` on PUT /tasks/<id>.
//!
//! `cargo run --release --example probe_task_status -- <task-id>`

use aha_cli::auth::{self, Overrides};
use aha_cli::client::AhaClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let id = std::env::args()
        .nth(1)
        .expect("usage: probe_task_status <task-id>");
    let creds = auth::resolve(&Overrides {
        subdomain: std::env::var("AHA_COMPANY").ok(),
        token: std::env::var("AHA_TOKEN").ok(),
    })?;
    let client = AhaClient::new(&creds)?;
    let base = std::env::var("AHA_BASE_URL").unwrap_or_else(|_| creds.base_url());
    let url = format!("{}/tasks/{id}", base.trim_end_matches('/'));

    // Pass `--all` to fan out every candidate; default to just one PUT then
    // a GET so we can see whether the value stuck without subsequent probes
    // overwriting it.
    let all = std::env::args().any(|a| a == "--all");
    let target: Vec<&str> = if all {
        vec![
            "completed",
            "Completed",
            "complete",
            "Complete",
            "done",
            "Done",
            "closed",
            "Closed",
            "4",
            "3",
            "2",
            "1",
        ]
    } else {
        vec!["complete"]
    };
    let candidates = target;

    let http = reqwest::Client::builder()
        .user_agent("aha-cli-probe")
        .build()?;

    // Distinguish 200 OK that doesn't persist from a real update. Tag each
    // PUT with a unique name; after sending, GET and report whether the
    // server kept the name and the requested status.
    let bodies: Vec<(String, reqwest::Method, String, serde_json::Value)> = vec![
        // Sub-resource patterns sometimes used by Aha! for transitions.
        (
            "POST /tasks/:id/complete".into(),
            reqwest::Method::POST,
            format!("{url}/complete"),
            serde_json::json!({}),
        ),
        (
            "PUT /tasks/:id/complete".into(),
            reqwest::Method::PUT,
            format!("{url}/complete"),
            serde_json::json!({}),
        ),
        (
            "POST /tasks/:id/done".into(),
            reqwest::Method::POST,
            format!("{url}/done"),
            serde_json::json!({}),
        ),
        (
            "PATCH task.status=complete".into(),
            reqwest::Method::PATCH,
            url.clone(),
            serde_json::json!({"task": {"status": "complete"}}),
        ),
    ];
    for (label, method, target_url, body) in bodies {
        let resp = http
            .request(method, &target_url)
            .bearer_auth(&creds.token)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(&body)
            .send()
            .await?;
        let put_status = resp.status();
        let resp_text = resp.text().await.unwrap_or_default();
        let put_snippet: String = resp_text.chars().take(140).collect();

        let after: serde_json::Value = client.get_json_raw(&format!("/tasks/{id}")).await?;
        let live_status = after
            .pointer("/task/status")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "(missing)".into());
        println!("{label:<32} -> {put_status} live_status={live_status}");
        println!("  put_resp={put_snippet}");
    }
    // suppress unused warning on `candidates`
    drop(candidates);

    // Final read.
    println!("---- final GET ----");
    let raw: serde_json::Value = client.get_json_raw(&format!("/tasks/{id}")).await?;
    println!("{}", serde_json::to_string_pretty(&raw)?);

    Ok(())
}
