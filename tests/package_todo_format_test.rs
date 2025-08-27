use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{error::Error, process::Command};

mod common;

#[test]
fn test_validate_incorrectly_formatted_package_todo() -> Result<(), Box<dyn Error>> {
    Command::cargo_bin("pks")
        .unwrap()
        .arg("--project-root")
        .arg("tests/fixtures/incorrectly_formatted_package_todo")
        .arg("validate")
        .assert()
        .failure()
        .stdout(predicate::str::contains("1 validation error(s) detected:"))
        .stdout(predicate::str::contains("is not in the expected format"))
        .stdout(predicate::str::contains("bin/packwerk update-todo"));

    common::teardown();
    Ok(())
}

#[test]
fn test_validate_correctly_formatted_package_todo() -> Result<(), Box<dyn Error>> {
    // This test uses an existing fixture that should have correctly formatted package_todo.yml
    Command::cargo_bin("pks")
        .unwrap()
        .arg("--project-root")
        .arg("tests/fixtures/contains_package_todo")
        .arg("validate")
        .assert()
        .success()
        .stdout(predicate::str::contains("Packwerk validate succeeded!"));

    common::teardown();
    Ok(())
}

