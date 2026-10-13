use assert_cmd::cargo::cargo_bin_cmd;
use std::{collections::BTreeSet, error::Error, fs};

mod common;

/// The files `list-included-files` lists, relative to the fixture's root
fn included_files(
    fixture: &common::Fixture,
) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let output = common::pks(fixture, &["--no-cache", "list-included-files"])
        .success()
        .get_output()
        .stdout
        .clone();
    // pks prints canonical paths, and the fixture's can go through a symlink,
    // as macOS's /var does
    let root = format!("{}/", fixture.root().canonicalize()?.display());
    Ok(String::from_utf8(output)?
        .lines()
        .map(|line| line.strip_prefix(&root).unwrap_or(line).to_owned())
        .collect())
}

#[test]
fn test_list_included_files_skips_hidden_files_and_directories(
) -> Result<(), Box<dyn Error>> {
    // As packwerk's globs don't match them
    let fixture = common::Fixture::new("simple_app");
    fs::create_dir_all(fixture.path("packs/foo/.hidden/app"))?;
    fs::write(fixture.path("packs/foo/.hidden/app/hidden.rb"), "")?;
    fs::write(fixture.path("packs/foo/.dotfile.rb"), "")?;

    let files = included_files(&fixture)?;
    assert!(
        files.contains("packs/foo/app/services/foo.rb"),
        "{:?}",
        files
    );
    assert!(
        !files.contains("packs/foo/.hidden/app/hidden.rb"),
        "{:?}",
        files
    );
    assert!(!files.contains("packs/foo/.dotfile.rb"), "{:?}", files);

    Ok(())
}

#[cfg(unix)]
#[test]
fn test_list_included_files_lists_a_symlink_to_a_file(
) -> Result<(), Box<dyn Error>> {
    use std::os::unix::fs::symlink;

    let fixture = common::Fixture::new("simple_app");
    let outside = tempfile::tempdir()?;
    fs::write(outside.path().join("linked.rb"), "")?;
    let services = fixture.path("packs/foo/app/services");
    symlink(outside.path().join("linked.rb"), services.join("linked.rb"))?;
    symlink(services.join("missing.rb"), services.join("broken.rb"))?;

    let files = included_files(&fixture)?;
    assert!(
        files.contains("packs/foo/app/services/linked.rb"),
        "{:?}",
        files
    );
    assert!(
        !files.contains("packs/foo/app/services/broken.rb"),
        "{:?}",
        files
    );

    Ok(())
}

#[cfg(unix)]
#[test]
fn test_list_included_files_doesnt_descend_into_a_symlinked_directory(
) -> Result<(), Box<dyn Error>> {
    use std::os::unix::fs::symlink;

    // As packwerk's globs don't, nor list one named like a Ruby file. Nor can
    // a link back up the tree make pks list the same files over and over.
    let fixture = common::Fixture::new("simple_app");
    let outside = tempfile::tempdir()?;
    fs::create_dir_all(outside.path().join("app"))?;
    fs::write(outside.path().join("app/outside.rb"), "")?;
    symlink(outside.path(), fixture.path("packs/linked"))?;
    symlink(
        outside.path(),
        fixture.path("packs/foo/app/services/directory.rb"),
    )?;
    symlink("..", fixture.path("packs/foo/loop"))?;

    let files = included_files(&fixture)?;
    assert!(
        files.contains("packs/foo/app/services/foo.rb"),
        "{:?}",
        files
    );
    assert!(
        !files.iter().any(|file| file.starts_with("packs/linked/")
            || file.starts_with("packs/foo/app/services/directory.rb")
            || file.starts_with("packs/foo/loop/")),
        "{:?}",
        files
    );

    Ok(())
}

#[test]
fn test_list_included_files_with_a_file_for_the_project_root(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg(fixture.path("packs/foo/app/services/foo.rb"))
        .arg("list-included-files")
        .assert()
        .code(2)
        .stderr(predicates::str::contains("No root pack found"));

    Ok(())
}
