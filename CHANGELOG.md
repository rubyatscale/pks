# Changelog

## Unreleased

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
