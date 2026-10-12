use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::error::Error;
mod common;

#[test]
fn test_expose_monkey_patches() -> Result<(), Box<dyn Error>> {
    // For the full test, see src/packs/monkey_patch_detection.rs
    // This just ensures the CLI is hooked up correctly to the internal API
    let expected_message_portion = String::from(
        "The following is a list of constants that are redefined by your app.",
    );
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/app_with_monkey_patches")
        .arg("--experimental-parser")
        .arg("expose-monkey-patches")
        .arg("--rubydir=tests/fixtures/app_with_monkey_patches/rubydir_stub")
        .arg("--gemdir=tests/fixtures/app_with_monkey_patches/gemdir_stub")
        .assert()
        .success()
        .stdout(predicate::str::contains(expected_message_portion));

    Ok(())
}

#[test]
fn test_expose_monkey_patches_warns_only_about_the_apps_syntax_errors(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("app_with_monkey_patches");
    // Ruby's and gems' sources aren't the app's to fix, so they don't warn
    let broken = "class Broken\n  def a(\nend\n";
    std::fs::write(fixture.path("rubydir_stub/broken.rb"), broken)?;
    std::fs::write(fixture.path("gemdir_stub/rails/broken.rb"), broken)?;
    std::fs::write(fixture.path("app/services/broken.rb"), broken)?;

    let rubydir = fixture.path("rubydir_stub");
    let gemdir = fixture.path("gemdir_stub");
    cargo_bin_cmd!("pks")
        .env_remove("RUST_LOG")
        .arg("--project-root")
        .arg(fixture.root())
        .arg("--experimental-parser")
        .arg("expose-monkey-patches")
        .arg(format!("--rubydir={}", rubydir.display()))
        .arg(format!("--gemdir={}", gemdir.display()))
        .assert()
        .success()
        .stderr(
            "Warning: `app/services/broken.rb` has a syntax error at line 3 \
             (unexpected kEND), so pks may have missed references in it.\n",
        );

    Ok(())
}
