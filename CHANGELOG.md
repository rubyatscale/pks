# Changelog

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
are recorded in todo files. Note that the entries live in the **referencing**
package's `package_todo.yml`, not the strict package's. Previously `pks check`
failed on every recorded strict violation, so a strict package could only be
green with no strict entries recorded against it.

**What changes, in `check`:** pks silently produces different (smaller) results
with no configuration change. Strict packages that were red because of
grandfathered violations go green. New references still fail, and a reference to
a different constant from an already-recorded file still fails.

**What changes, in `update`, and this is the half that touches committed files:**
previously `update` dropped every strict violation when regenerating todo files,
and a package left with no entries had its `package_todo.yml` deleted outright.
So `update` used to erase recorded strict entries, which silently un-did the
tolerance `check` now depends on. It retains them now. Expect `update` to
*re-add* strict entries to files in your repo, and to show up in a diff or a
stale-todo CI step. `update` still refuses to record an *unrecorded* strict
violation, so strict mode cannot be adopted by running it.

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
