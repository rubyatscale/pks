#!/bin/bash
# Measure `pks check` wall clock and per-phase timings against a real application.
#
# Usage:
#   PKS_APP=/path/to/rails/app bash dev/measure.sh [label]
#
# Point PKS_APP at the largest application you have access to. The phases this
# reports scale very differently with codebase size, so a small app will not tell
# you much.
#
# Emits:
#   - a hyperfine mean (warm cache)
#   - a per-phase table derived from the `--debug` tracing already in the tool
#
# The phase table is the important output. Total wall clock moves with machine
# load; the phase split is stable and tells you whether a change did what it was
# supposed to do. A step that does not move the phase it targeted did not work.
#
# Note on exit codes: `pks check` exits 1 whenever it finds violations, which is
# the normal state of any application worth benchmarking. Every invocation below
# has to tolerate that, or the script reports a failure that is not one.

set -euo pipefail

PKS_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PKS_BIN="${PKS_BIN:-$PKS_ROOT/target/release/pks}"
PKS_APP="${PKS_APP:-}"
LABEL="${1:-$(git -C "$PKS_ROOT" rev-parse --abbrev-ref HEAD)}"
RUNS="${RUNS:-5}"
WARMUP="${WARMUP:-2}"

# Branch names contain slashes, and the label defaults to the branch name, so it
# cannot be used in a filename as-is.
SAFE_LABEL="${LABEL//\//-}"
EXPORT_JSON="$PKS_ROOT/target/measure-$SAFE_LABEL.json"

if [ -z "$PKS_APP" ]; then
  echo "error: set PKS_APP to the root of a Rails app with a packwerk.yml" >&2
  echo "  e.g. PKS_APP=~/src/my_rails_app bash dev/measure.sh" >&2
  exit 1
fi

if [ ! -f "$PKS_APP/packwerk.yml" ]; then
  echo "error: no packwerk.yml found at $PKS_APP" >&2
  exit 1
fi

if ! command -v hyperfine >/dev/null 2>&1; then
  echo "error: hyperfine not installed (brew install hyperfine)" >&2
  exit 1
fi

echo "==> building release binary"
cargo build --release --manifest-path "$PKS_ROOT/Cargo.toml" 2>&1 | tail -2

# `hyperfine --ignore-failure` cannot distinguish "exited 1 because it found
# violations" from "could not be executed", and happily reports 0.0 us for a
# binary that does not exist. Check before measuring nothing.
if [ ! -x "$PKS_BIN" ]; then
  echo "error: no executable pks binary at $PKS_BIN" >&2
  exit 1
fi

mkdir -p "$PKS_ROOT/target"
cd "$PKS_APP"

echo
echo "==> [$LABEL] hyperfine: pks check (warm cache, ${WARMUP} warmup / ${RUNS} runs)"

# Capture rather than pipe straight into grep: a filter on the happy-path lines
# would otherwise swallow hyperfine's own error output, leaving a failed run
# indistinguishable from one that produced no measurements.
if ! hyperfine_out=$(hyperfine --ignore-failure \
      --warmup "$WARMUP" --runs "$RUNS" \
      --export-json "$EXPORT_JSON" \
      "$PKS_BIN check" 2>&1); then
  echo "error: hyperfine failed" >&2
  echo "$hyperfine_out" >&2
  exit 1
fi

if ! grep -E "Time|Range" <<<"$hyperfine_out"; then
  echo "error: could not parse hyperfine output" >&2
  echo "$hyperfine_out" >&2
  exit 1
fi

echo
echo "==> [$LABEL] phase breakdown (single --debug run)"

# `|| true` because a violation exit is expected; without it `pipefail` would
# abort the script here, after the table had already been printed, making the
# failure easy to miss.
debug_out=$("$PKS_BIN" --debug check 2>&1 || true)

# The tracing subscriber prints an uptime timestamp per event. Convert those
# absolute timestamps into per-phase durations by diffing consecutive lines.
# `ignore`-crate gitignore chatter is filtered out; it interleaves with our own
# spans and makes the diffs unattributable.
phase_table=$(sed -E 's/\x1b\[[0-9;]*m//g' <<<"$debug_out" \
  | grep DEBUG \
  | grep -v "gitignore file" \
  | awk '{
      t = $1; sub(/s$/, "", t)
      # Fields are: <uptime> <LEVEL> <file:line:> <message...>. Skip exactly the
      # first three rather than searching for field 4, which would match the
      # wrong offset for any message whose first word also occurs in the path.
      # The leading ` *` matters: the subscriber right-aligns the timestamp, so
      # these lines begin with whitespace.
      msg = ""
      if (match($0, /^ *[^ ]+ +[^ ]+ +[^ ]+ +/)) msg = substr($0, RLENGTH + 1)
      if (prev_t != "") printf "  %7.3f  %s\n", t - prev_t, prev_msg
      prev_t = t + 0; prev_msg = msg
    }
    END {
      if (prev_t == "") exit 1
      printf "  %7.3f  == last trace point at %ss ==\n", 0, prev_t
    }') || {
  echo "error: no --debug trace output; is the binary built from this checkout?" >&2
  echo "$debug_out" | tail -20 >&2
  exit 1
}

echo "$phase_table"

echo
echo "==> [$LABEL] done. hyperfine json: ${EXPORT_JSON#"$PKS_ROOT"/}"
