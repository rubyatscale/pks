# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this project is

`pks` is a Rust CLI tool — a high-performance reimplementation of Shopify's [packwerk](https://github.com/Shopify/packwerk) Ruby gem. It checks boundaries between Ruby "packs" (packages) in a modular Rails/Ruby application. The binary is named `pks`; the Rust library crate is named `packs`.

## Commands

```bash
# Build
cargo build
cargo build --release

# Run all tests
cargo test

# Run a single test by name
cargo test test_name

# Run a specific integration test file
cargo test --test check_test

# Lint
cargo clippy --all-targets --all-features
cargo fmt --all -- --check   # check only
cargo fmt --all               # apply formatting

# Check compilation without building
cargo check
```

Rustfmt is configured with `max_width=80` (`.rustfmt.toml`). CI runs clippy with `-Dwarnings` so all warnings are errors.

## Architecture

### Entry points
- `src/main.rs` — calls `cli::run()`, maps errors to exit codes (0 = ok, 1 = violations found, 2 = internal error)
- `src/lib.rs` — exposes `pub mod packs::cli` for the `serde_magnus` Ruby FFI consumers
- `src/packs.rs` — module declarations and top-level public functions (`check`, `update`, `validate`, `add_dependency`, etc.)

### Key modules
- `src/packs/cli.rs` — CLI argument parsing with `clap`. Builds `Configuration`, dispatches to `packs::*` functions.
- `src/packs/configuration.rs` — `Configuration` struct (the central data object passed everywhere). Built from `packwerk.yml` via `raw_configuration.rs` + `walk_directory.rs`.
- `src/packs/checker.rs` — Core violation-checking engine. Defines `CheckerInterface` and `ValidatorInterface` traits, `Violation`/`ViolationIdentifier` types, and `check_all()`/`update()`/`validate_all()`.
- `src/packs/checker/` — Individual checkers: `dependency`, `privacy`, `visibility`, `layer`, `folder_privacy`. Each implements `CheckerInterface`.
- `src/packs/parsing/` — File parsing. Two modes:
  - **Packwerk mode** (default): Zeitwerk-based constant resolution from file paths (`parsing/ruby/zeitwerk/`)
  - **Experimental mode** (`--experimental-parser`): AST-based using `lib-ruby-parser` (`parsing/ruby/experimental/`)
  - Both Ruby (`.rb`, `.rake`, `Gemfile`, `.gemspec`) and ERB (`.erb`) are supported.
- `src/packs/pack.rs` / `pack_set.rs` — `Pack` struct (represents one `package.yml`) and `PackSet` (the collection).
- `src/packs/package_todo.rs` — Read/write `package_todo.yml` files that record allowed violations.
- `src/packs/caching/` — Per-file MD5-based cache for parsed results (`tmp/cache/packwerk/` by default). `--no-cache` disables it.

### Data flow for `check`
1. `configuration::get()` reads `packwerk.yml`, walks the directory tree, builds `PackSet` and `Configuration`.
2. `reference_extractor::get_all_references()` processes files in parallel (via `rayon`), extracting `Reference` objects.
3. Each `CheckerInterface` impl runs against every reference to produce `Violation`s.
4. `CheckAllBuilder` separates violations into reportable, stale, and strict-mode violations by comparing against recorded violations in `package_todo.yml`.
5. Output is formatted via `text.rs` (packwerk format), `csv.rs`, or `json.rs`.

### Tests
Integration tests live in `tests/` and use fixture Ruby apps in `tests/fixtures/`. The `tests/common/mod.rs` provides shared test helpers. Most tests use `assert_cmd` to invoke the compiled binary.
