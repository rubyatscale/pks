use crate::packs::file_utils::file_read_contents;
use crate::packs::{
    parsing::{
        ruby::{
            namespace_calculator::possible_fully_qualified_constants,
            parse_utils::{
                constant_name, fetch_const_name, fetch_const_path_name,
                first_syntax_error, get_constant_assignment_definition,
                get_definition_from,
                get_reference_from_active_record_association, is_plain_call,
                loc_to_range, visit_constant_assignments, ParseError,
            },
        },
        ParsedDefinition, Range, UnresolvedReference,
    },
    Configuration, ProcessedFile,
};
use line_col::LineColLookup;
use ruby_prism::{
    CallNode, ClassNode, ConstantPathNode, ConstantReadNode, Location,
    ModuleNode, Visit,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct SuperclassReference {
    pub name: String,
    pub namespace_path: Vec<String>,
}

struct ReferenceCollector<'a> {
    pub references: Vec<UnresolvedReference>,
    pub definitions: Vec<ParsedDefinition>,
    pub current_namespaces: Vec<String>,
    pub line_col_lookup: LineColLookup<'a>,
    pub in_superclass: bool,
    pub superclasses: Vec<SuperclassReference>,
    pub custom_associations: Vec<String>,
}

impl ReferenceCollector<'_> {
    fn define_constant(
        &mut self,
        name: Result<String, ParseError>,
        location: &Location,
    ) {
        let definition = get_constant_assignment_definition(
            name,
            location,
            self.current_namespaces.to_owned(),
            &self.line_col_lookup,
        );

        if let Some(definition) = definition {
            self.definitions.push(definition);
        }
    }

    fn reference_constant(
        &mut self,
        name: Result<String, ParseError>,
        location: &Location,
    ) {
        let Ok(name) = name else {
            return;
        };

        if self.in_superclass {
            self.superclasses.push(SuperclassReference {
                name: name.to_owned(),
                namespace_path: self.current_namespaces.to_owned(),
            })
        }
        // In packwerk, NodeHelpers.enclosing_namespace_path ignores
        // namespaces where a superclass OR namespace is the same as the current reference name
        let matching_superclass_option = self
            .superclasses
            .iter()
            .find(|superclass| superclass.name == name);

        let namespace_path =
            if let Some(matching_superclass) = matching_superclass_option {
                matching_superclass.namespace_path.to_owned()
            } else {
                self.current_namespaces
                    .clone()
                    .into_iter()
                    .filter(|namespace| {
                        namespace != &name
                            || self
                                .superclasses
                                .iter()
                                .any(|superclass| superclass.name == name)
                    })
                    .collect::<Vec<String>>()
            };

        self.references.push(UnresolvedReference {
            name,
            namespace_path,
            location: loc_to_range(location, &self.line_col_lookup),
        })
    }
}

impl<'pr> Visit<'pr> for ReferenceCollector<'_> {
    fn visit_class_node(&mut self, node: &ClassNode<'pr>) {
        let namespace_result = fetch_const_name(&node.constant_path());
        // For now, we simply exit and stop traversing if we encounter an error when fetching the constant name of a class
        // We can iterate on this if this is different than the packwerk implementation
        if namespace_result.is_err() {
            return;
        }

        let namespace = namespace_result.unwrap();

        if let Some(inner) = node.superclass() {
            self.in_superclass = true;
            self.visit(&inner);
            self.in_superclass = false;
        }
        let location = loc_to_range(
            &node.constant_path().location(),
            &self.line_col_lookup,
        );

        let definition = get_definition_from(
            &namespace,
            &self.current_namespaces,
            &location,
        );

        let name = definition.fully_qualified_name.to_owned();
        let namespace_path = self.current_namespaces.to_owned();
        self.definitions.push(definition);

        // Packwerk also considers a definition to be a "reference"
        self.references.push(UnresolvedReference {
            name,
            namespace_path,
            location,
        });

        // Note – is there a way to use lifetime specifiers to get rid of this and
        // just keep current namespaces as a vector of string references or something else
        // more efficient?
        self.current_namespaces.push(namespace);

        if let Some(inner) = node.body() {
            self.visit(&inner);
        }

        self.current_namespaces.pop();
        self.superclasses.pop();
    }

    fn visit_call_node(&mut self, node: &CallNode<'pr>) {
        if is_plain_call(node) {
            let association_reference =
                get_reference_from_active_record_association(
                    node,
                    &self.current_namespaces,
                    &self.line_col_lookup,
                    &self.custom_associations,
                );

            if let Some(association_reference) = association_reference {
                self.references.push(association_reference);
            }
        }

        ruby_prism::visit_call_node(self, node);
    }

    visit_constant_assignments!();

    fn visit_module_node(&mut self, node: &ModuleNode<'pr>) {
        // A module whose name isn't a constant, as `module self::Foo`, or one
        // in a file that doesn't parse
        let Ok(namespace) = fetch_const_name(&node.constant_path()) else {
            return;
        };
        let location = loc_to_range(
            &node.constant_path().location(),
            &self.line_col_lookup,
        );

        let definition = get_definition_from(
            &namespace,
            &self.current_namespaces,
            &location,
        );

        let name = definition.fully_qualified_name.to_owned();
        let namespace_path = self.current_namespaces.to_owned();
        self.definitions.push(definition);

        // Packwerk also considers a definition to be a "reference"
        self.references.push(UnresolvedReference {
            name,
            namespace_path,
            location,
        });

        // Note – is there a way to use lifetime specifiers to get rid of this and
        // just keep current namespaces as a vector of string references or something else
        // more efficient?
        self.current_namespaces.push(namespace);

        if let Some(inner) = node.body() {
            self.visit(&inner);
        }

        self.current_namespaces.pop();
    }

    fn visit_constant_read_node(&mut self, node: &ConstantReadNode<'pr>) {
        self.reference_constant(
            Ok(constant_name(&node.name())),
            &node.location(),
        );
    }

    // Not visiting the parent: `Foo::Bar` is one reference, and in
    // `foo(Bar)::Baz`, neither constant is one
    fn visit_constant_path_node(&mut self, node: &ConstantPathNode<'pr>) {
        self.reference_constant(
            fetch_const_path_name(node.parent(), node.name()),
            &node.location(),
        );
    }
}

pub(crate) fn process_from_path(
    path: &Path,
    configuration: &Configuration,
) -> anyhow::Result<ProcessedFile> {
    let contents = file_read_contents(path, configuration)?;
    Ok(process_from_contents(contents, path, configuration))
}

pub(crate) fn process_from_contents(
    contents: String,
    path: &Path,
    configuration: &Configuration,
) -> ProcessedFile {
    process(contents, path, configuration, false)
}

/// For the Ruby that pks makes from an ERB template
pub(crate) fn process_from_template_contents(
    contents: String,
    path: &Path,
    configuration: &Configuration,
) -> ProcessedFile {
    process(contents, path, configuration, true)
}

fn process(
    contents: String,
    path: &Path,
    configuration: &Configuration,
    template: bool,
) -> ProcessedFile {
    let lookup = LineColLookup::new(&contents);
    let parse_result = ruby_prism::parse(contents.as_bytes());
    let syntax_error =
        first_syntax_error(parse_result.errors(), &lookup, template);

    let mut collector = ReferenceCollector {
        references: vec![],
        current_namespaces: vec![],
        definitions: vec![],
        line_col_lookup: lookup,
        in_superclass: false,
        superclasses: vec![],
        custom_associations: configuration.custom_associations.clone(),
    };

    collector.visit(&parse_result.node());

    let mut definition_to_location_map: HashMap<String, Range> = HashMap::new();

    for d in &collector.definitions {
        let parts: Vec<&str> = d.fully_qualified_name.split("::").collect();
        // We do this to handle nested constants, e.g.
        // class Foo::Bar
        // end
        for (index, _) in parts.iter().enumerate() {
            let combined = &parts[..=index].join("::");
            // If the map already contains the key, skip it.
            // This is helpful, e.g.
            // class Foo::Bar
            //  BAZ
            // end
            // The fully name for BAZ IS ::Foo::Bar::BAZ, so we do not want to overwrite
            // the definition location for ::Foo or ::Foo::Bar
            if !definition_to_location_map.contains_key(combined) {
                definition_to_location_map
                    .insert(combined.to_owned(), d.location.clone());
            }
        }
    }

    let unresolved_references = collector
        .references
        .into_iter()
        .filter(|r| {
            let mut should_ignore_local_reference = false;
            let namespace_path = r
                .namespace_path
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<&str>>();
            let possible_constants =
                possible_fully_qualified_constants(&namespace_path, &r.name);
            for constant_name in possible_constants {
                if let Some(location) = definition_to_location_map
                    .get(&constant_name)
                    .or(definition_to_location_map
                        .get(&format!("::{}", constant_name)))
                {
                    let reference_is_definition = location.start_row
                        == r.location.start_row
                        && location.start_col == r.location.start_col;
                    // In lib/packwerk/parsed_constant_definitions.rb, we don't count references when the reference is in the same place as the definition
                    // This is an idiosyncrasy we are porting over here for behavioral alignment, although we might be doing some unnecessary work.
                    should_ignore_local_reference = !reference_is_definition;
                }
            }
            !should_ignore_local_reference
        })
        .collect();

    let absolute_path = path.to_owned();

    // The packwerk parser uses a ConstantResolver constructed by constants inferred from the file system
    // see zeitwerk_utils for more.
    // For a parser that uses parsed constants, see the experimental parser
    let definitions = vec![];

    ProcessedFile {
        absolute_path,
        unresolved_references,
        definitions,
        syntax_error,
    }
}
