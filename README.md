# pks
![Logo](logo.png)

[![CI](https://github.com/rubyatscale/pks/actions/workflows/ci.yml/badge.svg)](https://github.com/rubyatscale/pks/actions)
[![Security Audit](https://github.com/rubyatscale/pks/actions/workflows/audit.yml/badge.svg)](https://github.com/rubyatscale/pks/actions?query=workflow%3A%22Security+audit%22++)

A 100% Rust implementation of [packwerk](https://github.com/Shopify/packwerk) and [packwerk-extensions](https://github.com/rubyatscale/packwerk-extensions), a gradual modularization platform for Ruby.

Currently, it ships the following checkers to help improve the boundaries between packages. These checkers are:
- A `dependency` checker requires that a pack specifies packs on which it depends. Cyclic dependencies are not allowed. See [packwerk](https://github.com/Shopify/packwerk)
- A `privacy` checker that ensures other packages are using your package's public API
- A `visibility` checker that allows packages to be private except to an explicit group of other packages.
- A `folder_privacy` checker that allows packages to their sibling packs and parent pack (to be used in an application that uses folder packs)
- A `layer` (formerly `architecture`) checker that allows packages to specify their "layer" and requires that each layer only communicate with layers below it.

See [Checkers](CHECKERS.md) for detailed descriptions.

# Fork
This repo was forked directly from https://github.com/alexevanczuk/packs

# Goals:

## Run 20x faster than `packwerk` on most projects
- Currently ~10-20x as fast as the ruby implementation. See [BENCHMARKS.md](https://github.com/rubyatscale/pks/blob/main/BENCHMARKS.md).
- Your mileage may vary!
- Other performance improvements are coming soon!

## Support non-Rails, non-zeitwerk apps
- Currently supports non-Rails apps through an experimental implementation
- Uses the same public API as `packwerk`, but has different behavior.
- See [EXPERIMENTAL_PARSER_USAGE.md](https://github.com/rubyatscale/pks/blob/main/EXPERIMENTAL_PARSER_USAGE.md) for more info

# Usage and Documentation
Once installed and added to your `$PATH`, just call `pks` to see the CLI help message and documentation.

```
Welcome! Please see https://github.com/rubyatscale/pks for more information!

Usage: pks [OPTIONS] <COMMAND>

Commands:
  greet                           Just saying hi
  create                          Create a new pack
  check                           Look for violations in the codebase
  check-contents                  Check file contents piped to stdin
  update                          Update package_todo.yml files with the current violations
  validate                        Look for validation errors in the codebase
  add-dependency                  Add a dependency from one pack to another
  check-unused-dependencies       Check for dependencies that when removed produce no violations.
  lint-package-yml-files          Lint package.yml files
  expose-monkey-patches           Expose monkey patches of the Ruby stdlib, gems your app uses, and your application itself
  delete-cache                    `rm -rf` on your cache directory, default `tmp/cache/packwerk`
  list-packs                      List packs based on configuration in packwerk.yml (for debugging purposes)
  list-included-files             List analyzed files based on configuration in packwerk.yml (for debugging purposes)
  list-definitions                List the constants that packs sees and where it sees them (for debugging purposes)
  help                            Print this message or the help of the given subcommand(s)

Options:
      --project-root <PROJECT_ROOT>  Path for the root of the project [default: .]
  -d, --debug                        Run with performance debug mode
  -e, --experimental-parser          Run with the experimental parser, which gets constant definitions directly from the AST
      --no-cache                     Run without the cache (good for CI, testing)
  -p, --print-files                  Print to console when files begin and finish processing (to identify files that panic when processing files concurrently)
  -h, --help                         Print help
  -V, --version                      Print version
```


# Installation
See [INSTALLATION.md](https://github.com/rubyatscale/pks/blob/main/INSTALLATION.md)

# Using with VSCode/RubyMine Extension
`packwerk` has a VSCode Extension: https://github.com/rubyatscale/packwerk-vscode/tree/main

It also has a RubyMine Extension: https://github.com/vinted/packwerk-intellij

Using the extension with `pks` is straightforward and results in a much more responsive experience.

Directions:
- Follow [INSTALLATION.md](https://github.com/rubyatscale/pks/blob/main/INSTALLATION.md) instructions to install `pks`
- Follow the [configuration](https://github.com/rubyatscale/packwerk-vscode/tree/main#configuration) directions to configure the extension to use `pks` instead of the ruby gem by setting the executable to `pks check`

# Not yet supported
- custom inflections
- custom load paths
- extensible plugin system

# Behavioral differences
There are still some known behavioral differences between `pks` and `packwerk`. If you find any, please file an issue!
- `package_paths` must not end in a slash, e.g. `pks/*/` is not supported, but `pks/*` is.
- A `**` in `package_paths` is supported, but is not a substitute for a single `*`, e.g. `pks/**` is supported and will match `pks/*/*/package.yml`, but will not match `pks/*/package.yml`. `pks/*` must be used to match that.

## Default Namespaces
`pks` supports Zeitwerk default namespaces. However, since it doesn't have access to the Rails runtime, you need to explicitly specify the namespaces in `packwerk.yml`.

For example, if you're using [`packs-rails`](https://github.com/rubyatscale/packs-rails) and [`automatic_namespaces`](https://github.com/gap777/automatic_namespaces) to configure your default namespaces, and you have
- `pks/foo/app/models/bar.rb` which is configured to define `Foo::Bar`
- `pks/foo/app/domain/baz.rb` which is configured to define `Foo::Baz`

You'll need to specify the default namespaces in `packwerk.yml` like so:
```yml
autoload_roots:
  packs/foo/app/models: "::Foo"
  packs/foo/app/domain: "::Foo"
```

## "check" error messages
The error messages resulting from running `pks check` can be customized with mustache-style interpolation. The available
variables are:
- violation_name
- referencing_pack_name
- defining_pack_name
- constant_name
- reference_location
- referencing_pack_relative_yml

Layer violations also have 
- defining_layer
- referencing_layer

Example:
packwerk.yml
```yml
checker_overrides:
  folder_privacy_error_template: "{{reference_location}} {{violation_name}} / Product Service Privacy Violation: `{{constant_name}}` belongs to the `{{defining_pack_name}}` product service, which is not visible to `{{referencing_pack_name}}` as it is a different product service. See https://go/pks-folder-privacy"
  layer_error_template: "{{reference_location}}Layer violation: `{{constant_name}}` belongs to `{{defining_pack_name}}` (whose layer is `{{defining_layer}}`) cannot be accessed from `{{referencing_pack_name}}` (whose layer is `{{referencing_layer}}`). See https://go/pks-layer"
  visibility_error_template: "{{reference_location}}Visibility violation: `{{constant_name}}` belongs to `{{defining_pack_name}}`, which is not visible to `{{referencing_pack_name}}`. See https://go/pks-visibility"
  privacy_error_template: "{{reference_location}}Privacy violation: `{{constant_name}}` is private to `{{defining_pack_name}}`, but referenced from `{{referencing_pack_name}}`. See https://go/pks-privacy"
  dependency_error_template: "{{reference_location}}Dependency violation: `{{constant_name}}` belongs to `{{defining_pack_name}}`, but `{{referencing_pack_relative_yml}}` does not specify a dependency on `{{defining_pack_name}}`. See https://go/pks-dependency"
```


# Benchmarks
See [BENCHMARKS.md](https://github.com/rubyatscale/pks/blob/main/BENCHMARKS.md)

# Kudos
- @alexevanczuk for https://github.com/alexevanczuk/packs
- Current (@gmcgibbon, @rafaelfranca), and Ex-Shopifolks (@exterm, @wildmaples) for open-sourcing and maintaining `packwerk`
- Gusties, and the [Ruby/Rails Modularity Slack Server](https://join.slack.com/t/rubymod/shared_invite/zt-1dgyrxji9-sihGNX43mVh5T6tw18hFaQ), for continued feedback and support
- @mzruya for the initial implementation and Rust inspiration
