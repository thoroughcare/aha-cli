//! Integration tests for `aha attachments download`.
//!
//! Two wiremock servers in each test: one acts as the Aha! API
//! (`/api/v1/attachments/:id` returns metadata pointing at the second),
//! the second hosts the raw bytes — emulating the real-world separation
//! between the API and the file CDN.
//!
//! Note: against the real `tcare.aha.io` the `download_url` rejects API
//! token auth (it expects a browser session cookie), so the live binary
//! errors with HTTP 500 / `/access_denied`. These tests cover the wiring;
//! they will pass once Aha! starts honoring the API token, or if/when we
//! switch to a different download path.

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const BYTES: &[u8] = b"\x89PNG\r\n\x1a\n\x00fake-png-bytes\x00";

fn write_creds(home: &std::path::Path) {
    fs::write(
        home.join(".netrc"),
        "machine tcare.aha.io login oauth password tok\n",
    )
    .unwrap();
}

async fn setup_servers(file_name: &str, content_type: &str) -> (MockServer, MockServer) {
    let cdn = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/files/att123/blob"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(BYTES.to_vec())
                .insert_header("content-type", content_type),
        )
        .mount(&cdn)
        .await;

    let api = MockServer::start().await;
    let download_url = format!("{}/files/att123/blob", cdn.uri());
    Mock::given(method("GET"))
        .and(path("/attachments/att123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachment": {
                "id": "att123",
                "file_name": file_name,
                "download_url": download_url,
                "content_type": content_type,
                "file_size": BYTES.len() as u64
            }
        })))
        .mount(&api)
        .await;
    (api, cdn)
}

#[tokio::test]
async fn download_writes_default_filename_in_cwd() {
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let cwd = tempfile::tempdir().unwrap();
    let (api, _cdn) = setup_servers("screenshot.png", "image/png").await;

    Command::cargo_bin("aha")
        .unwrap()
        .current_dir(cwd.path())
        .env("HOME", home.path())
        .env("AHA_BASE_URL", api.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["--no-json", "attachments", "download", "att123"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Wrote screenshot.png"));

    let written = fs::read(cwd.path().join("screenshot.png")).unwrap();
    assert_eq!(written, BYTES);
}

#[tokio::test]
async fn download_to_explicit_path() {
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let cwd = tempfile::tempdir().unwrap();
    let (api, _cdn) = setup_servers("screenshot.png", "image/png").await;
    let out_path = cwd.path().join("custom-name.bin");

    Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", api.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args([
            "--no-json",
            "attachments",
            "download",
            "att123",
            "-o",
            out_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let written = fs::read(&out_path).unwrap();
    assert_eq!(written, BYTES);
}

#[tokio::test]
async fn download_to_stdout_emits_raw_bytes() {
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let (api, _cdn) = setup_servers("trace.log", "text/plain").await;

    let out = Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", api.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["attachments", "download", "att123", "-o", "-"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, BYTES);
}

#[tokio::test]
async fn download_refuses_to_overwrite_without_force() {
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let cwd = tempfile::tempdir().unwrap();
    let (api, _cdn) = setup_servers("doc.pdf", "application/pdf").await;
    fs::write(cwd.path().join("doc.pdf"), b"original-contents").unwrap();

    Command::cargo_bin("aha")
        .unwrap()
        .current_dir(cwd.path())
        .env("HOME", home.path())
        .env("AHA_BASE_URL", api.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["attachments", "download", "att123"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));

    // Original file untouched.
    let after = fs::read(cwd.path().join("doc.pdf")).unwrap();
    assert_eq!(after, b"original-contents");
}

#[tokio::test]
async fn download_overwrites_with_force() {
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let cwd = tempfile::tempdir().unwrap();
    let (api, _cdn) = setup_servers("doc.pdf", "application/pdf").await;
    fs::write(cwd.path().join("doc.pdf"), b"original-contents").unwrap();

    Command::cargo_bin("aha")
        .unwrap()
        .current_dir(cwd.path())
        .env("HOME", home.path())
        .env("AHA_BASE_URL", api.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["attachments", "download", "att123", "--force"])
        .assert()
        .success();

    let after = fs::read(cwd.path().join("doc.pdf")).unwrap();
    assert_eq!(after, BYTES);
}

#[tokio::test]
async fn download_surfaces_gated_attachment_clearly() {
    // Aha! 302s gated attachments to /access_denied; without disabling
    // redirects we either chase to a 500 page or loop infinitely with
    // bearer. The download client must catch the 302 itself and report it.
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let cwd = tempfile::tempdir().unwrap();

    let api = MockServer::start().await;
    let download_url = format!("{}/files/att123/blob", api.uri());
    Mock::given(method("GET"))
        .and(path("/attachments/att123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachment": {
                "id": "att123",
                "file_name": "secret.pdf",
                "download_url": download_url,
                "content_type": "application/pdf",
                "file_size": null
            }
        })))
        .mount(&api)
        .await;
    Mock::given(method("GET"))
        .and(path("/files/att123/blob"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("location", "/attachments/att123/access_denied"),
        )
        .mount(&api)
        .await;

    Command::cargo_bin("aha")
        .unwrap()
        .current_dir(cwd.path())
        .env("HOME", home.path())
        .env("AHA_BASE_URL", api.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["attachments", "download", "att123"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("gated by Aha!"))
        .stderr(predicate::str::contains("access_denied"));

    // No file written.
    assert!(!cwd.path().join("secret.pdf").exists());
}

#[tokio::test]
async fn download_metadata_to_json_when_piped() {
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let cwd = tempfile::tempdir().unwrap();
    let (api, _cdn) = setup_servers("note.md", "text/markdown").await;

    let out = Command::cargo_bin("aha")
        .unwrap()
        .current_dir(cwd.path())
        .env("HOME", home.path())
        .env("AHA_BASE_URL", api.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["attachments", "download", "att123"])
        .output()
        .unwrap();
    assert!(out.status.success());
    // Piped → JSON metadata on stdout, file written to disk.
    let stdout = String::from_utf8(out.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("JSON");
    assert_eq!(parsed["file_name"], "note.md");
    assert_eq!(parsed["content_type"], "text/markdown");

    let written = fs::read(cwd.path().join("note.md")).unwrap();
    assert_eq!(written, BYTES);
}
