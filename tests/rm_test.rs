use assert_cmd::{assert::Assert, cargo::cargo_bin_cmd};
use predicates::prelude::*;
use pretty_assertions::assert_eq;
use std::{collections::BTreeMap, error::Error, fs, path::Path};

mod common;

// In the fixture, `packs/old` is about to be removed and every other pack
// still names it: the root and `packs/foo` depend on it, `packs/bar` and
// `packs/baz` list it in `visible_to`, `packs/baz` ignores it as a dependency,
// and the todo files of `packs/foo` and `packs/bar` record violations on it.
// No code references `Old` any more, so those todo entries are stale, as they
// are once the last use of a pack is deleted.

fn pks(fixture: &common::Fixture, args: &[&str]) -> Assert {
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg(fixture.root())
        .args(args)
        .assert()
}

fn read(fixture: &common::Fixture, path: &str) -> String {
    fs::read_to_string(fixture.path(path))
        .unwrap_or_else(|e| panic!("Could not read {}: {}", path, e))
}

fn snapshot(fixture: &common::Fixture) -> BTreeMap<String, String> {
    fn walk(root: &Path, dir: &Path, files: &mut BTreeMap<String, String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                let relative = path.strip_prefix(root).unwrap();
                files.insert(
                    relative.display().to_string(),
                    fs::read_to_string(&path).unwrap(),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(fixture.root(), fixture.root(), &mut files);
    files
}

fn reference_old_from_baz(fixture: &common::Fixture) {
    fs::write(
        fixture.path("packs/baz/app/services/baz.rb"),
        "module Baz\n  def self.old\n    Old\n  end\nend\n",
    )
    .unwrap();
}

#[test]
fn test_rm() -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("app_with_obsolete_pack");

    pks(&fixture, &["rm", "packs/old"])
        .success()
        .stdout(
            "\
Removed `packs/old` from ./package.yml (dependencies)
Removed `packs/old` from packs/bar/package.yml (visible_to)
Deleted packs/bar/package_todo.yml, which only recorded violations on `packs/old`
Removed `packs/old` from packs/baz/package.yml (ignored_dependencies, visible_to)
Removed `packs/old` from packs/foo/package.yml (dependencies)
Removed recorded violations on `packs/old` from packs/foo/package_todo.yml
Successfully removed `packs/old`!
",
        )
        .stderr("");

    assert!(!fixture.path("packs/old").exists());
    assert!(!fixture.path("packs/bar/package_todo.yml").exists());
    assert_eq!(read(&fixture, "package.yml"), "");
    // The comment above the removed entry goes with it.
    assert_eq!(
        read(&fixture, "packs/foo/package.yml"),
        "\
# Foo's package.
enforce_dependencies: true
dependencies:
- packs/baz
"
    );
    assert_eq!(
        read(&fixture, "packs/bar/package.yml"),
        "enforce_visibility: true\nvisible_to:\n- packs/foo\n"
    );
    assert_eq!(
        read(&fixture, "packs/baz/package.yml"),
        "enforce_visibility: true\nvisible_to: []\n"
    );
    assert_eq!(
        read(&fixture, "packs/foo/package_todo.yml"),
        "\
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
    files:
    - packs/foo/app/services/foo.rb
"
    );

    pks(&fixture, &["validate"]).success();
    pks(&fixture, &["check"]).success();

    Ok(())
}

#[test]
fn test_rm_accepts_a_trailing_slash() -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("app_with_obsolete_pack");

    pks(&fixture, &["rm", "packs/old/"]).success().stdout(
        predicate::str::contains("Successfully removed `packs/old`!"),
    );
    assert!(!fixture.path("packs/old").exists());

    Ok(())
}

#[test]
fn test_rm_refuses_a_pack_other_packs_reference() -> Result<(), Box<dyn Error>>
{
    let fixture = common::Fixture::new("app_with_obsolete_pack");
    reference_old_from_baz(&fixture);
    let before = snapshot(&fixture);

    pks(&fixture, &["rm", "packs/old"])
        .code(2)
        .stdout("packs/baz/app/services/baz.rb:3:4 references ::Old\n")
        .stderr(
            "Error: Found 1 reference from other packs to constants in \
             `packs/old`. Remove them, or pass `--force` to remove the pack \
             anyway.\n",
        );

    assert_eq!(snapshot(&fixture), before);

    Ok(())
}

#[test]
fn test_rm_force_removes_a_pack_other_packs_reference(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("app_with_obsolete_pack");
    reference_old_from_baz(&fixture);

    pks(&fixture, &["rm", "--force", "packs/old"])
        .success()
        .stdout(predicate::str::contains(
            "Successfully removed `packs/old`!",
        ));

    assert!(!fixture.path("packs/old").exists());
    // `Old` no longer resolves, so the reference left in baz.rb goes unreported.
    pks(&fixture, &["check"]).success();

    Ok(())
}

#[test]
fn test_rm_refuses_a_pack_with_lib_code_pks_cannot_resolve(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("app_with_obsolete_pack");
    fs::create_dir_all(fixture.path("packs/old/lib/old"))?;
    fs::write(
        fixture.path("packs/old/lib/old/helper.rb"),
        "module Old\n  module Helper\n    def self.help; end\n  end\nend\n",
    )?;
    // Rake task support files are not counted.
    fs::create_dir_all(fixture.path("packs/old/lib/tasks"))?;
    fs::write(fixture.path("packs/old/lib/tasks/old.rb"), "")?;
    let before = snapshot(&fixture);

    pks(&fixture, &["rm", "packs/old"])
        .code(2)
        .stdout("")
        .stderr(
        "Error: pks cannot resolve constants defined in `packs/old/lib` (1 \
         Ruby file), which is outside the autoload roots, so it cannot tell \
         whether other packs use them. Make sure none do, then pass `--force` \
         to remove the pack.\n",
    );
    assert_eq!(snapshot(&fixture), before);

    // The experimental parser reads definitions from every file, lib/ included.
    pks(&fixture, &["--experimental-parser", "rm", "packs/old"]).success();
    assert!(!fixture.path("packs/old").exists());

    Ok(())
}

#[test]
fn test_rm_refuses_with_both_reasons_at_once() -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("app_with_obsolete_pack");
    reference_old_from_baz(&fixture);
    fs::create_dir_all(fixture.path("packs/old/lib"))?;
    fs::write(fixture.path("packs/old/lib/a.rb"), "")?;
    fs::write(fixture.path("packs/old/lib/b.rb"), "")?;

    pks(&fixture, &["rm", "packs/old"])
        .code(2)
        .stdout("packs/baz/app/services/baz.rb:3:4 references ::Old\n")
        .stderr(
            "Error: Found 1 reference from other packs to constants in \
             `packs/old`. pks also cannot resolve constants defined in \
             `packs/old/lib` (2 Ruby files), which is outside the autoload \
             roots, so it cannot tell whether other packs use them. Remove \
             the references and make sure nothing uses those constants, then \
             pass `--force` to remove the pack.\n",
        );
    assert!(fixture.path("packs/old").exists());

    Ok(())
}

#[test]
fn test_rm_refuses_the_root_pack() -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("app_with_obsolete_pack");

    pks(&fixture, &["rm", "--force", "."])
        .code(2)
        .stderr("Error: The root pack cannot be removed\n");
    assert!(fixture.path("package.yml").exists());

    Ok(())
}

#[test]
fn test_rm_refuses_a_pack_containing_other_packs() -> Result<(), Box<dyn Error>>
{
    let fixture = common::Fixture::new("app_with_obsolete_pack");
    fs::create_dir_all(fixture.path("packs/old/nested"))?;
    fs::write(fixture.path("packs/old/nested/package.yml"), "")?;
    let before = snapshot(&fixture);

    // Not even with `--force`: references to the nested pack would be left.
    pks(&fixture, &["rm", "--force", "packs/old"])
        .code(2)
        .stderr(
            "Error: `packs/old` contains other packs, which must be removed \
         first: packs/old/nested\n",
        );
    assert_eq!(snapshot(&fixture), before);

    Ok(())
}

#[test]
fn test_rm_unknown_pack() -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("app_with_obsolete_pack");

    pks(&fixture, &["rm", "packs/nope"])
        .code(2)
        .stderr("Error: No pack found 'packs/nope'\n");

    Ok(())
}

#[test]
fn test_rm_rewrites_a_package_yml_it_cannot_edit_in_place(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("app_with_obsolete_pack");
    fs::write(
        fixture.path("packs/foo/package.yml"),
        "# Lost in the rewrite.\nenforce_dependencies: true\ndependencies: [packs/old, packs/baz]\n",
    )?;

    pks(&fixture, &["rm", "packs/old"]).success().stderr(
        "Warning: could not edit the dependencies list in \
         packs/foo/package.yml in place, so the file was rewritten and its \
         comments were not preserved.\n",
    );
    assert_eq!(
        read(&fixture, "packs/foo/package.yml"),
        "enforce_dependencies: true\ndependencies:\n- packs/baz\n"
    );

    Ok(())
}
