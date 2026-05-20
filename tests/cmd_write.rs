//! Cross-cutting safety rails for write commands: --dry-run, no-tty
//! safety, --body-file resolution, --editor via a fake `$EDITOR` script.

use std::fs;
use std::io::Write;

use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn write_creds(home: &std::path::Path) {
    fs::write(
        home.join(".netrc"),
        "machine tcare.aha.io login oauth password tok\n",
    )
    .unwrap();
}

#[tokio::test]
async fn write_without_yes_or_tty_bails() {
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let server = MockServer::start().await;
    // No mock registered — must not hit the wire.

    Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", server.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["features", "comment", "TC-1", "--body", "hello"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes"));
}

#[tokio::test]
async fn body_file_dash_reads_stdin() {
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/features/TC-1/comments"))
        .and(body_json(serde_json::json!({
            "comment": {"body": "piped body"}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "comment": {"id": "c1", "body": "piped body"}
        })))
        .mount(&server)
        .await;

    Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", server.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["features", "comment", "TC-1", "--body-file", "-", "--yes"])
        .write_stdin("piped body")
        .assert()
        .success();
}

#[tokio::test]
async fn body_file_reads_from_disk() {
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let server = MockServer::start().await;

    let scratch = tempfile::tempdir().unwrap();
    let body_file = scratch.path().join("body.md");
    let mut f = fs::File::create(&body_file).unwrap();
    writeln!(f, "from disk").unwrap();

    Mock::given(method("POST"))
        .and(path("/features/TC-1/comments"))
        .and(body_json(serde_json::json!({
            "comment": {"body": "from disk\n"}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "comment": {"id": "c1", "body": "from disk\n"}
        })))
        .mount(&server)
        .await;

    Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", server.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args([
            "features",
            "comment",
            "TC-1",
            "--body-file",
            body_file.to_str().unwrap(),
            "--yes",
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn editor_without_tty_errors_clearly() {
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let server = MockServer::start().await;

    Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", server.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["features", "comment", "TC-1", "--editor", "--yes"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--editor"));
}
