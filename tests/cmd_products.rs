use std::fs;

use assert_cmd::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn write_creds(home: &std::path::Path) {
    fs::write(
        home.join(".netrc"),
        "machine tcare.aha.io login oauth password tok\n",
    )
    .unwrap();
}

#[tokio::test]
async fn products_list_emits_json_when_piped() {
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/products"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [
                {"id":"1","reference_prefix":"TC","name":"Roadmap"},
                {"id":"2","reference_prefix":"X","name":"X-Product"}
            ],
            "pagination": {"current_page":1,"total_pages":1,"total_records":2}
        })))
        .mount(&server)
        .await;

    let out = Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", server.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["products", "list"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8(out.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("output should be JSON");
    assert_eq!(parsed[0]["name"], "Roadmap");
    assert_eq!(parsed[1]["reference_prefix"], "X");
}

#[tokio::test]
async fn products_list_renders_table_when_forced() {
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/products"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [{"id":"1","reference_prefix":"TC","name":"Roadmap"}],
            "pagination": {"current_page":1,"total_pages":1,"total_records":1}
        })))
        .mount(&server)
        .await;

    let out = Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", server.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["--no-json", "products", "list"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Roadmap"));
    assert!(stdout.contains("PREFIX"));
    assert!(stdout.contains("ID"));
    // Not JSON.
    assert!(serde_json::from_str::<serde_json::Value>(&stdout).is_err());
}
