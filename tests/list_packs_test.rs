use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::error::Error;

#[test]
fn lint_packs() -> Result<(), Box<dyn Error>> {
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--debug")
        .arg("list-packs")
        .assert()
        .success()
        .stdout(predicate::str::contains("package.yml"))
        .stdout(predicate::str::contains("packs/bar/package.yml"))
        .stdout(predicate::str::contains("packs/foo/package.yml"));
    Ok(())
}
