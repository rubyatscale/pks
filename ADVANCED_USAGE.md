# Advanced Usage

This document covers advanced configuration options and features in `pks`.

## Packs First Mode

Packs first mode can be used if your entire team is using `packs`. Currently, the only thing this does is change the copy in `package_todo.yml` files to reference `pks` instead of `packwerk`.

There are two ways to enable this:
1. Rename `packwerk.yml` to `packs.yml` and packs first mode will be automatically enabled.
2. Set `packs_first_mode: true` in your `packwerk.yml`

---

## Gitignore Support

### Overview

By default, `pks` automatically respects `.gitignore` files when analyzing your codebase. This means any files or directories listed in your `.gitignore` will be excluded from pack analysis.

This feature:
- ✅ Reduces noise by excluding generated files, temporary files, and vendor code
- ✅ Improves performance by skipping ignored directories during traversal
- ✅ Works automatically without any configuration
- ✅ Can be disabled if you need behavior identical to `packwerk`

### What Files Are Respected?

`pks` checks multiple gitignore sources in this order:

1. **Local `.gitignore`** - The `.gitignore` file in your repository root
2. **Global gitignore** - Your user-level gitignore file:
   - From `git config core.excludesFile` if set
   - Or `~/.gitignore_global` if it exists
   - Or `~/.config/git/ignore` if it exists
3. **Git exclude file** - The `.git/info/exclude` file in your repository

All standard gitignore features are supported:
- Pattern matching (e.g., `*.log`, `tmp/`)
- Directory exclusion (e.g., `node_modules/`)
- Negation patterns (e.g., `!important.log`)
- Comments (lines starting with `#`)

### Configuration

#### Disabling Gitignore Support

If you need to disable automatic gitignore support, add this to your `packwerk.yml` or `packs.yml`:

```yaml
# Disable automatic gitignore support
respect_gitignore: false
```

#### When to Disable?

You might want to disable gitignore support if:
- You have files in `.gitignore` that should still be analyzed by `pks`
- You're migrating from `packwerk` and want identical behavior
- You have custom `exclude:` patterns that you prefer to manage manually
- You need to analyze generated code that's normally gitignored

#### Default Behavior

If not specified, `respect_gitignore` defaults to `true` (enabled).

### Precedence of Ignore Rules

When determining whether to process a file, `pks` applies rules in this order (highest priority first):

1. **Include patterns** - Files matching `include:` patterns in configuration
2. **Gitignore patterns** - Files/directories in `.gitignore` (if `respect_gitignore: true`)
3. **Exclude patterns** - Files matching `exclude:` patterns in configuration
4. **Default exclusions** - Hardcoded exclusions (e.g., `{bin,node_modules,script,tmp,vendor}/**/*`)

This precedence allows you to:
- Override gitignore by explicitly including files via `include:` patterns
- Add additional exclusions beyond what's in `.gitignore` via `exclude:` patterns
- Layer multiple levels of filtering for fine-grained control

### Example Configuration

```yaml
# packwerk.yml

# Enable gitignore support (this is the default)
respect_gitignore: true

# Include patterns (highest priority - override gitignore)
include:
  - "**/*.rb"
  - "**/*.rake"
  - "**/*.erb"

# Exclude patterns (lower priority than gitignore)
exclude:
  - "{bin,node_modules,script,tmp,vendor}/**/*"
  - "test/fixtures/**/*"
```

**Example scenario:**

Given this configuration and a `.gitignore` containing `debug.log`:

- `app/models/user.rb` → ✅ Analyzed (matches include pattern)
- `tmp/cache/foo.rb` → ❌ Skipped (matches default exclusion)
- `debug.log` → ❌ Skipped (matches gitignore)
- `test/fixtures/data.rb` → ❌ Skipped (matches exclude pattern)

### How It Works

When `respect_gitignore: true` (default):
- ✅ Files in `.gitignore` are automatically skipped during directory walking
- ✅ Directories in `.gitignore` are not traversed (improves performance)
- ✅ Global gitignore patterns are applied
- ✅ Gitignore negation patterns (e.g., `!important.log`) are respected
- ✅ `.git/info/exclude` patterns are applied

When `respect_gitignore: false`:
- ❌ `.gitignore` files are completely ignored
- ✅ Only `include:` and `exclude:` patterns from configuration are used
- ✅ Behavior matches `packwerk` exactly

### Performance Implications

Enabling gitignore support typically has **neutral to positive** performance impact:
- ✅ Ignored directories are skipped entirely (faster directory walking)
- ✅ Fewer files need to be processed
- ✅ Pattern matching is highly optimized (uses the same engine as `ripgrep`)
- ✅ Gitignore matcher is built once and reused throughout the walk

In practice, this means:
- Large ignored directories like `node_modules/`, `tmp/`, or `vendor/` are skipped immediately
- No time wasted parsing or analyzing files that would be ignored anyway
- Memory usage is lower due to fewer files being tracked

### Troubleshooting

#### Files Are Unexpectedly Ignored

If files are being ignored that shouldn't be:

1. **Check your `.gitignore`** - Run `git check-ignore -v path/to/file.rb` to see which pattern is matching
   ```bash
   git check-ignore -v app/models/user.rb
   # Output: .gitignore:10:*.rb    app/models/user.rb
   ```

2. **Check global gitignore** - See where your global gitignore is located:
   ```bash
   git config --global core.excludesFile
   # Output: /Users/you/.gitignore_global
   
   # View its contents
   cat $(git config --global core.excludesFile)
   ```

3. **Disable temporarily** - Set `respect_gitignore: false` to confirm it's a gitignore issue
   ```yaml
   # packwerk.yml
   respect_gitignore: false
   ```

4. **Use negation patterns** - Add `!path/to/file.rb` to your `.gitignore` to explicitly include it
   ```gitignore
   # .gitignore
   *.log
   !important.log  # This file should NOT be ignored
   ```

5. **Override with include patterns** - Add explicit include patterns in your `packwerk.yml`:
   ```yaml
   include:
     - "path/to/important/file.rb"
   ```

#### Files Are Still Analyzed Despite Being in .gitignore

If gitignored files are still being analyzed:

1. **Check configuration** - Ensure `respect_gitignore: true` (or not set, since it defaults to `true`)
   ```yaml
   # packwerk.yml should have either:
   respect_gitignore: true
   # or nothing (defaults to true)
   ```

2. **Check include patterns** - Include patterns override gitignore; files matching `include:` will be processed even if gitignored
   ```yaml
   include:
     - "**/*.rb"  # This will include ALL .rb files, even if gitignored
   ```

3. **Check file location** - Only files within the project root can be affected by gitignore
   - Files must be relative to the repository root
   - Symlinked files outside the repo may not respect gitignore

4. **Verify .gitignore syntax** - Ensure your patterns are valid
   ```bash
   # Test if git itself recognizes the pattern
   git status  # Should not show the file if properly ignored
   git check-ignore path/to/file.rb  # Should output the path if ignored
   ```

#### Debugging Commands

Useful commands for debugging gitignore behavior:

```bash
# List all files that pks will analyze
pks list-included-files

# Check if a specific file would be ignored by git
git check-ignore -v path/to/file.rb

# See your global gitignore location
git config --global core.excludesFile

# View your global gitignore contents
cat ~/.gitignore_global
# or
cat ~/.config/git/ignore

# View repository-specific exclusions
cat .git/info/exclude

# Test gitignore patterns
echo "path/to/file.rb" | git check-ignore --stdin -v
```

### Compatibility with Packwerk

This feature is a **new addition** in `pks` and does not exist in `packwerk`. 

#### Migrating from Packwerk

If you're migrating from `packwerk` to `pks`:

1. **Default behavior change**: `pks` will automatically respect `.gitignore` files, while `packwerk` does not
2. **Files that may be affected**: Any files in your `.gitignore` that were previously analyzed by `packwerk` will now be skipped
3. **To get identical behavior**: Set `respect_gitignore: false` in your configuration
4. **Recommended approach**: Try the default behavior first; it usually works better and is faster

#### Example Migration

```yaml
# packwerk.yml - for packwerk-identical behavior
respect_gitignore: false

# Or accept the new default (recommended)
# respect_gitignore: true  # This is the default, no need to specify
```

---

## Custom Error Messages

The error messages resulting from running `pks check` can be customized with mustache-style interpolation. The available variables are:
- `violation_name`
- `referencing_pack_name`
- `defining_pack_name`
- `constant_name`
- `reference_location`
- `referencing_pack_relative_yml`

Layer violations also have:
- `defining_layer`
- `referencing_layer`

Example:
```yaml
# packwerk.yml
checker_overrides:
  privacy_error_template: "{{reference_location}}Privacy violation: `{{constant_name}}` is private to `{{defining_pack_name}}`. See https://go/pks-privacy"
  dependency_error_template: "{{reference_location}}Dependency violation: `{{constant_name}}` belongs to `{{defining_pack_name}}`. See https://go/pks-dependency"
```

See the main [README.md](README.md) for more details.
