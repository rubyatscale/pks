use pretty_assertions::assert_eq;
use serde_json::{json, Value};
use std::{error::Error, fs};

mod common;

use common::pks;

/// Runs pks with `args`, which include `list-references`, and returns its
/// output after checking it against the schema.
fn list_references(fixture: &common::Fixture, args: &[&str]) -> Value {
    let stdout = pks(fixture, args)
        .success()
        .stderr("")
        .get_output()
        .stdout
        .clone();
    let output = serde_json::from_slice(&stdout).expect("output is not JSON");
    common::validate_schema("schema/list-references-output.json", &output);
    output
}

// In simple_app, `packs/foo` references `::Bar`, which is private to
// `packs/bar` and which it doesn't depend on, and `::Baz`, which it depends on.

fn bar_from_foo() -> Value {
    json!({
        "file": "packs/foo/app/services/foo.rb",
        "line": 3,
        "column": 4,
        "constant_name": "::Bar",
        "referencing_pack_name": "packs/foo",
        "defining_pack_name": "packs/bar",
        "violations": [
            {
                "violation_type": "dependency",
                "strict": false,
                "recorded": false
            },
            { "violation_type": "privacy", "strict": false, "recorded": false }
        ]
    })
}

fn baz_from_foo() -> Value {
    json!({
        "file": "packs/foo/app/services/foo.rb",
        "line": 7,
        "column": 4,
        "constant_name": "::Baz",
        "referencing_pack_name": "packs/foo",
        "defining_pack_name": "packs/baz",
        "violations": []
    })
}

#[test]
fn test_list_references() -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");

    assert_eq!(
        list_references(&fixture, &["list-references"]),
        json!({ "references": [bar_from_foo(), baz_from_foo()] })
    );

    Ok(())
}

#[test]
fn test_list_references_says_whether_violations_are_strict_and_recorded(
) -> Result<(), Box<dyn Error>> {
    // Every pack is strict. `packs/foo` records both its violations on `::Bar`,
    // neither on `::Baz`, and only the privacy one on `::Qux`.
    let fixture = common::Fixture::new("uses_strict_mode_partially_recorded");

    assert_eq!(
        list_references(&fixture, &["list-references"]),
        json!({
            "references": [
                {
                    "file": "packs/foo/app/services/foo.rb",
                    "line": 3,
                    "column": 4,
                    "constant_name": "::Bar",
                    "referencing_pack_name": "packs/foo",
                    "defining_pack_name": "packs/bar",
                    "violations": [
                        {
                            "violation_type": "dependency",
                            "strict": true,
                            "recorded": true
                        },
                        {
                            "violation_type": "privacy",
                            "strict": true,
                            "recorded": true
                        }
                    ]
                },
                {
                    "file": "packs/foo/app/services/foo.rb",
                    "line": 7,
                    "column": 4,
                    "constant_name": "::Baz",
                    "referencing_pack_name": "packs/foo",
                    "defining_pack_name": "packs/baz",
                    "violations": [
                        {
                            "violation_type": "dependency",
                            "strict": true,
                            "recorded": false
                        },
                        {
                            "violation_type": "privacy",
                            "strict": true,
                            "recorded": false
                        }
                    ]
                },
                {
                    "file": "packs/foo/app/services/foo.rb",
                    "line": 11,
                    "column": 4,
                    "constant_name": "::Qux",
                    "referencing_pack_name": "packs/foo",
                    "defining_pack_name": "packs/qux",
                    "violations": [
                        {
                            "violation_type": "dependency",
                            "strict": true,
                            "recorded": false
                        },
                        {
                            "violation_type": "privacy",
                            "strict": true,
                            "recorded": true
                        }
                    ]
                }
            ]
        })
    );

    Ok(())
}

#[test]
fn test_list_references_to_given_packs() -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");

    assert_eq!(
        list_references(
            &fixture,
            &["list-references", "--defining-pack", "packs/baz"]
        ),
        json!({ "references": [baz_from_foo()] })
    );
    // With the trailing slash a shell can add when completing a path
    assert_eq!(
        list_references(
            &fixture,
            &[
                "list-references",
                "--defining-pack",
                "packs/bar/",
                "--defining-pack",
                "packs/baz",
            ]
        ),
        json!({ "references": [bar_from_foo(), baz_from_foo()] })
    );

    Ok(())
}

#[test]
fn test_list_references_from_given_packs() -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");

    assert_eq!(
        list_references(
            &fixture,
            &["list-references", "--referencing-pack", "packs/bar"]
        ),
        json!({ "references": [] })
    );
    assert_eq!(
        list_references(
            &fixture,
            &[
                "list-references",
                "--referencing-pack",
                "packs/foo",
                "--defining-pack",
                "packs/bar",
            ]
        ),
        json!({ "references": [bar_from_foo()] })
    );

    Ok(())
}

#[test]
fn test_list_references_prints_files_to_stderr() -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");

    let output = pks(&fixture, &["--print-files", "list-references"])
        .success()
        .get_output()
        .clone();
    let references: Value =
        serde_json::from_slice(&output.stdout).expect("output is not JSON");
    assert_eq!(
        references,
        json!({ "references": [bar_from_foo(), baz_from_foo()] })
    );
    assert!(String::from_utf8(output.stderr)?.contains("Started processing"));

    Ok(())
}

#[test]
fn test_list_references_from_given_packs_reads_only_their_files(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    // pks warns about a file that doesn't parse, but only if it reads it
    fs::write(fixture.path("packs/bar/app/services/broken.rb"), "def a(\n")?;

    assert_eq!(
        list_references(
            &fixture,
            &["list-references", "--referencing-pack", "packs/foo"]
        ),
        json!({ "references": [bar_from_foo(), baz_from_foo()] })
    );
    pks(&fixture, &["list-references"])
        .success()
        .stderr(predicates::str::contains("broken.rb"));

    Ok(())
}

#[test]
fn test_list_references_rejects_an_unknown_pack() -> Result<(), Box<dyn Error>>
{
    let fixture = common::Fixture::new("simple_app");

    for option in ["--defining-pack", "--referencing-pack"] {
        pks(&fixture, &["list-references", option, "packs/nope"])
            .code(2)
            .stdout("")
            .stderr("Error: No pack found 'packs/nope'\n");
    }

    Ok(())
}

#[test]
fn test_list_references_leaves_out_references_within_a_pack(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    // `packs/foo` defines `::Foo::Bar` too
    fs::write(
        fixture.path("packs/foo/app/services/foo_bar_user.rb"),
        "class FooBarUser\n  def bar\n    Foo::Bar\n  end\nend\n",
    )?;

    assert_eq!(
        list_references(&fixture, &["list-references"]),
        json!({ "references": [bar_from_foo(), baz_from_foo()] })
    );

    Ok(())
}

#[test]
fn test_list_references_in_erb() -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    fs::write(
        fixture.path("packs/foo/app/views/foo.erb"),
        "<%= Bar.bar %>\n<p><%= Bar.bar %></p>\n",
    )?;

    // pks keeps no source positions in ERB, so the template's references to
    // `::Bar` are listed once, with no line or column.
    assert_eq!(
        list_references(
            &fixture,
            &["list-references", "--defining-pack", "packs/bar"]
        ),
        json!({
            "references": [
                bar_from_foo(),
                {
                    "file": "packs/foo/app/views/foo.erb",
                    "line": null,
                    "column": null,
                    "constant_name": "::Bar",
                    "referencing_pack_name": "packs/foo",
                    "defining_pack_name": "packs/bar",
                    "violations": [
                        {
                            "violation_type": "dependency",
                            "strict": false,
                            "recorded": false
                        },
                        {
                            "violation_type": "privacy",
                            "strict": false,
                            "recorded": false
                        }
                    ]
                }
            ]
        })
    );

    Ok(())
}

#[test]
fn test_list_references_to_a_constant_defined_in_several_files(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    // The experimental parser resolves `::Bar` to each file that defines it.
    // The one under app/public makes only a dependency violation, since it is
    // public, and the one under app/services makes only a privacy violation,
    // since `packs/foo` ignores dependencies on that folder. Neither copy of
    // the reference makes both violations, whichever comes first.
    fs::create_dir_all(fixture.path("packs/bar/app/public"))?;
    fs::write(
        fixture.path("packs/bar/app/public/bar.rb"),
        "module Bar\n  def self.public_method; end\nend\n",
    )?;
    fs::write(
        fixture.path("packs/foo/package.yml"),
        "\
enforce_dependencies: true
enforce_privacy: true
dependencies:
- packs/baz
enforcement_globs_ignore:
- enforcements:
  - dependency
  ignores:
  - packs/bar/app/services/**
",
    )?;

    // As in `check`, the reference is listed once, with the violations it makes
    // against either definition. `packs/baz/app/services/baz.rb` is empty, so
    // the experimental parser finds no `::Baz` to resolve it to.
    assert_eq!(
        list_references(
            &fixture,
            &["--experimental-parser", "list-references"]
        ),
        json!({ "references": [bar_from_foo()] })
    );

    Ok(())
}

/// A reference from `packs/foo` to a constant that `packs/bar` keeps private,
/// which breaks the same two rules as `bar_from_foo`. `None` is a position in
/// ERB.
fn to_bar_pack(
    file: &str,
    position: Option<(u64, u64)>,
    constant_name: &str,
) -> Value {
    let mut reference = bar_from_foo();
    reference["file"] = json!(file);
    reference["line"] = json!(position.map(|(line, _)| line));
    reference["column"] = json!(position.map(|(_, column)| column));
    reference["constant_name"] = json!(constant_name);
    reference
}

#[test]
fn test_list_references_lists_each_use_of_a_constant_in_a_file(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    // `::Bar` twice on one line, then at the same column on the next two
    let file = "packs/foo/app/services/foo_multi.rb";
    fs::write(
        fixture.path(file),
        "module FooMulti\n  def bar\n    Bar.call(Bar.new)\n    Bar\n    \
         Bar\n  end\nend\n",
    )?;

    assert_eq!(
        list_references(&fixture, &["list-references"]),
        json!({
            "references": [
                bar_from_foo(),
                baz_from_foo(),
                to_bar_pack(file, Some((3, 4)), "::Bar"),
                to_bar_pack(file, Some((3, 13)), "::Bar"),
                to_bar_pack(file, Some((4, 4)), "::Bar"),
                to_bar_pack(file, Some((5, 4)), "::Bar"),
            ]
        })
    );

    Ok(())
}

#[test]
fn test_list_references_at_the_start_of_a_file() -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    // Line 1 and column 0 are positions, unlike ERB's line 0
    let file = "packs/foo/app/services/foo_first_line.rb";
    fs::write(fixture.path(file), "Bar.bar\n")?;

    assert_eq!(
        list_references(&fixture, &["list-references"]),
        json!({
            "references": [
                bar_from_foo(),
                baz_from_foo(),
                to_bar_pack(file, Some((1, 0)), "::Bar"),
            ]
        })
    );

    Ok(())
}

#[test]
fn test_list_references_lists_files_in_order() -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    // pks reads files in no particular order, so with this many, the order it
    // read them in would almost never be sorted
    let names = ["a", "b", "c", "d", "e", "f", "g", "h"];
    for name in names.iter().rev() {
        fs::write(
            fixture.path(&format!("packs/foo/app/services/ref_{name}.rb")),
            format!(
                "module Ref{}\n  def bar\n    Bar\n  end\nend\n",
                name.to_uppercase()
            ),
        )?;
    }

    let mut expected = vec![bar_from_foo(), baz_from_foo()];
    for name in names {
        expected.push(to_bar_pack(
            &format!("packs/foo/app/services/ref_{name}.rb"),
            Some((3, 4)),
            "::Bar",
        ));
    }
    assert_eq!(
        list_references(&fixture, &["list-references"]),
        json!({ "references": expected })
    );

    Ok(())
}

#[test]
fn test_list_references_in_erb_lists_each_file_and_constant(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    // Both constants are `packs/bar`'s
    fs::write(
        fixture.path("packs/foo/app/views/foo.erb"),
        "<%= Bar.bar %>\n<%= SomeConcern %>\n",
    )?;
    fs::write(
        fixture.path("packs/foo/app/views/other.erb"),
        "<%= Bar.bar %>\n",
    )?;

    assert_eq!(
        list_references(
            &fixture,
            &["list-references", "--defining-pack", "packs/bar"]
        ),
        json!({
            "references": [
                bar_from_foo(),
                to_bar_pack("packs/foo/app/views/foo.erb", None, "::Bar"),
                to_bar_pack(
                    "packs/foo/app/views/foo.erb",
                    None,
                    "::SomeConcern"
                ),
                to_bar_pack("packs/foo/app/views/other.erb", None, "::Bar"),
            ]
        })
    );

    Ok(())
}

#[test]
fn test_list_references_to_a_constant_defined_in_several_packs(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    // Under the experimental parser `::Bar` is now `packs/baz`'s as well, and
    // `packs/foo` depends on `packs/baz`, which doesn't keep it private
    fs::create_dir_all(fixture.path("packs/baz/app/models"))?;
    fs::write(
        fixture.path("packs/baz/app/models/bar.rb"),
        "module Bar\n  def self.other; end\nend\n",
    )?;

    let mut bar_in_baz = bar_from_foo();
    bar_in_baz["defining_pack_name"] = json!("packs/baz");
    bar_in_baz["violations"] = json!([]);
    assert_eq!(
        list_references(
            &fixture,
            &["--experimental-parser", "list-references"]
        ),
        json!({ "references": [bar_from_foo(), bar_in_baz] })
    );

    Ok(())
}

#[test]
fn test_list_references_says_when_a_violation_is_recorded_but_not_strict(
) -> Result<(), Box<dyn Error>> {
    // `packs/foo` doesn't depend on `packs/bar`, and its package_todo.yml
    // records the dependency violation in both files that use `::Bar`
    let fixture = common::Fixture::new("contains_package_todo");

    let reference = |file: &str| {
        json!({
            "file": file,
            "line": 3,
            "column": 4,
            "constant_name": "::Bar",
            "referencing_pack_name": "packs/foo",
            "defining_pack_name": "packs/bar",
            "violations": [
                {
                    "violation_type": "dependency",
                    "strict": false,
                    "recorded": true
                }
            ]
        })
    };
    assert_eq!(
        list_references(&fixture, &["list-references"]),
        json!({
            "references": [
                reference("packs/foo/app/services/foo.rb"),
                reference("packs/foo/app/services/other_foo.rb"),
            ]
        })
    );

    Ok(())
}

/// One reference from `packs/baz`, and one from `packs/foos/too`, which the
/// rule allows, to `::Foo` in `packs/foos/foo`
fn from_baz_and_foos_too(violation_type: &str) -> Value {
    let reference = |file: &str, referencing_pack_name: &str| {
        json!({
            "file": file,
            "line": 3,
            "column": 4,
            "constant_name": "::Foo",
            "referencing_pack_name": referencing_pack_name,
            "defining_pack_name": "packs/foos/foo",
            "violations": []
        })
    };
    let mut from_baz = reference("packs/baz/app/services/baz.rb", "packs/baz");
    from_baz["violations"] = json!([
        { "violation_type": violation_type, "strict": false, "recorded": false }
    ]);
    json!({
        "references": [
            from_baz,
            reference(
                "packs/foos/too/app/services/ref_foo.rb",
                "packs/foos/too"
            ),
        ]
    })
}

#[test]
fn test_list_references_with_a_visibility_violation(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("visibility_violations");

    assert_eq!(
        list_references(&fixture, &["list-references"]),
        from_baz_and_foos_too("visibility")
    );

    Ok(())
}

#[test]
fn test_list_references_with_a_folder_privacy_violation(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("folder_privacy_violations");

    assert_eq!(
        list_references(&fixture, &["list-references"]),
        from_baz_and_foos_too("folder_privacy")
    );

    Ok(())
}

#[test]
fn test_list_references_with_a_layer_violation() -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("layer_violations");

    assert_eq!(
        list_references(&fixture, &["list-references"]),
        json!({
            "references": [{
                "file": "packs/feature_flags/app/services/feature_flags.rb",
                "line": 2,
                "column": 0,
                "constant_name": "::Payments",
                "referencing_pack_name": "packs/feature_flags",
                "defining_pack_name": "packs/payments",
                "violations": [
                    {
                        "violation_type": "layer",
                        "strict": false,
                        "recorded": false
                    }
                ]
            }]
        })
    );

    Ok(())
}
