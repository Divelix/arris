#!/usr/bin/env bash
# The corpus benchmark against a baseline (ADR-0024 §4): what the nightly's
# `bench` job runs, and the same thing locally.
#
#   tools/bench-compare.sh                       against target/bench/baseline.json
#   tools/bench-compare.sh path/to/report.json   against another report
#   tools/bench-compare.sh --bless               the new run becomes the baseline
#
# Runs `cargo bench -p arris --bench corpus`, saves the report as
# `target/bench/latest.json` and, when the baseline exists, writes the
# comparison — every case's ratio, those past `bench::RATIO_FLAG` flagged —
# to `target/bench/comparison.md` and prints it. A missing baseline is not
# an error: the first run has nothing to compare against, so it says so and
# saves the report. Time never fails the script; only a bench that cannot
# run does. With `$GITHUB_STEP_SUMMARY` set, the comparison (or the note
# that there was no baseline) is appended to the job summary too.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

out=target/bench
baseline="$out/baseline.json"
bless=false
case "${1:-}" in
  --bless) bless=true ;;
  "") ;;
  *) baseline="$1" ;;
esac
mkdir -p "$out"
latest="$out/latest.json"
table="$out/comparison.md"
rm -f "$table"

args=(--save "$latest")
if [[ -f "$baseline" ]]; then
  args+=(--compare "$baseline" --table "$table")
  note=""
else
  note="No baseline at \`$baseline\`: nothing to compare this run against."
  echo "bench-compare: $note" >&2
fi

cargo bench -p arris --bench corpus -- "${args[@]}"

if $bless; then
  cp "$latest" "$out/baseline.json"
  echo "bench-compare: $latest is the new baseline" >&2
fi

if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  {
    echo "## Corpus benchmark"
    echo
    if [[ -f "$table" ]]; then cat "$table"; else echo "$note"; fi
  } >>"$GITHUB_STEP_SUMMARY"
fi
