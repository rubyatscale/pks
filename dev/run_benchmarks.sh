#!/bin/bash

# Regenerates BENCHMARKS.md by comparing pks against packwerk on a real app.
# Run from the root of the Rails application.
#
#   bash ../pks/dev/run_benchmarks.sh
#
# PKS_ROOT defaults to a sibling checkout (../pks). Override it when pks lives
# somewhere else, e.g. nested under a workspace directory:
#
#   PKS_ROOT=~/workspace/rubyatscale/pks bash $PKS_ROOT/dev/run_benchmarks.sh

set -euo pipefail

PKS_ROOT="${PKS_ROOT:-../pks}"
PKS_BIN="${PKS_BIN:-$PKS_ROOT/target/release/pks}"

if ! command -v hyperfine >/dev/null 2>&1; then
  echo "error: hyperfine not installed (brew install hyperfine)" >&2
  exit 1
fi

if [ ! -x "$PKS_BIN" ]; then
  echo "error: no pks binary at $PKS_BIN" >&2
  echo "  build it first: cargo build --release --manifest-path $PKS_ROOT/Cargo.toml" >&2
  echo "  or set PKS_ROOT / PKS_BIN" >&2
  exit 1
fi

mkdir -p tmp

# Check if the file exists before removing it
if [ -f "tmp/packs_benchmarks.md" ]; then
  rm tmp/packs_benchmarks.md
fi

echo "I use https://github.com/sharkdp/hyperfine to benchmark, which makes it easy to get consistent benchmarks. Note that benchmarks are done with cache only. While it's interesting to see the performance improvement on a cold cache, it's not representative of the performance of the tool in a real-world scenario, since most of the time the cache will be warm." >> tmp/packs_benchmarks.md
echo "To run these benchmarks on your application, run bash \$PKS_ROOT/dev/run_benchmarks.sh from the root of your application (PKS_ROOT defaults to ../pks)." >> tmp/packs_benchmarks.md

echo -e "\n## Hot Cache, with and without spring, entire codebase" >> tmp/packs_benchmarks.md

# --ignore-failure: these commands exit non-zero when they find violations, which
# is the normal state of an application worth benchmarking. Combined with `set -e`
# above, omitting it would abort the run and discard the results.
hyperfine --ignore-failure --warmup=2 --runs=3 --export-markdown tmp/bm.md \
  "$PKS_BIN update" \
  "$PKS_BIN --experimental-parser update" \
  'DISABLE_SPRING=1 bin/packwerk update' \
  'bin/packwerk update'

cat tmp/bm.md >> tmp/packs_benchmarks.md

mv tmp/packs_benchmarks.md "$PKS_ROOT/BENCHMARKS.md"
