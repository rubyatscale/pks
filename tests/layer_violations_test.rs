use assert_cmd::cargo::cargo_bin_cmd;
use std::error::Error;

mod common;

#[test]
fn test_check_with_error_template_overrides() -> Result<(), Box<dyn Error>> {
    let output = cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/layer_violations_with_overrides")
        .arg("--debug")
        .arg("check")
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let stripped_output =
        String::from_utf8_lossy(&strip_ansi_escapes::strip(output)).to_string();

    assert!(stripped_output.contains("1 violation(s) detected:"));
    assert!(stripped_output.contains("packs/feature_flags/app/services/feature_flags.rb:2:0\nLayer violation: `::Payments` belongs to `packs/payments` (whose layer is `product`) cannot be accessed from `packs/feature_flags` (whose layer is `utilities`). See https://go/pks-layer"));

    common::teardown();
    Ok(())
}
#[test]
fn test_check() -> Result<(), Box<dyn Error>> {
    let output = cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/layer_violations")
        .arg("--debug")
        .arg("check")
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let stripped_output =
        String::from_utf8_lossy(&strip_ansi_escapes::strip(output)).to_string();

    assert!(stripped_output.contains("1 violation(s) detected:"));
    assert!(stripped_output.contains("packs/feature_flags/app/services/feature_flags.rb:2:0\nLayer violation: `::Payments` belongs to `packs/payments` (whose layer is `product`) cannot be accessed from `packs/feature_flags` (whose layer is `utilities`)"));

    common::teardown();
    Ok(())
}

#[test]
fn test_check_enforce_layers_disabled() -> Result<(), Box<dyn Error>> {
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/layer_violations")
        .arg("--debug")
        .arg("--disable-enforce-layers")
        .arg("check")
        .assert()
        .success();

    common::teardown();
    Ok(())
}
