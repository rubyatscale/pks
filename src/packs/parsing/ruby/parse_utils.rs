use std::collections::HashSet;

use line_col::LineColLookup;
use ruby_prism::{
    CallNode, ConstantId, Diagnostic, Diagnostics, KeywordHashNode, Location,
    Node,
};

use crate::packs::parsing::{
    ParsedDefinition, Range, SyntaxError, UnresolvedReference,
};

use super::inflector_shim::to_class_case;

#[derive(Debug)]
pub enum ParseError {
    Metaprogramming,
    // Add more variants as needed for different error cases
}

pub fn get_definition_from(
    current_nesting: &String,
    parent_nesting: &[String],
    location: &Range,
) -> ParsedDefinition {
    let name = current_nesting.to_owned();

    let owned_namespace_path: Vec<String> = parent_nesting.to_vec();

    let fully_qualified_name = if !owned_namespace_path.is_empty() {
        let mut name_components = owned_namespace_path;
        name_components.push(name);
        format!("::{}", name_components.join("::"))
    } else {
        format!("::{}", name)
    };

    ParsedDefinition {
        fully_qualified_name,
        location: location.to_owned(),
    }
}

/// The first error Prism reports, which is where it lost its way. Errors that
/// follow from it can sit earlier in the file. A template's `yield` is no
/// error, since Rails runs a template as the body of a method.
pub fn first_syntax_error(
    mut errors: Diagnostics,
    lookup: &LineColLookup,
    template: bool,
) -> Option<SyntaxError> {
    let error = errors.find(|error| {
        !(template && error_message(error).as_deref() == Some("Invalid yield"))
    });
    error.map(|error| SyntaxError {
        line: Some(lookup.get(error.location().start_offset()).0),
        message: error_message(&error)
            .unwrap_or_else(|| String::from("its message isn't UTF-8")),
    })
}

/// Some of Prism's messages quote the file, and `Diagnostic::message` panics on
/// a quote that ends mid-character, as one can when a magic comment names an
/// encoding other than UTF-8. The crate gives no other way to read a message.
fn error_message(error: &Diagnostic) -> Option<String> {
    std::panic::catch_unwind(|| error.message().to_owned()).ok()
}

pub fn loc_to_range(loc: &Location, lookup: &LineColLookup) -> Range {
    let (start_row, start_col) = lookup.get(loc.start_offset());
    let (end_row, end_col) = lookup.get(loc.end_offset());

    Range {
        start_row,
        // line_col counts columns from 1, and packwerk from 0
        start_col: start_col - 1,
        end_row,
        end_col,
    }
}

pub fn constant_name(id: &ConstantId) -> String {
    String::from_utf8_lossy(id.as_slice()).into_owned()
}

/// `Foo`, `Foo::Bar` or `::Foo`, for a constant whose scope is only constants
pub fn fetch_const_name(node: &Node) -> Result<String, ParseError> {
    if let Some(constant) = node.as_constant_read_node() {
        Ok(constant_name(&constant.name()))
    } else if let Some(path) = node.as_constant_path_node() {
        fetch_const_path_name(path.parent(), path.name())
    } else {
        Err(ParseError::Metaprogramming)
    }
}

/// A parent of None is `::`, as in `::Foo`
pub fn fetch_const_path_name(
    parent: Option<Node>,
    name: Option<ConstantId>,
) -> Result<String, ParseError> {
    // Prism leaves the name out of `Foo::` in a file that doesn't parse
    let name = name.ok_or(ParseError::Metaprogramming)?;
    let parent_namespace = match parent {
        Some(parent) => fetch_const_name(&parent)?,
        None => String::new(),
    };
    Ok(format!("{}::{}", parent_namespace, constant_name(&name)))
}

const ASSOCIATION_METHOD_NAMES: [&str; 4] = [
    "has_one",
    "has_many",
    "belongs_to",
    "has_and_belongs_to_many",
];

pub fn get_reference_from_active_record_association(
    node: &CallNode,
    current_namespaces: &[String],
    line_col_lookup: &LineColLookup,
    custom_associations: &[String],
) -> Option<UnresolvedReference> {
    // TODO: Read in args, process associations as a separate class
    // These can get complicated! e.g. we can specify a class name
    let combined_associations: Vec<String> = custom_associations
        .iter()
        .map(|s| s.to_owned())
        .chain(ASSOCIATION_METHOD_NAMES.iter().copied().map(String::from))
        .collect();

    let is_association =
        combined_associations.contains(&constant_name(&node.name()));

    if is_association {
        let args: Vec<Node> = node
            .arguments()
            .map(|arguments| arguments.arguments().iter().collect())
            .unwrap_or_default();
        let first_arg: Option<&Node> = args.first();
        let inherited_resources =
            may_be_inherited_resources(current_namespaces);

        let mut name: Option<String> = None;
        for node in args.iter() {
            if let Some(kwargs) = node.as_keyword_hash_node() {
                // Rails reads a polymorphic association's class from a type
                // column at runtime, so the declaration names no class, even
                // with `class_name:`. Rails 8.1 rejects that option here, and
                // before 8.1 it was used only to load the class for
                // `counter_cache:`. With `counter_cache:`, 8.1 still loads the
                // class named after the association. pks counts neither load
                // as a reference.
                if is_polymorphic(&kwargs) && !inherited_resources {
                    return None;
                }
                if let Some(found) = extract_class_name_from_kwargs(&kwargs) {
                    name = Some(found);
                }
            }
        }

        if let Some(d) = first_arg.and_then(|arg| arg.as_symbol_node()) {
            if name.is_none() {
                // We singularize here because by convention Rails will singularize the class name as declared via a symbol,
                // e.g. `has_many :companies` will look for a class named `Company`, not `Companies`
                name = Some(to_class_case(
                    &String::from_utf8_lossy(d.unescaped()),
                    true,
                    &HashSet::new(), // todo: pass in acronyms here
                ));
            }
        }

        // let unwrapped_name = name.unwrap_or_else(|| {
        //     panic!("Could not find class name for association {:?}", &node,)
        // });
        // Later we should probably handle these cases!
        if name.is_some() {
            let unwrapped_name = name.unwrap_or_else(|| {
                panic!("Could not find class name for association {:?}", node)
            });

            Some(UnresolvedReference {
                name: unwrapped_name,
                namespace_path: current_namespaces.to_owned(),
                location: loc_to_range(&node.location(), line_col_lookup),
            })
        } else {
            None
        }
    } else {
        None
    }
}

fn extract_class_name_from_kwargs(kwargs: &KeywordHashNode) -> Option<String> {
    for pair_node in kwargs.elements().iter() {
        if let Some(pair) = pair_node.as_assoc_node() {
            if let Some(k) = pair.key().as_symbol_node() {
                if k.unescaped() == b"class_name" {
                    if let Some(v) = pair.value().as_string_node() {
                        return Some(
                            String::from_utf8_lossy(v.unescaped()).into_owned(),
                        );
                    }
                }
            }
        }
    }

    None
}

fn is_polymorphic(kwargs: &KeywordHashNode) -> bool {
    for pair_node in kwargs.elements().iter() {
        if let Some(pair) = pair_node.as_assoc_node() {
            if let (Some(k), Some(_)) =
                (pair.key().as_symbol_node(), pair.value().as_true_node())
            {
                if k.unescaped() == b"polymorphic" {
                    return true;
                }
            }
        }
    }

    false
}

/// InheritedResources' `belongs_to` loads the class even when polymorphic. It
/// runs in a controller, or at the top level of an ActiveAdmin `register` or
/// `controller do` block, which passes its options through. An Active Record
/// model declares its associations inside its class.
fn may_be_inherited_resources(current_namespaces: &[String]) -> bool {
    current_namespaces
        .last()
        .is_none_or(|namespace| namespace.ends_with("Controller"))
}

/// `name` is that of the constant assigned, as `FOO` or `Foo::BAR`
pub fn get_constant_assignment_definition(
    name: Result<String, ParseError>,
    location: &Location,
    current_namespaces: Vec<String>,
    line_col_lookup: &LineColLookup,
) -> Option<ParsedDefinition> {
    let name = name.ok()?;
    let fully_qualified_name = if !current_namespaces.is_empty() {
        let mut name_components = current_namespaces;
        name_components.push(name);
        format!("::{}", name_components.join("::"))
    } else {
        format!("::{}", name)
    };

    Some(ParsedDefinition {
        fully_qualified_name,
        location: loc_to_range(location, line_col_lookup),
    })
}

/// `foo&.bar`, `foo[1]` and `foo[1] = 2` are calls in Prism's tree, but pks has
/// never counted them as calls.
pub fn is_plain_call(node: &CallNode) -> bool {
    let index = matches!(node.name().as_slice(), b"[]" | b"[]=")
        && node.call_operator_loc().is_none();
    !node.is_safe_navigation() && !index
}

/// The `Visit` methods for the nodes that assign a constant, for an
/// `impl<'pr> Visit<'pr>` whose type has a `define_constant(name, location)`
/// method. They visit only the value assigned, so neither the constant nor its
/// scope is a reference.
macro_rules! visit_constant_assignments {
    () => {
        fn visit_constant_write_node(
            &mut self,
            node: &ruby_prism::ConstantWriteNode<'pr>,
        ) {
            self.define_constant(
                Ok($crate::packs::parsing::ruby::parse_utils::constant_name(
                    &node.name(),
                )),
                &node.location(),
            );
            self.visit(&node.value());
        }

        fn visit_constant_path_write_node(
            &mut self,
            node: &ruby_prism::ConstantPathWriteNode<'pr>,
        ) {
            let target = node.target();
            self.define_constant(
                $crate::packs::parsing::ruby::parse_utils::fetch_const_path_name(
                    target.parent(),
                    target.name(),
                ),
                &node.location(),
            );
            self.visit(&node.value());
        }

        fn visit_constant_or_write_node(
            &mut self,
            node: &ruby_prism::ConstantOrWriteNode<'pr>,
        ) {
            self.define_constant(
                Ok($crate::packs::parsing::ruby::parse_utils::constant_name(
                    &node.name(),
                )),
                &node.name_loc(),
            );
            self.visit(&node.value());
        }

        fn visit_constant_and_write_node(
            &mut self,
            node: &ruby_prism::ConstantAndWriteNode<'pr>,
        ) {
            self.define_constant(
                Ok($crate::packs::parsing::ruby::parse_utils::constant_name(
                    &node.name(),
                )),
                &node.name_loc(),
            );
            self.visit(&node.value());
        }

        fn visit_constant_operator_write_node(
            &mut self,
            node: &ruby_prism::ConstantOperatorWriteNode<'pr>,
        ) {
            self.define_constant(
                Ok($crate::packs::parsing::ruby::parse_utils::constant_name(
                    &node.name(),
                )),
                &node.name_loc(),
            );
            self.visit(&node.value());
        }

        fn visit_constant_path_or_write_node(
            &mut self,
            node: &ruby_prism::ConstantPathOrWriteNode<'pr>,
        ) {
            let target = node.target();
            self.define_constant(
                $crate::packs::parsing::ruby::parse_utils::fetch_const_path_name(
                    target.parent(),
                    target.name(),
                ),
                &target.location(),
            );
            self.visit(&node.value());
        }

        fn visit_constant_path_and_write_node(
            &mut self,
            node: &ruby_prism::ConstantPathAndWriteNode<'pr>,
        ) {
            let target = node.target();
            self.define_constant(
                $crate::packs::parsing::ruby::parse_utils::fetch_const_path_name(
                    target.parent(),
                    target.name(),
                ),
                &target.location(),
            );
            self.visit(&node.value());
        }

        fn visit_constant_path_operator_write_node(
            &mut self,
            node: &ruby_prism::ConstantPathOperatorWriteNode<'pr>,
        ) {
            let target = node.target();
            self.define_constant(
                $crate::packs::parsing::ruby::parse_utils::fetch_const_path_name(
                    target.parent(),
                    target.name(),
                ),
                &target.location(),
            );
            self.visit(&node.value());
        }

        // As in `A, B = 1, 2` or `rescue => A`
        fn visit_constant_target_node(
            &mut self,
            node: &ruby_prism::ConstantTargetNode<'pr>,
        ) {
            self.define_constant(
                Ok($crate::packs::parsing::ruby::parse_utils::constant_name(
                    &node.name(),
                )),
                &node.location(),
            );
        }

        fn visit_constant_path_target_node(
            &mut self,
            node: &ruby_prism::ConstantPathTargetNode<'pr>,
        ) {
            self.define_constant(
                $crate::packs::parsing::ruby::parse_utils::fetch_const_path_name(
                    node.parent(),
                    node.name(),
                ),
                &node.location(),
            );
        }
    };
}
pub(crate) use visit_constant_assignments;
