use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::error::Error;

mod common;

#[test]
fn graph_outputs_declared_edges_and_nodes() -> Result<(), Box<dyn Error>> {
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("graph")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"nodes\""))
        .stdout(predicate::str::contains("\"name\":\"packs/foo\""))
        .stdout(predicate::str::contains("\"from\":\"packs/foo\""))
        .stdout(predicate::str::contains("\"to\":\"packs/baz\""))
        .stdout(predicate::str::contains("\"kind\":\"declared\""));

    common::teardown();
    Ok(())
}

#[test]
fn graph_outputs_todo_edges() -> Result<(), Box<dyn Error>> {
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/contains_package_todo")
        .arg("graph")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"from\":\"packs/foo\""))
        .stdout(predicate::str::contains("\"to\":\"packs/bar\""))
        .stdout(predicate::str::contains("\"kind\":\"todo\""));

    common::teardown();
    Ok(())
}

#[test]
fn graph_outputs_ignored_edges() -> Result<(), Box<dyn Error>> {
    // In app_with_ignored_dependency, packs/foo declares packs/baz and ignores packs/bar.
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/app_with_ignored_dependency")
        .arg("graph")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"to\":\"packs/bar\""))
        .stdout(predicate::str::contains("\"kind\":\"ignored\""))
        .stdout(predicate::str::contains("\"kind\":\"declared\""));

    common::teardown();
    Ok(())
}

#[test]
fn graph_cli_output_is_deterministic() -> Result<(), Box<dyn Error>> {
    let first = cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("graph")
        .assert()
        .success();
    let second = cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("graph")
        .assert()
        .success();

    assert_eq!(
        first.get_output().stdout,
        second.get_output().stdout,
        "`pks graph` stdout must be byte-identical across runs"
    );

    common::teardown();
    Ok(())
}
