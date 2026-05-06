use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn help_lists_auth_subcommand() {
    Command::cargo_bin("aha")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("auth"));
}

#[test]
fn version_flag_prints_version() {
    Command::cargo_bin("aha")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("aha"));
}

#[test]
fn auth_check_stub_runs() {
    Command::cargo_bin("aha")
        .unwrap()
        .arg("auth")
        .arg("check")
        .assert()
        .success()
        .stderr(predicate::str::contains("not implemented yet"));
}
