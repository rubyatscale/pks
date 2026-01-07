#[cfg(test)]
pub mod tests {
    use pretty_assertions::assert_eq;
    use std::collections::{HashMap, HashSet};

    use crate::packs::{
        checker::{
            reference::Reference, CheckerInterface, ViolationIdentifier,
        },
        checker_configuration::CheckerType,
        pack::Pack,
        Configuration, PackSet, SourceLocation, Violation,
    };

    pub struct TestChecker {
        /// None means use the test default
        pub defining_pack: Option<Pack>,
        pub referencing_pack: Pack,
        /// None means no violation expected
        pub expected_violation: Option<Violation>,
        /// None means use the test default
        pub reference: Option<Reference>,
        /// None means use the test default
        pub referenced_constant_name: Option<String>,
        /// None means use the test default
        pub configuration: Option<Configuration>,
    }

    pub fn build_expected_violation(
        violation_type: CheckerType,
        strict: bool,
    ) -> Violation {
        build_expected_violation_with_constant(
            violation_type,
            strict,
            String::from("::Bar"),
        )
    }

    pub fn build_expected_violation_with_layers(
        violation_type: CheckerType,
        strict: bool,
        defining_layer: &str,
        referencing_layer: &str,
    ) -> Violation {
        Violation {
            identifier: ViolationIdentifier {
                violation_type,
                strict,
                file: String::from("packs/foo/app/services/foo.rb"),
                constant_name: String::from("::Bar"),
                referencing_pack_name: String::from("packs/foo"),
                defining_pack_name: String::from("packs/bar"),
            },
            source_location: SourceLocation { line: 3, column: 1 },
            referencing_pack_relative_yml: String::from(
                "packs/foo/package.yml",
            ),
            defining_layer: Some(defining_layer.to_string()),
            referencing_layer: Some(referencing_layer.to_string()),
        }
    }

    pub fn build_expected_violation_with_constant(
        violation_type: CheckerType,
        strict: bool,
        constant_name: String,
    ) -> Violation {
        Violation {
            identifier: ViolationIdentifier {
                violation_type,
                strict,
                file: String::from("packs/foo/app/services/foo.rb"),
                constant_name,
                referencing_pack_name: String::from("packs/foo"),
                defining_pack_name: String::from("packs/bar"),
            },
            source_location: SourceLocation { line: 3, column: 1 },
            referencing_pack_relative_yml: String::from(
                "packs/foo/package.yml",
            ),
            defining_layer: None,
            referencing_layer: None,
        }
    }

    impl Default for TestChecker {
        fn default() -> Self {
            TestChecker {
                defining_pack: Some(default_defining_pack()),
                referencing_pack: default_referencing_pack(),
                expected_violation: None,
                reference: None,
                configuration: None,
                referenced_constant_name: None,
            }
        }
    }

    pub fn default_defining_pack() -> Pack {
        Pack {
            name: "packs/bar".to_owned(),
            ..Pack::default()
        }
    }

    pub fn default_referencing_pack() -> Pack {
        use std::path::PathBuf;
        Pack {
            name: "packs/foo".to_owned(),
            relative_path: PathBuf::from("packs/foo"),
            ..Pack::default()
        }
    }

    pub fn test_check(
        checker: &impl CheckerInterface,
        test_checker: &mut TestChecker,
    ) -> anyhow::Result<()> {
        let constant_name = match test_checker.referenced_constant_name.take() {
            Some(name) => name.clone(),
            None => String::from("::TheConstant"),
        };
        let defing_pack_name = test_checker
            .defining_pack
            .as_ref()
            .map(|pack| pack.name.clone());
        let reference = test_checker.reference.take();
        let reference = reference.unwrap_or_else(|| Reference {
            constant_name: constant_name.clone(),
            defining_pack_name: defing_pack_name,
            referencing_pack_name: test_checker
                .referencing_pack
                .name
                .to_owned(),
            relative_referencing_file: String::from(
                "packs/foo/app/services/foo.rb",
            ),
            relative_defining_file: Some(String::from(
                "packs/bar/app/services/public/bar.rb",
            )),
            source_location: SourceLocation { line: 3, column: 1 },
        });

        let root_pack = Pack {
            name: String::from("."),
            ..Pack::default()
        };

        let mut packs = vec![root_pack, test_checker.referencing_pack.clone()];
        if let Some(pack) = test_checker.defining_pack.clone() {
            packs.push(pack);
        }

        let configuration =
            test_checker.configuration.take().unwrap_or_else(|| {
                Configuration {
                    pack_set: PackSet::build(
                        HashSet::from_iter(packs),
                        HashMap::new(),
                    )
                    .unwrap(),
                    ..Configuration::default()
                }
            });

        let result = checker.check(&reference, &configuration)?;

        assert_eq!(result, test_checker.expected_violation);

        Ok(())
    }
}
