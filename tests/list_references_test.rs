use assert_cmd::{assert::Assert, cargo::cargo_bin_cmd};
use jsonschema::Validator;
use pretty_assertions::assert_eq;
use serde_json::{json, Value};
use std::{error::Error, fs};

mod common;

fn pks(fixture: &common::Fixture, args: &[&str]) -> Assert {
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg(fixture.root())
        .args(args)
        .assert()
}

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
    validate_schema(&output);
    output
}

fn validate_schema(output: &Value) {
    let schema = fs::read_to_string("schema/list-references-output.json")
        .expect("Failed to read schema file");
    let schema: Value =
        serde_json::from_str(&schema).expect("Schema should be valid JSON");
    let validator =
        Validator::new(&schema).expect("Schema should be valid JSON Schema");
    let errors: Vec<String> = validator
        .iter_errors(output)
        .map(|e| format!("  - {}", e))
        .collect();
    assert!(
        errors.is_empty(),
        "JSON output does not match schema:\n{}",
        errors.join("\n")
    );
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
    // The experimental parser resolves `::Bar` to each file that defines it,
    // and only the one under app/services is private.
    fs::create_dir_all(fixture.path("packs/bar/app/public"))?;
    fs::write(
        fixture.path("packs/bar/app/public/bar.rb"),
        "module Bar\n  def self.public_method; end\nend\n",
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
