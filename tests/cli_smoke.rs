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
fn completions_zsh_emits_compdef() {
    Command::cargo_bin("aha")
        .unwrap()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("#compdef aha"));
}

#[test]
fn completions_bash_emits_function() {
    Command::cargo_bin("aha")
        .unwrap()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("_aha"));
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
