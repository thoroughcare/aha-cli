//! Integration tests for `aha auth ...` commands.
//!
//! Each test isolates `$HOME` to a tempdir so the real `~/.netrc` is never
//! read or written, and points the CLI at a `wiremock::MockServer` via the
//! `AHA_BASE_URL` env override.

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn me_response() -> serde_json::Value {
    serde_json::json!({
        "user": {
            "id": "7626760672407598886",
            "name": "Test User",
            "email": "test-user@example.com",
        }
    })
}

fn netrc_path(home: &std::path::Path) -> std::path::PathBuf {
    home.join(".netrc")
}

#[tokio::test]
async fn auth_check_reports_authenticated_user() {
    let home = tempfile::tempdir().unwrap();
    fs::write(
        netrc_path(home.path()),
        "machine tcare.aha.io\n  login oauth\n  password testtoken\n",
    )
    .unwrap();

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/me"))
        .and(header("authorization", "Bearer testtoken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(me_response()))
        .mount(&server)
        .await;

    Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", server.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["auth", "check"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Test User"))
        .stdout(predicate::str::contains("test-user@example.com"));
}

#[tokio::test]
async fn auth_check_fails_without_credentials() {
    let home = tempfile::tempdir().unwrap();

    Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["auth", "check"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("auth login"));
}

#[tokio::test]
async fn auth_check_surfaces_401_from_api() {
    let home = tempfile::tempdir().unwrap();
    fs::write(
        netrc_path(home.path()),
        "machine tcare.aha.io\n  login oauth\n  password badtoken\n",
    )
    .unwrap();

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/me"))
        .respond_with(ResponseTemplate::new(401).set_body_string("nope"))
        .mount(&server)
        .await;

    Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", server.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["auth", "check"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("401"));
}

#[tokio::test]
async fn auth_login_with_token_persists_to_netrc() {
    let home = tempfile::tempdir().unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/me"))
        .and(header("authorization", "Bearer freshly-pasted-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(me_response()))
        .mount(&server)
        .await;

    Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", server.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["auth", "login", "--with-token", "--subdomain", "tcare"])
        .write_stdin("freshly-pasted-token\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Saved credentials"));

    let saved = fs::read_to_string(netrc_path(home.path())).unwrap();
    assert!(saved.contains("machine tcare.aha.io"));
    assert!(saved.contains("password freshly-pasted-token"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(netrc_path(home.path()))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }
}

#[tokio::test]
async fn auth_login_with_token_rejects_bad_token() {
    let home = tempfile::tempdir().unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/me"))
        .respond_with(ResponseTemplate::new(401).set_body_string("nope"))
        .mount(&server)
        .await;

    Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", server.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["auth", "login", "--with-token", "--subdomain", "tcare"])
        .write_stdin("bad-token\n")
        .assert()
        .failure();

    // Nothing written when verification fails.
    assert!(!netrc_path(home.path()).exists());
}

#[tokio::test]
async fn auth_logout_removes_entry() {
    let home = tempfile::tempdir().unwrap();
    fs::write(
        netrc_path(home.path()),
        "machine other.example login a password b\n\
         machine tcare.aha.io login oauth password tok\n",
    )
    .unwrap();

    Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["auth", "logout", "--subdomain", "tcare"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed credentials"));

    let after = fs::read_to_string(netrc_path(home.path())).unwrap();
    assert!(!after.contains("tcare.aha.io"));
    assert!(after.contains("other.example"));
}

#[tokio::test]
async fn auth_whoami_json_when_piped() {
    let home = tempfile::tempdir().unwrap();
    fs::write(
        netrc_path(home.path()),
        "machine tcare.aha.io login oauth password tok\n",
    )
    .unwrap();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(me_response()))
        .mount(&server)
        .await;

    let out = Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", server.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["auth", "whoami"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8(out.stdout).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("piped output should be JSON");
    assert_eq!(parsed["email"], "test-user@example.com");
    assert_eq!(parsed["id"], "7626760672407598886");
    assert_eq!(parsed["subdomain"], "tcare");
}
