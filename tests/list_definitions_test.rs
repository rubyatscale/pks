use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::error::Error;
mod common;

#[test]
fn test_list_definitions_experimental() -> Result<(), Box<dyn Error>> {
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/app_with_monkey_patches")
        .arg("--debug")
        .arg("--experimental-parser")
        .arg("list-definitions")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"::Foo\" is defined at \"packs/foo/app/models/foo.rb\""
        ))
        .stdout(predicate::str::contains(
            "\"::Foo\" is defined at \"packs/foo/app/services/foo.rb\""
        ))
        .stdout(predicate::str::contains(
            "\"::String\" is defined at \"config/initializers/string_and_date_extensions.rb\""
        ))
        .stdout(predicate::str::contains(
            "\"::Date\" is defined at \"config/initializers/string_and_date_extensions.rb\""
        ))
        .stdout(predicate::str::contains(
            "\"::String\" is defined at \"config/initializers/ignored_string_and_date_extensions.rb\""
        ).not())
        .stdout(predicate::str::contains(
            "\"::Date\" is defined at \"config/initializers/ignored_string_and_date_extensions.rb\""
        ).not());

    Ok(())
}

#[test]
fn test_list_definitions_with_ambiguous_experimental(
) -> Result<(), Box<dyn Error>> {
    cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg("tests/fixtures/app_with_monkey_patches")
        .arg("--debug")
        .arg("--experimental-parser")
        .arg("list-definitions")
        .arg("--ambiguous")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"::Foo\" is defined at \"packs/foo/app/models/foo.rb\""
        ))
        .stdout(predicate::str::contains(
            "\"::Foo\" is defined at \"packs/foo/app/services/foo.rb\""
        ))
        .stdout(predicate::str::contains(
            "\"::String\" is defined at \"config/initializers/string_and_date_extensions.rb\""
        ).not())
        .stdout(predicate::str::contains(
            "\"::Date\" is defined at \"config/initializers/string_and_date_extensions.rb\""
        ).not())
        .stdout(predicate::str::contains(
            "\"::String\" is defined at \"config/initializers/ignored_string_and_date_extensions.rb\""
        ).not())
        .stdout(predicate::str::contains(
            "\"::Date\" is defined at \"config/initializers/ignored_string_and_date_extensions.rb\""
        ).not());

    Ok(())
}
