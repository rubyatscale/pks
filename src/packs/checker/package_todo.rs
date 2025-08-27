//! Package todo format validation checker.
//!
//! This module provides validation to ensure that package_todo.yml files maintain
//! the correct serialization format. This prevents issues where manual edits
//! (such as mass search-replace operations when renaming packs) result in
//! incorrectly formatted files that create noise when running `pks update`.

use std::fs;
use std::path::Path;

use crate::packs::{Configuration, PackageTodo};

use super::ValidatorInterface;

/// Validator that checks package_todo.yml files for correct serialization format.
///
/// This validator ensures that existing package_todo.yml files match their expected
/// serialized format by:
/// 1. Reading the current file content
/// 2. Deserializing it to a PackageTodo struct
/// 3. Re-serializing it using the standard serialization logic
/// 4. Comparing the result with the original file content
///
/// If differences are found, it reports validation errors with context-aware
/// suggestions for the correct command to run based on packs_first_mode.
pub struct Checker;

impl ValidatorInterface for Checker {
    /// Validates that all existing package_todo.yml files are correctly formatted.
    ///
    /// Iterates through all packs and checks their package_todo.yml files (if they exist)
    /// to ensure they match the expected serialization format.
    ///
    /// # Returns
    /// - `None` if all files are correctly formatted
    /// - `Some(Vec<String>)` containing error messages for incorrectly formatted files
    fn validate(&self, configuration: &Configuration) -> Option<Vec<String>> {
        let mut validation_errors = Vec::new();

        for pack in &configuration.pack_set.packs {
            let package_todo_path = pack.yml.parent().unwrap().join("package_todo.yml");
            
            // Skip packs that don't have package_todo.yml files
            if !package_todo_path.exists() {
                continue;
            }

            if let Err(error) = validate_package_todo_format(&package_todo_path, &pack.name, configuration.packs_first_mode) {
                validation_errors.push(error);
            }
        }

        if validation_errors.is_empty() {
            None
        } else {
            Some(validation_errors)
        }
    }
}

/// Validates the format of a single package_todo.yml file.
///
/// This function implements the core validation logic:
/// 1. Reads the current file content
/// 2. Deserializes it to ensure it's valid YAML and matches PackageTodo structure
/// 3. Re-serializes it using the standard serialization logic
/// 4. Compares the result with the original content
///
/// # Arguments
/// * `package_todo_path` - Path to the package_todo.yml file to validate
/// * `pack_name` - Name of the pack (used for generating the correct header)
/// * `packs_first_mode` - Whether the project uses packs.yml (affects command suggestions)
///
/// # Returns
/// * `Ok(())` if the file is correctly formatted
/// * `Err(String)` with a descriptive error message if validation fails
///
/// # Common causes of validation failures
/// - Missing `---` separator after header comments
/// - Incorrect ordering of violations or files (should be alphabetically sorted)
/// - Manual edits that break the standard serialization format
/// - Wrong header comment (should match packs_first_mode setting)
fn validate_package_todo_format(
    package_todo_path: &Path,
    pack_name: &str,
    packs_first_mode: bool,
) -> Result<(), String> {
    // Read the current file content
    let current_content = fs::read_to_string(package_todo_path)
        .map_err(|e| format!("Failed to read {}: {}", package_todo_path.display(), e))?;

    // Deserialize to ensure the file is valid and can be parsed
    let package_todo: PackageTodo = serde_yaml::from_str(&current_content)
        .map_err(|e| format!("Failed to parse {}: {}", package_todo_path.display(), e))?;

    // Re-serialize using the standard serialization logic to get the expected format
    let expected_content = crate::packs::package_todo::serialize_package_todo(
        &pack_name.to_string(),
        &package_todo,
        packs_first_mode,
    );

    // Compare the current content with the expected serialized format
    if current_content != expected_content {
        return Err(format!(
            "Package todo file {} is not in the expected format. Please run `{}` to fix it.",
            package_todo_path.display(),
            if packs_first_mode { "pks update" } else { "bin/packwerk update-todo" }
        ));
    }

    Ok(())
}