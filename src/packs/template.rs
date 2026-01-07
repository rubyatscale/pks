//! Template expansion for violation messages.
//!
//! This module centralizes all template expansion logic. Formatters call
//! `build_violation_vars()` to get a variable map, then `expand()` to
//! substitute placeholders in templates.

use std::collections::HashMap;

use super::checker::Violation;
use super::checker_configuration::CheckerConfiguration;

/// Expand a template by substituting all {{placeholder}} with values.
pub fn expand(template: &str, variables: &HashMap<&str, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in variables {
        let placeholder = format!("{{{{{}}}}}", key);
        result = result.replace(&placeholder, value);
    }
    result
}

/// Format reference location as file:line:column with newline.
pub fn format_reference_location(
    file: &str,
    line: usize,
    column: usize,
) -> String {
    format!("{}:{}:{}\n", file, line, column)
}

/// Build template variables from a Violation and its CheckerConfiguration.
/// Includes `reference_location` in plain format by default.
/// Callers can modify the returned map to colorize or clear it.
pub fn build_violation_vars(
    v: &Violation,
    checker_config: &CheckerConfiguration,
) -> HashMap<&'static str, String> {
    let mut map = HashMap::new();
    map.insert("violation_name", checker_config.pretty_checker_name());
    map.insert("constant_name", v.identifier.constant_name.clone());
    map.insert(
        "defining_pack_name",
        v.identifier.defining_pack_name.clone(),
    );
    map.insert(
        "referencing_pack_name",
        v.identifier.referencing_pack_name.clone(),
    );
    map.insert(
        "referencing_pack_relative_yml",
        v.referencing_pack_relative_yml.clone(),
    );
    // Include reference_location by default (plain format)
    map.insert(
        "reference_location",
        format_reference_location(
            &v.identifier.file,
            v.source_location.line,
            v.source_location.column,
        ),
    );
    // Layer-specific fields
    if let Some(ref layer) = v.defining_layer {
        map.insert("defining_layer", layer.clone());
    }
    if let Some(ref layer) = v.referencing_layer {
        map.insert("referencing_layer", layer.clone());
    }
    map
}

/// Wrap a reference location string with ANSI color codes.
pub fn colorize_reference_location(location: &str) -> String {
    let trimmed = location.trim_end_matches('\n');
    let suffix = if location.ends_with('\n') { "\n" } else { "" };
    format!("\x1b[36m{}\x1b[0m{}", trimmed, suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_simple() {
        let mut vars = HashMap::new();
        vars.insert("name", "World".to_string());
        assert_eq!(expand("Hello, {{name}}!", &vars), "Hello, World!");
    }

    #[test]
    fn test_expand_multiple() {
        let mut vars = HashMap::new();
        vars.insert("a", "1".to_string());
        vars.insert("b", "2".to_string());
        assert_eq!(expand("{{a}} + {{b}} = 3", &vars), "1 + 2 = 3");
    }

    #[test]
    fn test_expand_missing_var() {
        let vars = HashMap::new();
        assert_eq!(expand("Hello, {{name}}!", &vars), "Hello, {{name}}!");
    }

    #[test]
    fn test_format_reference_location() {
        assert_eq!(format_reference_location("foo.rb", 10, 5), "foo.rb:10:5\n");
    }

    #[test]
    fn test_colorize_reference_location() {
        assert_eq!(
            colorize_reference_location("foo.rb:10:5\n"),
            "\x1b[36mfoo.rb:10:5\x1b[0m\n"
        );
    }

    #[test]
    fn test_colorize_reference_location_no_newline() {
        assert_eq!(
            colorize_reference_location("foo.rb:10:5"),
            "\x1b[36mfoo.rb:10:5\x1b[0m"
        );
    }
}
