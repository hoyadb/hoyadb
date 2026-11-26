#!/bin/sh
# SPEC-0044 / TC-0048: Node host coverage, not real browser/mobile coverage.
set -eu
cd "$(dirname "$0")/../.."
mkdir -p target/quality
node --version > target/quality/javascript-node-version.txt
node --experimental-test-coverage \
    --test-reporter=tap --test-reporter-destination=stdout \
    --test-reporter=lcov --test-reporter-destination=target/quality/javascript.lcov \
    --test-coverage-include='**/packages/client/src/*.js' \
    --test-coverage-include='**/packages/testing/src/*.js' \
    --test packages/client/test/*.test.mjs \
    packages/testing/test/*.test.mjs > target/quality/javascript-coverage.txt 2>&1
cat target/quality/javascript-coverage.txt
