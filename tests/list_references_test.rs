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

#[test]
fn test_list_references_to_a_class_defined_from_the_top_level(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    // The experimental parser names this `::Taco`, not `::Outer::::Taco`
    fs::write(
        fixture.path("packs/bar/app/services/taco.rb"),
        "module Outer\n  class ::Taco\n    def self.make; end\n  end\nend\n",
    )?;
    let file = "packs/foo/app/services/foo_taco.rb";
    fs::write(fixture.path(file), "Taco.make\n")?;

    assert_eq!(
        list_references(
            &fixture,
            &["--experimental-parser", "list-references"]
        ),
        json!({
            "references": [
                bar_from_foo(),
                to_bar_pack(file, Some((1, 0)), "::Taco"),
            ]
        })
    );

    Ok(())
}

#[test]
fn test_list_references_inside_a_class_defined_from_the_top_level(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    fs::write(
        fixture.path("packs/bar/app/services/taco.rb"),
        "module Taco\n  class Shell\n    def self.fold; end\n  end\nend\n",
    )?;
    let file = "packs/foo/app/services/foo_taco.rb";
    // As in Ruby, `Shell` here is looked up in `::Taco`, not `::Outer::::Taco`
    fs::write(
        fixture.path(file),
        "module Outer\n  class ::Taco\n    Shell.fold\n  end\nend\n",
    )?;

    assert_eq!(
        list_references(
            &fixture,
            &["--experimental-parser", "list-references"]
        ),
        json!({
            "references": [
                bar_from_foo(),
                to_bar_pack(file, Some((3, 4)), "::Taco::Shell"),
            ]
        })
    );

    Ok(())
}

#[test]
fn test_list_references_to_a_class_defined_from_the_top_level_in_a_namespace(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    for (path, contents) in [
        ("outer.rb", "module Outer\nend\n"),
        ("aa.rb", "module Aa\nend\n"),
        ("aa/bb.rb", "module Aa::Bb\nend\n"),
    ] {
        let path = fixture.path(&format!("packs/bar/app/services/{}", path));
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, contents)?;
    }
    // As in packwerk, `::Outer::::Burrito` has no parent, so it isn't a
    // reference to `Outer`, and the parent of `::Aa::Bb::::Cee` is `::Aa`, not
    // `::Aa::Bb`
    let outer = "packs/foo/app/services/foo_outer.rb";
    fs::write(
        fixture.path(outer),
        "module Outer\n  class ::Burrito\n  end\nend\n",
    )?;
    let aa = "packs/foo/app/services/foo_aa.rb";
    fs::write(
        fixture.path(aa),
        "module Aa\n  module Bb\n    class ::Cee\n    end\n  end\nend\n",
    )?;

    assert_eq!(
        list_references(&fixture, &["list-references"]),
        json!({
            "references": [
                bar_from_foo(),
                baz_from_foo(),
                to_bar_pack(aa, Some((1, 7)), "::Aa"),
                to_bar_pack(aa, Some((2, 9)), "::Aa::Bb"),
                to_bar_pack(aa, Some((3, 10)), "::Aa::Bb::::Cee"),
                to_bar_pack(outer, Some((1, 7)), "::Outer"),
            ]
        })
    );

    Ok(())
}

#[test]
fn test_list_references_after_a_class_defined_from_the_top_level(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    fs::write(
        fixture.path("packs/bar/app/services/taco.rb"),
        "class Taco\n  def self.make; end\nend\n",
    )?;
    let file = "packs/foo/app/services/foo_taco.rb";
    // As in packwerk, which names this class `::::Taco`, the `Taco` inside it
    // and the `Taco` and `::Taco` after it aren't the file's own
    fs::write(
        fixture.path(file),
        "class ::Taco\n  def self.local; end\n  Taco.make\nend\n\
         Taco.make\n::Taco.make\n",
    )?;

    assert_eq!(
        list_references(&fixture, &["list-references"]),
        json!({
            "references": [
                bar_from_foo(),
                baz_from_foo(),
                to_bar_pack(file, Some((3, 2)), "::Taco"),
                to_bar_pack(file, Some((5, 0)), "::Taco"),
                to_bar_pack(file, Some((6, 0)), "::Taco"),
            ]
        })
    );

    Ok(())
}

#[test]
fn test_list_references_after_a_constant_assigned_from_the_top_level(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    fs::write(
        fixture.path("packs/bar/app/services/taco.rb"),
        "class Taco\n  def self.make; end\nend\n",
    )?;
    let file = "packs/foo/app/services/foo_taco.rb";
    // As in packwerk, which names this constant `::::Taco`, the `Taco` and
    // `::Taco` after it aren't the file's own
    fs::write(fixture.path(file), "::Taco = 1\nTaco.make\n::Taco.make\n")?;

    assert_eq!(
        list_references(&fixture, &["list-references"]),
        json!({
            "references": [
                bar_from_foo(),
                baz_from_foo(),
                to_bar_pack(file, Some((2, 0)), "::Taco"),
                to_bar_pack(file, Some((3, 0)), "::Taco"),
            ]
        })
    );

    Ok(())
}

#[test]
fn test_list_references_from_associations_by_the_models_name(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    fs::write(
        fixture.path("packs/bar/app/services/taco.rb"),
        "class Taco\n  def self.make; end\nend\n\
         module Outer\n  class Taco\n    def self.make; end\n  end\nend\n",
    )?;
    // Rails looks `:tacos` up from the model's name: as `Outer::Foo::Taco`,
    // `Outer::Taco`, then `Taco` in `Outer::Foo`, and as `Foo::Taco`, then
    // `Taco` in `class ::Foo`, which names the model `Foo` inside any module
    let compact = "packs/foo/app/services/foo_compact.rb";
    fs::write(
        fixture.path(compact),
        "class Outer::Foo\n  has_many :tacos\nend\n",
    )?;
    let top_level = "packs/foo/app/services/foo_top_level.rb";
    fs::write(
        fixture.path(top_level),
        "module Outer\n  class ::Foo\n    has_many :tacos\n  end\nend\n",
    )?;
    // When the class has the model's own name, Rails tries `::Taco` first, so
    // these don't find the model itself
    let own_name = "packs/foo/app/services/foo_own_name.rb";
    fs::write(
        fixture.path(own_name),
        "class Payroll::Taco\n  belongs_to :taco\nend\n\
         module Billing\n  class Taco\n    belongs_to :taco\n  end\nend\n",
    )?;

    assert_eq!(
        list_references(
            &fixture,
            &["--experimental-parser", "list-references"]
        ),
        json!({
            "references": [
                bar_from_foo(),
                to_bar_pack(compact, Some((2, 2)), "::Outer::Taco"),
                to_bar_pack(own_name, Some((2, 2)), "::Taco"),
                to_bar_pack(own_name, Some((6, 4)), "::Taco"),
                to_bar_pack(top_level, Some((3, 4)), "::Taco"),
            ]
        })
    );

    Ok(())
}

#[test]
fn test_list_references_from_associations_in_namespaced_models(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    fs::write(
        fixture.path("packs/bar/app/services/taco.rb"),
        "class Taco\n  def self.make; end\nend\n\
         module Outer\n  class Taco\n    def self.make; end\n  end\nend\n\
         module CoreExt\n  class Taco\n    def self.make; end\n  end\nend\n",
    )?;
    let nested = "packs/foo/app/services/foo_nested.rb";
    fs::write(
        fixture.path(nested),
        "module Outer\n  class Foo\n    has_many :tacos\n  end\nend\n",
    )?;
    // `class Object::Foo` is `::Foo` wherever it's written, so Rails looks
    // `:tacos` up as `Foo::Taco`, then `Taco`, not as `CoreExt::Taco`
    let through_object = "packs/foo/app/services/foo_through_object.rb";
    fs::write(
        fixture.path(through_object),
        "module CoreExt\n  class Object::Foo\n    has_many :tacos\n  end\nend\n",
    )?;

    assert_eq!(
        list_references(
            &fixture,
            &["--experimental-parser", "list-references"]
        ),
        json!({
            "references": [
                bar_from_foo(),
                to_bar_pack(nested, Some((3, 4)), "::Outer::Taco"),
                to_bar_pack(through_object, Some((3, 4)), "::Taco"),
            ]
        })
    );

    Ok(())
}

#[test]
fn test_list_references_to_constants_defined_through_object(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    // The experimental parser names these `::Taco`, `::Salsa` and `::GUAC`,
    // not `::CoreExt::Object::Taco` and so on
    fs::write(
        fixture.path("packs/bar/app/services/taco.rb"),
        "module CoreExt\n  class Object::Taco\n    def self.make; end\n  end\n\
         \n  module Object::Salsa\n    def self.pour; end\n  end\n\
         \n  Object::GUAC = 1\nend\n",
    )?;
    let file = "packs/foo/app/services/foo_taco.rb";
    fs::write(
        fixture.path(file),
        "Taco.make\n::Object::Salsa.pour\nObject::GUAC\n",
    )?;
    let template = "packs/foo/app/views/foo_taco.erb";
    fs::write(fixture.path(template), "<%= ::Object::Taco.make %>\n")?;

    assert_eq!(
        list_references(
            &fixture,
            &["--experimental-parser", "list-references"]
        ),
        json!({
            "references": [
                bar_from_foo(),
                to_bar_pack(file, Some((1, 0)), "::Taco"),
                to_bar_pack(file, Some((2, 0)), "::Salsa"),
                to_bar_pack(file, Some((3, 0)), "::GUAC"),
                to_bar_pack(template, None, "::Taco"),
            ]
        })
    );

    Ok(())
}

#[test]
fn test_list_references_from_associations_in_concerns(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    fs::write(
        fixture.path("packs/bar/app/services/taco.rb"),
        "class Taco\n  def self.make; end\nend\n\
         module Outer\n  class Taco\n    def self.make; end\n  end\nend\n\
         module Billing\n  class Taco\n    def self.make; end\n  end\nend\n",
    )?;
    // Rails looks `:tacos` up from the name of each model that includes the
    // concern, `Plain` and `Billing::Invoice`, which includes `Trackable`
    // through `Billing::Auditable`. With no model, the concern's name stands
    // in for one.
    let taggable = "packs/foo/app/services/foo_taggable.rb";
    let trackable = "packs/foo/app/services/foo_trackable.rb";
    let unused = "packs/foo/app/services/foo_unused.rb";
    for (file, contents) in [
        (
            taggable,
            "module Outer::Taggable\n  extend ActiveSupport::Concern\n\n  \
             included do\n    has_many :tacos\n  end\nend\n",
        ),
        (
            trackable,
            "module Trackable\n  extend ActiveSupport::Concern\n\n  \
             included do\n    belongs_to :taco\n  end\nend\n",
        ),
        (
            unused,
            "module Outer\n  module Unused\n    \
             extend ActiveSupport::Concern\n\n    included do\n      \
             has_many :tacos\n    end\n  end\nend\n",
        ),
        (
            "packs/foo/app/services/foo_auditable.rb",
            "module Billing\n  module Auditable\n    \
             extend ActiveSupport::Concern\n    include Trackable\n  end\n\
             end\n",
        ),
        (
            "packs/foo/app/services/foo_invoice.rb",
            "module Billing\n  class Invoice\n    include Outer::Taggable\n    \
             include Auditable\n  end\nend\n",
        ),
        (
            "packs/foo/app/services/foo_plain.rb",
            "class Plain\n  include Outer::Taggable\nend\n",
        ),
    ] {
        fs::write(fixture.path(file), contents)?;
    }
    // An association in a template too, which has no position
    let template = "packs/foo/app/views/foo_tacos.erb";
    fs::write(
        fixture.path(template),
        "<% class Plain; has_many :tacos; end %>\n",
    )?;

    assert_eq!(
        list_references(
            &fixture,
            &["--experimental-parser", "list-references"]
        ),
        json!({
            "references": [
                bar_from_foo(),
                to_bar_pack(taggable, Some((5, 4)), "::Billing::Taco"),
                to_bar_pack(taggable, Some((5, 4)), "::Taco"),
                to_bar_pack(trackable, Some((5, 4)), "::Billing::Taco"),
                to_bar_pack(unused, Some((6, 6)), "::Outer::Taco"),
                to_bar_pack(template, None, "::Taco"),
            ]
        })
    );

    Ok(())
}

#[test]
fn test_list_references_from_associations_named_for_their_model(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    fs::write(
        fixture.path("packs/bar/app/services/taco.rb"),
        "module Outer\n  class Taco\n    class Taco\n      \
         def self.make; end\n    end\n  end\nend\n\
         module Billing\n  class Taco\n    def self.make; end\n  end\n  \
         class Salsa\n    def self.pour; end\n  end\nend\n\
         class Salsa\n  def self.pour; end\nend\n",
    )?;
    // With no `::Taco`, Rails goes on to look `:tacos` up from the model's
    // name, and finds `Outer::Taco::Taco`
    let own_name = "packs/foo/app/services/foo_own_name.rb";
    fs::write(
        fixture.path(own_name),
        "class Outer::Taco\n  has_many :tacos\nend\n",
    )?;
    // inherited_resources looks up `belongs_to :taco` from the controller's
    // namespace, and not in the controller, and takes a class it's given from
    // the top level
    let controller = "packs/foo/app/services/foo_controller.rb";
    fs::write(
        fixture.path(controller),
        "class Billing::TacosController\n  class Taco\n    def self.local; end\n  \
         end\n\n  belongs_to :taco\n  belongs_to :sauce, class_name: \"Salsa\"\n\
         end\n",
    )?;
    // ActiveRecord's `has_many` in a model named `...Controller` is looked up
    // from the model's name, so it finds the model's own `Taco`
    fs::write(
        fixture.path("packs/foo/app/services/foo_motor_controller.rb"),
        "class Billing::TacoMotorController\n  class Taco\n    \
         def self.local; end\n  end\n\n  has_many :tacos\nend\n",
    )?;

    assert_eq!(
        list_references(
            &fixture,
            &["--experimental-parser", "list-references"]
        ),
        json!({
            "references": [
                bar_from_foo(),
                to_bar_pack(controller, Some((6, 2)), "::Billing::Taco"),
                to_bar_pack(controller, Some((7, 2)), "::Salsa"),
                to_bar_pack(own_name, Some((2, 2)), "::Outer::Taco::Taco"),
            ]
        })
    );

    Ok(())
}

#[test]
fn test_list_references_to_constants_defined_through_a_namespaces_object(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    // `Payments` defines an `Object` of its own, so `class Object::Charge` in
    // `Payments`, in another file, is `Payments::Object::Charge`
    fs::write(
        fixture.path("packs/bar/app/services/payments_object.rb"),
        "module Payments\n  class Object\n    def self.pay; end\n  end\nend\n",
    )?;
    fs::write(
        fixture.path("packs/bar/app/services/payments_charge.rb"),
        "module Payments\n  class Object::Charge\n    FEE = 1\n\n    \
         def self.make; end\n  end\nend\n",
    )?;
    // As does an `Object` that defines nothing
    fs::write(
        fixture.path("packs/bar/app/services/kitchen_object.rb"),
        "module Kitchen\n  class Object\n  end\nend\n\
         module Kitchen\n  class Object::Dish\n    def self.make; end\n  end\n\
         end\n",
    )?;
    let file = "packs/foo/app/services/foo_payments.rb";
    fs::write(
        fixture.path(file),
        "Payments::Object::Charge.make\nPayments::Object::Charge::FEE\n\
         ::Charge.make\nKitchen::Object::Dish.make\n",
    )?;

    assert_eq!(
        list_references(
            &fixture,
            &["--experimental-parser", "list-references"]
        ),
        json!({
            "references": [
                bar_from_foo(),
                to_bar_pack(file, Some((1, 0)), "::Payments::Object::Charge"),
                to_bar_pack(
                    file,
                    Some((2, 0)),
                    "::Payments::Object::Charge::FEE"
                ),
                to_bar_pack(file, Some((4, 0)), "::Kitchen::Object::Dish"),
            ]
        })
    );

    Ok(())
}

#[test]
fn test_list_references_to_constants_of_objects_ancestors(
) -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    // The top level's constants are `Object`'s, so those of `Kernel` and
    // `BasicObject` are found from anywhere
    fs::write(
        fixture.path("packs/bar/app/services/taco.rb"),
        "class BasicObject::Taco\n  def self.make; end\nend\n\
         module Kernel\n  SALSA = 1\n\n  class Pepper\n    def self.x; end\n  \
         end\nend\n",
    )?;
    let file = "packs/foo/app/services/foo_taco.rb";
    fs::write(
        fixture.path(file),
        "module Outer\n  Taco.make\n  ::Taco.make\n  SALSA\nend\n",
    )?;
    // Rails looks up `:peppers` in `Outer::Pepper` as `::Pepper`, which finds
    // `Kernel::Pepper`, but takes only a class of the very name it looks for
    // from the model's name, so `Billing::Thing` has none
    let associations = "packs/foo/app/services/foo_peppers.rb";
    fs::write(
        fixture.path(associations),
        "class Outer::Pepper\n  has_many :peppers\nend\n\
         class Billing::Thing\n  has_many :peppers\nend\n",
    )?;

    assert_eq!(
        list_references(
            &fixture,
            &["--experimental-parser", "list-references"]
        ),
        json!({
            "references": [
                bar_from_foo(),
                to_bar_pack(associations, Some((2, 2)), "::Kernel::Pepper"),
                to_bar_pack(file, Some((2, 2)), "::BasicObject::Taco"),
                to_bar_pack(file, Some((3, 2)), "::BasicObject::Taco"),
                to_bar_pack(file, Some((4, 2)), "::Kernel::SALSA"),
            ]
        })
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
fn test_list_references_in_anonymous_argument_forwarding(
) -> Result<(), Box<dyn Error>> {
    // Ruby 3.2's `*` and `**` forwarding
    let file = "packs/foo/app/services/forwarder.rb";
    for args in [&[][..], &["--experimental-parser"]] {
        let fixture = common::Fixture::new("simple_app");
        fs::write(
            fixture.path(file),
            "class Forwarder\n  def a(*) = Bar.b(*)\n  def c(**) = Bar.d(**)\n\
             end\n",
        )?;

        assert_eq!(
            list_references(
                &fixture,
                &[args, &["list-references", "--defining-pack", "packs/bar"]]
                    .concat()
            ),
            json!({
                "references": [
                    bar_from_foo(),
                    to_bar_pack(file, Some((2, 13)), "::Bar"),
                    to_bar_pack(file, Some((3, 14)), "::Bar"),
                ]
            })
        );
    }

    Ok(())
}

#[test]
fn test_list_references_in_a_deeply_nested_file() -> Result<(), Box<dyn Error>>
{
    // Prism and the walk of its tree recurse, and in a debug build this nesting
    // overflows a thread's default 2 MiB stack
    let file = "packs/foo/app/services/nested.rb";
    let depth = 500;
    for args in [&[][..], &["--experimental-parser"]] {
        let fixture = common::Fixture::new("simple_app");
        fs::write(
            fixture.path(file),
            format!("x = {}Bar{}\n", "[".repeat(depth), "]".repeat(depth)),
        )?;

        assert_eq!(
            list_references(
                &fixture,
                &[args, &["list-references", "--defining-pack", "packs/bar"]]
                    .concat()
            ),
            json!({
                "references": [
                    bar_from_foo(),
                    to_bar_pack(file, Some((1, 4 + depth as u64)), "::Bar"),
                ]
            })
        );
    }

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
fn test_list_references_from_associations() -> Result<(), Box<dyn Error>> {
    let fixture = common::Fixture::new("app_with_rails_relationships");
    // carrier.rb doesn't write `Census` or `Taco`, only `has_many :censuses`
    // and `has_many :tacos`, so each reference is at the start of its call.
    // notification.rb's polymorphic `belongs_to :event` infers none.
    let reference = |line: u64, constant_name: &str| {
        json!({
            "file": "packs/bar/app/models/carrier.rb",
            "line": line,
            "column": 2,
            "constant_name": constant_name,
            "referencing_pack_name": "packs/bar",
            "defining_pack_name": "packs/baz",
            "violations": [
                {
                    "violation_type": "privacy",
                    "strict": false,
                    "recorded": false
                }
            ]
        })
    };

    assert_eq!(
        list_references(&fixture, &["list-references"]),
        json!({
            "references": [reference(2, "::Census"), reference(3, "::Taco")]
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
