#!/usr/bin/env bash
# Wall-clock timings of the workspace's test suite, for the before/after of
# a change to how the suite is run (`docs/ARCHITECTURE.md` §Formats and
# tools). Not a benchmark harness: it times whole test binaries, not
# operations, and a `divan`/`criterion` bench of the boolean corpus stays
# its own backlog line.
#
#   tools/test-timings.sh                       256 cases (the default)
#   ARRIS_PROPTEST_CASES=1000 tools/test-timings.sh
#
# Prints a markdown table, slowest target first, of three measurements taken
# in one run so a before and an after compare like with like:
#
#   * every test binary run alone, one at a time — the per-target column,
#     and, summed, what `cargo test --workspace` costs, since cargo runs the
#     binaries one after another;
#   * the doctests, which no binary holds and `cargo nextest` does not run;
#   * `cargo nextest run --workspace`, which overlaps the binaries.
#
# Compilation is not timed: everything is built first. Needs `jq`.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

command -v jq >/dev/null || { echo "test-timings: needs jq" >&2; exit 2; }
command -v cargo-nextest >/dev/null || { echo "test-timings: needs cargo-nextest (AGENTS.md, \"Setup\")" >&2; exit 2; }

cases="${ARRIS_PROPTEST_CASES:-256}"

# Seconds with two decimals between two `date +%s%N` readings.
elapsed() { awk -v a="$1" -v b="$2" 'BEGIN { printf "%.2f", (b - a) / 1e9 }'; }

echo "test-timings: building" >&2
cargo test --workspace --no-run --quiet
cargo nextest run --workspace --no-run --cargo-quiet

# Every test executable, with the package it belongs to and the directory
# cargo would run it in, so a test that reads a relative path sees what it
# sees under `cargo test`.
targets=$(
  cargo test --workspace --no-run --message-format=json 2>/dev/null |
    jq -r 'select(.reason == "compiler-artifact" and .executable != null)
           | [ (.manifest_path | sub("/Cargo.toml$"; "")), .target.name, .executable ]
           | @tsv' | sort
)

rows=""
serial=0
while IFS=$'\t' read -r dir name exe; do
  [ -n "${exe:-}" ] || continue
  crate=$(basename "$dir")
  echo "test-timings: $crate::$name" >&2
  start=$(date +%s%N)
  (cd "$dir" && ARRIS_PROPTEST_CASES="$cases" "$exe" --quiet >/dev/null)
  secs=$(elapsed "$start" "$(date +%s%N)")
  rows+="$secs|$crate::$name"$'\n'
  serial=$(awk -v a="$serial" -v b="$secs" 'BEGIN { printf "%.2f", a + b }')
done <<<"$targets"

echo "test-timings: doctests" >&2
start=$(date +%s%N)
ARRIS_PROPTEST_CASES="$cases" cargo test --workspace --doc --quiet >/dev/null
doc=$(elapsed "$start" "$(date +%s%N)")

echo "test-timings: cargo nextest run --workspace" >&2
start=$(date +%s%N)
ARRIS_PROPTEST_CASES="$cases" cargo nextest run --workspace --cargo-quiet --status-level none --final-status-level none >/dev/null
nex=$(elapsed "$start" "$(date +%s%N)")

# Most targets are far below a second and would bury the two that are not,
# so everything under FLOOR is one summarising row.
FLOOR=0.10
sorted=$(mktemp)
trap 'rm -f "$sorted"' EXIT
printf '%s' "$rows" | sort -t'|' -k1,1gr >"$sorted"

echo
echo "\`ARRIS_PROPTEST_CASES=$cases\`, $(nproc) cores, $(date +%F)."
echo
echo "| Target, run alone | Seconds |"
echo "|---|---|"
rest=0
count=0
while IFS='|' read -r secs name; do
  if awk -v s="$secs" -v f="$FLOOR" 'BEGIN { exit !(s >= f) }'; then
    echo "| \`$name\` | $secs |"
  else
    count=$((count + 1))
    rest=$(awk -v a="$rest" -v b="$secs" 'BEGIN { printf "%.2f", a + b }')
  fi
done <"$sorted"
[ "$count" -gt 0 ] && echo "| *$count further targets, each under $FLOOR* | $rest |"
echo "| **serial total** — what \`cargo test --workspace\` costs | **$serial** |"
echo "| \`cargo test --workspace --doc\` | $doc |"
echo "| **\`cargo nextest run --workspace\`, wall clock** | **$nex** |"
