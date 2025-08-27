use std::fs;
use std::path::Path;

use crate::packs::{Configuration, PackageTodo};

use super::ValidatorInterface;

pub struct Checker;

impl ValidatorInterface for Checker {
    fn validate(&self, configuration: &Configuration) -> Option<Vec<String>> {
        let mut validation_errors = Vec::new();

        for pack in &configuration.pack_set.packs {
            let package_todo_path = pack.yml.parent().unwrap().join("package_todo.yml");
            
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

fn validate_package_todo_format(
    package_todo_path: &Path,
    pack_name: &str,
    packs_first_mode: bool,
) -> Result<(), String> {
    let current_content = fs::read_to_string(package_todo_path)
        .map_err(|e| format!("Failed to read {}: {}", package_todo_path.display(), e))?;

    let package_todo: PackageTodo = serde_yaml::from_str(&current_content)
        .map_err(|e| format!("Failed to parse {}: {}", package_todo_path.display(), e))?;

    let expected_content = crate::packs::package_todo::serialize_package_todo(
        &pack_name.to_string(),
        &package_todo,
        packs_first_mode,
    );

    if current_content != expected_content {
        return Err(format!(
            "Package todo file {} is not in the expected format. Please run `{}` to fix it.",
            package_todo_path.display(),
            if packs_first_mode { "pks update" } else { "bin/packwerk update-todo" }
        ));
    }

    Ok(())
}