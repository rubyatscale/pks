//! End-to-end flows for `check-unused-dependencies --auto-correct`.
//!
//! Each test builds a small project in a temporary directory, runs `pks` the
//! way a developer would, and compares every file in the project before and
//! after: the edits we intend must happen, and no other file may change.

use assert_cmd::cargo::cargo_bin_cmd;
use pretty_assertions::assert_eq;
use std::{collections::BTreeMap, fs, path::Path};
use tempfile::TempDir;

const LIBRARIES: [&str; 4] = ["alpha", "beta", "gamma", "delta"];

/// A project with one library pack per name in `LIBRARIES`, each defining a
/// module of the same name, plus the consumer packs given to `new`.
struct Project {
    dir: TempDir,
}

impl Project {
    fn new() -> Project {
        let project = Project {
            dir: TempDir::new().unwrap(),
        };
        project.write("packwerk.yml", "cache: false\n");
        project.write("package.yml", "enforce_dependencies: true\n");
        for name in LIBRARIES {
            project.write(
                &format!("packs/{name}/package.yml"),
                "enforce_dependencies: true\n",
            );
            project.write(
                &format!("packs/{name}/app/services/{name}.rb"),
                &format!("module {}\nend\n", module_name(name)),
            );
        }
        project
    }

    /// Adds a pack whose code references the modules of `uses`.
    fn consumer(self, name: &str, package_yml: &str, uses: &[&str]) -> Self {
        self.write(&format!("packs/{name}/package.yml"), package_yml);
        self.uses(name, uses);
        self
    }

    /// Replaces the consumer's code so that it references only `uses`.
    fn uses(&self, name: &str, uses: &[&str]) {
        let body: String = uses
            .iter()
            .map(|library| format!("    {}\n", module_name(library)))
            .collect();
        self.write(
            &format!("packs/{name}/app/services/{name}_service.rb"),
            &format!(
                "class {}Service\n  def call\n{body}  end\nend\n",
                module_name(name)
            ),
        );
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.dir.path().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.dir.path().join(relative)).unwrap()
    }

    /// Every file in the project except pks's cache, keyed by relative path.
    fn snapshot(&self) -> BTreeMap<String, String> {
        let mut files = BTreeMap::new();
        collect(self.dir.path(), self.dir.path(), &mut files);
        files
    }

    fn pks(&self, args: &[&str]) -> assert_cmd::assert::Assert {
        cargo_bin_cmd!("pks")
            .env_remove("RUST_LOG")
            .arg("--project-root")
            .arg(self.dir.path())
            .args(args)
            .assert()
    }

    /// Runs auto-correct and returns its stderr.
    fn auto_correct(&self) -> String {
        let output = self
            .pks(&["check-unused-dependencies", "--auto-correct"])
            .success()
            .get_output()
            .clone();
        String::from_utf8(output.stderr).unwrap()
    }

    /// The project is consistent: nothing unused, nothing missing, and every
    /// package.yml loads.
    fn assert_clean(&self) {
        self.pks(&["check-unused-dependencies"]).success();
        self.pks(&["check"]).success();
        self.pks(&["validate"]).success();
    }
}

fn collect(root: &Path, dir: &Path, files: &mut BTreeMap<String, String>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let relative = path.strip_prefix(root).unwrap().to_str().unwrap();
        if relative == "tmp" {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, files);
        } else {
            files.insert(
                relative.to_string(),
                fs::read_to_string(&path).unwrap(),
            );
        }
    }
}

fn module_name(pack: &str) -> String {
    let mut chars = pack.chars();
    let first = chars.next().unwrap().to_ascii_uppercase();
    format!("{first}{}", chars.as_str())
}

/// Asserts that exactly the files in `expected` changed, to those contents.
fn assert_changes(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
    expected: &[(&str, &str)],
) {
    let mut want = before.clone();
    for (path, contents) in expected {
        assert!(want.contains_key(*path), "{path} is not in the project");
        want.insert(path.to_string(), contents.to_string());
    }
    assert_eq!(after, &want);
}

/// The layout `packwerk init` and `pks create` start from, after a developer
/// has filled in a few dependencies and commented on them.
#[test]
fn removes_one_unused_dependency_from_a_documented_package_yml() {
    let before_yml = "\
# Turn on dependency checks for this package
enforce_dependencies: true

# Turn on privacy checks for this package
enforce_privacy: true

# A list of this package's dependencies
# Note that packages in this list require their own `package.yml` file
dependencies:
  # Shared models.
  - packs/alpha
  # Kept until the beta migration lands.
  - packs/beta
  - packs/gamma # used by the importer

# Specify which packages are allowed to see this one
visible_to:
  - packs/delta
";
    let project =
        Project::new().consumer("app", before_yml, &["alpha", "gamma"]);
    let before = project.snapshot();

    assert_eq!(project.auto_correct(), "");

    // The beta comment stays: it covers every entry down to the next blank
    // line or comment, and gamma is still one of them.
    let after_yml = "\
# Turn on dependency checks for this package
enforce_dependencies: true

# Turn on privacy checks for this package
enforce_privacy: true

# A list of this package's dependencies
# Note that packages in this list require their own `package.yml` file
dependencies:
  # Shared models.
  - packs/alpha
  # Kept until the beta migration lands.
  - packs/gamma # used by the importer

# Specify which packages are allowed to see this one
visible_to:
  - packs/delta
";
    assert_changes(
        &before,
        &project.snapshot(),
        &[("packs/app/package.yml", after_yml)],
    );
    project.assert_clean();
}

#[test]
fn removes_a_comment_with_the_only_entry_it_covers() {
    let project = Project::new().consumer(
        "app",
        "dependencies:\n  - packs/alpha\n\n  # Kept until the beta migration lands.\n  - packs/beta\n\n  - packs/gamma\n",
        &["alpha", "gamma"],
    );
    let before = project.snapshot();

    assert_eq!(project.auto_correct(), "");

    assert_changes(
        &before,
        &project.snapshot(),
        &[(
            "packs/app/package.yml",
            "dependencies:\n  - packs/alpha\n\n  - packs/gamma\n",
        )],
    );
    project.assert_clean();
}

#[test]
fn edits_every_pack_with_unused_dependencies_and_no_other_file() {
    let project = Project::new()
        .consumer(
            "orders",
            "dependencies:\n# Keep sorted.\n- packs/alpha\n- packs/beta\n",
            &["beta"],
        )
        .consumer(
            "billing",
            "enforce_dependencies: true # strict soon\ndependencies:\n- packs/gamma\n- packs/delta\n",
            &["gamma"],
        )
        .consumer(
            "shipping",
            "# Already tidy.\ndependencies:\n- packs/alpha\n",
            &["alpha"],
        );
    let before = project.snapshot();

    assert_eq!(project.auto_correct(), "");

    assert_changes(
        &before,
        &project.snapshot(),
        &[
            (
                "packs/orders/package.yml",
                "dependencies:\n# Keep sorted.\n- packs/beta\n",
            ),
            (
                "packs/billing/package.yml",
                "enforce_dependencies: true # strict soon\ndependencies:\n- packs/gamma\n",
            ),
        ],
    );
    project.assert_clean();
}

#[test]
fn a_second_run_changes_nothing() {
    let project = Project::new().consumer(
        "app",
        "dependencies:\n- packs/alpha\n# - packs/old\n- packs/beta\n",
        &["alpha"],
    );
    project.auto_correct();
    let after_first = project.snapshot();

    assert_eq!(project.auto_correct(), "");

    assert_eq!(project.snapshot(), after_first);
    assert_eq!(
        project.read("packs/app/package.yml"),
        "dependencies:\n- packs/alpha\n# - packs/old\n"
    );
}

#[test]
fn removing_the_last_dependency_keeps_the_key_while_comments_remain() {
    let before_yml = "\
enforce_dependencies: true
dependencies:
# - packs/delta  # re-add after the migration
- packs/alpha
- packs/beta
# Visibility, narrowest first:
# - only the delta pack may see this one
visible_to:
- packs/delta
";
    let project = Project::new().consumer("app", before_yml, &[]);
    let before = project.snapshot();

    assert_eq!(project.auto_correct(), "");

    let after_yml = "\
enforce_dependencies: true
dependencies:
# - packs/delta  # re-add after the migration
# Visibility, narrowest first:
# - only the delta pack may see this one
visible_to:
- packs/delta
";
    assert_changes(
        &before,
        &project.snapshot(),
        &[("packs/app/package.yml", after_yml)],
    );
    project.assert_clean();
}

#[test]
fn removing_the_last_dependency_removes_an_uncommented_block() {
    let project = Project::new().consumer(
        "app",
        "enforce_dependencies: true\ndependencies:\n  - packs/alpha\n  - packs/beta\n\nenforce_privacy: true\n",
        &[],
    );
    let before = project.snapshot();

    assert_eq!(project.auto_correct(), "");

    assert_changes(
        &before,
        &project.snapshot(),
        &[(
            "packs/app/package.yml",
            "enforce_dependencies: true\nenforce_privacy: true\n",
        )],
    );
    project.assert_clean();
}

/// A developer drops one usage, runs auto-correct, then drops another and runs
/// it again. The file must end up the same as when both go in one run.
#[test]
fn removing_dependencies_over_several_runs_matches_one_run() {
    let before_yml = "\
enforce_dependencies: true
dependencies:
# Core.
- packs/alpha
- packs/beta
# note

- packs/gamma
# - packs/old
- packs/delta
enforce_privacy: true
";
    let incremental = Project::new().consumer(
        "app",
        before_yml,
        &["alpha", "beta", "gamma", "delta"],
    );
    let mut steps = vec![];
    for uses in [
        &["alpha", "beta", "gamma"][..],
        &["beta", "gamma"],
        &["beta"],
        &[],
    ] {
        incremental.uses("app", uses);
        assert_eq!(incremental.auto_correct(), "");
        incremental.assert_clean();
        steps.push(incremental.read("packs/app/package.yml"));
    }

    for (index, uses) in [
        &["alpha", "beta", "gamma"][..],
        &["beta", "gamma"],
        &["beta"],
        &[],
    ]
    .iter()
    .enumerate()
    {
        let batch = Project::new().consumer("app", before_yml, uses);
        assert_eq!(batch.auto_correct(), "");
        assert_eq!(
            batch.read("packs/app/package.yml"),
            steps[index],
            "after removing all but {uses:?}"
        );
    }
    assert_eq!(
        steps.last().unwrap(),
        "\
enforce_dependencies: true
dependencies:
# note

# - packs/old
enforce_privacy: true
"
    );
}

#[test]
fn leaves_other_lists_that_name_the_same_packs_alone() {
    let before_yml = "\
metadata:
  owner: team-a
  dependencies:
  - packs/beta
dependencies:
- packs/alpha
- packs/beta
ignored_dependencies:
# Would cycle.
- packs/beta
visible_to:
- packs/beta
";
    let project = Project::new().consumer("app", before_yml, &["alpha"]);
    let before = project.snapshot();

    assert_eq!(project.auto_correct(), "");

    assert_changes(
        &before,
        &project.snapshot(),
        &[(
            "packs/app/package.yml",
            "\
metadata:
  owner: team-a
  dependencies:
  - packs/beta
dependencies:
- packs/alpha
ignored_dependencies:
# Would cycle.
- packs/beta
visible_to:
- packs/beta
",
        )],
    );
}

#[test]
fn preserves_crlf_line_endings_and_a_missing_final_newline() {
    let project = Project::new()
        .consumer(
            "windows",
            "# Edited on Windows.\r\ndependencies:\r\n- packs/alpha\r\n- packs/beta\r\n",
            &["alpha"],
        )
        .consumer(
            "unterminated",
            "dependencies:\n- packs/alpha\n- packs/beta\nenforce_privacy: true",
            &["alpha"],
        );
    let before = project.snapshot();

    assert_eq!(project.auto_correct(), "");

    assert_changes(
        &before,
        &project.snapshot(),
        &[
            (
                "packs/windows/package.yml",
                "# Edited on Windows.\r\ndependencies:\r\n- packs/alpha\r\n",
            ),
            (
                "packs/unterminated/package.yml",
                "dependencies:\n- packs/alpha\nenforce_privacy: true",
            ),
        ],
    );
    project.assert_clean();
}

/// A list the text edit can't handle is rewritten as before, with a warning,
/// and the other packs in the same run are still edited in place.
#[test]
fn falls_back_to_rewriting_only_the_files_it_cannot_edit() {
    let project = Project::new()
        .consumer(
            "flow",
            "# Lost by the rewrite.\nenforce_privacy: true\ndependencies: [packs/alpha, packs/gamma, packs/beta]\n",
            &["alpha"],
        )
        .consumer(
            "block",
            "# Kept.\ndependencies:\n- packs/alpha\n- packs/beta\n",
            &["alpha"],
        );
    let before = project.snapshot();

    let stderr = project.auto_correct();

    assert_eq!(
        stderr,
        "Warning: could not edit the dependencies list in \
         `packs/flow/package.yml` in place to remove `packs/beta`, \
         `packs/gamma`, so the file was rewritten: its comments were not \
         preserved and its keys may be reordered.\n"
    );
    assert_changes(
        &before,
        &project.snapshot(),
        &[
            (
                "packs/flow/package.yml",
                "enforce_privacy: true\ndependencies:\n- packs/alpha\n",
            ),
            (
                "packs/block/package.yml",
                "# Kept.\ndependencies:\n- packs/alpha\n",
            ),
        ],
    );
    project.assert_clean();
}

#[test]
fn without_auto_correct_reports_and_writes_nothing() {
    let project = Project::new().consumer(
        "app",
        "dependencies:\n- packs/alpha\n- packs/beta\n",
        &["alpha"],
    );
    let before = project.snapshot();

    project
        .pks(&["check-unused-dependencies"])
        .failure()
        .stdout(predicates::str::contains(
            "packs/app depends on packs/beta but does not use it",
        ));

    assert_eq!(project.snapshot(), before);
}

#[test]
fn never_removes_a_dependency_that_is_used() {
    let before_yml = "\
dependencies:
- packs/alpha
- \"packs/beta\"
- 'packs/gamma' # quoted
- packs/delta
";
    let project = Project::new().consumer(
        "app",
        before_yml,
        &["alpha", "beta", "gamma", "delta"],
    );
    let before = project.snapshot();

    project
        .pks(&["check-unused-dependencies", "--auto-correct"])
        .success();

    assert_eq!(project.snapshot(), before);
    project.assert_clean();
}
