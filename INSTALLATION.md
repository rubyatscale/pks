# Installation

## Option 1: dotslash

- Install [dotslash](https://dotslash-cli.com/docs/installation/).
- Download the `pks` DotSlash file from a release, for example https://github.com/rubyatscale/pks/releases/latest/download/pks, and save it to your Ruby project's `bin/` directory.
- Run `bin/pks`.

The file pins that release's binaries by hash, so everyone working on the project runs the same version. It covers macOS, Linux (x86_64 and aarch64), and Windows (x86_64).

## Option 2: Prebuilt binary

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

## Option 3: Build from source

- Install Rust: https://www.rust-lang.org/tools/install
- `cargo install --git https://github.com/rubyatscale/pks`

Note that `cargo install pks` installs the crates.io `pks` crate, which is the original [alexevanczuk/packs](https://github.com/alexevanczuk/packs) at 0.2.40, not this repository.
