# AGENTS.md

This file provides guidance to AI coding assistants like Claude Code and Cursor when working with code in this repository.

## About pks

`pks` (packs) is a high-performance Rust implementation of Ruby's packwerk tool for gradual modularization. It's 10-20x faster than the Ruby version while maintaining full compatibility.

## Essential Commands

### Development
```bash
# Build the project
cargo build

# Run all tests
cargo test

# Run a specific test
cargo test test_name

# Format code (required before committing)
cargo fmt

# Lint code
cargo clippy

# Check code without building
cargo check
```

### CLI Usage (after building)
```bash
# Core commands
pks check         # Look for violations
pks validate      # Look for validation errors
pks update        # Update package_todo.yml files
pks create        # Create new packs
```

## Architecture Overview

The codebase is organized around a modular checker system that validates various aspects of package boundaries:

- **Entry Points**: `src/main.rs` (CLI) and `src/lib.rs` (library)
- **Core Module**: `src/packs/` contains all business logic
- **Error Handling**: Uses `anyhow` for simplified `Result` handling throughout the codebase
- **Checker System**: Located in `src/packs/checker/`, implements different violation types:
  - `dependency.rs` - Package dependency violations
  - `privacy.rs` - Public API violations
  - `visibility.rs` - Package visibility violations
  - `layer.rs` - Architecture layer violations
  - `folder_privacy.rs` - Folder-level privacy violations

- **Parsing**: `src/packs/parsing/` handles Ruby and ERB code parsing using `lib-ruby-parser`
- **Performance**: Uses `rayon` for parallel processing and implements caching in `src/packs/caching/`
- **Configuration**: Managed through `src/packs/configuration.rs`, reads `packwerk.yml` and `package.yml` files

## Testing

Tests are fixture-based and located in `tests/`. Key test fixtures in `tests/fixtures/` represent different application scenarios. The test suite uses `assert_cmd` for CLI testing.

To run tests for a specific checker or feature, use:
```bash
cargo test checker_name
```

## Key Implementation Details

- **Rust version**: specified in `rust-toolchain.toml`
- **Parallel Processing**: Heavily uses `rayon` for performance
- **Serialization**: Uses `serde_yaml` for YAML files, with deterministic serialization for `package_todo.yml`
- **Ruby Parsing**: Uses `lib-ruby-parser` to generate ASTs for Ruby code analysis
- **Graph Operations**: Uses `petgraph` for dependency cycle detection

## Important Files to Know

- `src/packs/pack.rs` - Core Pack struct and package representation
- `src/packs/package_todo.rs` - Manages TODO violations in `package_todo.yml` files
- `src/packs/checker.rs` - Main violation checking orchestration
- `src/packs/cli.rs` - CLI command definitions and argument parsing

## Rust Style Guide

This codebase follows idiomatic Rust patterns. Here are key conventions:

### Error Handling

This codebase follows a principled approach to error handling: **panic for programming bugs, Result for recoverable errors**.

#### When to Use `.expect()` or `.unwrap()` (Panic)
Use panics for programming bugs - situations that should never happen if the code is written correctly:

```rust
// GOOD: Panic with descriptive message for programming bugs
fn defining_pack_name(&self) -> &str {
    self.defining_pack
        .as_ref()
        .map(|pack| pack.name.as_str())
        .unwrap_or_else(|| {
            panic!(
                "defining_pack_name() called for {:?} checker when defining_pack is None. \
                 This indicates checkable() was not called or returned true incorrectly.",
                self.checker_type
            )
        })
}

// GOOD: Debug assertions catch bugs during development
debug_assert!(
    checker.defining_pack.is_some() || !checker.needs_defining_pack(),
    "PackChecker created without defining_pack when one is required for {:?} checker",
    checker.checker_type
);
```

#### When to Use `Result` and `?` (Graceful Error Handling)
Use `Result` for recoverable errors - things that can go wrong due to external factors:

```rust
// GOOD: Use ? operator with anyhow::Result for external errors
fn process_pack(path: &Path) -> anyhow::Result<Pack> {
    let content = std::fs::read_to_string(path)?;  // File might not exist
    let pack = serde_yaml::from_str(&content)?;     // File might be malformed
    Ok(pack)
}

// GOOD: Handle potentially corrupted data gracefully
fn interpolate_violation_message(&self) -> anyhow::Result<String> {
    let defining_pack_name = self.defining_pack
        .as_ref()
        .map(|pack| pack.name.clone())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Cannot create violation message: defining_pack is None. \
                 This may indicate corrupted reference data."
            )
        })?;
    // ... rest of function
}
```

#### Key Error Handling Decision Framework
Ask yourself: **"If this fails, is it because:"**
- **My code has a bug?** → Use `.expect()` with descriptive message (crash and fix the bug)
- **External factors beyond my control?** → Use `Result` and `?` (handle gracefully)

```rust
// BAD: Generic unwrap() with no context
fn process_pack(path: &Path) -> Pack {
    let content = std::fs::read_to_string(path).unwrap();  // Could be external error!
    serde_yaml::from_str(&content).unwrap()                // No debugging info!
}
```

### Iterator Chains Over Loops

```rust
// GOOD: Use iterator chains for transformations
let violations: Vec<_> = packs
    .iter()
    .filter(|p| p.enforce_dependencies)
    .flat_map(|p| check_violations(p))
    .collect();

// BAD: Manual loops with push
let mut violations = Vec::new();
for pack in &packs {
    if pack.enforce_dependencies {
        for violation in check_violations(pack) {
            violations.push(violation);
        }
    }
}
```

### Pattern Matching

```rust
// GOOD: Use match for exhaustive handling
match checker_type {
    CheckerType::Dependency => check_dependencies(&pack),
    CheckerType::Privacy => check_privacy(&pack),
    CheckerType::Layer => check_layers(&pack),
}

// BAD: Chain of if-else (when all variants should be handled)
if checker_type == CheckerType::Dependency {
    check_dependencies(&pack)
} else if checker_type == CheckerType::Privacy {
    check_privacy(&pack)
} else {
    check_layers(&pack)
}
```

### Ownership and Borrowing

```rust
// GOOD: Borrow when you don't need ownership
fn validate_pack(pack: &Pack) -> bool {
    pack.dependencies.iter().all(|d| is_valid(d))
}

// BAD: Taking ownership unnecessarily
fn validate_pack(pack: Pack) -> bool {
    pack.dependencies.iter().all(|d| is_valid(d))
}
```

### Use of Option and Result

```rust
// GOOD: Use if let for single pattern matching
if let Some(config) = load_config()? {
    process_with_config(&config);
}

// GOOD: Use .ok() to convert Result to Option when errors can be ignored
let cache = load_cache().ok();

// BAD: Verbose match when if let would suffice
match load_config()? {
    Some(config) => process_with_config(&config),
    None => {},
}
```

### Parallel Processing with Rayon

```rust
// GOOD: Use rayon's parallel iterators
use rayon::prelude::*;
let results: Vec<_> = files
    .par_iter()
    .map(|f| process_file(f))
    .collect();

// BAD: Sequential processing when parallel would be better
let results: Vec<_> = files
    .iter()
    .map(|f| process_file(f))
    .collect();
```

## Development Workflow

1. Make changes to the relevant module
2. Run `cargo fmt` to format code
3. Run `cargo clippy` to check for lints
4. Run `cargo test` to ensure tests pass
5. For CLI changes, test with `cargo run -- [command]`

The project maintains compatibility with Ruby's packwerk, so any changes should preserve existing YAML file formats and CLI behavior.
