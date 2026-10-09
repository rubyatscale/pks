pub(crate) mod parser;

#[cfg(test)]
mod tests {

    use std::path::PathBuf;

    use crate::packs::parsing::erb::packwerk::parser::process_from_contents;
    use crate::packs::parsing::Range;
    use crate::packs::{Configuration, UnresolvedReference};

    #[test]
    fn trivial_case() {
        let contents: String = String::from("<%= Foo %>");
        let configuration = Configuration::default();

        assert_eq!(
            vec![UnresolvedReference {
                name: String::from("Foo"),
                namespace_path: vec![],
                location: Range::default()
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
    fn multiple_references() {
        let contents: String = String::from("<%= Foo %><%= Bar %>");
        let configuration = Configuration::default();
        assert_eq!(
            vec![
                UnresolvedReference {
                    name: String::from("Foo"),
                    namespace_path: vec![],
                    location: Range::default()
                },
                UnresolvedReference {
                    name: String::from("Bar"),
                    namespace_path: vec![],
                    location: Range::default()
                }
            ],
            process_from_contents(
                contents,
                &PathBuf::from("path/to/file.rb"),
                &configuration
            )
            .unresolved_references
        );
    }
    #[test]
    fn multiline_erb() {
        let contents: String = String::from(
            "/
<%
    Foo
%>
        ",
        );

        let configuration = Configuration::default();
        assert_eq!(
            vec![UnresolvedReference {
                name: String::from("Foo"),
                namespace_path: vec![],
                location: Range::default()
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
    fn erb_with_leading_hyphen_syntax() {
        let contents: String = String::from(
            "/
  <%- Foo %>
    <%= do_thing() %>
  <%- end %>
        ",
        );
        let configuration = Configuration::default();
        assert_eq!(
            vec![UnresolvedReference {
                name: String::from("Foo"),
                namespace_path: vec![],
                location: Range::default()
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
    fn erb_with_trailing_hyphen_syntax() {
        let contents: String = String::from(
            "/
<% Foo %>
<div>
  <div>
    <p>
      <% if condition %>
      <% else %>
      <% end -%>
    </p>
  </div>
</div>
        ",
        );
        let configuration = Configuration::default();
        assert_eq!(
            vec![UnresolvedReference {
                name: String::from("Foo"),
                namespace_path: vec![],
                location: Range::default()
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
    fn complex_multiline_erb() {
        let contents: String = String::from(
            "/
<%
    # Comment
    # Comment
    Foo
    # Comment
    # Comment
%>
        ",
        );
        let configuration = Configuration::default();
        assert_eq!(
            vec![UnresolvedReference {
                name: String::from("Foo"),
                namespace_path: vec![],
                location: Range::default()
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
    fn complex_erb() {
        let contents: String = String::from(
            "/
<!DOCTYPE html>
<html>
<head>
  <title>ERB Snippet</title>
</head>
<body>
  <% if Foo %>
    <h1>Hello, World!</h1>
  <% else %>
    <p>Welcome to the ERB snippet!</p>
  <% end %>

  <% unless Bar.empty? %>
    <ul>
      <% Baz.each do |item| %>
        <li><%= item %></li>
      <% end %>
    </ul>
  <% end %>

  <% for i in Boo %>
    <p>Iteration <%= Bee %></p>
  <% end %>
</body>
</html>
        ",
        );
        let configuration = Configuration::default();
        assert_eq!(
            vec![
                UnresolvedReference {
                    name: String::from("Foo"),
                    namespace_path: vec![],
                    location: Range::default()
                },
                UnresolvedReference {
                    name: String::from("Bar"),
                    namespace_path: vec![],
                    location: Range::default()
                },
                UnresolvedReference {
                    name: String::from("Baz"),
                    namespace_path: vec![],
                    location: Range::default()
                },
                UnresolvedReference {
                    name: String::from("Boo"),
                    namespace_path: vec![],
                    location: Range::default()
                },
                UnresolvedReference {
                    name: String::from("Bee"),
                    namespace_path: vec![],
                    location: Range::default()
                }
            ],
            process_from_contents(
                contents,
                &PathBuf::from("path/to/file.rb"),
                &configuration
            )
            .unresolved_references
        );
    }

    #[test]
    fn multiline_comment_does_not_hide_surrounding_references() {
        let configuration = Configuration::default();
        let expected: Vec<UnresolvedReference> = ["Foo", "Bar", "Baz"]
            .iter()
            .map(|name| UnresolvedReference {
                name: String::from(*name),
                namespace_path: vec![],
                location: Range::default(),
            })
            .collect();

        for text in [
            "second line.",
            "it's here",
            "the end",
            "see } here",
            "second (line",
            "class foo",
            "def x",
        ] {
            let contents = format!(
                "/
<%= Foo %>
<%# first line
    {text} %>
<%= Bar %>
<%= Baz %>
        "
            );
            assert_eq!(
                expected,
                process_from_contents(
                    contents,
                    &PathBuf::from("path/to/file.rb"),
                    &configuration
                )
                .unresolved_references,
                "comment ending with {text:?}"
            );
        }
    }

    #[test]
    fn comment_contents_are_not_references() {
        let contents: String = String::from(
            "/
<%# Qux %>
<%#
  Foo
%>
<%# uses
    Baz -%>
<%= Bar %>
        ",
        );
        let configuration = Configuration::default();
        assert_eq!(
            vec![UnresolvedReference {
                name: String::from("Bar"),
                namespace_path: vec![],
                location: Range::default()
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
    fn leading_encoding_comment_does_not_hide_references() {
        let contents: String =
            String::from("<%# encoding: iso-8859-1 %>\n<%= Foo %>");
        let configuration = Configuration::default();
        assert_eq!(
            vec![UnresolvedReference {
                name: String::from("Foo"),
                namespace_path: vec![],
                location: Range::default()
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
    fn literal_tag_is_text_not_ruby() {
        let contents: String = String::from(
            "/
<%= Foo %>
<%%= Qux %>
<%% if Quux %%>
<%= Bar %>
        ",
        );
        let configuration = Configuration::default();
        assert_eq!(
            vec![
                UnresolvedReference {
                    name: String::from("Foo"),
                    namespace_path: vec![],
                    location: Range::default()
                },
                UnresolvedReference {
                    name: String::from("Bar"),
                    namespace_path: vec![],
                    location: Range::default()
                }
            ],
            process_from_contents(
                contents,
                &PathBuf::from("path/to/file.rb"),
                &configuration
            )
            .unresolved_references
        );
    }

    #[test]
    fn hash_after_hyphen_or_space_is_code_not_comment() {
        let contents: String = String::from(
            "/
<%-# first line
  Foo %>
<% # first line
  Bar %>
        ",
        );
        let configuration = Configuration::default();
        assert_eq!(
            vec![
                UnresolvedReference {
                    name: String::from("Foo"),
                    namespace_path: vec![],
                    location: Range::default()
                },
                UnresolvedReference {
                    name: String::from("Bar"),
                    namespace_path: vec![],
                    location: Range::default()
                }
            ],
            process_from_contents(
                contents,
                &PathBuf::from("path/to/file.rb"),
                &configuration
            )
            .unresolved_references
        );
    }
}
