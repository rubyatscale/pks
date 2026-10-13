use dua_core::{DirectoryId, Entry, Options, Order};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    num::NonZeroUsize,
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

/// A name that starts with a dot. pks skips these, as packwerk's globs don't
/// match them.
fn is_hidden(file_name: &OsStr) -> bool {
    file_name.to_str().is_some_and(|name| name.starts_with('.'))
}

struct Directory {
    id: DirectoryId,
    parent: Option<DirectoryId>,
    depth: usize,
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

// We use dua-core to walk directories in parallel and compare them to the `include` and `exclude` patterns
// specified in the `RawConfiguration`
// https://docs.rs/dua-core/4.1.0/dua_core/fn.walk.html
// We only walk the directory once and pull all of the information we need from it,
// which is faster than walking the directory multiple times.
// Likely, we can organize this better by moving each piece of logic into its own function so this function
// allows for a sort of "visitor pattern" for different things that need to walk the directory.
//
// Note: `ignore::WalkBuilder` (used by ripgrep) handles gitignore natively and supports
// parallel traversal via `build_parallel()`. We use dua-core here because each entry it
// yields names the directory it's in, so we can propagate `current_package_yml` from parent
// to child directories without walking each file's ancestors. WalkBuilder's visitor pattern
// has no equivalent, so implementing that tracking would require per-file ancestor walks,
// an O(depth) regression for large repos.
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
    // The walk asks before it reads each directory, which lets us skip directories,
    // which we do as a performance optimization.
    //
    // Specifically – if an exclude glob matches an entire directory, we don't need to continue to
    // explore it. For example, instead of asking every file in `vendor/bundle/**/` if it should be excluded,
    // we'll save a lot of time by just skipping the entire directory.
    let read_directory = move |entry: &Entry| {
        if entry.depth == 0 {
            return true;
        }
        if is_hidden(&entry.file_name) {
            return false;
        }

        let absolute_dirname = entry.path();
        let relative_path = absolute_dirname
            .strip_prefix(absolute_root_ref.as_ref())
            .unwrap();

        // Check gitignore for directories (optimization: prune ignored directory trees early)
        // Files are checked separately in the main file processing loop below
        if let Some(gitignore) = gitignore_ref.as_ref() {
            if gitignore.matched(relative_path, true).is_ignore() {
                return false;
            }
        }

        // Then check explicit exclusions
        !excluded_dirs_ref.is_match(relative_path)
    };

    let threads =
        std::thread::available_parallelism().map_or(1, NonZeroUsize::get);
    let dua_walk = dua_core::walk(
        &absolute_root,
        threads,
        Order::Completion,
        Options::default().skip_metadata(),
        read_directory,
    );

    // dua-core's workers stop once a few batches of entries are waiting for
    // this loop, as they are while it parses a package.yml. This thread takes
    // the entries as they come, so the workers keep walking. A dua-core
    // release with https://github.com/Byron/dua-cli/pull/413 would make it
    // unnecessary.
    let (sender, walk) = std::sync::mpsc::channel();
    let receiving = std::thread::spawn(move || {
        for entry in dua_walk {
            if sender.send(entry).is_err() {
                break;
            }
        }
    });

    // A file can come before the directory it's in, and before the
    // package.yml there, so which package.yml each file belongs to is
    // settled once the walk is done
    let mut directories: Vec<Directory> = Vec::new();
    let mut package_ymls_in_directories: HashMap<DirectoryId, PathBuf> =
        HashMap::new();
    let mut included_files_in_directories: Vec<(PathBuf, DirectoryId)> =
        Vec::new();

    for entry in walk {
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

        let Ok(entry) = entry else {
            // Couldn't read a directory or entry. Being consistent with packwerk, which swallows this error and continues
            continue;
        };
        if entry.depth > 0 && is_hidden(&entry.file_name) {
            continue;
        }

        if let Some(id) = entry.directory_id {
            directories.push(Directory {
                id,
                parent: entry.parent_directory_id,
                depth: entry.depth,
            });
            continue;
        }

        let absolute_path = entry.path();

        // The walk doesn't follow symlinks, and packwerk's globs don't
        // descend into a symlinked directory either. A symlink to a file
        // counts as the file.
        if entry.file_type.is_symlink() && !absolute_path.is_file() {
            continue;
        }

        let directory = entry
            .parent_directory_id
            .expect("every entry but the root is in a directory");
        let is_package_yml = entry.file_name == "package.yml";
        if is_package_yml {
            package_ymls_in_directories
                .insert(directory, absolute_path.clone());
        }

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

        // A package.yml is the one the files in its directory belong to
        if is_package_yml
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
                included_files_in_directories.push((absolute_path, directory));
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

    if let Err(panic) = receiving.join() {
        std::panic::resume_unwind(panic);
    }

    // The package.yml that the files in each directory belong to. A
    // directory is shallower than those in it, so its parent's is known first.
    directories.sort_unstable_by_key(|directory| directory.depth);
    let mut package_ymls: HashMap<DirectoryId, PathBuf> = HashMap::new();
    for directory in directories {
        // Even if the parent has one, the existence of a new package.yml
        // file should override it.
        let package_yml = match package_ymls_in_directories.get(&directory.id) {
            Some(package_yml) => package_yml.clone(),
            None => match directory.parent {
                Some(parent) => package_ymls[&parent].clone(),
                None => PathBuf::from("package.yml"),
            },
        };
        package_ymls.insert(directory.id, package_yml);
    }
    for (absolute_path, directory) in included_files_in_directories {
        owning_package_yml_for_file
            .insert(absolute_path, package_ymls[&directory].clone());
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
