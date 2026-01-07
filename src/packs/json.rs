//! JSON output formatter for `pks check -o json`.
//!
//! Serializes check results (violations, stale TODOs, and summary) to JSON.
//! See `schema/check-output.json` for the JSON Schema specification.

use itertools::chain;
use serde::Serialize;

use super::checker::{build_strict_violation_message, CheckAllResult};

#[derive(Serialize)]
struct JsonOutput<'a> {
    violations: Vec<JsonViolation<'a>>,
    stale_todos: Vec<JsonStaleTodo<'a>>,
    summary: JsonSummary,
}

#[derive(Serialize)]
struct JsonViolation<'a> {
    violation_type: &'a str,
    file: &'a str,
    line: usize,
    column: usize,
    constant_name: &'a str,
    referencing_pack_name: &'a str,
    defining_pack_name: &'a str,
    strict: bool,
    message: String,
}

#[derive(Serialize)]
struct JsonStaleTodo<'a> {
    violation_type: &'a str,
    file: &'a str,
    constant_name: &'a str,
    referencing_pack_name: &'a str,
    defining_pack_name: &'a str,
}

#[derive(Serialize)]
struct JsonSummary {
    violation_count: usize,
    stale_todo_count: usize,
    strict_violation_count: usize,
    success: bool,
}

pub fn write_json<W: std::io::Write>(
    result: &CheckAllResult,
    writer: W,
) -> anyhow::Result<()> {
    let all_violations = chain!(
        &result.reportable_violations,
        &result.strict_mode_violations
    );

    // JSON outputs raw structured data - consumers can format as needed.
    // Location is provided as separate file/line/column fields.
    let violations: Vec<JsonViolation> = all_violations
        .map(|v| {
            let message = if v.identifier.strict {
                build_strict_violation_message(&v.identifier)
            } else {
                v.message.clone()
            };
            JsonViolation {
                violation_type: &v.identifier.violation_type,
                file: &v.identifier.file,
                line: v.source_location.line,
                column: v.source_location.column,
                constant_name: &v.identifier.constant_name,
                referencing_pack_name: &v.identifier.referencing_pack_name,
                defining_pack_name: &v.identifier.defining_pack_name,
                strict: v.identifier.strict,
                message,
            }
        })
        .collect();

    let stale_todos: Vec<JsonStaleTodo> = result
        .stale_violations
        .iter()
        .map(|v| JsonStaleTodo {
            violation_type: &v.violation_type,
            file: &v.file,
            constant_name: &v.constant_name,
            referencing_pack_name: &v.referencing_pack_name,
            defining_pack_name: &v.defining_pack_name,
        })
        .collect();

    let violation_count = violations.len();
    let stale_todo_count = stale_todos.len();
    let strict_violation_count = result.strict_mode_violations.len();
    let success = violation_count == 0
        && stale_todo_count == 0
        && strict_violation_count == 0;

    let output = JsonOutput {
        violations,
        stale_todos,
        summary: JsonSummary {
            violation_count,
            stale_todo_count,
            strict_violation_count,
            success,
        },
    };

    serde_json::to_writer(writer, &output)?;
    Ok(())
}
