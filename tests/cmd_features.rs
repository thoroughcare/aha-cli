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
