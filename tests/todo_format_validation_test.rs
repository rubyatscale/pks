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
fn test_validate_incorrectly_formatted_package_todo(
) -> Result<(), Box<dyn Error>> {
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
fn test_validate_correctly_formatted_package_todo() -> Result<(), Box<dyn Error>>
{
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

/// Tests that validation passes when violations exist but no package_todo.yml files.
///
/// This test uses a fixture with actual privacy violations but no package_todo.yml files
/// and verifies that the todo format validator still succeeds (it only validates existing
/// todo files, not whether violations should have todo files).
#[test]
fn test_validate_passes_when_violations_exist_but_no_todo_file(
) -> Result<(), Box<dyn Error>> {
    Command::cargo_bin("pks")
        .unwrap()
        .arg("--project-root")
        .arg("tests/fixtures/privacy_violation_overrides")
        .arg("validate")
        .assert()
        .success()
        .stdout(predicate::str::contains("Packwerk validate succeeded!"));

    common::teardown();
    Ok(())
}

/// Tests that validation succeeds when there are no violations and no package_todo.yml files.
///
/// This test uses a clean fixture with no violations and no package_todo.yml files
/// to verify the validator succeeds in the simplest case.
#[test]
fn test_validate_succeeds_when_no_violations_and_no_todo_files(
) -> Result<(), Box<dyn Error>> {
    Command::cargo_bin("pks")
        .unwrap()
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("validate")
        .assert()
        .success()
        .stdout(predicate::str::contains("Packwerk validate succeeded!"));

    common::teardown();
    Ok(())
}
