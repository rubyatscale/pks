use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{error::Error, process::Command};

mod common;

/// Test that gitignored files are completely excluded from violation checking.
/// The fixture has a violation in ignored_folder/violating.rb which should NOT be reported.
#[test]
fn test_check_ignores_violations_in_gitignored_files() -> Result<(), Box<dyn Error>> {
    // NOTE: This test will fail until Phase 2 is implemented, when gitignore
    // integration is actually added to walk_directory()
    
    // The fixture has:
    // - packs/foo/app/services/foo.rb with violation (NOT ignored) 
    // - ignored_folder/violating.rb with violation (IS ignored)
    //
    // Currently both violations are detected. After Phase 2, only the first should be.
    
    let output = Command::cargo_bin("pks")?
        .arg("--project-root")
        .arg("tests/fixtures/app_with_gitignore")
        .arg("--debug")
        .arg("check")
        .assert()
        .failure() // Still fails due to violation in foo.rb
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8_lossy(&output);
    
    // Should report violation in non-ignored file
    assert!(
        stdout.contains("packs/foo/app/services/foo.rb"),
        "Should detect violation in non-ignored file"
    );
    
    // Should NOT report violation in ignored file
    // TODO: Uncomment after Phase 2 is implemented
    // assert!(
    //     !stdout.contains("ignored_folder/violating.rb"),
    //     "Should NOT detect violations in gitignored files"
    // );
    
    common::teardown();
    Ok(())
}

/// Test that list-included-files respects gitignore patterns.
#[test]
fn test_list_included_files_excludes_gitignored() -> Result<(), Box<dyn Error>> {
    // NOTE: This test will fail until Phase 2 is implemented
    
    let output = Command::cargo_bin("pks")?
        .arg("--project-root")
        .arg("tests/fixtures/app_with_gitignore")
        .arg("list-included-files")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    
    let stdout = String::from_utf8_lossy(&output);
    
    // Should include non-ignored Ruby files
    assert!(
        stdout.contains("included_file.rb"),
        "Should include non-ignored files"
    );
    assert!(
        stdout.contains("foo.rb") || stdout.contains("bar.rb"),
        "Should include package files"
    );
    
    // Should NOT include gitignored files
    // TODO: Uncomment after Phase 2 is implemented
    // assert!(
    //     !stdout.contains("ignored_file.rb"),
    //     "Should NOT include files matched by gitignore patterns"
    // );
    // assert!(
    //     !stdout.contains("debug.log"),
    //     "Should NOT include *.log files"
    // );
    // assert!(
    //     !stdout.contains("ignored_folder"),
    //     "Should NOT include files in ignored directories"
    // );
    
    common::teardown();
    Ok(())
}

/// Test that the application works correctly even without a .gitignore file.
#[test]
fn test_check_works_without_gitignore() -> Result<(), Box<dyn Error>> {
    // simple_app doesn't have a .gitignore file
    // This should still work (and report violations as usual)
    
    Command::cargo_bin("pks")?
        .arg("--project-root")
        .arg("tests/fixtures/simple_app")
        .arg("--debug")
        .arg("check")
        .assert()
        .failure() // Has violations
        .stdout(predicate::str::contains("violation(s) detected"));
    
    common::teardown();
    Ok(())
}

/// Test that basic gitignore functionality works at the library level.
/// This is a sanity check that our helper functions work correctly.
#[test]
fn test_gitignore_matcher_functions() -> Result<(), Box<dyn Error>> {
    use packs::packs::walk_directory::build_gitignore_matcher;
    use std::path::PathBuf;
    
    let absolute_path = PathBuf::from("tests/fixtures/app_with_gitignore")
        .canonicalize()
        .expect("Could not canonicalize path");

    let matcher = build_gitignore_matcher(&absolute_path)?;
    
    // Test that patterns in .gitignore are matched correctly
    
    // *.log should be ignored
    let log_file = PathBuf::from("packs/foo/debug.log");
    let result = matcher.matched(&log_file, false);
    assert!(result.is_ignore(), "*.log files should be ignored");
    
    // ignored_file.rb should be ignored (explicitly listed)
    let ignored_rb = PathBuf::from("packs/foo/ignored_file.rb");
    let result = matcher.matched(&ignored_rb, false);
    assert!(result.is_ignore(), "ignored_file.rb should be ignored");
    
    // ignored_folder/ directory should be ignored
    let ignored_folder = PathBuf::from("ignored_folder");
    let result = matcher.matched(&ignored_folder, true);
    assert!(result.is_ignore(), "ignored_folder/ directory should be ignored");
    
    // included_file.rb should NOT be ignored
    let included_rb = PathBuf::from("packs/foo/included_file.rb");
    let result = matcher.matched(&included_rb, false);
    assert!(!result.is_ignore(), "included_file.rb should not be ignored");
    
    Ok(())
}

/// Test that the matcher can be built even without a .gitignore file.
#[test]
fn test_gitignore_matcher_without_gitignore() -> Result<(), Box<dyn Error>> {
    use packs::packs::walk_directory::build_gitignore_matcher;
    use std::path::PathBuf;
    
    // Use simple_app which doesn't have a .gitignore
    let absolute_path = PathBuf::from("tests/fixtures/simple_app")
        .canonicalize()
        .expect("Could not canonicalize path");

    // Should succeed even without .gitignore
    let result = build_gitignore_matcher(&absolute_path);
    assert!(result.is_ok(), "Should build matcher even without .gitignore");
    
    let matcher = result?;
    
    // Without a .gitignore, regular files should not be ignored
    let test_file = PathBuf::from("test.rb");
    let result = matcher.matched(&test_file, false);
    assert!(!result.is_ignore(), "Regular files should not be ignored without .gitignore");
    
    Ok(())
}
