//! `pks list-references`: every reference from one pack to a constant that
//! another pack defines, with the violations `check` finds in it, as JSON.
//!
//! See `schema/list-references-output.json` for the JSON Schema specification.

use std::{
    collections::HashSet,
    io::{BufWriter, Write},
};

use rayon::prelude::{ParallelIterator, ParallelSlice, ParallelSliceMut};
use serde::Serialize;

use super::{
    checker::{get_checkers, reference::Reference},
    checker_configuration::CheckerType,
    reference_extractor::get_all_references,
    Configuration,
};

#[derive(Serialize)]
struct JsonOutput<'a> {
    references: Vec<JsonReference<'a>>,
}

#[derive(Serialize)]
struct JsonReference<'a> {
    file: &'a str,
    line: Option<usize>,
    column: Option<usize>,
    constant_name: &'a str,
    referencing_pack_name: &'a str,
    defining_pack_name: &'a str,
    violations: Vec<JsonViolation>,
}

#[derive(Serialize)]
struct JsonViolation {
    violation_type: CheckerType,
    strict: bool,
    recorded: bool,
}

pub(crate) fn list(
    configuration: &Configuration,
    defining_packs: &[String],
    referencing_packs: &[String],
) -> anyhow::Result<()> {
    let defining_packs = pack_names(configuration, defining_packs)?;
    let referencing_packs = pack_names(configuration, referencing_packs)?;

    let all_references =
        get_all_references(configuration, &configuration.included_files)?;
    let mut references: Vec<&Reference> = all_references
        .iter()
        .filter(|reference| {
            let Some(defining) = reference.defining_pack_name.as_deref() else {
                return false; // A constant no pack defines, such as a gem's
            };
            let referencing = reference.referencing_pack_name.as_str();
            defining != referencing
                && selected(&defining_packs, defining)
                && selected(&referencing_packs, referencing)
        })
        .collect();
    // The order the schema promises. It also puts the copies of a reference
    // next to each other, and as they give the same output in any order, an
    // unstable sort will do.
    references.par_sort_unstable_by(|a, b| identity(a).cmp(&identity(b)));

    let checkers = get_checkers(configuration);
    let recorded_violations = &configuration.pack_set.all_violations;
    let references = references
        // A constant that a pack defines in several files yields a copy of the
        // reference for each file, and their violations can differ, as when
        // only one of the files is public. Like `check`, list each violation
        // that any copy makes.
        .par_chunk_by(|a, b| identity(a) == identity(b))
        .map(|copies| {
            let mut violations = Vec::new();
            for checker in &checkers {
                for reference in copies {
                    if let Some(violation) =
                        checker.check(reference, configuration)?
                    {
                        violations.push(JsonViolation {
                            strict: violation.strict,
                            recorded: recorded_violations
                                .contains(&violation.identifier),
                            violation_type: violation.identifier.violation_type,
                        });
                        break; // One of each type, as `check` lists them
                    }
                }
            }

            let reference = copies[0];
            // pks keeps no source positions in ERB, and reports line 0 there
            let location = (reference.source_location.line > 0)
                .then_some(&reference.source_location);
            Ok(JsonReference {
                file: &reference.relative_referencing_file,
                line: location.map(|location| location.line),
                column: location.map(|location| location.column),
                constant_name: &reference.constant_name,
                referencing_pack_name: &reference.referencing_pack_name,
                defining_pack_name: reference
                    .defining_pack_name
                    .as_deref()
                    .expect("references that no pack defines are filtered out"),
                violations,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let mut stdout = BufWriter::new(std::io::stdout().lock());
    serde_json::to_writer(&mut stdout, &JsonOutput { references })?;
    stdout.flush()?;
    Ok(())
}

/// The packs given on the command line, which must exist.
fn pack_names<'a>(
    configuration: &'a Configuration,
    names: &[String],
) -> anyhow::Result<HashSet<&'a str>> {
    names
        .iter()
        .map(|name| Ok(configuration.pack_set.for_pack(name)?.name.as_str()))
        .collect()
}

/// No packs given means every pack.
fn selected(packs: &HashSet<&str>, pack: &str) -> bool {
    packs.is_empty() || packs.contains(pack)
}

/// What sets a reference apart from the others. The copies of a reference made
/// for each file that defines its constant share it. So do all of an ERB file's
/// references to one constant, since they are all at line 0.
fn identity(reference: &Reference) -> (&str, usize, usize, &str, Option<&str>) {
    (
        &reference.relative_referencing_file,
        reference.source_location.line,
        reference.source_location.column,
        &reference.constant_name,
        reference.defining_pack_name.as_deref(),
    )
}
