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
async fn backlog_groups_features_by_release_and_epic() {
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/features"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "features": [
                {
                    "id":"1","reference_num":"TC-1","name":"Build foo",
                    "workflow_status":{"id":"s1","name":"In progress","complete":false},
                    "release":{"id":"r1","reference_num":"TC-R-1","name":"R1"},
                    "epic":{"id":"e1","reference_num":"TC-E-1","name":"E1"}
                },
                {
                    "id":"2","reference_num":"TC-2","name":"Done bar",
                    "workflow_status":{"id":"s2","name":"Shipped","complete":true},
                    "release":{"id":"r1","reference_num":"TC-R-1","name":"R1"},
                    "epic":{"id":"e1","reference_num":"TC-E-1","name":"E1"}
                },
                {
                    "id":"3","reference_num":"TC-3","name":"Stray",
                    "workflow_status":{"id":"s1","name":"Open","complete":false}
                }
            ],
            "pagination":{"current_page":1,"total_pages":1,"total_records":3}
        })))
        .mount(&server)
        .await;

    let out = Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", server.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["backlog"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let releases = parsed["releases"].as_array().unwrap();
    assert_eq!(releases.len(), 2);
    // BTreeMap orders strings; "(no release)" sorts before "TC-R-1".
    assert_eq!(releases[0]["release_ref"], "(no release)");
    assert_eq!(releases[1]["release_ref"], "TC-R-1");
    let epic = &releases[1]["epics"][0];
    assert_eq!(epic["epic_ref"], "TC-E-1");
    assert_eq!(epic["features"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn backlog_table_mode_prints_release_headers() {
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/features"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "features": [
                {
                    "id":"1","reference_num":"TC-1","name":"Build foo",
                    "workflow_status":{"id":"s1","name":"In progress","complete":false},
                    "release":{"id":"r1","reference_num":"TC-R-1","name":"R1"},
                    "epic":{"id":"e1","reference_num":"TC-E-1","name":"E1"}
                }
            ],
            "pagination":{"current_page":1,"total_pages":1,"total_records":1}
        })))
        .mount(&server)
        .await;

    let out = Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", server.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["--no-json", "backlog"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Release: TC-R-1"));
    assert!(stdout.contains("Epic: TC-E-1"));
    assert!(stdout.contains("Build foo"));
    assert!(stdout.contains("In progress"));
}
