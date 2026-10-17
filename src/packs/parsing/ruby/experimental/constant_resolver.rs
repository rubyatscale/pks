use tracing::debug;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use crate::packs::{
    constant_resolver::{ConstantDefinition, ConstantResolver},
    parsing::ruby::namespace_calculator::nest_constant_name,
};

#[derive(Default, Debug)]
pub struct ExperimentalConstantResolver {
    pub fully_qualified_constant_name_to_constant_definition_map:
        HashMap<String, Vec<ConstantDefinition>>,
}

impl ConstantResolver for ExperimentalConstantResolver {
    fn resolve(
        &self,
        fully_or_partially_qualified_constant: &str,
        namespace_path: &[&str],
    ) -> Option<Vec<ConstantDefinition>> {
        // If the fully_or_partially_qualified_constant is prefixed with ::, the namespace path is technically empty, since it's a global reference
        let (namespace_path, const_name) =
            if fully_or_partially_qualified_constant.starts_with("::") {
                // `resolve_constant` will add a leading :: before it makes a guess at the fully qualified name
                // so we remove it here and represent it as a relative constant with no namespace path
                let const_name = fully_or_partially_qualified_constant
                    .strip_prefix("::")
                    .unwrap();
                let namespace_path: &[&str] = &[];
                (namespace_path, const_name)
            } else {
                (namespace_path, fully_or_partially_qualified_constant)
            };

        let definitions =
            self.resolve_traversing_namespace_path(const_name, namespace_path);
        if !definitions.is_empty() {
            return Some(definitions);
        }
        // Then, as the top level's constants are `Object`'s, those of its
        // ancestors, as `BasicObject::Foo` for `Foo`, from anywhere
        let top_level = nest_constant_name(&[], const_name);
        for ancestor in ["::Kernel", "::BasicObject"] {
            let definitions = self.constant_for_fully_qualified_name(&format!(
                "{}{}",
                ancestor, top_level
            ));
            if !definitions.is_empty() {
                return Some(definitions);
            }
        }
        Some(vec![])
    }

    fn fully_qualified_constant_name_to_constant_definition_map(
        &self,
    ) -> &HashMap<String, Vec<ConstantDefinition>> {
        &self.fully_qualified_constant_name_to_constant_definition_map
    }
}

impl ExperimentalConstantResolver {
    pub fn create(
        constants: Vec<ConstantDefinition>,
        absolute_root: &Path,
        ignored_definitions: &HashMap<String, HashSet<PathBuf>>,
    ) -> Box<dyn ConstantResolver + Send + Sync> {
        debug!("Building constant resolver from constants vector");

        let mut fully_qualified_constant_to_constant_map: HashMap<
            String,
            Vec<ConstantDefinition>,
        > = HashMap::new();

        for constant in constants {
            if let Some(definition_location) =
                ignored_definitions.get(&constant.fully_qualified_name)
            {
                let relative_path = constant
                    .absolute_path_of_definition
                    .strip_prefix(absolute_root);

                // The constant could be defined outside of the root in certain contexts,
                // such as when using `expose-monkey-patches
                if let Ok(relative_path) = relative_path {
                    if definition_location.contains(relative_path) {
                        debug!(
                            "Ignoring definition of {:?} from {:?}",
                            constant.fully_qualified_name,
                            constant.absolute_path_of_definition
                        );
                        continue;
                    }
                }
            }

            let fully_qualified_constant_name =
                constant.fully_qualified_name.clone();

            let existing_definitions = fully_qualified_constant_to_constant_map
                .get(&fully_qualified_constant_name);

            if let Some(existing_definitions) = existing_definitions {
                let mut new_definitions = existing_definitions.clone();
                new_definitions.push(constant);
                fully_qualified_constant_to_constant_map
                    .insert(fully_qualified_constant_name, new_definitions);
            } else {
                fully_qualified_constant_to_constant_map
                    .insert(fully_qualified_constant_name, vec![constant]);
            }
        }

        debug!("Finished building constant resolver");

        Box::new(ExperimentalConstantResolver {
            fully_qualified_constant_name_to_constant_definition_map:
                fully_qualified_constant_to_constant_map,
        })
    }

    // In Ruby, say we have this code:
    //
    // module Foo
    //   module Bar
    //     module Baz
    //       Boo
    //     end
    //   end
    // end
    //
    // The `current_namespace_path` here is: ['Foo', 'Bar', 'Baz']
    // The `const_name` here is: `Boo`
    // Ruby constant resolution rules dictate that `Boo` could refer to any of the following,
    // in this specific order:
    //
    // ::Foo::Bar::Baz::Boo
    // ::Foo::Bar::Boo
    // ::Foo::Boo
    // ::Boo
    //
    // A namespace written from the top level starts the name again, so in
    // `module Foo; class ::Bar`, the path is ['Foo', '::Bar'], and `Boo` could
    // be ::Bar::Boo, ::Foo::Boo or ::Boo, the order of `Module.nesting`.
    //
    // We need to check each of these possibilities in order, and return the first one that exists
    // If none of them exist, return no definitions
    fn resolve_traversing_namespace_path<'a>(
        &'a self,
        const_name: &'a str,
        current_namespace_path: &'a [&str],
    ) -> Vec<ConstantDefinition> {
        let fully_qualified_name_guess =
            nest_constant_name(current_namespace_path, const_name);

        let definitions =
            self.constant_for_fully_qualified_name(&fully_qualified_name_guess);

        if !definitions.is_empty() {
            definitions
        } else {
            // In this case, we couldn't find a constant with the given name under the given namespace.
            // However, it's possible the constant is defined within the parent namespace.
            let split_result = current_namespace_path.split_last();
            match split_result {
                Some((_last, parent_namespace)) => self
                    .resolve_traversing_namespace_path(
                        const_name,
                        parent_namespace,
                    ),
                None => vec![],
            }
        }
    }

    fn constant_for_fully_qualified_name(
        &self,
        fully_qualified_name: &String,
    ) -> Vec<ConstantDefinition> {
        let ret = self
            .fully_qualified_constant_name_to_constant_definition_map
            .get(fully_qualified_name);

        match ret {
            Some(constant_definitions) => constant_definitions.to_owned(),
            None => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolved_names(
        constants: &[&str],
        name: &str,
        namespace_path: &[&str],
    ) -> Vec<String> {
        let constants = constants
            .iter()
            .map(|constant| ConstantDefinition {
                fully_qualified_name: (*constant).to_owned(),
                absolute_path_of_definition: PathBuf::from("/app/foo.rb"),
            })
            .collect();
        ExperimentalConstantResolver::create(
            constants,
            Path::new("/app"),
            &HashMap::new(),
        )
        .resolve(name, namespace_path)
        .unwrap_or_default()
        .into_iter()
        .map(|definition| definition.fully_qualified_name)
        .collect()
    }

    #[test]
    fn resolve_in_a_namespace_from_the_top_level() {
        // In `module Outer; class ::Foo; end; end`, as Ruby looks them up
        for (name, resolved) in [("Bar", "::Foo::Bar"), ("Baz", "::Outer::Baz")]
        {
            assert_eq!(
                resolved_names(
                    &["::Foo::Bar", "::Outer::Baz"],
                    name,
                    &["Outer", "::Foo"]
                ),
                [resolved],
                "{}",
                name
            );
        }
    }

    #[test]
    fn resolve_through_object() {
        for (constants, name, resolved) in [
            (&["::Bar"][..], "Object::Bar", "::Bar"),
            (&["::Bar"], "::Object::Bar", "::Bar"),
            (&["::Bar"], "::Object::Object::Bar", "::Bar"),
            (
                &["::ObjectSpace::Bar"],
                "::ObjectSpace::Bar",
                "::ObjectSpace::Bar",
            ),
            // `Object` is looked up as Ruby looks up any constant
            (
                &["::Bar", "::CoreExt::Object::Bar"],
                "Object::Bar",
                "::CoreExt::Object::Bar",
            ),
            (
                &["::Bar", "::CoreExt::Object::Bar"],
                "::Object::Bar",
                "::Bar",
            ),
        ] {
            assert_eq!(
                resolved_names(constants, name, &["CoreExt"]),
                [resolved],
                "{} among {:?}",
                name,
                constants
            );
        }
    }
}
