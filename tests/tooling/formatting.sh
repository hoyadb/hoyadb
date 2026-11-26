#!/bin/sh
# SPEC-0030 / TC-0033 F01: exercise layout, rather than just configuration keys.
set -eu

cd "$(dirname "$0")/../.."
fixture_dir=$(mktemp -d)
trap 'rm -rf "$fixture_dir"' EXIT HUP INT TERM

rustfmt --edition 2024 --emit stdout > "$fixture_dir/actual.rs" <<'RUST'
struct Point { x: i32, y: i32 }

enum Event { Moved { x: i32, y: i32 } }

fn point(value: Option<i32>, positive: bool) -> Point {
    let Some(x) = value else { return Point { x: 0, y: 0 }; };
    let y = if positive { 1 } else { -1 };
    Point { x, y }
}

fn required(value: Option<i32>) -> i32 {
    let Some(x) = value else { return 0 };
    x
}
RUST

cat > "$fixture_dir/expected.rs" <<'RUST'
struct Point {
    x: i32,
    y: i32,
}

enum Event {
    Moved {
        x: i32,
        y: i32,
    },
}

fn point(value: Option<i32>, positive: bool) -> Point {
    let Some(x) = value else {
        return Point {
            x: 0,
            y: 0,
        };
    };
    let y = if positive {
        1
    } else {
        -1
    };
    Point {
        x,
        y,
    }
}

fn required(value: Option<i32>) -> i32 {
    let Some(x) = value else {
        return 0;
    };
    x
}
RUST

diff -u "$fixture_dir/expected.rs" "$fixture_dir/actual.rs"
