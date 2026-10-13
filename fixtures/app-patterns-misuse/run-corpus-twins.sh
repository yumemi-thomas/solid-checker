#!/bin/bash
# Runs corpus-twins.json through the misuse ledger, one group per corpus install
# (each case's `install`), since the ledger takes one MISUSE_CASES_ROOT a run.
#   APPS_ROOT=<rc.13 corpus apps> CHROME=<chromium> SOLID_CHECKER_NATIVE_BIN=… SOLID_TYPEFACTS_BIN=… \
#     fixtures/app-patterns-misuse/run-corpus-twins.sh <out-dir>
set -eu
export REPO=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${1:?usage: run-corpus-twins.sh <out-dir>}
mkdir -p "$OUT"
for install in $(python3 -c "import json;print(' '.join(sorted({c['install'] for c in json.load(open('$REPO/fixtures/app-patterns-misuse/corpus-twins.json'))['cases']})))"); do
  group=$(echo "$install" | tr '/' '_')
  python3 - "$install" "$OUT/cases-$group.json" <<'PY'
import json,sys
install,out=sys.argv[1],sys.argv[2]
import os
d=json.load(open(os.environ['REPO']+'/fixtures/app-patterns-misuse/corpus-twins.json'))
d['cases']=[c for c in d['cases'] if c['install']==install]
json.dump(d,open(out,'w'),indent=1)
PY
  root="$APPS_ROOT/$install/.solid-checker-corpus-twins"
  rm -rf "$root"
  MISUSE_CASES_ROOT="$root" node "$REPO/benchmarks/reviewed-package-models/development/misuse-runtime-ledger.mjs" \
    "$OUT/result-$group.json" "$CHROME" --cases "$OUT/cases-$group.json" --concurrency 2
done
