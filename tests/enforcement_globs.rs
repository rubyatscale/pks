use assert_cmd::cargo::cargo_bin_cmd;
use std::error::Error;

mod common;

#[test]
fn test_check() -> Result<(), Box<dyn Error>> {
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/simple_app_with_enforcement_globs")
        .arg("--debug")
        .arg("check")
        .assert()
        .success();
    common::teardown();
    Ok(())
}
