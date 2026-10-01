# Changelog

## Unreleased

### Fixes

#### ERB comments no longer hide or invent references

pks read every line of a multi-line `<%# %>` comment after the first as Ruby
code. Depending on what the comment said, references elsewhere in the file were
missed, sometimes all of them, and constants named inside the comment were
reported as references. ERB comments are now skipped, as packwerk does. This
changes results in both directions: references that were missed are now
reported, and a `package_todo.yml` entry recorded for a constant named inside a
comment is no longer found, so `pks check` reports it as stale until you run
`pks update`.

pks also found nothing at all in an ERB file whose first tag was an encoding
comment, such as `<%# encoding: iso-8859-1 %>`, unless it named UTF-8 or
ASCII-8BIT. Those files are now checked like any other, so they may show new
violations.

#### `<%%` in ERB no longer hides references

`<%%` writes a literal `<%`, but pks read the rest of the tag as Ruby, so a file
using it lost some or all of its references. It's now treated as text, as Rails
does. packwerk parses the literal as Ruby instead, so it fails on `<%%=` and can
report constants from inside other literals.

#### Upgrading pks invalidates cached results

Cached results didn't record which version of pks produced them, so after an
upgrade pks kept serving old results for files that hadn't changed, including
results the ERB fixes above correct. Each entry now includes the pks version, so
the first run after an upgrade re-reads every file.

#### `check -o json` and `-o csv` list each strict violation once

A strict violation that fails the run is both a new violation and a strict-mode
violation, and the JSON and CSV formats listed it once for each, as two
identical entries. JSON also counted both in `violation_count`, so it could
exceed the number of distinct violations. Each violation now appears once, and
`violation_count` counts distinct violations. Text output is unchanged.

#### `check <paths>` and `check-contents` no longer flag other files' entries as stale

When given paths, `pks check` treated a `package_todo.yml` entry for any file
it had not checked as if that file had been deleted, and reported it as stale.
So checking one file, as editor integrations do, failed whenever any other file
had an entry. `check-contents` behaved the same way. A run given paths now only
considers entries for files at or under those paths. An entry for a file that
was deleted or is no longer included is still reported as stale when the run is
given that file, a directory containing it, or `.`, as a full `pks check` does.

#### `check` and `check-contents` warn about paths that match no included file

A path argument that matched no included file was silently skipped, so a typo,
a path containing `..`, an absolute directory, or an absolute path through a
symlink checked nothing without saying so. `check` and `check-contents` now warn
on stderr, naming the argument and, where there is one, the path to pass
instead. An existing file that just isn't included, such as a README or the
Gemfile, is still skipped quietly, so passing every changed file doesn't warn
about each of them. The exit code and stdout are unchanged, so `-o json` and
`-o csv` output still parses.

## 0.5.0

### Breaking Changes

#### Strict mode tolerates violations already recorded in `package_todo.yml`

Any checker set to `strict` now fails only on references that are **not** already
recorded in a `package_todo.yml`. This matches packwerk's
`unlisted_strict_mode_violations`
([Shopify/packwerk#368](https://github.com/Shopify/packwerk/pull/368)).

This is not limited to privacy and dependencies. The filter is checker-agnostic,
so `enforce_layers: strict`, `enforce_visibility: strict` and strict folder
privacy relax in exactly the same way. If you are using one of those to hold a
boundary hard, this affects you too.

**Who is affected:** any project with a strict checker whose existing violations
are recorded in todo files. Previously `pks check` failed on every recorded strict
violation, so a strict package could only be green with no strict entries
recorded against it.

Entries always live in the **referencing** package's `package_todo.yml`, which is
not always the strict package. For `enforce_privacy`, `enforce_visibility` and
folder privacy the enforcing package is the one being referenced, so look in the
*other* package's file. For `enforce_dependencies` and `enforce_layers` the
enforcing package is the referencing package, so the entries are in the strict
package's own file.

**What changes, in `check`:** pks silently produces different (smaller) results
with no configuration change. Strict packages that were red because of
grandfathered violations go green. New references still fail, and a reference to
a different constant from an already-recorded file still fails.

**What changes, in `update`, and this is the half that touches committed files:**
previously `update` dropped every strict violation when regenerating todo files,
and a package left with no entries had its `package_todo.yml` deleted outright.
So `update` used to erase recorded strict entries, which silently un-did the
tolerance `check` now depends on. It preserves them now.

To be precise about the direction, because it is easy to read this as the
opposite: `update` never *adds* a strict entry. An unrecorded strict violation is
still not written, so strict mode cannot be adopted by running `update`. What
changed is that it stops **deleting** the entries that are already committed. If
your workflow previously relied on `update` clearing them, expect those lines to
survive where they used to disappear.

**Adopting strict mode:** run `update` while the checker is still `true`, commit
the todo files, then set it to `strict`. Flipping first does not work, because
`update` will not record violations for a package that is already strict. See
CHECKERS.md.

**No opt out:** there is no config flag, matching packwerk. `--ignore-recorded-violations`
is *not* a drop-in replacement for the old behaviour, because it also disables
recorded-violation filtering everywhere else and will surface every recorded
violation of every type in every package. It is useful for seeing what the todo
files are grandfathering:

```sh
pks check --ignore-recorded-violations
```

### Fixes

#### Recorded violations in strict packs match their `package_todo.yml` entries

`package_todo.yml` has no field for `strict`, so a recorded violation in a pack
with a strict checker never matched its entry. `check` reported it as a new
violation and reported its todo entry as stale. Neither happens now, and
`--ignore-recorded-violations` no longer prints a spurious stale-todo line for
those entries.

#### Removed stray debug output from the privacy checker

A leftover `dbg!` wrote two lines to stderr for every reference checked against
a pack's `private_constants`, in release builds too.

### Performance

#### `pks check` compiles the inflector's regexes once

Constant-name inflection compiled its regexes on every call, roughly 50k times
per run on a large codebase. They're now compiled once, which cut total CPU time
for `pks check` by about 45% on a large application.

### Distribution

#### Releases are built by cargo-dist

This is the first release built by cargo-dist. Each release now publishes
`pks-<target>.tar.xz` archives for macOS and Linux, a `.zip` for Windows, and
`pks-installer.sh` and `pks-installer.ps1` installer scripts. The old asset names
(`pks-mac.tar.gz`, `x86_64-unknown-linux-gnu.tar.gz` and
`aarch64-unknown-linux-gnu.tar.gz`) are no longer published, so anything that
downloads release assets by name needs updating.

The dotslash `pks` file is still published under the same name. It now points
at the new archives, and it covers Windows as well as macOS and Linux. See
[INSTALLATION.md](https://github.com/rubyatscale/pks/blob/v0.5.0/INSTALLATION.md).

### Internal

#### Replaced `serde_yaml` with `yaml_serde`

`serde_yaml` was discontinued in March 2024, and its `unsafe-libyaml` backend has
been unreleased since. pks now depends on
[`yaml_serde`](https://github.com/yaml/yaml-serde), the YAML organization's
maintained fork, which is backed by `libyaml-rs` from the same org.

`yaml_serde` is an API-compatible fork whose only substantive changes are `no_std`
support and lint cleanups, so this is behavior-preserving: the bytes pks writes to
`package.yml` and `package_todo.yml` are unchanged, as are its YAML parse error
messages. No action is required.

## 0.4.0

### Breaking Changes

#### `respect_gitignore` defaults to `true`

pks now respects `.gitignore` files by default. Files and directories matched by
`.gitignore`, `.git/info/exclude`, or your global gitignore (`core.excludesFile`)
are excluded from analysis.

**Who is affected:** any project that previously relied on pks analyzing gitignored
paths — for example, vendored code checked into `.gitignore`-excluded directories,
or generated files that matter for boundary checking.

**What changes:** pks silently produces different (smaller) results without any
configuration change. This is intentional: most projects want gitignored files
excluded, and the old behavior (analyze everything) was rarely desired.

**Opt out:** add the following to `packwerk.yml` to restore the previous behavior:

```yaml
respect_gitignore: false
```
