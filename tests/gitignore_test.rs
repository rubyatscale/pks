use std::path::PathBuf;

#[test]
fn test_gitignore_matcher_with_fixture() -> anyhow::Result<()> {
    use packs::packs::walk_directory::build_gitignore_matcher;
    
    let absolute_path = PathBuf::from("tests/fixtures/app_with_gitignore")
        .canonicalize()
        .expect("Could not canonicalize path");

    // Read the gitignore file to see what patterns we have
    let gitignore_path = absolute_path.join(".gitignore");
    if let Ok(contents) = std::fs::read_to_string(&gitignore_path) {
        println!("Gitignore contents:\n{}", contents);
    }

    let matcher = build_gitignore_matcher(&absolute_path)?;
    
    // Test that patterns in .gitignore are matched correctly
    // Note: paths should be relative to the root
    
    // *.log should be ignored
    let log_file = PathBuf::from("packs/foo/debug.log");
    let result = matcher.matched(&log_file, false);
    println!("packs/foo/debug.log => {:?}", result);
    assert!(result.is_ignore(), "*.log files should be ignored");
    
    // ignored_file.rb should be ignored (explicitly listed)
    let ignored_rb = PathBuf::from("packs/foo/ignored_file.rb");
    let result = matcher.matched(&ignored_rb, false);
    println!("packs/foo/ignored_file.rb => {:?}", result);
    assert!(result.is_ignore(), "ignored_file.rb should be ignored");
    
    // ignored_folder/ directory should be ignored
    let ignored_folder = PathBuf::from("ignored_folder");
    let result = matcher.matched(&ignored_folder, true);
    println!("ignored_folder (dir) => {:?}", result);
    assert!(result.is_ignore(), "ignored_folder/ directory should be ignored");
    
    // Note: Files within ignored directories return Match::None when queried directly.
    // In Phase 2, we'll handle this by checking parent directories during the walk.
    // For now, we verify that the directory itself is ignored, which is sufficient
    // for early pruning during directory traversal.
    
    // included_file.rb should NOT be ignored
    let included_rb = PathBuf::from("packs/foo/included_file.rb");
    let result = matcher.matched(&included_rb, false);
    assert!(!result.is_ignore(), "included_file.rb should not be ignored");
    
    Ok(())
}

#[test]
fn test_gitignore_matcher_without_gitignore() -> anyhow::Result<()> {
    use packs::packs::walk_directory::build_gitignore_matcher;
    
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

