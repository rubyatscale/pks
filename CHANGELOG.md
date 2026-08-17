# Changelog

## Unreleased

### Breaking Changes

#### Strict mode tolerates violations already recorded in `package_todo.yml`

`enforce_privacy: strict` and `enforce_dependencies: strict` now fail only on
references that are **not** already recorded in a `package_todo.yml`. This matches
packwerk's `unlisted_strict_mode_violations` (Shopify/packwerk#368).

**Who is affected:** any project with a strict pack whose existing violations are
recorded in todo files. Previously `pks check` failed on every one of them, so a
strict pack could only be green with an empty todo list.

**What changes:** pks silently produces different (smaller) results without any
configuration change. Strict packs that were red because of grandfathered
violations go green. New references into a strict pack still fail, and `pks update`
still refuses to record an unrecorded strict violation, so strict mode cannot be
silenced by running it.

**Opt out:** there is no config flag, matching packwerk. To see everything the todo
files are grandfathering, run:

```
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
