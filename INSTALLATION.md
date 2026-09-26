# Installation

## Option 1: Prebuilt binary

From v0.5.0, every release publishes prebuilt binaries for macOS, Linux, and Windows, along with installer scripts that put `pks` in `$CARGO_HOME/bin` (usually `~/.cargo/bin`).

macOS and Linux:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/rubyatscale/pks/releases/latest/download/pks-installer.sh | sh
```

Windows (PowerShell):

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/rubyatscale/pks/releases/latest/download/pks-installer.ps1 | iex"
```

To pin a version, replace `latest/download` with `download/v0.5.0` (or whichever version you want). You can also download a `pks-<target>.tar.xz` archive, or the `.zip` on Windows, straight from the [releases page](https://github.com/rubyatscale/pks/releases).

## Option 2: dotslash (v0.4.0 and earlier)

Releases up to v0.4.0 include a [dotslash](https://dotslash-cli.com/docs/installation/) `pks` file, for example https://github.com/rubyatscale/pks/releases/download/v0.4.0/pks. Save it to your Ruby project's `bin/` directory and run `bin/pks`. Releases from v0.5.0 on don't publish a dotslash file.

## Option 3: Build from source

- Install Rust: https://www.rust-lang.org/tools/install
- `cargo install --git https://github.com/rubyatscale/pks`

Note that `cargo install pks` installs the crates.io `pks` crate, which is the original [alexevanczuk/packs](https://github.com/alexevanczuk/packs) at 0.2.40, not this repository.
