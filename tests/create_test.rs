use assert_cmd::Command;
use predicates::prelude::*;
use pretty_assertions::assert_eq;
use std::{error::Error, fs, path::Path};

mod common;

#[test]
fn test_create() -> Result<(), Box<dyn Error>> {
    common::delete_foobar();

    Command::cargo_bin("pks")?
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("create")
        .arg("packs/foobar")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Successfully created `packs/foobar`!",
        ));

    let actual = fs::read_to_string(
        "tests/fixtures/simple_app/packs/foobar/package.yml",
    ).unwrap_or_else(|_| panic!("Could not read file tests/fixtures/simple_app/packs/foobar/package.yml"));
    assert!(actual.contains("enforce_dependencies: true"));
    assert!(actual.contains("enforce_privacy: true"));
    assert!(actual.contains("enforce_layers: true"));
    assert!(Path::new(
        "tests/fixtures/simple_app/packs/foobar/app/public/foobar"
    )
    .exists());
    assert!(Path::new(
        "tests/fixtures/simple_app/packs/foobar/app/services/foobar"
    )
    .exists());
    assert!(Path::new("tests/fixtures/simple_app/packs/foobar/spec").exists());

    let expected_readme = String::from("\
Welcome to `packs/foobar`!

If you're the author, please consider replacing this file with a README.md, which may contain:
- What your pack is and does
- How you expect people to use your pack
- Example usage of your pack's public API and where to find it
- Limitations, risks, and important considerations of usage
- How to get in touch with eng and other stakeholders for questions or issues pertaining to this pack
- What SLAs/SLOs (service level agreements/objectives), if any, your package provides
- When in doubt, keep it simple
- Anything else you may want to include!

README.md should change as your public API changes.

See https://github.com/rubyatscale/pks#readme for more info!");

    let actual_readme =
        fs::read_to_string("tests/fixtures/simple_app/packs/foobar/README.md").unwrap_or_else(|e| {
            panic!("Could not read file tests/fixtures/simple_app/packs/foobar/README.md: {}", e)
        });

    assert_eq!(expected_readme, actual_readme);

    common::teardown();
    common::delete_foobar();

    Ok(())
}

#[test]
fn test_create_with_readme_template_default_path() -> Result<(), Box<dyn Error>>
{
    common::delete_foobaz();

    fs::write(
        "tests/fixtures/simple_packs_first_app/README_TEMPLATE.md",
        "This is a test custom README template",
    )?;

    Command::cargo_bin("pks")?
        .arg("--project-root")
        .arg("tests/fixtures/simple_packs_first_app")
        .arg("create")
        .arg("packs/foobaz")
        .assert()
        .success();

    let expected_readme = String::from("This is a test custom README template");
    let actual_readme =
        fs::read_to_string("tests/fixtures/simple_packs_first_app/packs/foobaz/README.md").unwrap_or_else(|e| {
            panic!("Could not read file tests/fixtures/simple_packs_first_app/packs/foobaz/README.md: {}", e)
        });

    assert_eq!(expected_readme, actual_readme);

    common::teardown();
    common::delete_foobaz();
    fs::remove_file(
        "tests/fixtures/simple_packs_first_app/README_TEMPLATE.md",
    )?;

    Ok(())
}

#[test]
fn test_create_with_readme_template_custom_path() -> Result<(), Box<dyn Error>>
{
    common::delete_foobar_app_with_custom_readme();

    Command::cargo_bin("pks")?
        .arg("--project-root")
        .arg("tests/fixtures/app_with_custom_readme")
        .arg("create")
        .arg("packs/foobar")
        .assert()
        .success();

    let expected_readme = String::from("README template\n\nThis is a test\n");

    let actual_readme =
        fs::read_to_string("tests/fixtures/app_with_custom_readme/packs/foobar/README.md").unwrap_or_else(|e| {
            panic!("Could not read file tests/fixtures/app_with_custom_readme/packs/foobar/README.md: {}", e)
        });

    assert_eq!(expected_readme, actual_readme);

    common::teardown();
    common::delete_foobar_app_with_custom_readme();

    Ok(())
}

#[test]
fn test_create_already_exists() -> Result<(), Box<dyn Error>> {
    Command::cargo_bin("pks")?
        .arg("--project-root")
        .arg("tests/fixtures/simple_packs_first_app")
        .arg("create")
        .arg("packs/foo")
        .assert()
        .success()
        .stdout(predicate::str::contains("`packs/foo` already exists!"));

    common::teardown();
    Ok(())
}
