pub(crate) mod parser;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::packs::parsing::ruby::packwerk::parser::process_from_contents;
    use crate::packs::parsing::Range;
    use crate::packs::{Configuration, UnresolvedReference};

    #[test]
    fn trivial_case() {
        let contents: String = String::from("Foo");
        let configuration = Configuration::default();
        assert_eq!(
            vec![UnresolvedReference {
                name: String::from("Foo"),
                namespace_path: vec![],
                location: Range {
                    start_row: 1,
                    start_col: 0,
                    end_row: 1,
                    end_col: 4
                }
            }],
            process_from_contents(
                contents,
                &PathBuf::from("path/to/file.rb"),
                &configuration
            )
            .unresolved_references
        );
    }

    #[test]
    fn nested_constant() {
        let contents: String = String::from("Foo::Bar");
        let configuration = Configuration::default();
        assert_eq!(
            vec![UnresolvedReference {
                name: String::from("Foo::Bar"),
                namespace_path: vec![],
                location: Range {
                    start_row: 1,
                    start_col: 0,
                    end_row: 1,
                    end_col: 9
                }
            }],
            process_from_contents(
                contents,
                &PathBuf::from("path/to/file.rb"),
                &configuration
            )
            .unresolved_references
        );
    }

    #[test]
    fn deeply_nested_constant() {
        let contents: String = String::from("Foo::Bar::Baz");
        let configuration = Configuration::default();
        assert_eq!(
            vec![UnresolvedReference {
                name: String::from("Foo::Bar::Baz"),
                namespace_path: vec![],
                location: Range {
                    start_row: 1,
                    start_col: 0,
                    end_row: 1,
                    end_col: 14
                }
            }],
            process_from_contents(
                contents,
                &PathBuf::from("path/to/file.rb"),
                &configuration
            )
            .unresolved_references
        );
    }

    #[test]
    fn very_deeply_nested_constant() {
        let contents: String = String::from("Foo::Bar::Baz::Boo");
        let configuration = Configuration::default();
        assert_eq!(
            vec![UnresolvedReference {
                name: String::from("Foo::Bar::Baz::Boo"),
                namespace_path: vec![],
                location: Range {
                    start_row: 1,
                    start_col: 0,
                    end_row: 1,
                    end_col: 19
                }
            }],
            process_from_contents(
                contents,
                &PathBuf::from("path/to/file.rb"),
                &configuration
            )
            .unresolved_references
        );
    }

    #[test]
    fn class_definition() {
        let contents: String = String::from(
            "\
class Foo
end
            ",
        );

        let configuration = Configuration::default();
        assert_eq!(
            vec![UnresolvedReference {
                name: String::from("::Foo"),
                namespace_path: vec![],
                location: Range {
                    start_row: 1,
                    start_col: 6,
                    end_row: 1,
                    end_col: 10
                }
            }],
            process_from_contents(
                contents,
                &PathBuf::from("path/to/file.rb"),
                &configuration
            )
            .unresolved_references
        );
    }

    #[test]
    fn class_namespaced_constant() {
        let contents: String = String::from(
            "\
class Foo
  Bar
end
        ",
        );

        let configuration = Configuration::default();
        assert_eq!(
            UnresolvedReference {
                name: String::from("Bar"),
                namespace_path: vec![String::from("Foo")],
                location: Range {
                    start_row: 2,
                    start_col: 2,
                    end_row: 2,
                    end_col: 6
                }
            },
            *process_from_contents(
                contents,
                &PathBuf::from("path/to/file.rb"),
                &configuration
            )
            .unresolved_references
            .get(1)
            .unwrap()
        );
    }

    #[test]
    fn deeply_class_namespaced_constant() {
        let contents: String = String::from(
            "\
class Foo
  class Bar
    Baz
  end
end
        ",
        );

        let configuration = Configuration::default();
        assert_eq!(
            UnresolvedReference {
                name: String::from("Baz"),
                namespace_path: vec![String::from("Foo"), String::from("Bar")],
                location: Range {
                    start_row: 3,
                    start_col: 4,
                    end_row: 3,
                    end_col: 8
                }
            },
            *process_from_contents(
                contents,
                &PathBuf::from("path/to/file.rb"),
                &configuration
            )
            .unresolved_references
            .get(2)
            .unwrap()
        );
    }

    #[test]
    fn very_deeply_class_namespaced_constant() {
        let contents: String = String::from(
            "\
class Foo
  class Bar
    class Baz
      Boo
    end
  end
end
        ",
        );

        let configuration = Configuration::default();
        assert_eq!(
            UnresolvedReference {
                name: String::from("Boo"),
                namespace_path: vec![
                    String::from("Foo"),
                    String::from("Bar"),
                    String::from("Baz")
                ],
                location: Range {
                    start_row: 4,
                    start_col: 6,
                    end_row: 4,
                    end_col: 10
                }
            },
            *process_from_contents(
                contents,
                &PathBuf::from("path/to/file.rb"),
                &configuration
            )
            .unresolved_references
            .get(3)
            .unwrap()
        );
    }

    #[test]
    fn module_namespaced_constant() {
        let contents: String = String::from(
            "\
module Foo
  Bar
end
        ",
        );

        let configuration = Configuration::default();
        assert_eq!(
            vec![
                UnresolvedReference {
                    name: String::from("::Foo"),
                    namespace_path: vec![],
                    location: Range {
                        start_row: 1,
                        start_col: 7,
                        end_row: 1,
                        end_col: 11
                    }
                },
                UnresolvedReference {
                    name: String::from("Bar"),
                    namespace_path: vec![String::from("Foo")],
                    location: Range {
                        start_row: 2,
                        start_col: 2,
                        end_row: 2,
                        end_col: 6
                    }
                }
            ],
            process_from_contents(
                contents,
                &PathBuf::from("path/to/file.rb"),
                &configuration
            )
            .unresolved_references,
        );
    }

    #[test]
    fn deeply_module_namespaced_constant() {
        let contents: String = String::from(
            "\
module Foo
  module Bar
    Baz
  end
end
        ",
        );

        let configuration = Configuration::default();
        assert_eq!(
            UnresolvedReference {
                name: String::from("Baz"),
                namespace_path: vec![String::from("Foo"), String::from("Bar")],
                location: Range {
                    start_row: 3,
                    start_col: 4,
                    end_row: 3,
                    end_col: 8
                }
            },
            *process_from_contents(
                contents,
                &PathBuf::from("path/to/file.rb"),
                &configuration
            )
            .unresolved_references
            .get(2)
            .unwrap()
        );
    }

    #[test]
    fn very_deeply_module_namespaced_constant() {
        let contents: String = String::from(
            "\
module Foo
  module Bar
    module Baz
      Boo
    end
  end
end
        ",
        );

        let configuration = Configuration::default();
        assert_eq!(
            UnresolvedReference {
                name: String::from("Boo"),
                namespace_path: vec![
                    String::from("Foo"),
                    String::from("Bar"),
                    String::from("Baz")
                ],
                location: Range {
                    start_row: 4,
                    start_col: 6,
                    end_row: 4,
                    end_col: 10
                }
            },
            *process_from_contents(
                contents,
                &PathBuf::from("path/to/file.rb"),
                &configuration
            )
            .unresolved_references
            .get(3)
            .unwrap()
        );
    }

    #[test]
    fn mixed_namespaced_constant() {
        let contents: String = String::from(
            "\
class Foo
  module Bar
    class Baz
      Boo
    end
  end
end
        ",
        );

        let configuration = Configuration::default();
        assert_eq!(
            UnresolvedReference {
                name: String::from("Boo"),
                namespace_path: vec![
                    String::from("Foo"),
                    String::from("Bar"),
                    String::from("Baz")
                ],
                location: Range {
                    start_row: 4,
                    start_col: 6,
                    end_row: 4,
                    end_col: 10
                },
            },
            *process_from_contents(
                contents,
                &PathBuf::from("path/to/file.rb"),
                &configuration
            )
            .unresolved_references
            .get(3)
            .unwrap()
        );
    }

    #[test]
    // https://www.rubydoc.info/gems/rubocop/RuboCop/Cop/Style/ClassAndModuleChildren
    fn compact_style_class_definition_constant() {
        let contents: String = String::from(
            "\
class Foo::Bar
  Baz
end
        ",
        );

        let configuration = Configuration::default();
        assert_eq!(
            UnresolvedReference {
                name: String::from("Baz"),
                namespace_path: vec![String::from("Foo::Bar")],
                location: Range {
                    start_row: 2,
                    start_col: 2,
                    end_row: 2,
                    end_col: 6
                }
            },
            *process_from_contents(
                contents,
                &PathBuf::from("path/to/file.rb"),
                &configuration
            )
            .unresolved_references
            .get(1)
            .unwrap(),
        );
    }

    #[test]
    // https://www.rubydoc.info/gems/rubocop/RuboCop/Cop/Style/ClassAndModuleChildren
    fn compact_style_with_module_constant() {
        let contents: String = String::from(
            "\
class Foo::Bar
  module Baz
  end
end
        ",
        );

        let configuration = Configuration::default();
        assert_eq!(
            UnresolvedReference {
                name: String::from("::Foo::Bar::Baz"),
                namespace_path: vec![String::from("Foo::Bar"),],
                location: Range {
                    start_row: 2,
                    start_col: 9,
                    end_row: 2,
                    end_col: 13
                }
            },
            *process_from_contents(
                contents,
                &PathBuf::from("path/to/file.rb"),
                &configuration
            )
            .unresolved_references
            .get(1)
            .unwrap()
        );
    }

    #[test]
    // https://www.rubydoc.info/gems/rubocop/RuboCop/Cop/Style/ClassAndModuleChildren
    fn array_of_constant() {
        let contents: String = String::from("[Foo]");
        let configuration = Configuration::default();

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;

        assert_eq!(references.len(), 1);
        let reference = references
            .first()
            .expect("There should be a reference at index 0");

        assert_eq!(
            UnresolvedReference {
                name: String::from("Foo"),
                namespace_path: vec![],
                location: Range {
                    start_row: 1,
                    start_col: 1,
                    end_row: 1,
                    end_col: 5
                }
            },
            *reference
        );
    }
    #[test]
    // https://www.rubydoc.info/gems/rubocop/RuboCop/Cop/Style/ClassAndModuleChildren
    fn array_of_multiple_constants() {
        let contents: String = String::from("[Foo, Bar]");
        let configuration = Configuration::default();

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;
        assert_eq!(references.len(), 2);
        let reference1 = references
            .first()
            .expect("There should be a reference at index 0");

        assert_eq!(
            UnresolvedReference {
                name: String::from("Foo"),
                namespace_path: vec![],
                location: Range {
                    start_row: 1,
                    start_col: 1,
                    end_row: 1,
                    end_col: 5
                }
            },
            *reference1
        );
        let reference2 = references
            .get(1)
            .expect("There should be a reference at index 1");

        assert_eq!(
            UnresolvedReference {
                name: String::from("Bar"),
                namespace_path: vec![],
                location: Range {
                    start_row: 1,
                    start_col: 6,
                    end_row: 1,
                    end_col: 10
                }
            },
            *reference2,
        );
    }

    #[test]
    // https://www.rubydoc.info/gems/rubocop/RuboCop/Cop/Style/ClassAndModuleChildren
    fn array_of_nested_constant() {
        let contents: String = String::from("[Baz::Boo]");
        let configuration = Configuration::default();

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;
        assert_eq!(references.len(), 1);
        let reference = references
            .first()
            .expect("There should be a reference at index 0");

        assert_eq!(
            UnresolvedReference {
                name: String::from("Baz::Boo"),
                namespace_path: vec![],
                location: Range {
                    start_row: 1,
                    start_col: 1,
                    end_row: 1,
                    end_col: 10
                }
            },
            *reference,
        );
    }

    #[test]
    // https://www.rubydoc.info/gems/rubocop/RuboCop/Cop/Style/ClassAndModuleChildren
    fn globally_referenced_constant() {
        let contents: String = String::from("::Foo");
        let configuration = Configuration::default();

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;
        assert_eq!(references.len(), 1);
        let reference = references
            .first()
            .expect("There should be a reference at index 0");

        assert_eq!(
            UnresolvedReference {
                name: String::from("::Foo"),
                namespace_path: vec![],
                location: Range {
                    start_row: 1,
                    start_col: 0,
                    end_row: 1,
                    end_col: 6
                }
            },
            *reference,
        );
    }

    #[test]
    // https://www.rubydoc.info/gems/rubocop/RuboCop/Cop/Style/ClassAndModuleChildren
    fn metaprogrammatically_referenced_constant() {
        let contents: String = String::from("described_class::Foo");
        let configuration = Configuration::default();

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;

        assert_eq!(references.len(), 0);
    }

    #[test]
    fn ignore_local_constant() {
        let contents: String = String::from(
            "\
class Foo
  BAR = 1
  def use_bar
    puts BAR
  end
end
        ",
        );

        let configuration = Configuration::default();
        assert_eq!(
            process_from_contents(
                contents,
                &PathBuf::from("path/to/file.rb"),
                &configuration
            )
            .unresolved_references,
            vec![UnresolvedReference {
                name: String::from("::Foo"),
                namespace_path: vec![],
                location: Range {
                    start_row: 1,
                    start_col: 6,
                    end_row: 1,
                    end_col: 10
                }
            }]
        )
    }

    #[test]
    fn ignore_local_constant_under_nested_module() {
        let contents: String = String::from(
            "\
class Foo
  class Baz
    BAR = 1
    def use_bar
      puts BAR
    end
  end
end
        ",
        );

        let configuration = Configuration::default();
        assert_eq!(
            process_from_contents(
                contents,
                &PathBuf::from("path/to/file.rb"),
                &configuration
            )
            .unresolved_references,
            vec![
                UnresolvedReference {
                    name: String::from("::Foo"),
                    namespace_path: vec![],
                    location: Range {
                        start_row: 1,
                        start_col: 6,
                        end_row: 1,
                        end_col: 10
                    }
                },
                UnresolvedReference {
                    name: String::from("::Foo::Baz"),
                    namespace_path: vec![String::from("Foo"),],
                    location: Range {
                        start_row: 2,
                        start_col: 8,
                        end_row: 2,
                        end_col: 12
                    }
                }
            ]
        );
    }

    #[test]
    fn super_classes_are_references() {
        let contents: String = String::from(
            "\
class Foo < Bar
end
        ",
        );

        let configuration = Configuration::default();

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;
        assert_eq!(references.len(), 2);
        let first_reference = references
            .first()
            .expect("There should be a reference at index 0");
        assert_eq!(
            UnresolvedReference {
                name: String::from("Bar"),
                namespace_path: vec![],
                location: Range {
                    start_row: 1,
                    start_col: 12,
                    end_row: 1,
                    end_col: 16
                }
            },
            *first_reference,
        );
    }

    #[test]
    fn compact_nested_classes_are_references() {
        let contents: String = String::from(
            "\
class Foo::Bar
end
        ",
        );
        let configuration = Configuration::default();

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;
        assert_eq!(references.len(), 1);
        let first_reference = references
            .first()
            .expect("There should be a reference at index 0");
        assert_eq!(
            UnresolvedReference {
                name: String::from("::Foo::Bar"),
                namespace_path: vec![],
                location: Range {
                    start_row: 1,
                    start_col: 6,
                    end_row: 1,
                    end_col: 15
                }
            },
            *first_reference,
        );
    }

    #[test]
    fn regular_nested_classes_are_references() {
        let contents: String = String::from(
            "\
class Foo
  class Bar
  end
end
        ",
        );

        let configuration = Configuration::default();

        let references: Vec<UnresolvedReference> = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;
        assert_eq!(
            references,
            vec![
                UnresolvedReference {
                    name: String::from("::Foo"),
                    namespace_path: vec![],
                    location: Range {
                        start_row: 1,
                        start_col: 6,
                        end_row: 1,
                        end_col: 10
                    }
                },
                UnresolvedReference {
                    name: String::from("::Foo::Bar"),
                    namespace_path: vec![String::from("Foo"),],
                    location: Range {
                        start_row: 2,
                        start_col: 8,
                        end_row: 2,
                        end_col: 12
                    }
                }
            ]
        );
    }
    #[test]
    fn const_assignments_are_references() {
        let contents: String = String::from(
            "\
FOO = BAR
",
        );
        let configuration = Configuration::default();

        let references: Vec<UnresolvedReference> = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;

        assert_eq!(references.len(), 1);
        let first_reference = references
            .first()
            .expect("There should be a reference at index 0");

        assert_eq!(
            UnresolvedReference {
                name: String::from("BAR"),
                namespace_path: vec![],
                location: Range {
                    start_row: 1,
                    start_col: 6,
                    end_row: 1,
                    end_col: 10
                }
            },
            *first_reference
        )
    }

    #[test]
    fn constant_assignments_define_local_constants() {
        // Only the value assigned is a reference, not a target or its scope,
        // nor a later use of a target, which is defined here
        for contents in [
            "A = Bar\nA",
            "A::B = Bar\nA::B",
            "A ||= Bar\nA",
            "A &&= Bar\nA",
            "A += Bar\nA",
            "A::B ||= Bar\nA::B",
            "A::B &&= Bar\nA::B",
            "A::B += Bar\nA::B",
            "A, B::C = Bar\nA\nB::C",
            "begin\nrescue Bar => A\nend\nA",
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
        }
    }

    #[test]
    fn class_or_module_named_by_an_expression() {
        // pks skips the class or module, but not what follows it
        for keyword in ["class", "module"] {
            let contents = format!(
                "module Outer\n  {} self::Foo\n    Bar\n  end\n  Baz\nend\n",
                keyword
            );
            let references: Vec<(String, Vec<String>)> = process_from_contents(
                contents,
                &PathBuf::from("path/to/file.rb"),
                &Configuration::default(),
            )
            .unresolved_references
            .into_iter()
            .map(|reference| (reference.name, reference.namespace_path))
            .collect();
            assert_eq!(
                references,
                vec![
                    (String::from("::Outer"), vec![]),
                    (String::from("Baz"), vec![String::from("Outer")]),
                ],
                "{}",
                keyword
            );
        }
    }

    #[test]
    fn class_defined_in_class_eval() {
        // packwerk names the class as one in `Foo`, but looks up the constants
        // in it, and names those defined around it, as if it were in `M`
        let contents = "\
module M
  Foo.class_eval do
    class Bar
      Baz
    end
    QUX = 1
    QUX
  end
end
";
        let references: Vec<(String, Vec<String>)> = process_from_contents(
            contents.to_owned(),
            &PathBuf::from("path/to/file.rb"),
            &Configuration::default(),
        )
        .unresolved_references
        .into_iter()
        .map(|reference| (reference.name, reference.namespace_path))
        .collect();
        assert_eq!(
            references,
            vec![
                (String::from("::M"), vec![]),
                (String::from("Foo"), vec![String::from("M")]),
                (String::from("::M::Foo::Bar"), vec![String::from("M")]),
                (
                    String::from("Baz"),
                    vec![String::from("M"), String::from("Bar")]
                ),
            ]
        );
    }

    #[test]
    fn classes_defined_in_blocks_and_assignments() {
        // The name packwerk gives `Bar`, from the `class_eval` blocks and
        // `Class.new` or `Module.new` assignments around it. It names none in
        // a block passed with `&.` or to `super`.
        for (contents, bar) in [
            ("Foo.class_eval do class Bar; end end", Some("::Foo::Bar")),
            ("Foo.class_eval { module Bar; end }", Some("::Foo::Bar")),
            (
                "Foo.class_eval(1) do |x| class Bar; end end",
                Some("::Foo::Bar"),
            ),
            ("A::B.class_eval do class Bar; end end", Some("::A::B::Bar")),
            (
                "::Foo.class_eval do class Bar; end end",
                Some("::::Foo::Bar"),
            ),
            (
                "module M; self::Foo.class_eval do class Bar; end end; end",
                Some("::M::::Foo::Bar"),
            ),
            (
                "module M; Foo.class_eval do class ::Bar; end end; end",
                Some("::M::Foo::::Bar"),
            ),
            (
                "A.class_eval do B.class_eval do class Bar; end end end",
                Some("::A::B::Bar"),
            ),
            ("Foo.class_eval(class Bar; end) do end", Some("::Foo::Bar")),
            ("Foo = Class.new do class Bar; end end", Some("::Foo::Bar")),
            (
                "A::B = Module.new do module Bar; end end",
                Some("::A::B::Bar"),
            ),
            (
                "Foo = Class.new(Base) { class Bar; end }",
                Some("::Foo::Bar"),
            ),
            ("Foo = Class.new(class Bar; self; end)", Some("::Foo::Bar")),
            (
                "Foo = Class.new do A.class_eval do class Bar; end end end",
                Some("::Foo::A::Bar"),
            ),
            (
                "Foo.class_eval do end.then do class Bar; end end",
                Some("::Bar"),
            ),
            ("Foo.module_eval do class Bar; end end", Some("::Bar")),
            ("Foo.class_exec do class Bar; end end", Some("::Bar")),
            ("foo.class_eval do class Bar; end end", Some("::Bar")),
            ("self.class_eval do class Bar; end end", Some("::Bar")),
            ("class_eval do class Bar; end end", Some("::Bar")),
            ("Foo.class_eval { _1; class Bar; end }", Some("::Bar")),
            ("Foo.class_eval { it; class Bar; end }", Some("::Bar")),
            ("Foo&.class_eval { _1; class Bar; end }", Some("::Bar")),
            ("Foo = Class.new { _1; class Bar; end }", Some("::Bar")),
            ("Foo = Class&.new(class Bar; self; end)", Some("::Bar")),
            ("Foo = ::Class.new do class Bar; end end", Some("::Bar")),
            ("Foo = Struct.new(:a) do class Bar; end end", Some("::Bar")),
            ("Foo = (Class.new do class Bar; end end)", Some("::Bar")),
            ("Foo ||= Class.new do class Bar; end end", Some("::Bar")),
            ("Foo, Baz = Class.new do class Bar; end end", Some("::Bar")),
            ("Foo&.class_eval do class Bar; end end", None),
            ("Foo&.then do class Bar; end end", None),
            ("foo::Baz.class_eval do class Bar; end end", None),
            ("Foo = Class&.new do class Bar; end end", None),
            ("super do class Bar; end end", None),
            ("super(1) do class Bar; end end", None),
        ] {
            let references: Vec<String> = process_from_contents(
                contents.to_owned(),
                &PathBuf::from("path/to/file.rb"),
                &Configuration::default(),
            )
            .unresolved_references
            .into_iter()
            .map(|reference| reference.name)
            .filter(|name| name.ends_with("Bar"))
            .collect();
            assert_eq!(
                references,
                bar.into_iter().collect::<Vec<_>>(),
                "{}",
                contents
            );
        }
    }

    #[test]
    fn association_calls() {
        // Where each finds `Bar`. pks has never read a `&.` call as an
        // association, and a block doesn't move where one starts.
        for (body, bar) in [
            ("self.has_many :bars", Some((2, 2))),
            ("self&.has_many :bars", None),
            ("has_many :bars do\n    Baz\n  end", Some((2, 2))),
        ] {
            let contents = format!("class Foo\n  {}\nend\n", body);
            let references = process_from_contents(
                contents,
                &PathBuf::from("path/to/file.rb"),
                &Configuration::default(),
            )
            .unresolved_references;
            let found = references
                .iter()
                .find(|reference| reference.name == "Bar")
                .map(|reference| {
                    (reference.location.start_row, reference.location.start_col)
                });
            assert_eq!(found, bar, "{}", body);
        }
    }

    #[test]
    fn has_one_association() {
        let contents: String = String::from(
            "\
class Foo
  has_one :some_user_model
end
        ",
        );

        let configuration = Configuration::default();

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;

        assert_eq!(references.len(), 2);
        let first_reference = references
            .get(1)
            .expect("There should be a reference at index 0");

        assert_eq!(
            UnresolvedReference {
                name: String::from("SomeUserModel"),
                namespace_path: vec![String::from("Foo")],
                location: Range {
                    start_row: 2,
                    start_col: 2,
                    end_row: 2,
                    end_col: 27
                }
            },
            *first_reference,
        );
    }

    #[test]
    fn has_one_association_with_class_name() {
        let contents: String = String::from(
            "\
class Foo
  has_one :some_user_model, class_name: 'User'
end
        ",
        );

        let configuration = Configuration::default();

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;

        assert_eq!(references.len(), 2);
        let first_reference = references
            .get(1)
            .expect("There should be a reference at index 0");

        assert_eq!(
            UnresolvedReference {
                name: String::from("User"),
                namespace_path: vec![String::from("Foo")],
                location: Range {
                    start_row: 2,
                    start_col: 2,
                    end_row: 2,
                    end_col: 47
                }
            },
            *first_reference,
        );
    }

    #[test]
    fn has_many_association() {
        let contents: String = String::from(
            "\
class Foo
  has_many :some_user_models
end
        ",
        );
        let configuration = Configuration::default();

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;

        assert_eq!(references.len(), 2);
        let first_reference = references
            .get(1)
            .expect("There should be a reference at index 0");

        assert_eq!(
            UnresolvedReference {
                name: String::from("SomeUserModel"),
                namespace_path: vec![String::from("Foo")],
                location: Range {
                    start_row: 2,
                    start_col: 2,
                    end_row: 2,
                    end_col: 29
                }
            },
            *first_reference,
        );
    }

    #[test]
    fn has_many_association_with_custom_inflection() {
        let contents: String = String::from(
            "\
class Foo
  has_many :my_statuses
end
        ",
        );

        let configuration = Configuration::default();

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;

        assert_eq!(references.len(), 2);
        let first_reference = references
            .get(1)
            .expect("There should be a reference at index 0");

        assert_eq!(
            UnresolvedReference {
                name: String::from("MyStatus"),
                namespace_path: vec![String::from("Foo")],
                location: Range {
                    start_row: 2,
                    start_col: 2,
                    end_row: 2,
                    end_col: 24
                }
            },
            *first_reference,
        );
    }

    #[test]
    fn belongs_to_association_with_custom_inflection() {
        let contents: String = String::from(
            "\
class Foo
  belongs_to :status
end
        ",
        );
        let configuration = Configuration::default();

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;
        assert_eq!(references.len(), 2);
        let first_reference = references
            .get(1)
            .expect("There should be a reference at index 0");
        assert_eq!(
            UnresolvedReference {
                name: String::from("Status"),
                namespace_path: vec![String::from("Foo")],
                location: Range {
                    start_row: 2,
                    start_col: 2,
                    end_row: 2,
                    end_col: 21
                }
            },
            *first_reference,
        );
    }

    #[test]
    fn has_many_association_with_custom_inflection_2() {
        let contents: String = String::from(
            "\
class Foo
  has_many :my_leaves
end
        ",
        );

        let configuration = Configuration::default();

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;
        assert_eq!(references.len(), 2);
        let first_reference = references
            .get(1)
            .expect("There should be a reference at index 0");
        assert_eq!(
            UnresolvedReference {
                name: String::from("MyLeave"),
                namespace_path: vec![String::from("Foo")],
                location: Range {
                    start_row: 2,
                    start_col: 2,
                    end_row: 2,
                    end_col: 22
                }
            },
            *first_reference,
        );
    }

    #[test]
    fn association_with_custom_inflection_3() {
        let contents: String = String::from(
            "\
class Foo
  has_many :data
end
        ",
        );
        let configuration = Configuration::default();

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;
        assert_eq!(references.len(), 2);
        let first_reference = references
            .get(1)
            .expect("There should be a reference at index 0");
        assert_eq!(
            UnresolvedReference {
                name: String::from("Datum"),
                namespace_path: vec![String::from("Foo")],
                location: Range {
                    start_row: 2,
                    start_col: 2,
                    end_row: 2,
                    end_col: 17
                }
            },
            *first_reference,
        );
    }

    #[test]
    fn association_with_user_inputted_custom_inflection_3() {
        let contents: String = String::from(
            "\
class Foo
  cache_belongs_to :data
end
        ",
        );
        let configuration = Configuration {
            custom_associations: vec!["cache_belongs_to".to_owned()],
            ..Configuration::default()
        };

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;
        assert_eq!(references.len(), 2);
        let first_reference = references
            .get(1)
            .expect("There should be a reference at index 0");
        assert_eq!(
            UnresolvedReference {
                name: String::from("Datum"),
                namespace_path: vec![String::from("Foo")],
                location: Range {
                    start_row: 2,
                    start_col: 2,
                    end_row: 2,
                    end_col: 25
                }
            },
            *first_reference,
        );
    }

    #[test]
    fn has_many_association_with_class_name_after_block() {
        let contents: String = String::from(
            "\
class Foo
  has_one :bar, -> { my_scope }, as: :owner, class_name: 'SpecialClass'
end
        ",
        );
        let configuration = Configuration::default();

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;
        assert_eq!(references.len(), 2);
        let first_reference = references
            .get(1)
            .expect("There should be a reference at index 0");
        assert_eq!(
            UnresolvedReference {
                name: String::from("SpecialClass"),
                namespace_path: vec![String::from("Foo")],
                location: Range {
                    start_row: 2,
                    start_col: 2,
                    end_row: 2,
                    end_col: 72
                }
            },
            *first_reference,
        );
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

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;
        assert_eq!(references.len(), 1);
        let reference = references
            .first()
            .expect("There should be a reference at index 0");
        assert_eq!(reference.name, String::from("::Foo"));
    }

    #[test]
    fn polymorphic_belongs_to_association_with_class_name() {
        let contents: String = String::from(
            "\
class Foo
  belongs_to :event, class_name: 'Event', polymorphic: true
end
        ",
        );
        let configuration = Configuration::default();

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;
        assert_eq!(references.len(), 1);
        let reference = references
            .first()
            .expect("There should be a reference at index 0");
        assert_eq!(reference.name, String::from("::Foo"));
    }

    #[test]
    fn polymorphic_belongs_to_association_with_scope() {
        let contents: String = String::from(
            "\
class Foo
  belongs_to :event, -> { merge(Taco.all) }, polymorphic: true
end
        ",
        );
        let configuration = Configuration::default();

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;
        assert_eq!(references.len(), 2);
        let reference = references
            .get(1)
            .expect("There should be a reference at index 1");
        assert_eq!(
            UnresolvedReference {
                name: String::from("Taco"),
                namespace_path: vec![String::from("Foo")],
                location: Range {
                    start_row: 2,
                    start_col: 32,
                    end_row: 2,
                    end_col: 37
                }
            },
            *reference,
        );
    }

    #[test]
    fn polymorphic_belongs_to_association_in_controller() {
        let contents: String = String::from(
            "\
class CommentsController
  belongs_to :event, polymorphic: true
end
        ",
        );
        let configuration = Configuration::default();

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;
        assert_eq!(references.len(), 2);
        let reference = references
            .get(1)
            .expect("There should be a reference at index 1");
        assert_eq!(
            UnresolvedReference {
                name: String::from("Event"),
                namespace_path: vec![String::from("CommentsController")],
                location: Range {
                    start_row: 2,
                    start_col: 2,
                    end_row: 2,
                    end_col: 39
                }
            },
            *reference,
        );
    }

    #[test]
    fn polymorphic_belongs_to_association_in_class_inside_controller() {
        let contents: String = String::from(
            "\
module Admin
  class CommentsController
    belongs_to :event, polymorphic: true

    class Form
      belongs_to :post, polymorphic: true
    end
  end
end
        ",
        );
        let configuration = Configuration::default();

        let names: Vec<String> = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references
        .into_iter()
        .map(|reference| reference.name)
        .collect();
        assert!(names.contains(&String::from("Event")));
        assert!(!names.contains(&String::from("Post")));
    }

    #[test]
    fn polymorphic_belongs_to_association_outside_any_class() {
        let contents: String = String::from(
            "\
ActiveAdmin.register Comment do
  belongs_to :event, polymorphic: true
end
        ",
        );
        let configuration = Configuration::default();

        let names: Vec<String> = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references
        .into_iter()
        .map(|reference| reference.name)
        .collect();
        assert!(names.contains(&String::from("Event")));
    }

    #[test]
    fn belongs_to_association_with_polymorphic_false() {
        let contents: String = String::from(
            "\
class Foo
  belongs_to :event, polymorphic: false
end
        ",
        );
        let configuration = Configuration::default();

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;
        assert_eq!(references.len(), 2);
        let reference = references
            .get(1)
            .expect("There should be a reference at index 1");
        assert_eq!(
            UnresolvedReference {
                name: String::from("Event"),
                namespace_path: vec![String::from("Foo")],
                location: Range {
                    start_row: 2,
                    start_col: 2,
                    end_row: 2,
                    end_col: 40
                }
            },
            *reference,
        );
    }

    #[test]
    fn it_uses_the_namespace_of_inherited_class_when_referencing_inherited_class(
    ) {
        let contents: String = String::from(
            "\
class Foo < Bar
  Bar
end
        ",
        );

        let configuration = Configuration::default();

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;
        assert_eq!(references.len(), 3);
        let reference = references
            .get(2)
            .expect("There should be a reference at index 0");
        assert_eq!(
            UnresolvedReference {
                name: String::from("Bar"),
                namespace_path: vec![],
                location: Range {
                    start_row: 2,
                    start_col: 2,
                    end_row: 2,
                    end_col: 6
                }
            },
            *reference,
        );
    }

    #[test]
    fn it_ignores_locally_defined_nested_constants() {
        let contents: String = String::from(
            "\
class Foo
  class Bar
    Foo::Bar
  end
end
        ",
        );
        let configuration = Configuration::default();

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;
        assert_eq!(references.len(), 2);
        let first_reference = references
            .first()
            .expect("There should be a reference at index 0");
        let second_reference = references
            .get(1)
            .expect("There should be a reference at index 0");

        assert_eq!(first_reference.name, String::from("::Foo"));
        assert_eq!(second_reference.name, String::from("::Foo::Bar"));
    }

    #[test]
    fn it_ignores_references_to_subsets_of_locally_defined_nested_constants() {
        let contents: String = String::from(
            "\
class Foo::Bar
  Foo
end
        ",
        );
        let configuration = Configuration::default();

        let references = process_from_contents(
            contents,
            &PathBuf::from("path/to/file.rb"),
            &configuration,
        )
        .unresolved_references;
        assert_eq!(references.len(), 1);
        let reference = references
            .first()
            .expect("There should be a reference at index 0");
        assert_eq!(
            UnresolvedReference {
                name: String::from("::Foo::Bar"),
                namespace_path: vec![],
                location: Range {
                    start_row: 1,
                    start_col: 6,
                    end_row: 1,
                    end_col: 15
                }
            },
            *reference,
        );
    }
}
