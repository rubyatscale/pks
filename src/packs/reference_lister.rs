//! `pks list-references`: every reference from one pack to a constant that
//! another pack defines, with the violations `check` finds in it, as JSON.
//!
//! See `schema/list-references-output.json` for the JSON Schema specification.

use std::{
    collections::HashSet,
    io::{BufWriter, Write},
    path::PathBuf,
};

use rayon::prelude::{ParallelIterator, ParallelSlice};
use serde::Serialize;

use super::{
    checker::get_checkers,
    checker_configuration::CheckerType,
    reference_extractor::{between_packs, get_all_references, identity},
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

    // Only the referencing packs' own files can reference from them
    let owned_files;
    let files = if referencing_packs.is_empty() {
        &configuration.included_files
    } else {
        owned_files = files_of(configuration, &referencing_packs)?;
        &owned_files
    };
    let all_references = get_all_references(configuration, files)?;
    let references =
        between_packs(&all_references, &defining_packs, &referencing_packs);

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

/// The included files that the packs own
fn files_of(
    configuration: &Configuration,
    packs: &HashSet<&str>,
) -> anyhow::Result<HashSet<PathBuf>> {
    let mut files = HashSet::new();
    for file in &configuration.included_files {
        let owner = configuration.pack_set.for_file(file)?;
        if owner.is_some_and(|pack| packs.contains(pack.name.as_str())) {
            files.insert(file.clone());
        }
    }
    Ok(files)
}
