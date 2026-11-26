#!/bin/zsh
# Per-admission environment observation, outside all sample clocks.
echo "=== $1 $(date -u +%FT%T.%NZ 2>/dev/null || date -u +%FT%TZ)"; uptime; pmset -g therm | grep -E "Limit|warning"; pmset -g batt | head -2
ps -axo pid,pcpu,etime,command | sort -k2 -rn | head -6 | cut -c1-140
echo "competing:"; ps -axo pid,pcpu,etime,command | grep -Ei "cargo |rustc|gradle|metro|xcodebuild|qemu-system|emulator|pytest|unittest|node --test|wasm-pack|codex" | grep -v grep | cut -c1-140
