use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
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
async fn todos_show_surfaces_body_and_attachments() {
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/tasks/t1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "task": {
                "id": "t1",
                "name": "Investigate bug",
                "status": "pending",
                "body": "Reproduce with the attached log.",
                "attachments": [{
                    "id": "att1",
                    "file_name": "trace.log",
                    "download_url": "https://example.com/files/att1",
                    "content_type": "text/plain",
                    "file_size": 9001
                }]
            }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/tasks/t1/comments"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "comments": [{
                "id": "c1",
                "body": "follow-up",
                "user": {"id": "u1", "name": "Reviewer", "email": "rev@example.com"},
                "created_at": "2026-04-12T10:00:00Z",
                "attachments": [{
                    "id": "att2",
                    "file_name": "diff.patch",
                    "content_type": "text/x-diff",
                    "file_size": 256
                }]
            }],
            "pagination": {"current_page":1,"total_pages":1,"total_records":1}
        })))
        .mount(&server)
        .await;

    // JSON path — assert the full payload survives.
    let out = Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", server.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["todos", "show", "t1"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let parsed: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(parsed["todo"]["body"], "Reproduce with the attached log.");
    assert_eq!(parsed["todo"]["attachments"][0]["file_name"], "trace.log");
    assert_eq!(
        parsed["comments"][0]["attachments"][0]["file_name"],
        "diff.patch"
    );
    assert_eq!(parsed["comments"][0]["user"]["email"], "rev@example.com");

    // Table mode — body, attachments, comments all visible.
    Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", server.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["--no-json", "todos", "show", "t1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("body:"))
        .stdout(predicate::str::contains("Reproduce with the attached log."))
        .stdout(predicate::str::contains("attachments:"))
        .stdout(predicate::str::contains("trace.log"))
        .stdout(predicate::str::contains("comments: 1 entries"))
        .stdout(predicate::str::contains("diff.patch"));
}
