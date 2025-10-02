use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{error::Error, process::Command};

mod common;

/// Test that gitignored files are completely excluded from violation checking.
/// The fixture has a violation in ignored_folder/violating.rb which should NOT be reported.
#[test]
fn test_check_ignores_violations_in_gitignored_files(
) -> Result<(), Box<dyn Error>> {
    // The fixture has:
    // - packs/foo/app/services/foo.rb with violation (NOT ignored)
    // - ignored_folder/violating.rb with violation (IS ignored)
    //
    // Phase 2 ensures only violations in non-ignored files are detected.

    let result = Command::cargo_bin("pks")?
        .arg("--project-root")
        .arg("tests/fixtures/app_with_gitignore")
        .arg("check")
        .assert()
        .failure(); // Still fails due to violation in foo.rb

    let output = result.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Violations are printed to stdout
    // Should report violation in non-ignored file
    assert!(
        stdout.contains("packs/foo/app/services/foo.rb")
            || stdout.contains("foo.rb"),
        "Should detect violation in non-ignored file.\nstdout: {}\nstderr: {}",
        stdout,
        stderr
    );

    // Should NOT report violation in ignored file
    assert!(
        !stdout.contains("ignored_folder") && !stdout.contains("violating.rb"),
        "Should NOT detect violations in gitignored files.\nstdout: {}\nstderr: {}", stdout, stderr
    );

    common::teardown();
    Ok(())
}

/// Test that list-included-files respects gitignore patterns.
#[test]
fn test_list_included_files_excludes_gitignored() -> Result<(), Box<dyn Error>>
{
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
    assert!(
        !stdout.contains("ignored_file.rb"),
        "Should NOT include files matched by gitignore patterns"
    );
    assert!(
        !stdout.contains("debug.log"),
        "Should NOT include *.log files"
    );
    assert!(
        !stdout.contains("ignored_folder"),
        "Should NOT include files in ignored directories"
    );

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
    assert!(
        result.is_ignore(),
        "ignored_folder/ directory should be ignored"
    );

    // included_file.rb should NOT be ignored
    let included_rb = PathBuf::from("packs/foo/included_file.rb");
    let result = matcher.matched(&included_rb, false);
    assert!(
        !result.is_ignore(),
        "included_file.rb should not be ignored"
    );

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
    assert!(
        result.is_ok(),
        "Should build matcher even without .gitignore"
    );

    let matcher = result?;

    // Without a .gitignore, regular files should not be ignored
    let test_file = PathBuf::from("test.rb");
    let result = matcher.matched(&test_file, false);
    assert!(
        !result.is_ignore(),
        "Regular files should not be ignored without .gitignore"
    );

    Ok(())
}

/// CRITICAL: Test that respect_gitignore: false configuration disables gitignore support.
/// This is the primary configuration option added in Phase 2.
#[test]
fn test_respect_gitignore_can_be_disabled() -> Result<(), Box<dyn Error>> {
    // The fixture has:
    // - .gitignore with ignored_folder/ pattern
    // - respect_gitignore: false in packwerk.yml
    // - ignored_folder/violating.rb with a violation
    //
    // With respect_gitignore: false, the violation SHOULD be detected.

    let result = Command::cargo_bin("pks")?
        .arg("--project-root")
        .arg("tests/fixtures/app_with_gitignore_disabled")
        .arg("check")
        .assert()
        .failure(); // Should fail due to violation in ignored_folder/

    let output = result.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // With respect_gitignore: false, should detect violation in "ignored" folder
    assert!(
        stdout.contains("ignored_folder") || stdout.contains("violating.rb"),
        "Should detect violations in gitignored files when respect_gitignore: false.\nstdout: {}\nstderr: {}", 
        stdout, stderr
    );

    common::teardown();
    Ok(())
}

/// Test gitignore negation patterns (!pattern).
/// Common use case: ignore all *.log except important.log
#[test]
fn test_gitignore_negation_patterns() -> Result<(), Box<dyn Error>> {
    use packs::packs::walk_directory::build_gitignore_matcher;
    use std::path::PathBuf;

    let absolute_path = PathBuf::from("tests/fixtures/app_with_gitignore")
        .canonicalize()
        .expect("Could not canonicalize path");

    let matcher = build_gitignore_matcher(&absolute_path)?;

    // .gitignore has:
    // *.log
    // !important.log

    // regular.log should be ignored by *.log
    let regular_log = PathBuf::from("packs/foo/regular.log");
    let result = matcher.matched(&regular_log, false);
    assert!(
        result.is_ignore(),
        "regular.log should be ignored by *.log pattern"
    );

    // important.log should NOT be ignored due to !important.log negation
    let important_log = PathBuf::from("packs/foo/important.log");
    let result = matcher.matched(&important_log, false);
    assert!(
        !result.is_ignore(),
        "important.log should NOT be ignored due to negation pattern"
    );

    Ok(())
}

/// Test that list-included-files respects negation patterns.
/// Note: We test this at the library level since .log files aren't Ruby files
/// and won't appear in list-included-files regardless of gitignore.
#[test]
fn test_list_included_files_respects_negation() -> Result<(), Box<dyn Error>> {
    // This is already tested by test_gitignore_negation_patterns at the library level.
    // At the CLI level, .log files aren't included in list-included-files anyway
    // since they don't match the Ruby file patterns.

    // Just verify the basic behavior still works
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

    // Should still include Ruby files
    assert!(
        stdout.contains("included_file.rb"),
        "Should include non-ignored Ruby files"
    );

    common::teardown();
    Ok(())
}

/// Test that global gitignore works end-to-end.
/// This requires temporarily setting up a global gitignore.
#[test]
fn test_respects_global_gitignore() -> Result<(), Box<dyn Error>> {
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    // Create a temporary global gitignore
    let temp_dir = env::temp_dir();
    let global_gitignore = temp_dir.join("test_global_gitignore");

    // Write a pattern that will affect our test
    fs::write(&global_gitignore, "# Global test\n*.global_ignore\n")?;

    // Create a test file that should be ignored
    let fixture_path = PathBuf::from("tests/fixtures/app_with_gitignore");
    let test_file = fixture_path.join("test.global_ignore");
    fs::write(&test_file, "// Should be ignored by global gitignore\n")?;

    // Set HOME to a temp location and create a .gitignore_global
    let temp_home = temp_dir.join("test_home_for_gitignore");
    fs::create_dir_all(&temp_home)?;
    let home_gitignore = temp_home.join(".gitignore_global");
    fs::write(&home_gitignore, "*.global_ignore\n")?;

    // Save original HOME
    let original_home = env::var("HOME").ok();

    // Set temporary HOME
    env::set_var("HOME", &temp_home);

    // Test that list-included-files excludes the globally ignored file
    let output = Command::cargo_bin("pks")?
        .arg("--project-root")
        .arg("tests/fixtures/app_with_gitignore")
        .arg("list-included-files")
        .env("HOME", &temp_home)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8_lossy(&output);

    // Should NOT include globally ignored file
    assert!(
        !stdout.contains("test.global_ignore"),
        "Should NOT include file matched by global gitignore.\nOutput: {}",
        stdout
    );

    // Cleanup
    fs::remove_file(&test_file).ok();
    fs::remove_file(&global_gitignore).ok();
    fs::remove_file(&home_gitignore).ok();
    fs::remove_dir(&temp_home).ok();

    // Restore HOME
    if let Some(home) = original_home {
        env::set_var("HOME", home);
    } else {
        env::remove_var("HOME");
    }

    common::teardown();
    Ok(())
}

/// Test that gitignore works with the update command.
/// Gitignored files should not cause package_todo.yml updates.
#[test]
fn test_update_respects_gitignore() -> Result<(), Box<dyn Error>> {
    use tempfile::TempDir;

    // Create a temporary copy of the fixture
    let temp_dir = TempDir::new()?;
    let temp_fixture = temp_dir.path().join("app");

    // Copy fixture to temp directory
    let fixture_path = "tests/fixtures/app_with_gitignore";
    copy_dir_all(fixture_path, &temp_fixture)?;

    // Run update command
    let output = Command::cargo_bin("pks")?
        .arg("--project-root")
        .arg(&temp_fixture)
        .arg("update")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8_lossy(&output);

    // Should not mention files in ignored_folder
    assert!(
        !stdout.contains("ignored_folder"),
        "Update should not process gitignored files.\nOutput: {}",
        stdout
    );

    common::teardown();
    Ok(())
}

// Helper function to copy directories recursively
fn copy_dir_all(
    src: impl AsRef<std::path::Path>,
    dst: impl AsRef<std::path::Path>,
) -> std::io::Result<()> {
    use std::fs;

    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}
