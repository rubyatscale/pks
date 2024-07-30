use anyhow::Context;

use crate::packs::{
    pack::{write_pack_to_disk, Pack},
    PackageTodo,
};

use super::Configuration;

pub fn create(
    configuration: &Configuration,
    name: String,
) -> anyhow::Result<()> {
    let existing_pack = configuration.pack_set.for_pack(&name);
    if existing_pack.is_ok() {
        println!("`{}` already exists!", &name);
        return Ok(());
    }
    let new_pack_path =
        configuration.absolute_root.join(&name).join("package.yml");

    let new_pack = Pack::from_contents(
        &new_pack_path,
        &configuration.absolute_root,
        "enforce_dependencies: true",
        PackageTodo::default(),
    )?;

    write_pack_to_disk(&new_pack)?;

    let readme = format!(
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

See https://github.com/rubyatscale/packs#readme for more info!",
    new_pack.name
);

    let readme_path = configuration.absolute_root.join(&name).join("README.md");
    std::fs::write(readme_path, readme).context("Failed to write README.md")?;

    println!("Successfully created `{}`!", name);
    Ok(())
}
