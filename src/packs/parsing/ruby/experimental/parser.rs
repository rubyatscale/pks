use crate::packs::file_utils::file_read_contents;
use crate::packs::{
    parsing::{
        ruby::{
            namespace_calculator::nest_constant_name,
            parse_utils::{
                constant_name, fetch_const_name, fetch_const_path_name,
                first_syntax_error,
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
    CallAndWriteNode, CallNode, CallOperatorWriteNode, CallOrWriteNode,
    CallTargetNode, ClassNode, ConstantPathNode, ConstantReadNode, DefNode,
    Location, ModuleNode, Visit,
};
use std::path::Path;

struct ReferenceCollector<'a> {
    pub references: Vec<UnresolvedReference>,
    pub definitions: Vec<ParsedDefinition>,
    pub current_namespaces: Vec<String>,
    pub line_col_lookup: LineColLookup<'a>,
    pub behavioral_change_in_namespace: bool,
    pub custom_associations: Vec<String>,
}

impl ReferenceCollector<'_> {
    fn define_constant(
        &mut self,
        name: Result<String, ParseError>,
        location: &Location,
    ) {
        if let Ok(name) = name {
            let location = loc_to_range(location, &self.line_col_lookup);
            let definition = self.definition(&defined_name(name), location);
            self.definitions.push(definition);
        }
    }

    fn definition(&self, name: &str, location: Range) -> ParsedDefinition {
        let namespaces: Vec<&str> =
            self.current_namespaces.iter().map(String::as_str).collect();
        ParsedDefinition {
            fully_qualified_name: nest_constant_name(&namespaces, name),
            location,
        }
    }

    /// Rails looks up an association's class from its model's name, so these
    /// are that name's: `["Foo"]` in `module Outer; class ::Foo`, and
    /// `["Outer", "Foo"]` in `class Outer::Foo`.
    fn model_namespaces(&self) -> Vec<String> {
        let Some((model, namespaces)) = self.current_namespaces.split_last()
        else {
            return vec![];
        };
        let namespaces: Vec<&str> =
            namespaces.iter().map(String::as_str).collect();
        nest_constant_name(&namespaces, model)
            .split("::")
            .skip(1)
            .map(str::to_owned)
            .collect()
    }

    fn reference_constant(
        &mut self,
        name: Result<String, ParseError>,
        location: &Location,
    ) {
        let Ok(name) = name else {
            return;
        };

        let namespace_path = self
            .current_namespaces
            .clone()
            .into_iter()
            .filter(|namespace| namespace != &name)
            .collect::<Vec<String>>();

        self.references.push(UnresolvedReference {
            name,
            namespace_path,
            location: loc_to_range(location, &self.line_col_lookup),
        })
    }

    /// `a.b ||= c` and the targets of `a.b, c.d = e` call `a.b=` too
    fn on_call_assignment(&mut self, safe_navigation: bool) {
        if !safe_navigation {
            self.behavioral_change_in_namespace = true;
        }
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

        let namespace = defined_name(namespace_result.unwrap());

        if let Some(inner) = node.superclass() {
            self.visit(&inner);
        }
        let location = loc_to_range(
            &node.constant_path().location(),
            &self.line_col_lookup,
        );

        let definition = self.definition(&namespace, location);

        // Note – is there a way to use lifetime specifiers to get rid of this and
        // just keep current namespaces as a vector of string references or something else
        // more efficient?
        self.current_namespaces.push(namespace);

        // Each time we open up a new class/module, we reset the behavioral change flag
        let previous_behavioral_change = self.behavioral_change_in_namespace;
        self.behavioral_change_in_namespace = false;

        if let Some(inner) = node.body() {
            self.visit(&inner);
        }

        if self.behavioral_change_in_namespace {
            self.definitions.push(definition);
        }

        // When we're done visiting the class/module, we restore the previous behavioral change flag
        // to account for nested class/module definitions
        self.behavioral_change_in_namespace = previous_behavioral_change;

        self.current_namespaces.pop();
    }

    fn visit_call_node(&mut self, node: &CallNode<'pr>) {
        // `private_constant` is not considered to be a behavioral change
        if is_plain_call(node) && node.name().as_slice() != b"private_constant"
        {
            self.behavioral_change_in_namespace = true;

            let association_reference =
                get_reference_from_active_record_association(
                    node,
                    &self.current_namespaces,
                    &self.line_col_lookup,
                    &self.custom_associations,
                );

            if let Some(mut association_reference) = association_reference {
                association_reference.namespace_path = self.model_namespaces();
                self.references.push(association_reference);
            }
        }
        ruby_prism::visit_call_node(self, node);
    }

    fn visit_call_or_write_node(&mut self, node: &CallOrWriteNode<'pr>) {
        self.on_call_assignment(node.is_safe_navigation());
        ruby_prism::visit_call_or_write_node(self, node);
    }

    fn visit_call_and_write_node(&mut self, node: &CallAndWriteNode<'pr>) {
        self.on_call_assignment(node.is_safe_navigation());
        ruby_prism::visit_call_and_write_node(self, node);
    }

    fn visit_call_operator_write_node(
        &mut self,
        node: &CallOperatorWriteNode<'pr>,
    ) {
        self.on_call_assignment(node.is_safe_navigation());
        ruby_prism::visit_call_operator_write_node(self, node);
    }

    fn visit_call_target_node(&mut self, node: &CallTargetNode<'pr>) {
        self.on_call_assignment(node.is_safe_navigation());
        ruby_prism::visit_call_target_node(self, node);
    }

    visit_constant_assignments!();

    fn visit_module_node(&mut self, node: &ModuleNode<'pr>) {
        let namespace = defined_name(
            fetch_const_name(&node.constant_path()).unwrap_or("".to_owned()),
        );
        let location = loc_to_range(
            &node.constant_path().location(),
            &self.line_col_lookup,
        );

        let definition = self.definition(&namespace, location);

        // Note – is there a way to use lifetime specifiers to get rid of this and
        // just keep current namespaces as a vector of string references or something else
        // more efficient?
        self.current_namespaces.push(namespace);

        // Each time we open up a new class/module, we reset the behavioral change flag
        let previous_behavioral_change = self.behavioral_change_in_namespace;
        self.behavioral_change_in_namespace = false;

        if let Some(inner) = node.body() {
            self.visit(&inner);
        }

        if self.behavioral_change_in_namespace {
            self.definitions.push(definition);
        }

        // When we're done visiting the class/module, we restore the previous behavioral change flag
        // to account for nested class/module definitions
        self.behavioral_change_in_namespace = previous_behavioral_change;

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

    // A `def` with a receiver, as `def self.foo`, too
    fn visit_def_node(&mut self, node: &DefNode<'pr>) {
        self.behavioral_change_in_namespace = true;
        ruby_prism::visit_def_node(self, node);
    }
}

/// The name of a class, module or constant as defined, with `Object::Foo` taken
/// for `::Foo`. Ruby looks `Object` up as it does any constant, so a namespace
/// could define an `Object` of its own, but pks can't look it up here, and takes
/// it that none does.
fn defined_name(name: String) -> String {
    match name.strip_prefix("Object::") {
        Some(name) => format!("::{}", name),
        None => name,
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
    // Rails runs a Builder template, as it does an ERB one, as a method body
    let template = path.extension().is_some_and(|ext| ext == "builder");
    process(contents, path, configuration, template)
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
        behavioral_change_in_namespace: false,
        custom_associations: configuration.custom_associations.clone(),
    };

    collector.visit(&parse_result.node());

    let unresolved_references = collector.references;

    let absolute_path = path.to_owned();

    // The packwerk parser uses a ConstantResolver constructed by constants inferred from the file system
    // see zeitwerk_utils for more.
    // For a parser that uses parsed constants, see the experimental parser
    let definitions = collector.definitions;

    ProcessedFile {
        absolute_path,
        unresolved_references,
        definitions,
        syntax_error,
    }
}
