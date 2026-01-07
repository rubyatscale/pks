use itertools::chain;

use super::checker::{
    build_strict_violation_message, CheckAllResult, Violation,
};
use super::template::{build_violation_vars, expand};
use super::Configuration;

/// Build message from violation using template expansion.
/// For CSV, reference_location uses the default plain format.
fn build_message(v: &Violation, config: &Configuration) -> String {
    if v.identifier.strict {
        build_strict_violation_message(&v.identifier)
    } else {
        let checker_config =
            &config.checker_configuration[&v.identifier.violation_type];
        let template = checker_config.checker_error_template();
        // Uses default reference_location from build_violation_vars (plain format)
        let vars = build_violation_vars(v, checker_config);
        expand(&template, &vars)
    }
}

pub fn write_csv<W: std::io::Write>(
    result: &CheckAllResult,
    config: &Configuration,
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
            let message = build_message(violation, config);
            wtr.serialize((
                identifier.violation_type.to_string(),
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
