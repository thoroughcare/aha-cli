use std::fs;

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
async fn features_show_deep_emits_full_payload_as_json() {
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/features/TC-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "feature": {
                "id": "100",
                "reference_num": "TC-1",
                "name": "Add browse view",
                "workflow_status": {"id":"s1","name":"In progress","complete":false}
            }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/features/TC-1/requirements"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "requirements": [{"id":"r1","reference_num":"TC-1-1","name":"Req A"}],
            "pagination": {"current_page":1,"total_pages":1,"total_records":1}
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/features/TC-1/comments"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "comments": [],
            "pagination": {"current_page":1,"total_pages":1,"total_records":0}
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/features/TC-1/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tasks": [{"id":"t1","name":"Write tests","status":"pending"}],
            "pagination": {"current_page":1,"total_pages":1,"total_records":1}
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/tasks/t1/comments"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "comments": [],
            "pagination": {"current_page":1,"total_pages":1,"total_records":0}
        })))
        .mount(&server)
        .await;

    let out = Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", server.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["features", "show", "TC-1"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8(out.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("output should be JSON");
    assert_eq!(parsed["feature"]["reference_num"], "TC-1");
    assert_eq!(parsed["requirements"][0]["reference_num"], "TC-1-1");
    assert_eq!(parsed["todos"][0]["todo"]["name"], "Write tests");
}

#[tokio::test]
async fn features_list_walks_paginated_response() {
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let server = MockServer::start().await;

    let mk = |refnum: &str, name: &str| serde_json::json!({"id": refnum, "reference_num": refnum, "name": name});
    let page1 = serde_json::json!({
        "features": (1..=200).map(|i| mk(&format!("TC-{i}"), &format!("F{i}"))).collect::<Vec<_>>(),
        "pagination": {"current_page":1,"total_pages":2,"total_records":250}
    });
    let page2 = serde_json::json!({
        "features": (201..=250).map(|i| mk(&format!("TC-{i}"), &format!("F{i}"))).collect::<Vec<_>>(),
        "pagination": {"current_page":2,"total_pages":2,"total_records":250}
    });

    Mock::given(method("GET"))
        .and(path("/features"))
        .and(wiremock::matchers::query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page1))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/features"))
        .and(wiremock::matchers::query_param("page", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page2))
        .mount(&server)
        .await;

    let out = Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", server.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["features", "list"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 250);
    assert_eq!(arr[0]["reference_num"], "TC-1");
    assert_eq!(arr[249]["reference_num"], "TC-250");
}

#[tokio::test]
async fn create_feature_posts_envelope_and_announces_on_stderr() {
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/products/TC/features"))
        .and(body_json(serde_json::json!({
            "feature": {"name": "Add browse view"}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "feature": {
                "id": "100",
                "reference_num": "TC-42",
                "name": "Add browse view"
            }
        })))
        .mount(&server)
        .await;

    let out = Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", server.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args([
            "features",
            "create",
            "--product",
            "TC",
            "--name",
            "Add browse view",
            "--yes",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Created feature TC-42"),
        "stderr missing announce line: {stderr}"
    );
}

#[tokio::test]
async fn edit_feature_dry_run_makes_no_request() {
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let server = MockServer::start().await;

    // No mock registered → wiremock would error on unexpected calls.
    let out = Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", server.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["features", "edit", "TC-1", "--name", "Renamed", "--dry-run"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("dry-run"),
        "missing dry-run preview: {stdout}"
    );
    assert!(
        stdout.contains("PUT /features/TC-1"),
        "missing path: {stdout}"
    );
    assert!(
        stdout.contains("\"name\": \"Renamed\""),
        "missing name in body: {stdout}"
    );
}

#[tokio::test]
async fn edit_feature_with_add_tag_does_get_then_put() {
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let server = MockServer::start().await;

    // GET fetches existing tags.
    Mock::given(method("GET"))
        .and(path("/features/TC-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "feature": {
                "id": "100",
                "reference_num": "TC-1",
                "name": "feat",
                "tags": ["alpha", "beta"]
            }
        })))
        .expect(1)
        .mount(&server)
        .await;
    // PUT receives the merged tag list.
    Mock::given(method("PUT"))
        .and(path("/features/TC-1"))
        .and(body_json(serde_json::json!({
            "feature": {"tags": "alpha,beta,gamma"}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "feature": {"id": "100", "reference_num": "TC-1", "name": "feat", "tags": ["alpha","beta","gamma"]}
        })))
        .expect(1)
        .mount(&server)
        .await;

    Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", server.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["features", "edit", "TC-1", "--add-tag", "gamma", "--yes"])
        .assert()
        .success();
}

#[tokio::test]
async fn comment_on_feature_posts_envelope_with_body() {
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/features/TC-1/comments"))
        .and(body_json(serde_json::json!({
            "comment": {"body": "looks good"}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "comment": {"id": "c1", "body": "looks good"}
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
            "--body",
            "looks good",
            "--yes",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Posted comment"));
}

#[tokio::test]
async fn comment_surfaces_422_validation_error() {
    let home = tempfile::tempdir().unwrap();
    write_creds(home.path());
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/features/TC-1/comments"))
        .respond_with(ResponseTemplate::new(422).set_body_string("body too short"))
        .mount(&server)
        .await;

    Command::cargo_bin("aha")
        .unwrap()
        .env("HOME", home.path())
        .env("AHA_BASE_URL", server.uri())
        .env_remove("AHA_TOKEN")
        .env_remove("AHA_COMPANY")
        .args(["features", "comment", "TC-1", "--body", "x", "--yes"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("422"))
        .stderr(predicate::str::contains("body too short"));
}
