use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::{error::Error, fs, path::Path};
mod common;

fn assert_check_unused_dependencies(cmd: &str) -> Result<(), Box<dyn Error>> {
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/app_with_dependency_cycles")
        .arg("--debug")
        .arg(cmd)
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "packs/bar depends on packs/foo but does not use it",
        ))
        .stdout(predicate::str::contains(
            "packs/foo depends on packs/bar but does not use it",
        ))
        .stderr(predicate::str::contains(
           "Error: Found 3 unused dependencies. Run `packs check-unused-dependencies --auto-correct` to remove them.")
        );
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

fn assert_auto_correct_unused_dependencies(
    cmd: &str,
    flag: &str,
) -> Result<(), Box<dyn Error>> {
    common::set_up_fixtures();

    let expected_before_autocorrect = r#"# Header comment: proves comments above the first key survive.
enforce_dependencies: true
enforce_privacy: true
dependencies:
# Comment inside the dependencies block.
- packs/bar
- packs/baz
# Trailing comment, after the list and before another key.
layer: technical_services
"#;
    let foo_package_yml = fs::read_to_string("tests/fixtures/app_with_unnecessary_dependencies/packs/foo/package.yml").unwrap();
    assert_eq!(foo_package_yml, expected_before_autocorrect);

    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/app_with_unnecessary_dependencies")
        .arg("--debug")
        .arg(cmd)
        .arg(flag)
        .assert()
        .success();

    // Comments in all three positions survive, and the key order is unchanged:
    // the correction deletes the one list item and nothing else.
    let expected_autocorrect = r#"# Header comment: proves comments above the first key survive.
enforce_dependencies: true
enforce_privacy: true
dependencies:
# Comment inside the dependencies block.
- packs/bar
# Trailing comment, after the list and before another key.
layer: technical_services
"#;
    let after_autocorrect = fs::read_to_string("tests/fixtures/app_with_unnecessary_dependencies/packs/foo/package.yml").unwrap();
    assert_eq!(after_autocorrect, expected_autocorrect);

    assert_no_unused_dependencies(Path::new(
        "tests/fixtures/app_with_unnecessary_dependencies",
    ));

    Ok(())
}

fn assert_no_unused_dependencies(project_root: &Path) {
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg(project_root)
        .arg("check-unused-dependencies")
        .assert()
        .success();
}

fn auto_correct_foo_package_yml(before: &str) -> (String, String) {
    let fixture = common::Fixture::new("app_with_unnecessary_dependencies");
    let package_yml = fixture.path("packs/foo/package.yml");
    fs::write(&package_yml, before).unwrap();

    let output = cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg(fixture.root())
        .arg("check-unused-dependencies")
        .arg("--auto-correct")
        .assert()
        .success()
        .get_output()
        .clone();

    assert_no_unused_dependencies(fixture.root());
    (
        fs::read_to_string(&package_yml).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[test]
fn test_auto_correct_unnecessary_dependencies() -> Result<(), Box<dyn Error>> {
    assert_auto_correct_unused_dependencies(
        "check-unused-dependencies",
        "--auto-correct",
    )?;
    assert_auto_correct_unused_dependencies("check-unused-dependencies", "-a")?;
    assert_auto_correct_unused_dependencies(
        "check-unnecessary-dependencies",
        "-a",
    )?;
    assert_auto_correct_unused_dependencies(
        "check-unnecessary-dependencies",
        "--auto-correct",
    )
}

#[test]
fn test_check_unnecessary_dependencies_no_issue() -> Result<(), Box<dyn Error>>
{
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--debug")
        .arg("check-unused-dependencies")
        .assert()
        .success();
    Ok(())
}

#[test]
fn test_auto_correct_preserves_comments_in_hand_edited_layouts() {
    let before = "\
# Header comment.
enforce_dependencies: true
enforce_privacy: true
dependencies: # keep sorted
  # Comment inside the dependencies block.
  - \"packs/bar\"

  - packs/baz  # unused
ignored_dependencies:
  # Deliberately ignored: bop would cycle.
  - packs/bop
";
    let expected = "\
# Header comment.
enforce_dependencies: true
enforce_privacy: true
dependencies: # keep sorted
  # Comment inside the dependencies block.
  - \"packs/bar\"

ignored_dependencies:
  # Deliberately ignored: bop would cycle.
  - packs/bop
";

    let (after, stderr) = auto_correct_foo_package_yml(before);
    assert_eq!(after, expected);
    assert_eq!(stderr, "");
}

#[test]
fn test_auto_correct_falls_back_to_rewriting_layouts_it_cannot_edit() {
    let before = "\
# This comment is lost by the rewrite.
enforce_dependencies: true
enforce_privacy: true
dependencies: [packs/bar, packs/baz]
";

    let (after, stderr) = auto_correct_foo_package_yml(before);
    assert!(!after.contains("packs/baz"), "{after}");
    assert!(after.contains("- packs/bar"), "{after}");
    assert!(
        stderr.contains("could not edit the dependencies list"),
        "{stderr}"
    );
}
