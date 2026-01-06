use assert_cmd::cargo::cargo_bin_cmd;
use jsonschema::Validator;
use predicates::prelude::*;
use std::{error::Error, fs};

mod common;

fn validate_check_output_schema(json_value: &serde_json::Value) {
    let schema_str = fs::read_to_string("schema/check-output.json")
        .expect("Failed to read schema file");
    let schema: serde_json::Value =
        serde_json::from_str(&schema_str).expect("Schema should be valid JSON");
    let validator =
        Validator::new(&schema).expect("Schema should be valid JSON Schema");
    if !validator.is_valid(json_value) {
        let errors: Vec<String> = validator
            .iter_errors(json_value)
            .map(|e| format!("  - {}", e))
            .collect();
        panic!("JSON output does not match schema:\n{}", errors.join("\n"));
    }
}

pub fn stripped_output(output: Vec<u8>) -> String {
    String::from_utf8_lossy(&strip_ansi_escapes::strip(output)).to_string()
}

#[test]
fn test_check_with_privacy_dependency_error_template_overrides(
) -> Result<(), Box<dyn Error>> {
    let output = cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/privacy_violation_overrides")
        .arg("--debug")
        .arg("check")
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let stripped_output = stripped_output(output);

    assert!(stripped_output.contains("2 violation(s) detected:"));
    assert!(stripped_output.contains("packs/foo/app/services/foo.rb:3:4\nDependency violation: `::Bar` belongs to `packs/bar`, but `packs/foo/package.yml` does not specify a dependency on `packs/bar`. See https://go/pks-dependency"));
    assert!(stripped_output.contains("packs/foo/app/services/foo.rb:3:4\nPrivacy violation: `::Bar` is private to `packs/bar`, but referenced from `packs/foo`. See https://go/pks-privacy"));

    common::teardown();
    Ok(())
}
#[test]
fn test_check() -> Result<(), Box<dyn Error>> {
    let output = cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--debug")
        .arg("check")
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let stripped_output = stripped_output(output);

    assert!(stripped_output.contains("2 violation(s) detected:"));
    assert!(stripped_output.contains("packs/foo/app/services/foo.rb:3:4\nDependency violation: `::Bar` belongs to `packs/bar`, but `packs/foo/package.yml` does not specify a dependency on `packs/bar`."));
    assert!(stripped_output.contains("packs/foo/app/services/foo.rb:3:4\nPrivacy violation: `::Bar` is private to `packs/bar`, but referenced from `packs/foo`"));

    common::teardown();
    Ok(())
}

#[test]
fn test_check_enforce_privacy_disabled() -> Result<(), Box<dyn Error>> {
    let output = cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--debug")
        .arg("--disable-enforce-privacy")
        .arg("check")
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let stripped_output = stripped_output(output);

    assert!(stripped_output.contains("1 violation(s) detected:"));
    assert!(stripped_output.contains("packs/foo/app/services/foo.rb:3:4\nDependency violation: `::Bar` belongs to `packs/bar`, but `packs/foo/package.yml` does not specify a dependency on `packs/bar`."));

    common::teardown();
    Ok(())
}

#[test]
fn test_check_enforce_dependency_disabled() -> Result<(), Box<dyn Error>> {
    let output = cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--debug")
        .arg("--disable-enforce-dependencies")
        .arg("check")
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let stripped_output = stripped_output(output);

    assert!(stripped_output.contains("1 violation(s) detected:"));
    assert!(stripped_output.contains("packs/foo/app/services/foo.rb:3:4\nPrivacy violation: `::Bar` is private to `packs/bar`, but referenced from `packs/foo`"));

    common::teardown();
    Ok(())
}

#[test]
fn test_check_with_single_file() -> Result<(), Box<dyn Error>> {
    let output = cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--debug")
        .arg("check")
        .arg("packs/foo/app/services/foo.rb")
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let stripped_output = stripped_output(output);

    assert!(stripped_output.contains("2 violation(s) detected:"));
    assert!(stripped_output.contains("packs/foo/app/services/foo.rb:3:4\nDependency violation: `::Bar` belongs to `packs/bar`, but `packs/foo/package.yml` does not specify a dependency on `packs/bar`."));
    assert!(stripped_output.contains("packs/foo/app/services/foo.rb:3:4\nPrivacy violation: `::Bar` is private to `packs/bar`, but referenced from `packs/foo`"));

    common::teardown();
    Ok(())
}

#[test]
fn test_check_with_single_file_experimental_parser(
) -> Result<(), Box<dyn Error>> {
    let output = cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--debug")
        .arg("--experimental-parser")
        .arg("check")
        .arg("packs/foo/app/services/foo.rb")
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let stripped_output = stripped_output(output);

    assert!(stripped_output.contains("2 violation(s) detected:"));
    assert!(stripped_output.contains("packs/foo/app/services/foo.rb:3:4\nDependency violation: `::Bar` belongs to `packs/bar`, but `packs/foo/package.yml` does not specify a dependency on `packs/bar`."));
    assert!(stripped_output.contains("packs/foo/app/services/foo.rb:3:4\nPrivacy violation: `::Bar` is private to `packs/bar`, but referenced from `packs/foo`"));

    common::teardown();
    Ok(())
}

#[test]
fn test_check_with_package_todo_file() -> Result<(), Box<dyn Error>> {
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/contains_package_todo")
        .arg("--debug")
        .arg("check")
        .assert()
        .code(0)
        .stdout(predicate::str::contains("No violations detected!"));

    common::teardown();

    Ok(())
}

#[test]
fn test_check_with_package_todo_file_csv() -> Result<(), Box<dyn Error>> {
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/contains_package_todo")
        .arg("--debug")
        .arg("check")
        .arg("-o")
        .arg("csv")
        .assert()
        .code(0)
        .stdout(predicate::str::contains("Violation,Strict?,File,Constant,Referencing Pack,Defining Pack,Message"))
        .stdout(predicate::str::contains("No violations detected!"));

    common::teardown();

    Ok(())
}

#[test]
fn test_check_with_package_todo_file_ignoring_recorded_violations(
) -> Result<(), Box<dyn Error>> {
    let output = cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/contains_package_todo")
        .arg("--debug")
        .arg("check")
        .arg("--ignore-recorded-violations")
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let stripped_output = stripped_output(output);
    assert!(stripped_output.contains("2 violation(s) detected:"));
    assert!(stripped_output.contains("packs/foo/app/services/foo.rb:3:4\nDependency violation: `::Bar` belongs to `packs/bar`, but `packs/foo/package.yml` does not specify a dependency on `packs/bar`."));
    assert!(stripped_output.contains("packs/foo/app/services/other_foo.rb:3:4\nDependency violation: `::Bar` belongs to `packs/bar`, but `packs/foo/package.yml` does not specify a dependency on `packs/bar`."));

    common::teardown();

    Ok(())
}

#[test]
fn test_check_with_experimental_parser() -> Result<(), Box<dyn Error>> {
    let output = cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--experimental-parser")
        .arg("--debug")
        .arg("check")
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let stripped_output = stripped_output(output);

    assert!(stripped_output.contains("2 violation(s) detected:"));
    assert!(stripped_output.contains("packs/foo/app/services/foo.rb:3:4\nDependency violation: `::Bar` belongs to `packs/bar`, but `packs/foo/package.yml` does not specify a dependency on `packs/bar`."));
    assert!(stripped_output.contains("packs/foo/app/services/foo.rb:3:4\nPrivacy violation: `::Bar` is private to `packs/bar`, but referenced from `packs/foo`"));

    common::teardown();
    Ok(())
}

#[test]
fn test_check_with_stale_violations() -> Result<(), Box<dyn Error>> {
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/contains_stale_violations")
        .arg("check")
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "There were stale violations found, please run `packs update`",
        ));

    common::teardown();
    Ok(())
}

#[test]
fn test_check_with_stale_violations_when_file_no_longer_exists(
) -> Result<(), Box<dyn Error>> {
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/contains_stale_violations_no_file")
        .arg("check")
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "There were stale violations found, please run `packs update`",
        ));

    common::teardown();
    Ok(())
}

#[test]
fn test_check_with_relationship_violations() -> Result<(), Box<dyn Error>> {
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/app_with_rails_relationships")
        .arg("check")
        .assert()
        .code(1)
        .stdout(predicate::str::contains("2 violation(s) detected:"))
        .stdout(predicate::str::contains("Privacy violation: `::Taco` is private to `packs/baz`, but referenced from `packs/bar`"))
        .stdout(predicate::str::contains("Privacy violation: `::Census` is private to `packs/baz`, but referenced from `packs/bar`"));

    common::teardown();
    Ok(())
}

#[test]
fn test_check_without_stale_violations() -> Result<(), Box<dyn Error>> {
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/contains_package_todo")
        .arg("check")
        .assert()
        .code(0)
        .stdout(
            predicate::str::contains(
                "There were stale violations found, please run `packs update`",
            )
            .not(),
        );

    common::teardown();
    Ok(())
}

#[test]
fn test_check_with_strict_mode() -> Result<(), Box<dyn Error>> {
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/uses_strict_mode")
        .arg("check")
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "packs/foo cannot have privacy violations on packs/bar because strict mode is enabled for privacy violations in the enforcing pack's package.yml file",
        ))
        .stdout(predicate::str::contains(
            "packs/foo cannot have dependency violations on packs/bar because strict mode is enabled for dependency violations in the enforcing pack's package.yml file",
        ));

    common::teardown();
    Ok(())
}

#[test]
fn test_check_with_strict_mode_output_csv() -> Result<(), Box<dyn Error>> {
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/uses_strict_mode")
        .arg("check")
        .arg("-o")
        .arg("csv")
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Violation,Strict?,File,Constant,Referencing Pack,Defining Pack,Message"))
        .stdout(predicate::str::contains("privacy,true,packs/foo/app/services/foo.rb,::Bar,packs/foo,packs/bar,packs/foo cannot have privacy violations on packs/bar because strict mode is enabled for privacy violations in the enforcing pack\'s package.yml file"))
        .stdout(predicate::str::contains(
            "privacy,true,packs/foo/app/services/foo.rb,::Bar,packs/foo,packs/bar,packs/foo cannot have privacy violations on packs/bar because strict mode is enabled for privacy violations in the enforcing pack\'s package.yml file",
        ));

    common::teardown();
    Ok(())
}

#[test]
fn test_check_contents() -> Result<(), Box<dyn Error>> {
    let project_root = "tests/fixtures/simple_app";
    let relative_path = "packs/foo/app/services/foo.rb";
    let foo_rb_contents =
        fs::read_to_string(format!("{}/{}", project_root, relative_path))?;

    let output = cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg(project_root)
        .arg("--debug")
        .arg("check-contents")
        .arg(relative_path)
        .write_stdin(format!("\n\n\n{}", foo_rb_contents))
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let stripped_output = stripped_output(output);

    assert!(stripped_output.contains("2 violation(s) detected:"));
    assert!(stripped_output.contains("packs/foo/app/services/foo.rb:6:4\nDependency violation: `::Bar` belongs to `packs/bar`, but `packs/foo/package.yml` does not specify a dependency on `packs/bar`."));
    assert!(stripped_output.contains("packs/foo/app/services/foo.rb:6:4\nPrivacy violation: `::Bar` is private to `packs/bar`, but referenced from `packs/foo`"));

    common::teardown();
    Ok(())
}

#[test]
fn test_check_contents_ignoring_recorded_violations(
) -> Result<(), Box<dyn Error>> {
    let project_root = "tests/fixtures/contains_package_todo";
    let relative_path = "packs/foo/app/services/foo.rb";
    let foo_rb_contents =
        fs::read_to_string(format!("{}/{}", project_root, relative_path))?;

    let output = cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg(project_root)
        .arg("--debug")
        .arg("check-contents")
        .arg("--ignore-recorded-violations")
        .arg(relative_path)
        .write_stdin(format!("\n\n\n{}", foo_rb_contents))
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let stripped_output = stripped_output(output);
    assert!(stripped_output.contains("1 violation(s) detected:"));
    assert!(stripped_output.contains("packs/foo/app/services/foo.rb:6:4\nDependency violation: `::Bar` belongs to `packs/bar`, but `packs/foo/package.yml` does not specify a dependency on `packs/bar`."));

    common::teardown();
    Ok(())
}

#[test]
fn test_check_with_json_output_format_violations() -> Result<(), Box<dyn Error>>
{
    let output = cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("check")
        .arg("-o")
        .arg("json")
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let json_output: serde_json::Value =
        serde_json::from_slice(&output).expect("Output should be valid JSON");

    validate_check_output_schema(&json_output);

    assert_eq!(json_output["summary"]["violation_count"], 2);
    assert_eq!(json_output["summary"]["stale_todo_count"], 0);
    assert_eq!(json_output["summary"]["strict_violation_count"], 0);
    assert_eq!(json_output["summary"]["success"], false);

    let violations = json_output["violations"].as_array().unwrap();
    assert_eq!(violations.len(), 2);

    let dependency_violation = violations
        .iter()
        .find(|v| v["violation_type"].as_str().unwrap() == "dependency")
        .expect("Should have a dependency violation");

    assert_eq!(
        dependency_violation["file"].as_str().unwrap(),
        "packs/foo/app/services/foo.rb"
    );
    assert_eq!(
        dependency_violation["constant_name"].as_str().unwrap(),
        "::Bar"
    );
    assert_eq!(
        dependency_violation["referencing_pack_name"]
            .as_str()
            .unwrap(),
        "packs/foo"
    );
    assert_eq!(
        dependency_violation["defining_pack_name"].as_str().unwrap(),
        "packs/bar"
    );
    assert!(!dependency_violation["strict"].as_bool().unwrap());
    assert!(dependency_violation["message"]
        .as_str()
        .unwrap()
        .contains("Dependency violation"));

    let privacy_violation = violations
        .iter()
        .find(|v| v["violation_type"].as_str().unwrap() == "privacy")
        .expect("Should have a privacy violation");

    assert_eq!(
        privacy_violation["file"].as_str().unwrap(),
        "packs/foo/app/services/foo.rb"
    );
    assert_eq!(
        privacy_violation["constant_name"].as_str().unwrap(),
        "::Bar"
    );
    assert_eq!(
        privacy_violation["referencing_pack_name"].as_str().unwrap(),
        "packs/foo"
    );
    assert_eq!(
        privacy_violation["defining_pack_name"].as_str().unwrap(),
        "packs/bar"
    );
    assert!(!privacy_violation["strict"].as_bool().unwrap());
    assert!(privacy_violation["message"]
        .as_str()
        .unwrap()
        .contains("Privacy violation"));

    common::teardown();
    Ok(())
}

#[test]
fn test_check_with_json_output_format_stale_todos() -> Result<(), Box<dyn Error>>
{
    let output = cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/contains_stale_violations")
        .arg("check")
        .arg("-o")
        .arg("json")
        .assert()
        .code(1) // Stale todos are violations that should fail
        .get_output()
        .stdout
        .clone();

    let json_output: serde_json::Value =
        serde_json::from_slice(&output).expect("Output should be valid JSON");

    validate_check_output_schema(&json_output);

    // The fixture has multiple stale todos
    assert_eq!(json_output["summary"]["stale_todo_count"], 3);
    assert_eq!(json_output["summary"]["violation_count"], 0);
    assert_eq!(json_output["summary"]["strict_violation_count"], 0);
    assert_eq!(json_output["summary"]["success"], false);

    let stale_todos = json_output["stale_todos"].as_array().unwrap();
    assert_eq!(stale_todos.len(), 3);

    // Find the specific stale todo for ::Bar dependency violation
    let bar_dependency_stale = stale_todos
        .iter()
        .find(|t| {
            t["constant_name"].as_str().unwrap() == "::Bar"
                && t["violation_type"].as_str().unwrap() == "dependency"
        })
        .expect("Should have stale dependency todo for ::Bar");

    assert_eq!(
        bar_dependency_stale["file"].as_str().unwrap(),
        "packs/foo/app/services/foo.rb"
    );
    assert_eq!(
        bar_dependency_stale["referencing_pack_name"]
            .as_str()
            .unwrap(),
        "packs/foo"
    );
    assert_eq!(
        bar_dependency_stale["defining_pack_name"].as_str().unwrap(),
        "packs/bar"
    );

    // Find the stale todo for ::Foo privacy violation
    let foo_privacy_stale = stale_todos
        .iter()
        .find(|t| {
            t["constant_name"].as_str().unwrap() == "::Foo"
                && t["violation_type"].as_str().unwrap() == "privacy"
        })
        .expect("Should have stale privacy todo for ::Foo");

    assert_eq!(
        foo_privacy_stale["file"].as_str().unwrap(),
        "packs/bar/app/services/bar.rb"
    );
    assert_eq!(
        foo_privacy_stale["referencing_pack_name"].as_str().unwrap(),
        "packs/bar"
    );
    assert_eq!(
        foo_privacy_stale["defining_pack_name"].as_str().unwrap(),
        "packs/foo"
    );

    common::teardown();
    Ok(())
}

#[test]
fn test_check_with_json_output_format_empty() -> Result<(), Box<dyn Error>> {
    let output = cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/contains_package_todo")
        .arg("check")
        .arg("-o")
        .arg("json")
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();

    let json_output: serde_json::Value =
        serde_json::from_slice(&output).expect("Output should be valid JSON");

    validate_check_output_schema(&json_output);

    assert_eq!(json_output["summary"]["violation_count"], 0);
    assert_eq!(json_output["summary"]["stale_todo_count"], 0);
    assert_eq!(json_output["summary"]["strict_violation_count"], 0);
    assert_eq!(json_output["summary"]["success"], true);
    assert!(json_output["violations"].as_array().unwrap().is_empty());
    assert!(json_output["stale_todos"].as_array().unwrap().is_empty());

    common::teardown();
    Ok(())
}

#[test]
fn test_check_with_nonexistent_project_root() -> Result<(), Box<dyn Error>> {
    // Exit code 2 for internal errors (non-existent project root)
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/does_not_exist")
        .arg("check")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("Error:"));

    Ok(())
}

#[test]
fn test_check_with_invalid_output_format() -> Result<(), Box<dyn Error>> {
    // Exit code 2 for argument parsing errors (clap's default)
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("check")
        .arg("-o")
        .arg("invalid_format")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("invalid value 'invalid_format'"));

    Ok(())
}
