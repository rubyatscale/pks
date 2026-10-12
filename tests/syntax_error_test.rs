use std::{error::Error, fs};

mod common;

/// simple_app, with the cache on, since a file with a syntax error must warn on
/// every run rather than only when it is first parsed
fn fixture_app() -> Result<common::Fixture, Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");
    let packwerk_yml = fixture.path("packwerk.yml");
    let contents = fs::read_to_string(&packwerk_yml)?;
    fs::write(
        &packwerk_yml,
        contents.replace("cache: false", "cache: true"),
    )?;
    Ok(fixture)
}

fn check_stderr(
    fixture: &common::Fixture,
    args: &[&str],
) -> Result<String, Box<dyn Error>> {
    let args = [args, &["check"]].concat();
    let output = common::pks(fixture, &args).get_output().clone();
    Ok(String::from_utf8(output.stderr)?)
}

#[test]
fn test_check_warns_about_a_ruby_file_with_a_syntax_error(
) -> Result<(), Box<dyn Error>> {
    // simple_app's empty `packs/baz/app/services/baz.rb` makes no warning
    let warning = "Warning: `packs/foo/app/services/broken.rb` has a syntax \
                   error at line 7 (unexpected 'end'; expected a `)` to close \
                   the parameters), so pks may have missed references in it.\n";
    for args in [&[][..], &["--experimental-parser"]] {
        let fixture = fixture_app()?;
        fs::write(
            fixture.path("packs/foo/app/services/broken.rb"),
            "class Broken\n  def a\n    Bar\n  end\n\n  def b(\nend\n",
        )?;

        assert_eq!(check_stderr(&fixture, args)?, warning);
        assert_eq!(check_stderr(&fixture, args)?, warning);
    }

    Ok(())
}

#[test]
fn test_check_allows_yield_in_erb() -> Result<(), Box<dyn Error>> {
    // Rails runs a template as the body of a method, so `yield` is no error
    for args in [&[][..], &["--experimental-parser"]] {
        let fixture = fixture_app()?;
        fs::write(
            fixture.path("packs/foo/app/views/layout.erb"),
            "<p><%= yield %></p>\n<%= yield(:footer) %>\n",
        )?;

        assert_eq!(check_stderr(&fixture, args)?, "");
    }

    Ok(())
}

#[test]
fn test_check_warns_about_an_erb_file_with_a_syntax_error(
) -> Result<(), Box<dyn Error>> {
    // pks keeps no ERB positions, so the warning gives no line
    let warning = "Warning: `packs/foo/app/views/broken.erb` has a syntax \
                   error (unexpected end-of-input; expected a `)` to close the \
                   arguments), so pks may have missed references in it.\n";
    for args in [&[][..], &["--experimental-parser"]] {
        let fixture = fixture_app()?;
        fs::write(
            fixture.path("packs/foo/app/views/broken.erb"),
            "<%= Bar.bar( %>\n",
        )?;

        assert_eq!(check_stderr(&fixture, args)?, warning);
    }

    Ok(())
}
