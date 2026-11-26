#!/bin/sh
# SPEC-0044 / TC-0048 QUALITY02/09. cargo-mutants copies the dirty tree.
set -eu
cd "$(dirname "$0")/../.."
PATH="$PWD/target/quality-tools/bin:$PATH"
export PATH
test "$(cargo mutants --version)" = 'cargo-mutants 27.1.0'
mkdir -p target/quality
python3 tests/tooling/quality.py snapshot target/quality/mutation-source.json
python3 tests/tooling/quality.py metadata target/quality/mutation-measurement.json \
    --kind mutations --command cargo mutants --workspace "$@"
# A listing or early failure must never reuse a previous successful report.
rm -f target/quality/mutation/mutants.out/outcomes.json
status=0
cargo mutants --workspace "$@" || status=$?
validation_status=0
python3 tests/tooling/quality.py mutations target/quality/mutation/mutants.out/outcomes.json \
    --snapshot target/quality/mutation-source.json \
    --measurement target/quality/mutation-measurement.json --tool-exit "$status" \
    --output target/quality/mutation-summary.json || validation_status=$?
if [ "$status" -ne 0 ]; then exit "$status"; fi
exit "$validation_status"
