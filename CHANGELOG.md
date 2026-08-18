# Changelog

<!-- Whoever cuts the next release: retitle `## Unreleased` below to the version
     being tagged. `.github/workflows/release.yml` builds the GitHub Release body
     from cargo-dist's `announcement_github_body`, which matches a heading against
     the tag, so an entry left under `## Unreleased` silently misses the release
     notes. Pre-1.0, the breaking change below wants 0.5.0 rather than 0.4.x. -->

## Unreleased

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
