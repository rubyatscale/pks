use ignore::gitignore::{Gitignore, GitignoreBuilder};
use jwalk::WalkDirGeneric;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};
use tracing::debug;

use super::{
    file_utils::build_glob_set, pack::Pack, raw_configuration::RawConfiguration,
};

pub struct WalkDirectoryResult {
    pub included_files: HashSet<PathBuf>,
    pub included_packs: HashSet<Pack>,
    pub owning_package_yml_for_file: HashMap<PathBuf, PathBuf>,
}

#[derive(Debug, Default, Clone)]
struct ProcessReadDirState {
    current_package_yml: PathBuf,
}

impl jwalk::ClientState for ProcessReadDirState {
    type ReadDirState = ProcessReadDirState;

    type DirEntryState = ProcessReadDirState;
}

/// Locates the global gitignore file from git config (`core.excludesFile`).
///
/// Returns `Some(PathBuf)` if `core.excludesFile` is configured and the file
/// exists, `None` otherwise. Reads git config files directly without spawning
/// a subprocess.
pub fn get_global_gitignore() -> Option<PathBuf> {
    ignore::gitignore::gitconfig_excludes_path().filter(|p| p.exists())
}

/// Builds a gitignore matcher that respects local and global gitignore files.
///
/// This function constructs a `Gitignore` matcher by combining:
/// - Local `.gitignore` file in the repository root
/// - Global gitignore file (from `core.excludesFile` git config)
/// - `.git/info/exclude` file in the repository
///
/// # Arguments
/// * `absolute_root` - The absolute path to the repository root
///
/// # Returns
/// A `Gitignore` matcher that can be used to check if paths should be ignored,
/// or an error if the matcher cannot be built.
pub fn build_gitignore_matcher(
    absolute_root: &Path,
) -> anyhow::Result<Gitignore> {
    let mut builder = GitignoreBuilder::new(absolute_root);

    // Add local .gitignore
    let local_gitignore = absolute_root.join(".gitignore");
    if local_gitignore.exists() {
        if let Some(err) = builder.add(&local_gitignore) {
            return Err(anyhow::anyhow!(
                "Failed to add local .gitignore: {}",
                err
            ));
        }
    }

    // Add global gitignore
    if let Some(global_gitignore) = get_global_gitignore() {
        if let Some(err) = builder.add(&global_gitignore) {
            return Err(anyhow::anyhow!(
                "Failed to add global gitignore: {}",
                err
            ));
        }
    }

    // Add .git/info/exclude
    let git_exclude = absolute_root.join(".git/info/exclude");
    if git_exclude.exists() {
        if let Some(err) = builder.add(&git_exclude) {
            return Err(anyhow::anyhow!(
                "Failed to add .git/info/exclude: {}",
                err
            ));
        }
    }

    Ok(builder.build()?)
}

// We use jwalk to walk directories in parallel and compare them to the `include` and `exclude` patterns
// specified in the `RawConfiguration`
// https://docs.rs/jwalk/0.8.1/jwalk/struct.WalkDirGeneric.html#method.process_read_dir
// We only walk the directory once and pull all of the information we need from it,
// which is faster than walking the directory multiple times.
// Likely, we can organize this better by moving each piece of logic into its own function so this function
// allows for a sort of "visitor pattern" for different things that need to walk the directory.
//
// Note: `ignore::WalkBuilder` (used by ripgrep) handles gitignore natively and supports
// parallel traversal via `build_parallel()`. We keep jwalk here because its
// `ProcessReadDirState` mechanism lets us propagate `current_package_yml` state from parent
// to child directories in a single pass. WalkBuilder's visitor pattern has no equivalent
// hook, so implementing that tracking would require per-file ancestor walks — an O(depth)
// regression for large repos.
pub(crate) fn walk_directory(
    absolute_root: PathBuf,
    raw: &RawConfiguration,
) -> anyhow::Result<WalkDirectoryResult> {
    debug!("Beginning directory walk");

    let mut included_files: HashSet<PathBuf> = HashSet::new();
    let mut included_packs: HashSet<Pack> = HashSet::new();
    let mut owning_package_yml_for_file: HashMap<PathBuf, PathBuf> =
        HashMap::new();

    // Create this vector outside of the closure to avoid reallocating it
    let default_excluded_dirs = [
        "node_modules/**/*",
        "vendor/**/*",
        "tmp/**/*",
        ".git/**/*",
        "public/**/*",
        "bin/**/*",
        "log/**/*",
        "sorbet/**/*",
    ];
    let mut all_excluded_dirs: Vec<String> = Vec::new();
    all_excluded_dirs
        .extend(default_excluded_dirs.iter().map(|s| s.to_string()));

    let excluded_globs = &raw.exclude;
    all_excluded_dirs.extend(excluded_globs.to_owned());

    let all_excluded_dirs_set = build_glob_set(&all_excluded_dirs);
    let excluded_dirs_ref = Arc::new(all_excluded_dirs_set);

    let absolute_root_ref = Arc::new(absolute_root.clone());

    let includes_set = build_glob_set(&raw.include);
    let excludes_set = build_glob_set(&raw.exclude);
    let package_paths_set = build_glob_set(&raw.package_paths);

    // Build gitignore matcher if enabled
    let gitignore_matcher = if raw.respect_gitignore {
        match build_gitignore_matcher(&absolute_root) {
            Ok(matcher) => Some(matcher),
            Err(e) => {
                debug!("Failed to build gitignore matcher: {}. Continuing without gitignore support.", e);
                None
            }
        }
    } else {
        None
    };

    let gitignore_ref = Arc::new(gitignore_matcher);
    let gitignore_ref_for_loop = gitignore_ref.clone();

    // TODO: Pull directory walker into separate module. Allow it to be called with implementations of a trait
    // so separate concerns can each be in their own place.
    //
    // WalkDirGeneric allows you to customize the directory walk, such as skipping directories,
    // which we do as a performance optimization.
    //
    // Specifically – if an exclude glob matches an entire directory, we don't need to continue to
    // explore it. For example, instead of asking every file in `vendor/bundle/**/` if it should be excluded,
    // we'll save a lot of time by just skipping the entire directory.
    //
    // For more information, check out the docs: https://docs.rs/jwalk/0.8.1/jwalk/#extended-example
    let current_package_yml = PathBuf::from("package.yml");

    let walk_dir = WalkDirGeneric::<ProcessReadDirState>::new(&absolute_root)
        .follow_links(true)
        .root_read_dir_state(ProcessReadDirState {
            current_package_yml,
        })
        .process_read_dir(
            move |_depth, absolute_dirname, read_dir_state, children| {
                // We need to let the compiler know that we are using a reference and not the value itself.
                // We need to then clone the Arc to get a new reference, which is a new pointer to the value/data
                // (with an increase to the reference count).
                let cloned_excluded_dirs = excluded_dirs_ref.clone();
                let cloned_absolute_root = absolute_root_ref.clone();
                let cloned_gitignore = gitignore_ref.clone();
                let package_yml = absolute_dirname.join("package.yml");

                // Even if the parent has set this on children, the existence of a new
                // package.yml file should override it.
                if package_yml.exists() {
                    read_dir_state.current_package_yml = package_yml;
                }

                children.iter_mut().for_each(|child_dir_entry_result| {
                    if let Ok(child_dir_entry) = child_dir_entry_result {
                        let child_absolute_dirname = child_dir_entry.path();
                        child_dir_entry
                            .client_state
                            .current_package_yml
                            .clone_from(&read_dir_state.current_package_yml);

                        let relative_path = child_absolute_dirname
                            .strip_prefix(cloned_absolute_root.as_ref())
                            .unwrap();

                        // Check gitignore for directories only (optimization: prune ignored directory trees early)
                        // Files are checked separately in the main file processing loop below
                        if let Some(gitignore) = cloned_gitignore.as_ref() {
                            let is_dir = child_dir_entry.file_type.is_dir();
                            if is_dir
                                && gitignore
                                    .matched(relative_path, true)
                                    .is_ignore()
                            {
                                child_dir_entry.read_children_path = None;
                            }
                        }

                        // Then check explicit exclusions
                        if cloned_excluded_dirs.as_ref().is_match(relative_path)
                        {
                            child_dir_entry.read_children_path = None;
                        }
                    }
                });
            },
        );

    for entry in walk_dir {
        // I was using this to explore what directories were being walked to potentially
        // find performance improvements.
        // Write the entry out to a log file:
        // use std::io::Write;
        // let mut file = std::fs::OpenOptions::new()
        //     .create(true)
        //     .append(true)
        //     .open("tmp/pks_log.txt")
        //     .unwrap();
        // writeln!(file, "{:?}", entry).unwrap();

        let unwrapped_entry = entry;
        if let Err(_e) = unwrapped_entry {
            // Encountered an invalid symlink. Being consistent with packwerk, which swallows this error and continues
            continue;
        }
        let unwrapped_entry = unwrapped_entry.unwrap();

        // Note that we could also get the dir from absolute_path.is_dir()
        // However, this data appears to be cached on the FileType struct, so we'll use that instead,
        // which is much faster!
        if unwrapped_entry.file_type.is_dir() {
            continue;
        }

        let absolute_path = unwrapped_entry.path();

        let relative_path = absolute_path
            .strip_prefix(&absolute_root)
            .unwrap()
            .to_owned();

        // Skip gitignored files (if gitignore support is enabled)
        if let Some(gitignore) = gitignore_ref_for_loop.as_ref() {
            if gitignore.matched(&relative_path, false).is_ignore() {
                continue;
            }
        }

        let current_package_yml =
            &unwrapped_entry.client_state.current_package_yml;

        if &absolute_path == current_package_yml
            // Ideally, we don't need the second part of this conditional, but it's here
            // because there is a bug where the root pack doesn't match package_paths.
            // We know we always want the root pack to be registered, since it's the catch-all pack for
            // where constants are defined if they are not in another pack.
            // We can remove this once we fix the bug.
            && (package_paths_set.is_match(relative_path.parent().unwrap()) || absolute_path.parent().unwrap() == absolute_root)
        {
            let pack = Pack::from_path(&absolute_path, &absolute_root)?;
            included_packs.insert(pack);
        }

        // This could be one line, but I'm keeping it separate for debugging purposes
        if includes_set.is_match(&relative_path) {
            if !excludes_set.is_match(&relative_path) {
                included_files.insert(absolute_path.clone());
                owning_package_yml_for_file
                    .insert(absolute_path, current_package_yml.clone());
            } else {
                // println!("file excluded: {}", relative_path.display())
            }
        } else {
            // println!(
            //     "file not included: {:?}, {:?}",
            //     relative_path.display(),
            //     &raw.include
            // )
        }
    }

    debug!("Finished directory walk");

    Ok(WalkDirectoryResult {
        included_files,
        included_packs,
        owning_package_yml_for_file,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::packs::{
        raw_configuration::RawConfiguration, walk_directory::walk_directory,
    };

    use super::{build_gitignore_matcher, get_global_gitignore};

    #[test]
    fn test_walk_directory() -> anyhow::Result<()> {
        let absolute_path = PathBuf::from("tests/fixtures/simple_app")
            .canonicalize()
            .expect("Could not canonicalize path");

        let raw_config = RawConfiguration {
            include: vec!["**/*".to_string()],
            ..RawConfiguration::default()
        };

        let walk_directory_result =
            walk_directory(absolute_path.clone(), &raw_config);
        assert!(walk_directory_result.is_ok());
        let included_files = walk_directory_result?.included_files;

        let node_module_file =
            absolute_path.join("node_modules/subfolder/file.rb");
        let contains_bad_file = included_files.contains(&node_module_file);
        assert!(!contains_bad_file);

        let node_module_file = absolute_path.join("node_modules/file.rb");
        let contains_bad_file = included_files.contains(&node_module_file);
        assert!(!contains_bad_file);

        Ok(())
    }

    #[test]
    fn test_get_global_gitignore_returns_option() {
        // This test just ensures the function runs without panicking
        // and returns the correct type. We can't guarantee what it returns
        // since it depends on the environment.
        let result = get_global_gitignore();

        // If it returns Some, the path should exist
        if let Some(path) = result {
            assert!(path.exists());
        }
    }

    #[test]
    fn test_build_gitignore_matcher_with_simple_app() -> anyhow::Result<()> {
        let absolute_path = PathBuf::from("tests/fixtures/simple_app")
            .canonicalize()
            .expect("Could not canonicalize path");

        // Should succeed even if no .gitignore exists
        let result = build_gitignore_matcher(&absolute_path);
        assert!(result.is_ok());

        Ok(())
    }

    #[test]
    fn test_build_gitignore_matcher_returns_usable_matcher(
    ) -> anyhow::Result<()> {
        let absolute_path = PathBuf::from("tests/fixtures/simple_app")
            .canonicalize()
            .expect("Could not canonicalize path");

        let matcher = build_gitignore_matcher(&absolute_path)?;

        // The matcher should be usable (this just tests it doesn't panic)
        let test_path = PathBuf::from("test.rb");
        let _result = matcher.matched(&test_path, false);

        Ok(())
    }

    #[test]
    fn test_build_gitignore_matcher_with_malformed_gitignore(
    ) -> anyhow::Result<()> {
        use std::fs;
        use std::io::Write;

        // Create a temporary directory for this test
        let temp_dir =
            std::env::temp_dir().join("pks_test_malformed_gitignore");
        fs::create_dir_all(&temp_dir)?;

        // Create a gitignore with potentially problematic content
        // Note: The ignore crate is quite permissive, so most "malformed"
        // content is actually handled gracefully
        let gitignore_path = temp_dir.join(".gitignore");
        let mut file = fs::File::create(&gitignore_path)?;

        // Write some edge case patterns
        writeln!(file, "# This is a comment")?;
        writeln!(file)?; // Blank line
        writeln!(file, "*.log")?;
        writeln!(file, "   ")?; // Whitespace-only line
        writeln!(file, "temp/")?;

        // The matcher should build successfully even with edge cases
        let result = build_gitignore_matcher(&temp_dir);
        assert!(
            result.is_ok(),
            "Should handle gitignore with comments, blank lines, and whitespace"
        );

        // Clean up
        fs::remove_dir_all(&temp_dir)?;

        Ok(())
    }

    #[test]
    fn test_build_gitignore_matcher_with_git_info_exclude() -> anyhow::Result<()>
    {
        use std::fs;
        use std::io::Write;

        // Create a temporary directory structure
        let temp_dir = std::env::temp_dir().join("pks_test_git_exclude");
        let git_info_dir = temp_dir.join(".git/info");
        fs::create_dir_all(&git_info_dir)?;

        // Create .git/info/exclude file
        let exclude_path = git_info_dir.join("exclude");
        let mut file = fs::File::create(&exclude_path)?;
        writeln!(file, "excluded_by_git.txt")?;

        // Build matcher
        let matcher = build_gitignore_matcher(&temp_dir)?;

        // Test that the pattern from .git/info/exclude is respected
        let excluded_file = PathBuf::from("excluded_by_git.txt");
        let result = matcher.matched(&excluded_file, false);
        assert!(
            result.is_ignore(),
            "Should respect patterns from .git/info/exclude"
        );

        // Clean up
        fs::remove_dir_all(&temp_dir)?;

        Ok(())
    }
}
