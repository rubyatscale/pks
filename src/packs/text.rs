//! Text output formatter for `pks check`.
//!
//! Formats check results as human-readable text with optional color output.

use super::bin_locater;

/// Controls whether output should include ANSI color codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Colored,
    Plain,
}
use super::checker::{
    build_strict_violation_message, CheckAllResult, Violation,
};

/// Format a file:line:column location, optionally with color
fn format_location(
    file: &str,
    line: usize,
    column: usize,
    color_mode: ColorMode,
) -> String {
    match color_mode {
        ColorMode::Colored => {
            format!("\x1b[36m{}:{}:{}\x1b[0m", file, line, column)
        }
        ColorMode::Plain => format!("{}:{}:{}", file, line, column),
    }
}

const REFERENCE_LOCATION_PLACEHOLDER: &str = "{{reference_location}}";

/// Format a violation message with optional colorization of the location.
///
/// This function is responsible for substituting `{{reference_location}}` in custom templates.
/// - If the message contains `{{reference_location}}`, substitute it with the formatted location
/// - Otherwise, prepend the location on its own line (default behavior)
fn format_violation_message(
    violation: &Violation,
    color_mode: ColorMode,
) -> String {
    let location = format_location(
        &violation.identifier.file,
        violation.source_location.line,
        violation.source_location.column,
        color_mode,
    );

    if violation.message.contains(REFERENCE_LOCATION_PLACEHOLDER) {
        // Custom template uses {{reference_location}} - substitute it
        violation
            .message
            .replace(REFERENCE_LOCATION_PLACEHOLDER, &format!("{}\n", location))
    } else {
        // Default template - prepend location
        format!("{}\n{}", location, violation.message)
    }
}

pub fn write_text<W: std::io::Write>(
    result: &CheckAllResult,
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
        sorted_violations.sort_by(|a, b| a.message.cmp(&b.message));

        writeln!(writer, "{} violation(s) detected:", sorted_violations.len())?;

        for violation in sorted_violations {
            let formatted = format_violation_message(violation, color_mode);
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
    use crate::packs::SourceLocation;
    use std::collections::HashSet;

    fn sample_violation() -> Violation {
        Violation {
            message: "Privacy violation: `Foo` is private".to_string(),
            identifier: ViolationIdentifier {
                violation_type: "Privacy".to_string(),
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
        }
    }

    #[test]
    fn test_format_location_with_color() {
        let result = format_location("foo.rb", 10, 5, ColorMode::Colored);
        assert_eq!(result, "\x1b[36mfoo.rb:10:5\x1b[0m");
    }

    #[test]
    fn test_format_location_without_color() {
        let result = format_location("foo.rb", 10, 5, ColorMode::Plain);
        assert_eq!(result, "foo.rb:10:5");
    }

    #[test]
    fn test_format_violation_message_with_color() {
        let violation = sample_violation();
        let result = format_violation_message(&violation, ColorMode::Colored);
        assert_eq!(
            result,
            "\x1b[36mfoo/bar/file.rb:10:5\x1b[0m\nPrivacy violation: `Foo` is private"
        );
    }

    #[test]
    fn test_format_violation_message_without_color() {
        let violation = sample_violation();
        let result = format_violation_message(&violation, ColorMode::Plain);
        assert_eq!(
            result,
            "foo/bar/file.rb:10:5\nPrivacy violation: `Foo` is private"
        );
    }

    #[test]
    fn test_write_text_no_violations() {
        let result = CheckAllResult {
            reportable_violations: HashSet::new(),
            stale_violations: Vec::new(),
            strict_mode_violations: HashSet::new(),
        };

        let mut output = Vec::new();
        write_text(&result, &mut output, ColorMode::Plain).unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "No violations detected!\n"
        );
    }

    #[test]
    fn test_write_text_with_violations() {
        let result = CheckAllResult {
            reportable_violations: [sample_violation()].into_iter().collect(),
            stale_violations: Vec::new(),
            strict_mode_violations: HashSet::new(),
        };

        let mut output = Vec::new();
        write_text(&result, &mut output, ColorMode::Plain).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("1 violation(s) detected:"));
        assert!(text.contains("foo/bar/file.rb:10:5"));
        assert!(text.contains("Privacy violation: `Foo` is private"));
    }

    fn custom_template_violation() -> Violation {
        Violation {
            // Message with {{reference_location}} placeholder (from custom template)
            message: "{{reference_location}}Custom privacy error for `Foo`"
                .to_string(),
            identifier: ViolationIdentifier {
                violation_type: "Privacy".to_string(),
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
        }
    }

    #[test]
    fn test_format_violation_message_with_custom_template_no_color() {
        let violation = custom_template_violation();
        let result = format_violation_message(&violation, ColorMode::Plain);
        assert_eq!(
            result,
            "foo/bar/file.rb:10:5\nCustom privacy error for `Foo`"
        );
    }

    #[test]
    fn test_format_violation_message_with_custom_template_with_color() {
        let violation = custom_template_violation();
        let result = format_violation_message(&violation, ColorMode::Colored);
        assert_eq!(
            result,
            "\x1b[36mfoo/bar/file.rb:10:5\x1b[0m\nCustom privacy error for `Foo`"
        );
    }
}
