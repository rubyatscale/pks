use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{error::Error, fs, process::Command};
mod common;

fn assert_check_unused_dependencies(cmd: &str) -> Result<(), Box<dyn Error>> {
    Command::cargo_bin("pks")?
        .arg("--project-root")
        .arg("tests/fixtures/app_with_dependency_cycles")
        .arg("--debug")
        .arg(&cmd)
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "packs/bar depends on packs/foo but does not use it",
        ))
        .stdout(predicate::str::contains(
            "packs/foo depends on packs/bar but does not use it",
        ))
        .stderr(predicate::str::contains(
           format!("Error: Found 3 unnecessary dependencies. Run `packs {}| --auto-correct` to remove them.", &cmd),
        ));
    Ok(())
}

#[test]
fn test_check_unnecessary_dependencies() -> Result<(), Box<dyn Error>> {
    assert_check_unused_dependencies("check-unnecessary-dependencies")
}

#[test]
fn test_check_unused_dependencies() -> Result<(), Box<dyn Error>> {
    assert_check_unused_dependencies("check-unused-dependencies")
}


fn assert_auto_correct_unused_dependencies(cmd: &str, flag: &str) -> Result<(), Box<dyn Error>> {
    Command::cargo_bin("pks")?
        .arg("--project-root")
        .arg("tests/fixtures/app_with_unnecessary_dependencies")
        .arg("--debug")
        .arg(&cmd)
        .arg(&flag)
        .assert()
        .success();

    let expected_autocorrect = [
        "enforce_dependencies: true",
        "enforce_privacy: true",
        "layer: technical_services",
        "dependencies:",
        "- packs/bar\n",
    ]
    .join("\n");
    let after_autocorrect = fs::read_to_string("tests/fixtures/app_with_unnecessary_dependencies/packs/foo/package.yml").unwrap();
    assert_eq!(after_autocorrect, expected_autocorrect);

    Ok(())
}

#[test]
fn test_auto_correct_unnecessary_dependencies() -> Result<(), Box<dyn Error>> {
    assert_auto_correct_unused_dependencies("check-unused-dependencies", "--auto-correct")?;
    assert_auto_correct_unused_dependencies("check-unused-dependencies", "-a")?;
    assert_auto_correct_unused_dependencies("check-unnecessary-dependencies", "-a")?;
    assert_auto_correct_unused_dependencies("check-unnecessary-dependencies", "--auto-correct")
}

#[test]
fn test_check_unnecessary_dependencies_no_issue() -> Result<(), Box<dyn Error>>
{
    Command::cargo_bin("pks")?
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--debug")
        .arg("check-unused-dependencies")
        .assert()
        .success();
    Ok(())
}
