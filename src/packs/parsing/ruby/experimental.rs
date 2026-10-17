mod constant_resolver;
pub(crate) mod parser;

use self::constant_resolver::ExperimentalConstantResolver;
use self::parser::process_from_path_with_objects;
use super::namespace_calculator::nest_constant_name;
use crate::packs::{
    constant_resolver::{ConstantDefinition, ConstantResolver},
    file_utils::{get_file_type, SupportedFileType},
    process_files_with_cache, Configuration, ProcessedFile,
    UnresolvedAssociation, UnresolvedReference,
};

use rayon::prelude::{
    IntoParallelIterator, IntoParallelRefMutIterator, ParallelIterator,
};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

/// Reads the files, and the resolver for their constants. A constant written
/// through a namespace's `Object` and the class of an association depend on
/// the rest of the app, so they wait until every file is read.
pub fn process_files(
    paths: &HashSet<PathBuf>,
    configuration: &Configuration,
) -> anyhow::Result<(Vec<ProcessedFile>, Box<dyn ConstantResolver + Send + Sync>)>
{
    let mut processed_files = process_files_with_cache(
        paths,
        configuration.get_cache(),
        configuration,
    )?;
    name_through_objects(&mut processed_files, configuration)?;
    let constant_resolver = get_experimental_constant_resolver(
        &configuration.absolute_root,
        &processed_files,
        &configuration.ignored_definitions,
    );
    resolve_associations(&mut processed_files, constant_resolver.as_ref());
    Ok((processed_files, constant_resolver))
}

/// `class Object::Bar` in `module Payments` defines `::Payments::Object::Bar`
/// where the app defines a `Payments::Object`, and `::Bar` otherwise, so a
/// file that defines a constant that way is read again, knowing the
/// namespaces' `Object`s that the others define. That can name another
/// `Object`, so it goes on until the names stop changing.
fn name_through_objects(
    processed_files: &mut [ProcessedFile],
    configuration: &Configuration,
) -> anyhow::Result<()> {
    let mut objects = HashSet::new();
    // A few times at most, in case the names of two `Object`s depend on each
    // other's
    for _ in 0..8 {
        let found = objects_in_namespaces(processed_files);
        if found == objects {
            break;
        }
        objects = found;
        processed_files
            .par_iter_mut()
            .filter(|processed_file| {
                processed_file.object_in_a_namespace
                    && get_file_type(&processed_file.absolute_path)
                        == Some(SupportedFileType::Ruby)
            })
            .try_for_each(|processed_file| -> anyhow::Result<()> {
                *processed_file = process_from_path_with_objects(
                    &processed_file.absolute_path,
                    configuration,
                    &objects,
                )?;
                Ok(())
            })?;
    }
    Ok(())
}

/// The namespaces' own `Object`s, as `::Payments::Object`, that the files
/// open, or define a constant in
fn objects_in_namespaces(processed_files: &[ProcessedFile]) -> HashSet<String> {
    let opened = processed_files
        .iter()
        .flat_map(|processed_file| processed_file.namespace_objects.clone());
    let defined_in = processed_files
        .iter()
        .flat_map(|processed_file| &processed_file.definitions)
        .flat_map(|definition| {
            let name = definition.fully_qualified_name.as_str();
            name.match_indices("::Object")
                .filter_map(move |(start, part)| {
                    let end = start + part.len();
                    let whole =
                        end == name.len() || name[end..].starts_with("::");
                    // `::Object` itself is the top level's
                    (start > 0 && whole).then(|| name[..end].to_owned())
                })
        });
    opened.chain(defined_in).collect()
}

/// Turns each association into a reference, looked up as Rails does from the
/// name of its model. In a module, as a concern's `included` block, the
/// models are the classes that include it, directly or through other modules,
/// and where none does, the module's own name stands in for theirs.
fn resolve_associations(
    processed_files: &mut [ProcessedFile],
    constant_resolver: &(dyn ConstantResolver + Send + Sync),
) {
    let includers = includers(processed_files, constant_resolver);
    processed_files.par_iter_mut().for_each(|processed_file| {
        for association in std::mem::take(&mut processed_file.associations) {
            let models = if association.in_module {
                models_including(&association.model, &includers)
            } else {
                vec![association.model.clone()]
            };
            for model in models {
                let Some(reference) = association_reference(
                    &association,
                    &model,
                    constant_resolver,
                ) else {
                    continue;
                };
                if !processed_file.unresolved_references.contains(&reference) {
                    processed_file.unresolved_references.push(reference);
                }
            }
        }
    });
}

/// For each module, the classes and modules that include or prepend it, and
/// whether each is a module
fn includers(
    processed_files: &[ProcessedFile],
    constant_resolver: &(dyn ConstantResolver + Send + Sync),
) -> HashMap<String, Vec<(String, bool)>> {
    let mut includers: HashMap<String, Vec<(String, bool)>> = HashMap::new();
    for inclusion in processed_files
        .iter()
        .flat_map(|processed_file| &processed_file.inclusions)
    {
        let namespace_path: Vec<&str> = inclusion
            .namespace_path
            .iter()
            .map(String::as_str)
            .collect();
        let modules: HashSet<String> = constant_resolver
            .resolve(&inclusion.name, &namespace_path)
            .unwrap_or_default()
            .into_iter()
            .map(|definition| definition.fully_qualified_name)
            .collect();
        for module in modules {
            let includer =
                (inclusion.includer.clone(), inclusion.includer_is_module);
            let module_includers = includers.entry(module).or_default();
            if !module_includers.contains(&includer) {
                module_includers.push(includer);
            }
        }
    }
    includers
}

/// The classes that include `module`, directly or through other modules, or
/// the module itself where none does
fn models_including(
    module: &str,
    includers: &HashMap<String, Vec<(String, bool)>>,
) -> Vec<String> {
    let mut models: Vec<String> = vec![];
    let mut seen = HashSet::from([module.to_owned()]);
    let mut modules = vec![module.to_owned()];
    while let Some(module) = modules.pop() {
        for (includer, is_module) in
            includers.get(&module).into_iter().flatten()
        {
            if *is_module {
                if seen.insert(includer.clone()) {
                    modules.push(includer.clone());
                }
            } else if !models.contains(includer) {
                models.push(includer.clone());
            }
        }
    }
    if models.is_empty() {
        models.push(module.to_owned());
    }
    models.sort();
    models
}

/// The reference that `association` in `model` makes, if Rails finds its
/// class, as it looks it up
fn association_reference(
    association: &UnresolvedAssociation,
    model: &str,
    constant_resolver: &(dyn ConstantResolver + Send + Sync),
) -> Option<UnresolvedReference> {
    let name = &association.name;
    let mut namespaces: Vec<&str> = model.split("::").skip(1).collect();
    let reference = |name: String, namespace_path: Vec<String>| {
        Some(UnresolvedReference {
            name,
            namespace_path,
            location: association.location.clone(),
        })
    };
    let model_name = namespaces.last().copied();
    if association.belongs_to
        && model_name.is_some_and(|model| model.ends_with("Controller"))
    {
        // inherited_resources takes a class it's given from the top level,
        // and looks up one it isn't from the controller's namespaces
        if association.names_its_class {
            return reference(name.clone(), vec![]);
        }
        namespaces.pop();
        let namespace_path =
            namespaces.into_iter().map(str::to_owned).collect();
        return reference(name.clone(), namespace_path);
    }
    if model_name == Some(name.as_str()) {
        // For a class with the model's own name, as in `Outer::Taco`, Rails
        // tries `::Taco` first (`_klass` in reflection.rb), which finds a
        // constant of `Kernel` or `BasicObject` too
        let top_level = format!("::{}", name);
        let defined = constant_resolver
            .resolve(&top_level, &[])
            .is_some_and(|definitions| !definitions.is_empty());
        if defined {
            return reference(top_level, vec![]);
        }
    }
    // Then from the model's name, as `Outer::Foo::Taco`, `Outer::Taco`, then
    // `Taco` in `Outer::Foo`, taking only a constant of that very name
    // (`compute_type` in inheritance.rb)
    let definitions = constant_resolver
        .fully_qualified_constant_name_to_constant_definition_map();
    (0..=namespaces.len())
        .rev()
        .map(|depth| nest_constant_name(&namespaces[..depth], name))
        .find(|candidate| definitions.contains_key(candidate))
        .and_then(|candidate| reference(candidate, vec![]))
}

pub fn get_experimental_constant_resolver(
    absolute_root: &Path,
    processed_files: &Vec<ProcessedFile>,
    ignored_definitions: &HashMap<String, HashSet<PathBuf>>,
) -> Box<dyn ConstantResolver + Send + Sync> {
    let constants = processed_files
        .into_par_iter()
        .flat_map(|processed_file| {
            processed_file
                .definitions
                .iter()
                .map(|definition| {
                    let fully_qualified_name =
                        definition.fully_qualified_name.to_owned();
                    ConstantDefinition {
                        fully_qualified_name,
                        absolute_path_of_definition: processed_file
                            .absolute_path
                            .to_owned(),
                    }
                })
                .collect::<Vec<ConstantDefinition>>()
        })
        .collect::<Vec<ConstantDefinition>>();

    ExperimentalConstantResolver::create(
        constants,
        absolute_root,
        ignored_definitions,
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use std::collections::HashMap;

    use crate::packs::parsing::ruby::experimental::parser::process_from_contents;
    use crate::packs::parsing::{ParsedDefinition, Range};
    use crate::packs::{Configuration, ProcessedFile, UnresolvedReference};
    use pretty_assertions::assert_eq;

    #[test]
    fn objects_in_namespaces_of_definitions() {
        let processed_file = ProcessedFile {
            definitions: [
                "::Object",
                "::Payments::Object",
                "::Billing::Object::Charge",
                "::Aa::Object::Bb::Object",
                "::Cc::ObjectSpace",
            ]
            .map(|name| ParsedDefinition {
                fully_qualified_name: name.to_owned(),
                location: Range::default(),
            })
            .to_vec(),
            ..ProcessedFile::default()
        };
        let mut objects: Vec<String> =
            super::objects_in_namespaces(&[processed_file])
                .into_iter()
                .collect();
        objects.sort();
        assert_eq!(
            objects,
            [
                "::Aa::Object",
                "::Aa::Object::Bb::Object",
                "::Billing::Object",
                "::Payments::Object",
            ]
        );
    }

    #[test]
    fn models_including_modules_that_include_each_other() {
        let includers = HashMap::from([
            ("::A".to_owned(), vec![("::B".to_owned(), true)]),
            (
                "::B".to_owned(),
                vec![("::A".to_owned(), true), ("::Foo".to_owned(), false)],
            ),
        ]);
        assert_eq!(super::models_including("::A", &includers), ["::Foo"]);
        assert_eq!(super::models_including("::C", &includers), ["::C"]);
    }

    #[test]
    fn trivial_case() {
        let contents: String = String::from("Foo");

        let configuration = Configuration::default();
        let absolute_path = PathBuf::from("path/to/file.rb");
        let unresolved_references = vec![UnresolvedReference {
            name: String::from("Foo"),
            namespace_path: vec![],
            location: Range {
                start_row: 1,
                start_col: 0,
                end_row: 1,
                end_col: 4,
            },
        }];

        let definitions = vec![];

        let actual =
            process_from_contents(contents, &absolute_path, &configuration);
        let expected = ProcessedFile {
            absolute_path,
            unresolved_references,
            definitions,
            syntax_error: None,
            ..ProcessedFile::default()
        };
        assert_eq!(expected, actual);
    }

    #[test]
    fn nested_constant() {
        let configuration = Configuration::default();
        let contents: String = String::from("Foo::Bar");

        let absolute_path = PathBuf::from("path/to/file.rb");
        let unresolved_references = vec![UnresolvedReference {
            name: String::from("Foo::Bar"),
            namespace_path: vec![],
            location: Range {
                start_row: 1,
                start_col: 0,
                end_row: 1,
                end_col: 9,
            },
        }];

        let definitions = vec![];

        let actual =
            process_from_contents(contents, &absolute_path, &configuration);
        let expected = ProcessedFile {
            absolute_path,
            unresolved_references,
            definitions,
            syntax_error: None,
            ..ProcessedFile::default()
        };
        assert_eq!(expected, actual);
    }

    #[test]
    fn deeply_nested_constant() {
        let configuration = Configuration::default();

        let contents: String = String::from("Foo::Bar::Baz");

        let absolute_path = PathBuf::from("path/to/file.rb");
        let unresolved_references = vec![UnresolvedReference {
            name: String::from("Foo::Bar::Baz"),
            namespace_path: vec![],
            location: Range {
                start_row: 1,
                start_col: 0,
                end_row: 1,
                end_col: 14,
            },
        }];

        let definitions = vec![];

        let actual =
            process_from_contents(contents, &absolute_path, &configuration);
        let expected = ProcessedFile {
            absolute_path,
            unresolved_references,
            definitions,
            syntax_error: None,
            ..ProcessedFile::default()
        };
        assert_eq!(expected, actual);
    }

    #[test]
    fn very_deeply_nested_constant() {
        let configuration = Configuration::default();

        let contents: String = String::from("Foo::Bar::Baz::Boo");

        let absolute_path = PathBuf::from("path/to/file.rb");
        let unresolved_references = vec![UnresolvedReference {
            name: String::from("Foo::Bar::Baz::Boo"),
            namespace_path: vec![],
            location: Range {
                start_row: 1,
                start_col: 0,
                end_row: 1,
                end_col: 19,
            },
        }];

        let definitions = vec![];

        let actual =
            process_from_contents(contents, &absolute_path, &configuration);
        let expected = ProcessedFile {
            absolute_path,
            unresolved_references,
            definitions,
            syntax_error: None,
            ..ProcessedFile::default()
        };
        assert_eq!(expected, actual);
    }

    #[test]
    fn class_definition_no_body() {
        let contents: String = String::from(
            "\
class Foo
end
            ",
        );

        let configuration = Configuration::default();

        let absolute_path = PathBuf::from("path/to/file.rb");
        let unresolved_references = vec![];

        let definitions = vec![];

        let actual =
            process_from_contents(contents, &absolute_path, &configuration);
        let expected = ProcessedFile {
            absolute_path,
            unresolved_references,
            definitions,
            syntax_error: None,
            ..ProcessedFile::default()
        };
        assert_eq!(expected, actual);
    }

    #[test]
    fn class_definition_some_body() {
        let contents: String = String::from(
            "\
class Foo
  def foo
  end
end
            ",
        );

        let configuration = Configuration::default();

        let absolute_path = PathBuf::from("path/to/file.rb");
        let unresolved_references = vec![];

        let definitions = vec![ParsedDefinition {
            fully_qualified_name: String::from("::Foo"),
            location: Range {
                start_row: 1,
                start_col: 6,
                end_row: 1,
                end_col: 10,
            },
        }];

        let actual =
            process_from_contents(contents, &absolute_path, &configuration);
        let expected = ProcessedFile {
            absolute_path,
            unresolved_references,
            definitions,
            syntax_error: None,
            ..ProcessedFile::default()
        };
        assert_eq!(expected, actual);
    }

    #[test]
    fn class_definition_some_body_with_class_method() {
        let contents: String = String::from(
            "\
class Foo
  def self.foo
  end
end
            ",
        );

        let configuration = Configuration::default();

        let absolute_path = PathBuf::from("path/to/file.rb");
        let unresolved_references = vec![];

        let definitions = vec![ParsedDefinition {
            fully_qualified_name: String::from("::Foo"),
            location: Range {
                start_row: 1,
                start_col: 6,
                end_row: 1,
                end_col: 10,
            },
        }];

        let actual =
            process_from_contents(contents, &absolute_path, &configuration);
        let expected = ProcessedFile {
            absolute_path,
            unresolved_references,
            definitions,
            syntax_error: None,
            ..ProcessedFile::default()
        };
        assert_eq!(expected, actual);
    }

    #[test]
    fn class_definition_some_method_call() {
        let contents: String = String::from(
            "\
class Foo
  method_call
end
            ",
        );

        let configuration = Configuration::default();

        let absolute_path = PathBuf::from("path/to/file.rb");
        let unresolved_references = vec![];

        let definitions = vec![ParsedDefinition {
            fully_qualified_name: String::from("::Foo"),
            location: Range {
                start_row: 1,
                start_col: 6,
                end_row: 1,
                end_col: 10,
            },
        }];

        let actual =
            process_from_contents(contents, &absolute_path, &configuration);
        let expected = ProcessedFile {
            absolute_path,
            unresolved_references,
            definitions,
            syntax_error: None,
            ..ProcessedFile::default()
        };
        assert_eq!(expected, actual);
    }

    #[test]
    fn polymorphic_belongs_to_association() {
        let contents: String = String::from(
            "\
class Foo
  belongs_to :event, polymorphic: true
end
            ",
        );

        let configuration = Configuration::default();

        let absolute_path = PathBuf::from("path/to/file.rb");
        let unresolved_references = vec![];

        let definitions = vec![ParsedDefinition {
            fully_qualified_name: String::from("::Foo"),
            location: Range {
                start_row: 1,
                start_col: 6,
                end_row: 1,
                end_col: 10,
            },
        }];

        let actual =
            process_from_contents(contents, &absolute_path, &configuration);
        let expected = ProcessedFile {
            absolute_path,
            unresolved_references,
            definitions,
            syntax_error: None,
            ..ProcessedFile::default()
        };
        assert_eq!(expected, actual);
    }

    fn definition_names(contents: &str) -> Vec<String> {
        process_from_contents(
            contents.to_owned(),
            &PathBuf::from("path/to/file.rb"),
            &Configuration::default(),
        )
        .definitions
        .into_iter()
        .map(|definition| definition.fully_qualified_name)
        .collect()
    }

    #[test]
    fn class_definition_with_calls_that_count() {
        // Each changes the class's behavior
        for body in [
            "@a.b",
            "BAR.[](1)",
            "self.a ||= 1",
            "self.a &&= 1",
            "self.a += 1",
            "self.a, self.b = 1, 2",
            "/(?<a>.)/ =~ BAR",
        ] {
            let contents = format!("class Foo\n  {}\nend\n", body);
            assert_eq!(definition_names(&contents), vec!["::Foo"], "{}", body);
        }
    }

    #[test]
    fn class_definition_with_calls_that_dont_count() {
        for body in ["BAR[1]", "BAR[1] = 2", "@a&.b", "@a&.b ||= 1"] {
            let contents = format!("class Foo\n  {}\nend\n", body);
            assert!(definition_names(&contents).is_empty(), "{}", body);
        }
    }

    #[test]
    fn constant_assignments() {
        // Each defines its targets, and only the value assigned is a reference,
        // not a target or its scope
        for (contents, definitions) in [
            ("A = Bar", &["::A"][..]),
            ("A::B = Bar", &["::A::B"]),
            ("A ||= Bar", &["::A"]),
            ("A &&= Bar", &["::A"]),
            ("A += Bar", &["::A"]),
            ("A::B ||= Bar", &["::A::B"]),
            ("A::B &&= Bar", &["::A::B"]),
            ("A::B += Bar", &["::A::B"]),
            ("A, B::C = Bar", &["::A", "::B::C"]),
            ("begin\nrescue Bar => A\nend", &["::A"]),
        ] {
            let references: Vec<String> = process_from_contents(
                contents.to_owned(),
                &PathBuf::from("path/to/file.rb"),
                &Configuration::default(),
            )
            .unresolved_references
            .into_iter()
            .map(|reference| reference.name)
            .collect();
            assert_eq!(references, vec!["Bar"], "{}", contents);
            assert_eq!(definition_names(contents), definitions, "{}", contents);
        }
    }

    #[test]
    fn definitions_from_the_top_level() {
        for (contents, definitions) in [
            ("class ::Foo; def a; end; end", &["::Foo"][..]),
            (
                "module Outer; class ::Foo; def a; end; end; end",
                &["::Foo"],
            ),
            (
                "module Outer; class ::Foo::Bar; def a; end; end; end",
                &["::Foo::Bar"],
            ),
            (
                "module Outer; module ::Foo; BAR = 1; end; end",
                &["::Foo::BAR"],
            ),
            ("module Outer; ::BAR = 1; end", &["::BAR"]),
            (
                "module Outer; module ::Foo; def a; end; end; end",
                &["::Foo"],
            ),
            (
                "module A; class ::B; module C; BAR = 1; end; end; end",
                &["::B::C::BAR"],
            ),
            (
                "module A; module ::B; module ::C; BAR = 1; end; end; end",
                &["::C::BAR"],
            ),
        ] {
            assert_eq!(definition_names(contents), definitions, "{}", contents);
        }
    }

    #[test]
    fn definitions_in_object() {
        for (contents, definitions) in [
            (
                "module CoreExt; class Object::Bar; def a; end; end; end",
                &["::Bar"][..],
            ),
            ("class Object; class Bar; def a; end; end; end", &["::Bar"]),
            (
                "module CoreExt; Object::BAR = 1; ::Object::BAZ = 1; end",
                &["::BAR", "::BAZ"],
            ),
            (
                "module CoreExt; class ::Object; BAR = 1; end; end",
                &["::BAR"],
            ),
            ("module CoreExt; ::Object::Object::BAR = 1; end", &["::BAR"]),
            (
                "module CoreExt; module Object::Bar; BAZ = 1; end; end",
                &["::Bar::BAZ"],
            ),
            (
                "module CoreExt; class Object::Bar; BAZ = 1; end; end",
                &["::Bar::BAZ"],
            ),
            ("class Object; def a; end; end", &["::Object"]),
            (
                "class ObjectSpace::Bar; def a; end; end",
                &["::ObjectSpace::Bar"],
            ),
            // A class written `Object` in a namespace is the namespace's own
            (
                "module CoreExt; class Object; BAR = 1; end; end",
                &["::CoreExt::Object::BAR"],
            ),
        ] {
            assert_eq!(definition_names(contents), definitions, "{}", contents);
        }
    }

    #[test]
    fn class_definition_with_private_constant() {
        let contents: String = String::from(
            "\
class Foo
  class Bar
    def bar; end
  end

  private_constant :Bar
end
            ",
        );

        let configuration = Configuration::default();

        let absolute_path = PathBuf::from("path/to/file.rb");
        let unresolved_references = vec![];

        let definitions = vec![ParsedDefinition {
            fully_qualified_name: String::from("::Foo::Bar"),
            location: Range {
                start_row: 2,
                start_col: 8,
                end_row: 2,
                end_col: 12,
            },
        }];

        let actual =
            process_from_contents(contents, &absolute_path, &configuration);
        let expected = ProcessedFile {
            absolute_path,
            unresolved_references,
            definitions,
            syntax_error: None,
            ..ProcessedFile::default()
        };
        assert_eq!(expected, actual);
    }

    #[test]
    fn nested_class_definition_before_function_def() {
        let contents: String = String::from(
            "\
class Bar
  class Baz
  end

  def hello
    1
  end
end
            ",
        );

        let configuration = Configuration::default();

        let absolute_path = PathBuf::from("path/to/file.rb");
        let unresolved_references = vec![];

        let definitions = vec![ParsedDefinition {
            fully_qualified_name: String::from("::Bar"),
            location: Range {
                start_row: 1,
                start_col: 6,
                end_row: 1,
                end_col: 10,
            },
        }];

        let actual =
            process_from_contents(contents, &absolute_path, &configuration);
        let expected = ProcessedFile {
            absolute_path,
            unresolved_references,
            definitions,
            syntax_error: None,
            ..ProcessedFile::default()
        };

        assert_eq!(expected, actual);
    }

    #[test]
    fn nested_class_definition_after_function_def() {
        let contents: String = String::from(
            "\
class Bar
  def hello
    1
  end

  class Baz
  end
end
            ",
        );

        let configuration = Configuration::default();

        let absolute_path = PathBuf::from("path/to/file.rb");
        let unresolved_references = vec![];

        let definitions = vec![ParsedDefinition {
            fully_qualified_name: String::from("::Bar"),
            location: Range {
                start_row: 1,
                start_col: 6,
                end_row: 1,
                end_col: 10,
            },
        }];

        let actual =
            process_from_contents(contents, &absolute_path, &configuration);
        let expected = ProcessedFile {
            absolute_path,
            unresolved_references,
            definitions,
            syntax_error: None,
            ..ProcessedFile::default()
        };

        assert_eq!(expected, actual);
    }
}
