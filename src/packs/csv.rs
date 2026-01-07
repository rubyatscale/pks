use itertools::chain;

use super::checker::{build_strict_violation_message, CheckAllResult};

fn format_message_with_location(v: &super::checker::Violation) -> String {
    format!(
        "{}:{}:{}\n{}",
        v.identifier.file,
        v.source_location.line,
        v.source_location.column,
        v.message
    )
}

pub fn write_csv<W: std::io::Write>(
    result: &CheckAllResult,
    writer: W,
) -> anyhow::Result<()> {
    let mut wtr = csv::Writer::from_writer(writer);
    wtr.write_record([
        "Violation",
        "Strict?",
        "File",
        "Constant",
        "Referencing Pack",
        "Defining Pack",
        "Message",
    ])?;

    if !&result.reportable_violations.is_empty()
        || !&result.strict_mode_violations.is_empty()
    {
        let all = chain!(
            &result.reportable_violations,
            &result.strict_mode_violations
        );

        for violation in all {
            let identifier = &violation.identifier;
            let message = if violation.identifier.strict {
                build_strict_violation_message(&violation.identifier)
            } else {
                format_message_with_location(violation)
            };
            wtr.serialize((
                &identifier.violation_type,
                &identifier.strict,
                &identifier.file,
                &identifier.constant_name,
                &identifier.referencing_pack_name,
                &identifier.defining_pack_name,
                &message,
            ))?;
        }
    } else {
        wtr.serialize(("No violations detected!", "", "", "", "", "", ""))?;
    }
    wtr.flush()?;
    Ok(())
}
