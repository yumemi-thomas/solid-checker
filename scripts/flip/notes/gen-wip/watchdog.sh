#!/bin/zsh
# watchdog.sh: kill any process running a binary under $S/bins or $S/{wt,base-wt}/rust/target whose RSS passes 12 GB. Runs until killed.
S=$HOME/EDF/tmp/flip-gen-await
while true; do
  ps -axo pid=,rss=,comm= | while read -r pid rss comm; do
    case "$comm" in $S/bins/*|$S/wt/rust/target/*|$S/base-wt/rust/target/*|$S/wt/bin/*|$S/base-wt/bin/*)
      if (( rss > 12582912 )); then echo "$(date '+%F %T') WATCHDOG kill $pid $comm rss_kb=$rss" >> $S/watchdog.log; kill -KILL $pid; fi;;
    esac
  done
  sleep 1
done
