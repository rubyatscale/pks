# Installation
## Option 1:
- Install [dotslash](https://dotslash-cli.com/docs/installation/)
- Download the latest packs release dotslash `pks` file. Example: https://github.com/rubyatscale/pks/releases/tag/v0.2.21/pks
- Save the `pks` file to your ruby project's bin/ directory. You'll then have a `bin/pks` file in your project.
- Use `bin/pks` to execute the CLI.

## Option 2:
- Install Rust: https://www.rust-lang.org/tools/install
  - Note: If `which cargo` returns a path, skip this step!
  - TLDR: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`, and you're done!
- `cargo install pks` (it's like `gem install`)
