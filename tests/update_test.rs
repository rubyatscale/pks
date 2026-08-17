use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use serial_test::serial;
use std::{error::Error, path::Path};
mod common;
use pretty_assertions::assert_eq;

#[test]
#[serial]
// This and the next test are run in serial because they both use the same fixtures.
fn update() -> Result<(), Box<dyn Error>> {
    test_update("update")
}

#[test]
#[serial]
fn update_todo() -> Result<(), Box<dyn Error>> {
    test_update("update-todo")
}

fn test_update(command: &str) -> Result<(), Box<dyn Error>> {
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--debug")
        .arg(command)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Successfully updated package_todo.yml files!",
        ));

    let package_todo_yml_filepath =
        Path::new("tests/fixtures/simple_app/packs/foo/package_todo.yml");
    let actual = std::fs::read_to_string(package_todo_yml_filepath)?;
    let expected = String::from(
        "\
# This file contains a list of dependencies that are not part of the long term plan for the
# 'packs/foo' package.
# We should generally work to reduce this list over time.
#
# You can regenerate this file using the following command:
#
# bin/packwerk update-todo
---
packs/bar:
  \"::Bar\":
    violations:
    - dependency
    - privacy
    files:
    - packs/foo/app/services/foo.rb
",
    );
    std::fs::remove_file(package_todo_yml_filepath)?;
    assert_eq!(expected, actual);

    common::teardown();

    Ok(())
}

#[test]
#[serial]
fn test_update_with_experimental_parser() -> Result<(), Box<dyn Error>> {
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--debug")
        .arg("--experimental-parser")
        .arg("update")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Successfully updated package_todo.yml files!",
        ));

    let package_todo_yml_filepath =
        Path::new("tests/fixtures/simple_app/packs/foo/package_todo.yml");
    let actual = std::fs::read_to_string(package_todo_yml_filepath)?;
    let expected = String::from(
        "\
# This file contains a list of dependencies that are not part of the long term plan for the
# 'packs/foo' package.
# We should generally work to reduce this list over time.
#
# You can regenerate this file using the following command:
#
# bin/packwerk update-todo
---
packs/bar:
  \"::Bar\":
    violations:
    - dependency
    - privacy
    files:
    - packs/foo/app/services/foo.rb
",
    );
    std::fs::remove_file(package_todo_yml_filepath)?;
    assert_eq!(expected, actual);

    common::teardown();

    Ok(())
}

#[test]
fn test_update_with_stale_violations() -> Result<(), Box<dyn Error>> {
    common::set_up_fixtures();

    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/contains_stale_violations")
        .arg("update")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Successfully updated package_todo.yml files!",
        ));

    let package_todo_yml_filepath = Path::new(
        "tests/fixtures/contains_stale_violations/packs/foo/package_todo.yml",
    );
    let actual = std::fs::read_to_string(package_todo_yml_filepath)?;
    let expected = String::from(
        "\
# This file contains a list of dependencies that are not part of the long term plan for the
# 'packs/foo' package.
# We should generally work to reduce this list over time.
#
# You can regenerate this file using the following command:
#
# bin/packwerk update-todo
---
packs/bar:
  \"::Bar\":
    violations:
    - privacy
    files:
    - packs/foo/app/services/foo.rb
",
    );

    assert_eq!(expected, actual);

    let package_todo_yml_filepath = Path::new(
        "tests/fixtures/contains_stale_violations/packs/bar/package_todo.yml",
    );
    assert!(!package_todo_yml_filepath.exists());
    common::set_up_fixtures();

    Ok(())
}

#[test]
fn test_update_with_packs_first_app() -> Result<(), Box<dyn Error>> {
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/simple_packs_first_app")
        .arg("update")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Successfully updated package_todo.yml files!",
        ));

    let package_todo_yml_filepath = Path::new(
        "tests/fixtures/simple_packs_first_app/packs/foo/package_todo.yml",
    );
    let actual = std::fs::read_to_string(package_todo_yml_filepath)?;
    let expected = String::from(
        "\
# This file contains a list of dependencies that are not part of the long term plan for the
# 'packs/foo' package.
# We should generally work to reduce this list over time.
#
# You can regenerate this file using the following command:
#
# pks update
---
packs/bar:
  \"::Bar\":
    violations:
    - dependency
    - privacy
    files:
    - packs/foo/app/services/foo.rb
",
    );
    std::fs::remove_file(package_todo_yml_filepath)?;
    assert_eq!(expected, actual);

    common::teardown();

    Ok(())
}

#[test]
#[serial]
// This and the round-trip test below both mutate
// tests/fixtures/uses_strict_mode_round_trip, so they run in serial and each
// restores the fixture on the way out.
fn test_update_preserves_recorded_strict_violations() -> anyhow::Result<()> {
    common::set_up_uses_strict_mode_round_trip_fixture();

    let path = Path::new(
        "tests/fixtures/uses_strict_mode_round_trip/packs/foo/package_todo.yml",
    );

    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/uses_strict_mode_round_trip")
        .arg("update")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Successfully updated package_todo.yml files!",
        ))
        // The violation is recorded, so `check` tolerates it. Claiming it must
        // be fixed for `check` to succeed would be false.
        .stdout(
            predicate::str::contains(
                "These violations must be fixed for `check` to succeed.",
            )
            .not(),
        );

    assert!(
        path.exists(),
        "update must not delete the todo file that grandfathers a recorded strict violation"
    );
    let contents = std::fs::read_to_string(path)?;
    assert!(
        contents.contains("\"::Bar\""),
        "the recorded strict violation must survive update, got:\n{}",
        contents
    );

    common::set_up_uses_strict_mode_round_trip_fixture();
    Ok(())
}

#[test]
#[serial]
fn test_check_update_check_round_trip_with_strict_mode() -> anyhow::Result<()> {
    common::set_up_uses_strict_mode_round_trip_fixture();

    let assert_check_is_clean = || {
        cargo_bin_cmd!("pks")
            .arg("--project-root")
            .arg("tests/fixtures/uses_strict_mode_round_trip")
            .arg("check")
            .assert()
            .code(0)
            .stdout(predicate::str::contains("No violations detected!"));
    };

    // A routine `update` between two checks must not turn a green build red.
    assert_check_is_clean();
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/uses_strict_mode_round_trip")
        .arg("update")
        .assert()
        .success();
    assert_check_is_clean();

    common::set_up_uses_strict_mode_round_trip_fixture();
    Ok(())
}

#[test]
fn test_update_with_strict_violations() -> anyhow::Result<()> {
    let path = Path::new(
        "tests/fixtures/contains_strict_violations/packs/foo/package_todo.yml",
    );
    let _ignore = std::fs::remove_file(path);

    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/contains_strict_violations")
        .arg("update")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "packs/foo cannot have privacy violations on packs/bar because strict mode is enabled for privacy violations in the enforcing pack's package.yml file",
        ))
        .stdout(predicate::str::contains("1 strict mode violation(s) detected."))
        .stdout(predicate::str::contains(
            "Successfully updated package_todo.yml files!",
        ));

    assert!(
        !path.exists(),
        "todo should not be created for strict violations"
    );
    Ok(())
}
