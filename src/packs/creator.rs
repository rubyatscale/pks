use anyhow::Context;

use crate::packs::{
    pack::{write_pack_to_disk, Pack},
    PackageTodo,
};

use super::Configuration;

pub enum CreateResult {
    Success,
    AlreadyExists,
}

pub const NEW_PACKAGE_YML_CONTENTS: &str = "enforce_dependencies: true
enforce_privacy: true
enforce_layers: true";

pub fn create(
    configuration: &Configuration,
    name: &str,
) -> anyhow::Result<CreateResult> {
    let existing_pack = configuration.pack_set.for_pack(name);
    if existing_pack.is_ok() {
        return Ok(CreateResult::AlreadyExists);
    }

    let new_pack_path = configuration.absolute_root.join(name);

    let new_pack = Pack::from_contents(
        &new_pack_path.join("package.yml"),
        &configuration.absolute_root,
        NEW_PACKAGE_YML_CONTENTS,
        PackageTodo::default(),
    )?;

    write_pack_to_disk(&new_pack)?;
    let pack_name =
        &name.split('/').last().context("unable to find pack name")?;
    std::fs::create_dir_all(new_pack_path.join("app/public/").join(pack_name))
        .context(format!("failed to create app/public/{}", &name))?;
    std::fs::create_dir_all(
        new_pack_path.join("app/services/").join(pack_name),
    )
    .context(format!("failed to create app/services/{}", &name))?;
    if is_rails(configuration) {
        std::fs::create_dir_all(new_pack_path.join("app/controllers/"))
            .context("failed to create app/controllers")?;
    }
    if is_rspec(configuration) {
        std::fs::create_dir_all(new_pack_path.join("spec"))
            .context("failed to create spec")?;
    }

    let readme = readme(name);
    let readme_path = &new_pack_path.join("README.md");
    std::fs::write(readme_path, readme).context("Failed to write README.md")?;

    Ok(CreateResult::Success)
}

fn readme(pack_name: &str) -> String {
    format!(
"Welcome to `{}`!

If you're the author, please consider replacing this file with a README.md, which may contain:
- What your pack is and does
- How you expect people to use your pack
- Example usage of your pack's public API and where to find it
- Limitations, risks, and important considerations of usage
- How to get in touch with eng and other stakeholders for questions or issues pertaining to this pack
- What SLAs/SLOs (service level agreements/objectives), if any, your package provides
- When in doubt, keep it simple
- Anything else you may want to include!

README.md should change as your public API changes.

See https://github.com/rubyatscale/pks#readme for more info!",
    pack_name
)
}

fn is_rails(configuration: &Configuration) -> bool {
    gemfile_contains(configuration, "rails")
}

fn is_rspec(configuration: &Configuration) -> bool {
    gemfile_contains(configuration, "rspec")
}

fn gemfile_contains(configuration: &Configuration, val: &str) -> bool {
    match std::fs::read_to_string(configuration.absolute_root.join("Gemfile")) {
        Ok(as_string) => as_string.contains(val),
        _ => false,
    }
}
