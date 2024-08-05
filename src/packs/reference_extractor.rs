use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use rayon::iter::{IntoParallelIterator, ParallelIterator};
use ruby_references::references::{
    all_references, configuration::ExtraReferenceFieldsFn,
};

use super::{
    checker::reference::Reference, file_utils::expand_glob,
    parsing::ruby::rails_utils, Configuration, SourceLocation,
};

struct PackageNames {
    owning_pack_name_for_file: HashMap<PathBuf, String>,
}

impl PackageNames {
    fn new(owning_pack_name_for_file: HashMap<PathBuf, String>) -> Self {
        PackageNames {
            owning_pack_name_for_file,
        }
    }

    pub fn find_pack_name(&self, file_path: &PathBuf) -> Option<String> {
        self.owning_pack_name_for_file.get(file_path).cloned()
    }
}

impl ExtraReferenceFieldsFn for PackageNames {
    fn extra_reference_fields_fn(
        &self,
        referencing_file_path: &PathBuf,
        defining_file_path: Option<&PathBuf>,
    ) -> HashMap<String, String> {
        let mut extra_fields = HashMap::new();
        if let Some(referencing_pack) =
            self.find_pack_name(referencing_file_path)
        {
            extra_fields
                .insert("referencing_pack_name".to_string(), referencing_pack);
        }
        if let Some(file_path) = defining_file_path {
            if let Some(defining_pack) = self.find_pack_name(file_path) {
                extra_fields
                    .insert("defining_pack_name".to_string(), defining_pack);
            }
        }
        extra_fields
    }
}

fn autoload_paths_from_config(
    configuration: &Configuration,
) -> HashMap<PathBuf, String> {
    let mut autoload_paths: HashMap<PathBuf, String> = configuration
        .pack_set
        .packs
        .iter()
        .flat_map(|pack| pack.default_autoload_roots())
        .map(|path| (path, String::from("")))
        .collect();
    configuration
        .autoload_roots
        .iter()
        .for_each(|(rel_path, ns)| {
            let abs_path = configuration.absolute_root.join(rel_path);
            let ns = if ns == "::Object" {
                String::from("")
            } else {
                ns.to_owned()
            };
            expand_glob(abs_path.to_str().unwrap())
                .iter()
                .for_each(|path| {
                    autoload_paths.insert(path.to_owned(), ns.clone());
                });
        });
    autoload_paths
}

pub(crate) fn get_all_references(
    configuration: &Configuration,
    absolute_paths: &HashSet<PathBuf>,
) -> anyhow::Result<Vec<Reference>> {
    let pack_names = PackageNames::new(
        configuration.pack_set.owning_pack_name_for_file.clone(),
    );

    let extra_reference_fields_fn =
        Some(Box::new(pack_names) as Box<dyn ExtraReferenceFieldsFn>);
    let ref_config =
        ruby_references::references::configuration::Configuration {
            absolute_root: configuration.absolute_root.clone(),
            autoload_paths: autoload_paths_from_config(configuration),
            acronyms: rails_utils::get_acronyms_from_disk(
                &configuration.inflections_path,
            ),
            included_files: absolute_paths.clone(),
            include_reference_is_definition: false,
            cache_enabled: true,
            extra_reference_fields_fn,
            ..Default::default()
        };

    let refs = all_references(&ref_config)?;
    let pks_references = refs
        .into_par_iter()
        .filter_map(|r| {
            if r.extra_fields.get("referencing_pack_name").is_none() {
                None
            } else {
                Some(Reference {
                    constant_name: r.constant_name,
                    defining_pack_name: r
                        .extra_fields
                        .get("defining_pack_name")
                        .cloned(),
                    relative_defining_file: r.relative_defining_file,
                    referencing_pack_name: r
                        .extra_fields
                        .get("referencing_pack_name")
                        .cloned()
                        .unwrap(),
                    relative_referencing_file: r.relative_referencing_file,
                    source_location: SourceLocation {
                        line: r.source_location.line,
                        column: r.source_location.column,
                    },
                })
            }
        })
        .collect();

    Ok(pks_references)
}
