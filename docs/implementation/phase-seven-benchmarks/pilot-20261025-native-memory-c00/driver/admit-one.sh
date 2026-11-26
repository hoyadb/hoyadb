#!/bin/zsh
# Admit exactly the currently requested coordinates once, then wait for held/failure (bounded).
O=$1; N=$2; OBS=$O.observations.txt; R=$(pgrep -f "benchmarks/run.py --output $O")
[ -n "$R" ] || { echo "runner gone"; exit 2; }
[ -z "$(pgrep -P $R)" ] || { echo "child exists before admission"; exit 3; }
held=$(grep -c "^BETWEEN_WORKERS" $O.console.txt); [ "$held" -eq $((N-1)) ] || { echo "held count $held != $((N-1))"; exit 4; }
awaits=$(grep -c "^AWAIT_ADMISSION" $O.console.txt); [ "$awaits" -eq "$N" ] || { echo "await count $awaits != $N"; exit 5; }
line=$(grep "^AWAIT_ADMISSION" $O.console.txt | tail -1); coords=${line#AWAIT_ADMISSION }
target/phase-seven-browser-bench/opus/observe.sh "before-admit-$N" >> $OBS 2>&1
echo "ADMIT $coords" >> $O.control; echo "admitted $N $(date -u +%FT%TZ)" >> $OBS
start=$(date +%s); child=""
while true; do
  [ $(grep -c "^BETWEEN_WORKERS" $O.console.txt) -ge $N ] && break
  pgrep -f "benchmarks/run.py --output $O" >/dev/null || break
  c=$(pgrep -P $R | head -1); [ -n "$c" ] && child=$c
  if [ $(( $(date +%s) - start )) -gt 600 ] && [ -n "$c" ] && ps -o command= -p $c | grep -q "release/hoyadb-benchmark --config"; then
    echo "SIGTERM exact child $c after cap $(date -u +%FT%TZ)" >> $OBS; kill -TERM $c; sleep 5
  fi
  sleep 0.5
done
echo "worker $N child=$child done $(date -u +%FT%TZ) elapsed_s=$(( $(date +%s) - start ))" >> $OBS
target/phase-seven-browser-bench/opus/observe.sh "after-worker-$N" >> $OBS 2>&1
tail -2 $O.console.txt | cut -c1-160
