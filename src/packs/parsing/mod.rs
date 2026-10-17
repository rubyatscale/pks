use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

pub(crate) mod ruby;
pub(crate) use ruby::experimental::parser::process_from_path as process_from_ruby_path_experimental;
pub(crate) use ruby::packwerk::parser::process_from_path as process_from_ruby_path;
mod erb;
pub(crate) use erb::experimental::parser::process_from_path as process_from_erb_path_experimental;
pub(crate) use erb::packwerk::parser::process_from_path as process_from_erb_path;

use crate::packs::file_utils::is_stdin_file;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

use super::{
    caching::{cache::Cache, CacheResult},
    file_utils::{get_file_type, SupportedFileType},
    Configuration, ProcessedFile,
};

pub fn process_file(
    path: &Path,
    configuration: &Configuration,
) -> anyhow::Result<ProcessedFile> {
    if configuration.print_files {
        eprintln!("Started processing {}", path.display());
    }
    let file_type_option = get_file_type(path);

    let result = if let Some(file_type) = file_type_option {
        match file_type {
            SupportedFileType::Ruby => {
                if configuration.experimental_parser {
                    process_from_ruby_path_experimental(path, configuration)
                } else {
                    process_from_ruby_path(path, configuration)
                }
            }
            SupportedFileType::Erb => {
                if configuration.experimental_parser {
                    process_from_erb_path_experimental(path, configuration)
                } else {
                    process_from_erb_path(path, configuration)
                }
            }
        }
    } else {
        // Later, we can perhaps have this error, since in theory the Configuration.intersect
        // method should make sure we never get any files we can't handle.
        Ok(ProcessedFile {
            absolute_path: path.to_path_buf(),
            ..ProcessedFile::default()
        })
    };

    if configuration.print_files {
        eprintln!("Finished processing {}", path.display());
    }

    result
}

/// The file's syntax error, if it is one of the app's own. The Ruby and gem
/// sources that `expose-monkey-patches` also reads would warn about syntax that
/// pks can't parse, which isn't the app's to fix.
fn syntax_error_to_warn_about<'a>(
    processed_file: &'a ProcessedFile,
    configuration: &Configuration,
) -> Option<&'a SyntaxError> {
    processed_file.syntax_error.as_ref().filter(|_| {
        configuration
            .included_files
            .contains(&processed_file.absolute_path)
    })
}

fn warn_about_syntax_error(
    path: &Path,
    error: &SyntaxError,
    configuration: &Configuration,
) {
    let file = path
        .strip_prefix(&configuration.absolute_root)
        .unwrap_or(path);
    let line = error
        .line
        .map(|line| format!(" at line {}", line))
        .unwrap_or_default();
    eprintln!(
        "Warning: `{}` has a syntax error{} ({}), so pks may have missed \
         references in it.",
        file.display(),
        line,
        error.message
    );
}

/// The first error Prism reports in a file. What it can't parse, pks
/// can't find references in.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SyntaxError {
    /// None in ERB, whose lines are those of the Ruby that pks makes from it
    pub line: Option<usize>,
    pub message: String,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct UnresolvedReference {
    pub name: String,
    pub namespace_path: Vec<String>,
    pub location: Range,
}

/// An association, as `has_many :tacos`, in a class or module. Rails looks
/// up its class from the name of the model that declares it, which in a
/// module, as a concern's `included` block, is that of each model that
/// includes the module, so the experimental parser leaves it to be looked up
/// once every file is read.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct UnresolvedAssociation {
    /// The class's name, as `Taco`
    pub name: String,
    /// The fully qualified name of the class or module it's declared in
    pub model: String,
    pub in_module: bool,
    /// `belongs_to`, which in a controller is inherited_resources'
    pub belongs_to: bool,
    /// Whether it names its class, as `class_name: "Taco"`
    pub names_its_class: bool,
    pub location: Range,
}

/// `include Taggable` or `prepend Taggable` in the class or module `includer`
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct Inclusion {
    /// The fully qualified name of the class or module that includes it
    pub includer: String,
    pub includer_is_module: bool,
    /// The module's name as written, as `Taggable`
    pub name: String,
    /// The namespaces it's written in
    pub namespace_path: Vec<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone, Default)]
pub struct Range {
    pub start_row: usize,
    pub start_col: usize,
    pub end_row: usize,
    pub end_col: usize,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Eq)]
pub struct ParsedDefinition {
    pub fully_qualified_name: String,
    pub location: Range,
}

pub fn process_files_with_cache(
    paths: &HashSet<PathBuf>,
    cache: Box<dyn Cache + Send + Sync>,
    configuration: &Configuration,
) -> anyhow::Result<Vec<ProcessedFile>> {
    paths
        .par_iter()
        .map(|absolute_path| -> anyhow::Result<ProcessedFile> {
            let processed_file = if is_stdin_file(absolute_path, configuration)
            {
                process_file(absolute_path, configuration)?
            } else {
                match cache.get(absolute_path)? {
                    CacheResult::Processed(processed_file) => processed_file,
                    CacheResult::Miss(empty_cache_entry) => {
                        let processed_file =
                            process_file(absolute_path, configuration)?;
                        // So the warning comes back until the file is fixed
                        if syntax_error_to_warn_about(
                            &processed_file,
                            configuration,
                        )
                        .is_none()
                        {
                            cache.write(&empty_cache_entry, &processed_file)?;
                        }
                        processed_file
                    }
                }
            };
            if let Some(error) =
                syntax_error_to_warn_about(&processed_file, configuration)
            {
                warn_about_syntax_error(absolute_path, error, configuration);
            }
            Ok(processed_file)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::packs::file_utils::get_file_type;

    use super::*;

    fn assert_is_ruby(filename: &str) {
        assert_eq!(
            SupportedFileType::Ruby,
            get_file_type(Path::new(filename)).expect("Should be supported")
        )
    }

    fn assert_is_erb(filename: &str) {
        assert_eq!(
            SupportedFileType::Erb,
            get_file_type(Path::new(filename)).expect("Should be supported")
        )
    }

    #[test]
    fn identifies_ruby_files() {
        assert_is_ruby("foo.rb");
        assert_is_ruby("foo.rake");
        assert_is_ruby("Gemfile");
        assert_is_ruby("my_gem.gemspec");
    }

    #[test]
    fn identifies_erb_files() {
        assert_is_erb("foo.erb");
    }
}
