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
fn auth_help_lists_subcommands() {
    Command::cargo_bin("aha")
        .unwrap()
        .args(["auth", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("login"))
        .stdout(predicate::str::contains("check"))
        .stdout(predicate::str::contains("whoami"))
        .stdout(predicate::str::contains("logout"));
}
