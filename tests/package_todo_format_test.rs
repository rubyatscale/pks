//! Tests for package_todo.yml format validation.
//!
//! These tests verify that the `pks validate` command correctly identifies
//! when package_todo.yml files are not in the expected serialization format
//! and provides appropriate error messages and suggestions.

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{error::Error, process::Command};

mod common;

/// Tests that validation fails for incorrectly formatted package_todo.yml files.
///
/// This test uses a fixture with a package_todo.yml file that has violations in
/// the wrong order (::Baz should come after ::Bar when sorted alphabetically).
/// The validation should fail and suggest running the appropriate update command.
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

/// Tests that validation passes for correctly formatted package_todo.yml files.
///
/// This test uses an existing fixture that has a properly formatted package_todo.yml
/// file (with correct ordering, headers, and format). The validation should succeed.
#[test]
fn test_validate_correctly_formatted_package_todo() -> Result<(), Box<dyn Error>> {
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

