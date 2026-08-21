use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

//
// For more information about this file's naming convention, see
// https://doc.rust-lang.org/book/ch11-03-test-organization.html
//

/// A private copy of a fixture app, for tests that run `pks` against it.
///
/// Tests within a binary run on parallel threads, and running `pks` writes into
/// the fixture (`tmp/cache/packwerk/...`), while several helpers in this module
/// delete or rewrite fixture files. Sharing one on-disk copy between threads
/// therefore races: one test removes a directory another is mid-way through
/// writing into.
///
/// Copying is cheap -- the largest fixture is 64 KB -- and buys real isolation,
/// so tests stay parallel and need no teardown. The copy is removed when the
/// `Fixture` is dropped.
///
/// One assumption to be aware of: the copy itself is only race-free because
/// `cargo test` runs test *binaries* sequentially. Files not yet converted to
/// this helper (`check_test.rs`, `check_unused_dependencies.rs`, and others)
/// still call `teardown()`, which deletes `tests/fixtures/*/tmp/cache/packwerk`
/// across every fixture. If that ran while `copy_dir_recursive` was mid-walk of
/// the same subtree, `read_dir`/`copy` would fail with `NotFound` and the panic
/// below would fire. Cargo finishes each binary, teardowns included, before
/// starting the next, so this cannot happen today -- but a move to
/// `cargo-nextest`, which runs binaries concurrently, would expose it. Converting
/// the remaining callers off `teardown()` removes the assumption entirely.
#[allow(dead_code)]
pub struct Fixture {
    // Held for its Drop impl: removing it deletes the copy.
    _dir: TempDir,
    root: PathBuf,
}

#[allow(dead_code)]
impl Fixture {
    /// Copies `tests/fixtures/<name>` into a fresh temporary directory.
    pub fn new(name: &str) -> Fixture {
        let dir = TempDir::new().expect("could not create temp dir");
        let root = dir.path().join(name);
        let source = Path::new("tests/fixtures").join(name);
        copy_dir_recursive(&source, &root).unwrap_or_else(|e| {
            panic!("could not copy fixture {}: {}", source.display(), e)
        });
        Fixture { _dir: dir, root }
    }

    /// The fixture root, to pass to `--project-root`.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// A path inside the fixture.
    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }
}

fn copy_dir_recursive(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        // `file_type()` does not follow symlinks, so a symlink to a directory
        // would take the `fs::copy` branch below and fail with a directory
        // target. No fixture contains a symlink today (`find tests/fixtures
        // -type l` is empty); handle it here if one ever needs to.
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[allow(dead_code)]
pub fn teardown() {
    glob::glob("tests/fixtures/*/tmp/cache/packwerk")
        .expect("Failed to read glob pattern")
        .filter_map(Result::ok)
        .for_each(|cache_dir| {
            if let Err(err) = fs::remove_dir_all(&cache_dir) {
                eprintln!(
                    "Failed to remove {} during test teardown: {}",
                    cache_dir.display(),
                    err
                );
            }
        });
}

#[allow(dead_code)]
pub fn delete_foobar() {
    let directory = PathBuf::from("tests/fixtures/simple_app/packs/foobar");
    if let Err(err) = fs::remove_dir_all(directory) {
        eprintln!(
            "Failed to remove tests/fixtures/simple_app/packs/foobar during test teardown: {}",
            err
        );
    }
}

#[allow(dead_code)]
pub fn delete_foobaz() {
    let directory =
        PathBuf::from("tests/fixtures/simple_packs_first_app/packs/foobaz");
    if let Err(err) = fs::remove_dir_all(directory) {
        eprintln!(
            "Failed to remove tests/fixtures/simple_packs_first_app/packs/foobaz during test teardown: {}",
            err
        );
    }
}

#[allow(dead_code)]
pub fn delete_foobar_app_with_custom_readme() {
    let directory =
        PathBuf::from("tests/fixtures/app_with_custom_readme/packs/foobar");
    if let Err(err) = fs::remove_dir_all(directory) {
        eprintln!(
            "Failed to remove tests/fixtures/app_with_custom_readme/packs/foobar during test teardown: {}",
            err
        );
    }
}

// The round-trip strict-mode fixture. Its todo file records a strict violation,
// which is the state `check` tolerance depends on, and its source file provides
// the reference that entry points at. Tests here mutate both, so they restore
// via the guard below rather than a trailing call: a panicking test would skip
// a trailing restore and leave a deleted fixture in the tree, which the
// pre-commit hook makes easy to commit by accident.
#[allow(dead_code)]
pub const ROUND_TRIP_TODO_PATH: &str =
    "tests/fixtures/uses_strict_mode_round_trip/packs/foo/package_todo.yml";

#[allow(dead_code)]
pub const ROUND_TRIP_SOURCE_PATH: &str =
    "tests/fixtures/uses_strict_mode_round_trip/packs/foo/app/services/foo.rb";

// Violation types are in sorted order, matching what `update` writes, so tests
// can assert byte equality against this rather than only grepping for a key.
#[allow(dead_code)]
pub const ROUND_TRIP_TODO: &str = "\
# This file contains a list of dependencies that are not part of the long term plan for the
# 'packs/foo' package.
# We should generally work to reduce this list over time.
#
# You can regenerate this file using the following command:
#
# bin/packwerk update-todo
---
packs/bar:
  \"::Bar\":
    violations:
    - dependency
    - privacy
    files:
    - packs/foo/app/services/foo.rb
";

#[allow(dead_code)]
pub const ROUND_TRIP_SOURCE: &str = "\
module Foo
  def calls_bar_without_stated_dependency
    Bar
  end
end
";

/// Restores the round-trip fixture when it goes out of scope, panic or not.
///
/// Bind it to a named variable, `let _fixture = RoundTripFixture::set_up();`.
/// Binding to `let _` drops it immediately and silently removes the protection,
/// which `#[must_use]` cannot catch.
#[allow(dead_code)]
#[must_use = "bind this to a named variable; `let _` drops the guard immediately"]
pub struct RoundTripFixture;

#[allow(dead_code)]
impl RoundTripFixture {
    pub fn set_up() -> Self {
        // Panicking here is fine and informative: the test has not run yet.
        fs::write(ROUND_TRIP_TODO_PATH, ROUND_TRIP_TODO).unwrap();
        fs::write(ROUND_TRIP_SOURCE_PATH, ROUND_TRIP_SOURCE).unwrap();
        Self
    }
}

impl Drop for RoundTripFixture {
    fn drop(&mut self) {
        // Deliberately does not unwrap. This runs while a failing test is
        // unwinding, and a panic here would be a panic-during-panic, which
        // aborts the whole test binary and replaces the real failure with an
        // abort. Report and carry on, as `teardown()` does.
        for (path, contents) in [
            (ROUND_TRIP_TODO_PATH, ROUND_TRIP_TODO),
            (ROUND_TRIP_SOURCE_PATH, ROUND_TRIP_SOURCE),
        ] {
            if let Err(err) = fs::write(path, contents) {
                eprintln!(
                    "Failed to restore {} during teardown: {}",
                    path, err
                );
            }
        }
    }
}

// In case we want our tests to call `update` or otherwise mutate the file system
#[allow(dead_code)]
pub fn set_up_fixtures() {
    let contains_stale_violations_bar_todo = String::from("\
# This file contains a list of dependencies that are not part of the long term plan for the
# 'packs/foo' package.
# We should generally work to reduce this list over time.
#
# You can regenerate this file using the following command:
#
# bin/packwerk update-todo
---
packs/foo:
  \"::Foo\":
    violations:
    - dependency
    - privacy
    files:
    - packs/bar/app/services/bar.rb

");

    // Rewrite tests/fixtures/contains_stale_violations/packs/bar/package_todo.yml with the above contents,
    // whether it is present or not:
    fs::write(
        "tests/fixtures/contains_stale_violations/packs/bar/package_todo.yml",
        contains_stale_violations_bar_todo,
    )
    .unwrap();

    let contains_stale_violations_foo_todo = String::from("\
# This file contains a list of dependencies that are not part of the long term plan for the
# 'packs/foo' package.
# We should generally work to reduce this list over time.
#
# You can regenerate this file using the following command:
#
# bin/packwerk update-todo
---
packs/bar:
  \"::Bar\":
    violations:
    - dependency
    - privacy
    files:
    - packs/foo/app/services/foo.rb
");

    fs::write(
        "tests/fixtures/contains_stale_violations/packs/foo/package_todo.yml",
        contains_stale_violations_foo_todo,
    )
    .unwrap();

    let pack_yml = PathBuf::from(
        "tests/fixtures/app_with_missing_dependency/packs/foo/package.yml",
    );
    let pack_yml_contents = String::from(
        "\
enforce_dependencies: true
dependencies:
- packs/bar
",
    );

    fs::write(pack_yml, pack_yml_contents).unwrap();

    let pack_yml = PathBuf::from(
        "tests/fixtures/app_with_missing_dependency/packs/bar/package.yml",
    );
    let pack_yml_contents = String::from(
        "\
enforce_dependencies: true
",
    );

    fs::write(pack_yml, pack_yml_contents).unwrap();

    let pack_yml = PathBuf::from(
        "tests/fixtures/app_with_missing_dependency/packs/baz/package.yml",
    );
    let pack_yml_contents = String::from(
        "\
enforce_dependencies: true
",
    );

    fs::write(pack_yml, pack_yml_contents).unwrap();

    let pack_yml = PathBuf::from(
        "tests/fixtures/app_with_unnecessary_dependencies/packs/foo/package.yml",
    );
    let pack_yml_contents = String::from(
        "\
enforce_dependencies: true
enforce_privacy: true
layer: technical_services
dependencies:
- packs/bar
- packs/baz
",
    );

    fs::write(pack_yml, pack_yml_contents).unwrap();

    let pack_yml = PathBuf::from(
        "tests/fixtures/app_with_missing_dependencies/packs/baz/package.yml",
    );
    let pack_yml_contents = String::from(
        "\
enforce_dependencies: true
",
    );

    fs::write(pack_yml, pack_yml_contents).unwrap();
}
