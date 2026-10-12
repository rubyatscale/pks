use std::{collections::HashSet, path::PathBuf};

use rayon::prelude::{
    IntoParallelRefIterator, ParallelIterator, ParallelSliceMut,
};
use tracing::debug;

use crate::packs::{
    get_experimental_constant_resolver, get_zeitwerk_constant_resolver,
    process_files_with_cache, ProcessedFile,
};

use super::{checker::reference::Reference, Configuration};

pub(crate) fn get_all_references(
    configuration: &Configuration,
    absolute_paths: &HashSet<PathBuf>,
) -> anyhow::Result<Vec<Reference>> {
    let cache = configuration.get_cache();

    debug!("Getting unresolved references (using cache if possible)");

    let (constant_resolver, processed_files_to_check) = if configuration
        .experimental_parser
    {
        // The experimental parser needs *all* processed files to get definitions
        let all_processed_files: Vec<ProcessedFile> = process_files_with_cache(
            &configuration.included_files,
            cache,
            configuration,
        )?;

        let constant_resolver = get_experimental_constant_resolver(
            &configuration.absolute_root,
            &all_processed_files,
            &configuration.ignored_definitions,
        );

        let processed_files_to_check = all_processed_files
            .into_iter()
            .filter(|processed_file| {
                absolute_paths.contains(&processed_file.absolute_path)
            })
            .collect();

        (constant_resolver, processed_files_to_check)
    } else {
        let processed_files: Vec<ProcessedFile> =
            process_files_with_cache(absolute_paths, cache, configuration)?;

        // The zeitwerk constant resolver doesn't look at processed files to get definitions
        let constant_resolver = get_zeitwerk_constant_resolver(
            &configuration.pack_set,
            &configuration.constant_resolver_configuration(),
        );

        (constant_resolver, processed_files)
    };

    debug!("Turning unresolved references into fully qualified references");
    let references: anyhow::Result<Vec<Reference>> = processed_files_to_check
        .par_iter()
        .try_fold(
            Vec::new,
            // Start with an empty vector for each thread
            |mut acc, processed_file| {
                // Try to fold results within a thread
                for unresolved_ref in &processed_file.unresolved_references {
                    let mut refs = Reference::from_unresolved_reference(
                        configuration,
                        constant_resolver.as_ref(),
                        unresolved_ref,
                        &processed_file.absolute_path,
                    )?;
                    acc.append(&mut refs); // Collect references, return error if any
                }
                Ok(acc)
            },
        )
        .try_reduce(
            Vec::new, // Start with an empty vector for the reduction
            |mut acc, mut vec| {
                // Try to reduce results across threads
                acc.append(&mut vec); // Combine vectors, no error expected here
                Ok(acc)
            },
        );
    debug!("Finished turning unresolved references into fully qualified references");

    references
}

/// The references from one pack to a constant that another pack defines,
/// limited to the given packs and sorted by `identity`, which puts the copies
/// of a reference, one for each file that defines its constant, next to each
/// other.
pub(crate) fn between_packs<'a>(
    references: &'a [Reference],
    defining_packs: &HashSet<&str>,
    referencing_packs: &HashSet<&str>,
) -> Vec<&'a Reference> {
    let mut references: Vec<&Reference> = references
        .iter()
        .filter(|reference| {
            let Some(defining) = reference.defining_pack_name.as_deref() else {
                return false; // A constant no pack defines, such as a gem's
            };
            let referencing = reference.referencing_pack_name.as_str();
            defining != referencing
                && selected(defining_packs, defining)
                && selected(referencing_packs, referencing)
        })
        .collect();
    // The copies give the same output in any order, so an unstable sort will do
    references.par_sort_unstable_by(|a, b| identity(a).cmp(&identity(b)));
    references
}

/// No packs given means every pack.
fn selected(packs: &HashSet<&str>, pack: &str) -> bool {
    packs.is_empty() || packs.contains(pack)
}

/// What sets a reference apart from the others. The copies of a reference made
/// for each file that defines its constant share it. So do all of an ERB file's
/// references to one constant, since they are all at line 0.
pub(crate) fn identity(
    reference: &Reference,
) -> (&str, usize, usize, &str, Option<&str>) {
    (
        &reference.relative_referencing_file,
        reference.source_location.line,
        reference.source_location.column,
        &reference.constant_name,
        reference.defining_pack_name.as_deref(),
    )
}
