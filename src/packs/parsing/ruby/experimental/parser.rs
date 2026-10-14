use crate::packs::file_utils::file_read_contents;
use crate::packs::{
    parsing::{
        ruby::{
            namespace_calculator::nest_constant_name,
            parse_utils::{
                constant_name, fetch_const_name, fetch_const_path_name,
                first_syntax_error,
                get_reference_from_active_record_association, is_plain_call,
                loc_to_range, names_its_class, visit_constant_assignments,
                ParseError,
            },
        },
        Inclusion, ParsedDefinition, Range, UnresolvedAssociation,
        UnresolvedReference,
    },
    Configuration, ProcessedFile,
};
use line_col::LineColLookup;
use ruby_prism::{
    CallAndWriteNode, CallNode, CallOperatorWriteNode, CallOrWriteNode,
    CallTargetNode, ClassNode, ConstantPathNode, ConstantReadNode, DefNode,
    Location, ModuleNode, Visit,
};
use std::{collections::HashSet, path::Path};

struct ReferenceCollector<'a> {
    pub references: Vec<UnresolvedReference>,
    pub definitions: Vec<ParsedDefinition>,
    pub associations: Vec<UnresolvedAssociation>,
    pub inclusions: Vec<Inclusion>,
    pub current_namespaces: Vec<String>,
    /// Whether each of `current_namespaces` is a module
    pub modules: Vec<bool>,
    pub line_col_lookup: LineColLookup<'a>,
    pub behavioral_change_in_namespace: bool,
    pub custom_associations: Vec<String>,
    /// The namespaces' own `Object`s that the app defines, as
    /// `::Payments::Object`
    pub objects: &'a HashSet<String>,
    pub object_in_a_namespace: bool,
    /// The namespaces' own `Object`s that the file opens
    pub namespace_objects: Vec<String>,
}

impl ReferenceCollector<'_> {
    fn define_constant(
        &mut self,
        name: Result<String, ParseError>,
        location: &Location,
    ) {
        if let Ok(name) = name {
            let location = loc_to_range(location, &self.line_col_lookup);
            let name = self.defined_name(name);
            let definition = self.definition(&name, location);
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

    /// The name of a class, module or constant as defined. Ruby looks up the
    /// `Object` of `Object::Foo` from the innermost namespace out, so it's a
    /// namespace's own where the app defines one, and otherwise `::Object`,
    /// whose constants are the top level's, so `Object::Foo` is `::Foo`.
    fn defined_name(&mut self, name: String) -> String {
        let Some(rest) = name.strip_prefix("Object::") else {
            return name;
        };
        let namespaces: Vec<&str> =
            self.current_namespaces.iter().map(String::as_str).collect();
        if !namespaces.is_empty() {
            self.object_in_a_namespace = true;
        }
        for depth in (1..=namespaces.len()).rev() {
            let object = nest_constant_name(&namespaces[..depth], "Object");
            if self.objects.contains(&object) {
                return format!("{}::{}", object, rest);
            }
        }
        format!("::{}", rest)
    }

    /// The fully qualified name of the class or module the code is in
    fn namespace_name(&self) -> Option<String> {
        let (last, namespaces) = self.current_namespaces.split_last()?;
        let namespaces: Vec<&str> =
            namespaces.iter().map(String::as_str).collect();
        Some(nest_constant_name(&namespaces, last))
    }

    /// Rails looks up an association's class from its model's name, so in a
    /// class or module it waits for the rest of the app
    fn associate(&mut self, node: &CallNode, reference: UnresolvedReference) {
        match (self.namespace_name(), self.modules.last()) {
            (Some(model), Some(&in_module))
                if !reference.name.starts_with("::") =>
            {
                self.associations.push(UnresolvedAssociation {
                    name: reference.name,
                    model,
                    in_module,
                    belongs_to: node.name().as_slice() == b"belongs_to",
                    names_its_class: names_its_class(node),
                    location: reference.location,
                });
            }
            _ => self.references.push(UnresolvedReference {
                namespace_path: vec![],
                ..reference
            }),
        }
    }

    /// Opens a class or module, noting one that is a namespace's own
    /// `Object`, as `class Object` in `module CoreExt`, whether or not it
    /// defines anything
    fn open_namespace(&mut self, namespace: String, module: bool) {
        self.current_namespaces.push(namespace);
        self.modules.push(module);
        if let Some(name) = self.namespace_name() {
            if name.ends_with("::Object") && name != "::Object" {
                self.namespace_objects.push(name);
            }
        }
    }

    /// `include Taggable` or `prepend Taggable` in a class or module
    fn include(&mut self, node: &CallNode) {
        let (Some(includer), Some(&includer_is_module)) =
            (self.namespace_name(), self.modules.last())
        else {
            return;
        };
        let Some(arguments) = node.arguments() else {
            return;
        };
        for argument in arguments.arguments().iter() {
            if let Ok(name) = fetch_const_name(&argument) {
                self.inclusions.push(Inclusion {
                    includer: includer.clone(),
                    includer_is_module,
                    name,
                    namespace_path: self.current_namespaces.clone(),
                });
            }
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

        let namespace = self.defined_name(namespace_result.unwrap());

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
        self.open_namespace(namespace, false);

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
        self.modules.pop();
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

            if let Some(association_reference) = association_reference {
                self.associate(node, association_reference);
            }

            if node.receiver().is_none()
                && matches!(node.name().as_slice(), b"include" | b"prepend")
            {
                self.include(node);
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
        let namespace = self.defined_name(
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
        self.open_namespace(namespace, true);

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
        self.modules.pop();
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

pub(crate) fn process_from_path(
    path: &Path,
    configuration: &Configuration,
) -> anyhow::Result<ProcessedFile> {
    process_from_path_with_objects(path, configuration, &HashSet::new())
}

/// With `objects`, the namespaces' own `Object`s that the app defines
pub(crate) fn process_from_path_with_objects(
    path: &Path,
    configuration: &Configuration,
    objects: &HashSet<String>,
) -> anyhow::Result<ProcessedFile> {
    let contents = file_read_contents(path, configuration)?;
    // Rails runs a Builder template, as it does an ERB one, as a method body
    let template = path.extension().is_some_and(|ext| ext == "builder");
    Ok(process(contents, path, configuration, template, objects))
}

#[cfg(test)]
pub(crate) fn process_from_contents(
    contents: String,
    path: &Path,
    configuration: &Configuration,
) -> ProcessedFile {
    let template = path.extension().is_some_and(|ext| ext == "builder");
    process(contents, path, configuration, template, &HashSet::new())
}

/// For the Ruby that pks makes from an ERB template
pub(crate) fn process_from_template_contents(
    contents: String,
    path: &Path,
    configuration: &Configuration,
) -> ProcessedFile {
    process(contents, path, configuration, true, &HashSet::new())
}

fn process(
    contents: String,
    path: &Path,
    configuration: &Configuration,
    template: bool,
    objects: &HashSet<String>,
) -> ProcessedFile {
    let lookup = LineColLookup::new(&contents);
    let parse_result = ruby_prism::parse(contents.as_bytes());
    let syntax_error =
        first_syntax_error(parse_result.errors(), &lookup, template);

    let mut collector = ReferenceCollector {
        references: vec![],
        definitions: vec![],
        associations: vec![],
        inclusions: vec![],
        current_namespaces: vec![],
        modules: vec![],
        line_col_lookup: lookup,
        behavioral_change_in_namespace: false,
        custom_associations: configuration.custom_associations.clone(),
        objects,
        object_in_a_namespace: false,
        namespace_objects: vec![],
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
        associations: collector.associations,
        inclusions: collector.inclusions,
        object_in_a_namespace: collector.object_in_a_namespace,
        namespace_objects: collector.namespace_objects,
        syntax_error,
    }
}
