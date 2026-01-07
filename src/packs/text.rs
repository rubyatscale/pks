//! Text output formatter for `pks check`.
//!
//! Formats check results as human-readable text with optional color output.

use super::bin_locater;
use super::checker::{
    build_strict_violation_message, CheckAllResult, Violation,
};
use super::template::{
    build_violation_vars, colorize_reference_location, expand,
};
use super::Configuration;

/// Controls whether output should include ANSI color codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Colored,
    Plain,
}

/// Format a violation message with optional colorization of the location.
fn format_violation_message(
    violation: &Violation,
    config: &Configuration,
    color_mode: ColorMode,
) -> String {
    let checker_config =
        &config.checker_configuration[&violation.identifier.violation_type];
    let template = checker_config.checker_error_template();

    let mut vars = build_violation_vars(violation, checker_config);
    // Colorize reference_location if color mode is enabled
    if color_mode == ColorMode::Colored {
        if let Some(loc) = vars.get("reference_location").cloned() {
            vars.insert(
                "reference_location",
                colorize_reference_location(&loc),
            );
        }
    }
    expand(&template, &vars)
}

pub fn write_text<W: std::io::Write>(
    result: &CheckAllResult,
    config: &Configuration,
    mut writer: W,
    color_mode: ColorMode,
) -> anyhow::Result<()> {
    if !result.has_violations() {
        writeln!(writer, "No violations detected!")?;
        return Ok(());
    }

    if !result.reportable_violations.is_empty() {
        let mut sorted_violations: Vec<&Violation> =
            result.reportable_violations.iter().collect();
        sorted_violations.sort_by(|a, b| {
            (&a.identifier.file, &a.identifier.constant_name)
                .cmp(&(&b.identifier.file, &b.identifier.constant_name))
        });

        writeln!(writer, "{} violation(s) detected:", sorted_violations.len())?;

        for violation in sorted_violations {
            let formatted =
                format_violation_message(violation, config, color_mode);
            writeln!(writer, "{}\n", formatted)?;
        }
    }

    if !result.stale_violations.is_empty() {
        writeln!(
            writer,
            "There were stale violations found, please run `{} update`",
            bin_locater::packs_bin_name(),
        )?;
    }

    if !result.strict_mode_violations.is_empty() {
        for v in result.strict_mode_violations.iter() {
            let error_message = build_strict_violation_message(&v.identifier);
            writeln!(writer, "{}", error_message)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packs::checker::{Violation, ViolationIdentifier};
    use crate::packs::checker_configuration::CheckerType;
    use crate::packs::SourceLocation;
    use std::collections::HashSet;

    fn sample_violation() -> Violation {
        Violation {
            identifier: ViolationIdentifier {
                violation_type: CheckerType::Privacy,
                strict: false,
                file: "foo/bar/file.rb".to_string(),
                constant_name: "Foo".to_string(),
                referencing_pack_name: "bar".to_string(),
                defining_pack_name: "foo".to_string(),
            },
            source_location: SourceLocation {
                line: 10,
                column: 5,
            },
            referencing_pack_relative_yml: "bar/package.yml".to_string(),
            defining_layer: None,
            referencing_layer: None,
        }
    }

    #[test]
    fn test_write_text_no_violations() {
        let config = Configuration::default();
        let result = CheckAllResult {
            reportable_violations: HashSet::new(),
            stale_violations: Vec::new(),
            strict_mode_violations: HashSet::new(),
        };

        let mut output = Vec::new();
        write_text(&result, &config, &mut output, ColorMode::Plain).unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "No violations detected!\n"
        );
    }

    #[test]
    fn test_write_text_with_violations() {
        let config = Configuration::default();
        let result = CheckAllResult {
            reportable_violations: [sample_violation()].into_iter().collect(),
            stale_violations: Vec::new(),
            strict_mode_violations: HashSet::new(),
        };

        let mut output = Vec::new();
        write_text(&result, &config, &mut output, ColorMode::Plain).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("1 violation(s) detected:"));
        assert!(text.contains("foo/bar/file.rb:10:5"));
        assert!(text.contains("Privacy violation"));
        assert!(text.contains("`Foo`"));
    }

    #[test]
    fn test_write_text_with_color() {
        let config = Configuration::default();
        let result = CheckAllResult {
            reportable_violations: [sample_violation()].into_iter().collect(),
            stale_violations: Vec::new(),
            strict_mode_violations: HashSet::new(),
        };

        let mut output = Vec::new();
        write_text(&result, &config, &mut output, ColorMode::Colored).unwrap();
        let text = String::from_utf8(output).unwrap();
        // Check that ANSI codes are present for the location
        assert!(text.contains("\x1b[36mfoo/bar/file.rb:10:5\x1b[0m"));
    }
}
