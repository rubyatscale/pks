use std::{collections::HashSet, path::Path};

use anyhow::{bail, Context};

use super::{
    checker::reference::Reference,
    get_zeitwerk_constant_resolver,
    pack::{write_pack_to_disk, Pack},
    package_todo,
    reference_extractor::{between_packs, get_all_references, identity},
    Configuration,
};

/// Deletes the pack's directory after removing it from other packs' lists and
/// the violations their `package_todo.yml` files record on it.
pub(crate) fn remove(
    configuration: &Configuration,
    name: &str,
    force: bool,
) -> anyhow::Result<()> {
    let pack = configuration.pack_set.for_pack(name)?;
    let directory = configuration.absolute_root.join(&pack.relative_path);
    if directory == configuration.absolute_root {
        bail!("The root pack cannot be removed");
    }

    let nested_packs = nested_packs(configuration, pack);
    if !nested_packs.is_empty() {
        bail!(
            "`{}` contains other packs, which must be removed first: {}",
            pack.name,
            nested_packs.join(", ")
        );
    }

    if !force {
        refuse_if_in_use(configuration, pack)?;
    }

    let mut other_packs: Vec<&Pack> = configuration
        .pack_set
        .packs
        .iter()
        .filter(|other| other.name != pack.name)
        .collect();
    other_packs.sort_by(|a, b| a.name.cmp(&b.name));

    for other in other_packs {
        remove_from_lists(other, &pack.name)?;
        remove_recorded_violations(configuration, other, &pack.name)?;
    }

    // Last, because a partial delete can take package.yml with it, and then
    // `rm` could no longer find the pack to remove the references to it.
    std::fs::remove_dir_all(&directory)
        .with_context(|| format!("Failed to delete {}", directory.display()))?;
    println!("Successfully removed `{}`!", pack.name);

    Ok(())
}

/// Refuses while other packs may still use the pack's constants, because once
/// the pack is gone `check` cannot see those references.
fn refuse_if_in_use(
    configuration: &Configuration,
    pack: &Pack,
) -> anyhow::Result<()> {
    let all_references =
        get_all_references(configuration, &configuration.included_files)?;
    let references = references_from_other_packs(&all_references, pack);
    for reference in &references {
        println!(
            "{}:{}:{} references {}",
            reference.relative_referencing_file,
            reference.source_location.line,
            reference.source_location.column,
            reference.constant_name
        );
    }

    let references = counted(references.len(), "reference");
    let unchecked_files = unchecked_lib_files(configuration, pack);
    let unchecked = counted(unchecked_files, "Ruby file").map(|files| {
        format!(
            "cannot resolve constants defined in `{}` ({}), which is outside \
             the autoload roots, so it cannot tell whether other packs use \
             them",
            pack.relative_path.join("lib").display(),
            files
        )
    });
    match (references, unchecked) {
        (None, None) => Ok(()),
        (Some(references), None) => bail!(
            "Found {} from other packs to constants in `{}`. Remove them, or \
             pass `--force` to remove the pack anyway.",
            references,
            pack.name
        ),
        (None, Some(unchecked)) => bail!(
            "pks {}. Make sure none do, then pass `--force` to remove the pack.",
            unchecked
        ),
        (Some(references), Some(unchecked)) => bail!(
            "Found {} from other packs to constants in `{}`. pks also {}. \
             Remove the references and make sure nothing uses those \
             constants, then pass `--force` to remove the pack.",
            references,
            pack.name,
            unchecked
        ),
    }
}

/// `None` for zero, so callers can match on whether there is anything to say.
fn counted(count: usize, noun: &str) -> Option<String> {
    match count {
        0 => None,
        1 => Some(format!("1 {}", noun)),
        _ => Some(format!("{} {}s", count, noun)),
    }
}

/// Counts the pack's Ruby files under `lib/` that define no constant pks knows.
/// The Zeitwerk resolver learns constants only from autoload roots, so
/// references to these files go unresolved. The experimental parser reads
/// every file, so it has no such gap. Nothing references Rake tasks in
/// `lib/tasks/` by constant.
fn unchecked_lib_files(configuration: &Configuration, pack: &Pack) -> usize {
    if configuration.experimental_parser {
        return 0;
    }
    let constant_resolver = get_zeitwerk_constant_resolver(
        &configuration.pack_set,
        &configuration.constant_resolver_configuration(),
    );
    let definition_files: HashSet<&Path> = constant_resolver
        .fully_qualified_constant_name_to_constant_definition_map()
        .values()
        .flatten()
        .map(|definition| definition.absolute_path_of_definition.as_path())
        .collect();

    let lib = configuration
        .absolute_root
        .join(&pack.relative_path)
        .join("lib");
    let tasks = lib.join("tasks");
    configuration
        .included_files
        .iter()
        .filter(|file| {
            file.starts_with(&lib)
                && !file.starts_with(&tasks)
                && file.extension().is_some_and(|extension| extension == "rb")
                && !definition_files.contains(file.as_path())
        })
        .count()
}

fn nested_packs(configuration: &Configuration, pack: &Pack) -> Vec<String> {
    let mut nested: Vec<String> = configuration
        .pack_set
        .packs
        .iter()
        .filter(|other| {
            other.name != pack.name
                && other.relative_path.starts_with(&pack.relative_path)
        })
        .map(|other| other.name.clone())
        .collect();
    nested.sort();
    nested
}

fn references_from_other_packs<'a>(
    references: &'a [Reference],
    pack: &Pack,
) -> Vec<&'a Reference> {
    let defining_packs = HashSet::from([pack.name.as_str()]);
    between_packs(references, &defining_packs, &HashSet::new())
        // A constant defined in several of the pack's files yields one per file
        .chunk_by(|a, b| identity(a) == identity(b))
        .map(|copies| copies[0])
        .collect()
}

/// Serializes the whole pack, as `add-dependency` does, so the file loses its
/// comments and takes pks's key order.
fn remove_from_lists(pack: &Pack, removed: &str) -> anyhow::Result<()> {
    let mut updated = pack.clone();
    let mut lists = Vec::new();
    if updated.dependencies.remove(removed) {
        lists.push("dependencies");
    }
    if updated.ignored_dependencies.remove(removed) {
        lists.push("ignored_dependencies");
    }
    if updated
        .visible_to
        .as_mut()
        .is_some_and(|visible_to| visible_to.remove(removed))
    {
        lists.push("visible_to");
    }
    if lists.is_empty() {
        return Ok(());
    }

    write_pack_to_disk(&updated)?;
    println!(
        "Removed `{}` from {} ({})",
        removed,
        pack.relative_yml().display(),
        lists.join(", ")
    );
    Ok(())
}

fn remove_recorded_violations(
    configuration: &Configuration,
    pack: &Pack,
    removed: &str,
) -> anyhow::Result<()> {
    let recorded = &pack.package_todo.violations_by_defining_pack;
    if !recorded.contains_key(removed) {
        return Ok(());
    }

    let package_todo_yml = pack.relative_path.join("package_todo.yml");
    if recorded.len() == 1 {
        package_todo::delete_package_todo_from_disk(pack)?;
        println!(
            "Deleted {}, which only recorded violations on `{}`",
            package_todo_yml.display(),
            removed
        );
    } else {
        let mut package_todo = pack.package_todo.clone();
        package_todo.violations_by_defining_pack.remove(removed);
        package_todo::write_package_todo_to_disk(
            pack,
            &package_todo,
            configuration.packs_first_mode,
        )?;
        println!(
            "Removed recorded violations on `{}` from {}",
            removed,
            package_todo_yml.display()
        );
    }

    Ok(())
}
