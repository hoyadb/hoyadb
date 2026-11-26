#!/bin/sh
# SPEC-0044 / TC-0048 QUALITY01/09. Native Rust; branches unknown if uninstrumented.
set -eu
cd "$(dirname "$0")/../.."
PATH="$PWD/target/quality-tools/bin:$PATH"
export PATH
test "$(cargo llvm-cov --version)" = 'cargo-llvm-cov 0.9.1'
mkdir -p target/quality
# A failed collector must not attach old counts to the new source snapshot.
rm -f target/quality/coverage.json target/quality/coverage-summary.json
python3 tests/tooling/quality.py snapshot target/quality/coverage-source.json
python3 tests/tooling/quality.py metadata target/quality/coverage-measurement.json \
    --kind coverage --command cargo llvm-cov --workspace --offline --locked --json \
    --output-path target/quality/coverage.json "$@"
status=0
cargo llvm-cov --workspace --offline --locked --json \
    --output-path target/quality/coverage.json "$@" || status=$?
quality_baseline_path=${HOYADB_COVERAGE_BASELINE:-$(python3 -c 'import json; m=json.load(open("target/quality/coverage-measurement.json"))["comparison"]; print("tests/tooling/baselines/"+m["target"]+"-"+m["profile"]+".json")')}
validation_status=0
python3 tests/tooling/quality.py coverage target/quality/coverage.json \
    --snapshot target/quality/coverage-source.json \
    --measurement target/quality/coverage-measurement.json --baseline "$quality_baseline_path" \
    --tool-exit "$status" --output target/quality/coverage-summary.json || validation_status=$?
if [ "$status" -ne 0 ]; then exit "$status"; fi
exit "$validation_status"
