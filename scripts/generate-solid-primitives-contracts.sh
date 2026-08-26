#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
  echo "usage: $0 /path/to/solid-primitives" >&2
  exit 2
fi

root=$1
checker=${SOLID_CHECKER_BIN:-}
if [ -z "$checker" ]; then
  echo "SOLID_CHECKER_BIN must point to a built solid-checker binary" >&2
  exit 2
fi
repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
failures=$(mktemp)
trap 'rm -f "$failures"' EXIT

contract_digest() {
  find "$root/packages" -name solid-reactivity.json -print | sort | xargs shasum | shasum | awk '{print $1}'
}

pass=1
while [ "$pass" -le 12 ]; do
  before=$(contract_digest)
  : > "$failures"
  generated=0

  for package_json in "$root"/packages/*/package.json; do
    package_dir=${package_json%/package.json}
    name=$(bun -e 'process.stdout.write(require(process.argv[1]).name)' "$package_json")
    output="$package_dir/solid-reactivity.json"
    if SOLID_CHECKER_NATIVE_BIN="$checker" bun \
      "$repository_root/packages/cli/bin/solid-checker.mjs" \
      contract generate \
      --package-root "$package_dir" \
      --output "$output"; then
      generated=$((generated + 1))
    else
      printf '%s\n' "$name" >> "$failures"
    fi
  done

  after=$(contract_digest)
  if [ ! -s "$failures" ] && [ "$pass" -gt 1 ] && [ "$before" = "$after" ]; then
    echo "generated $generated Solid Primitives contracts to a fixed point in $pass passes"
    exit 0
  fi
  pass=$((pass + 1))
done

if [ -s "$failures" ]; then
  echo "failed package contracts:" >&2
  sed 's/^/  /' "$failures" >&2
else
  echo "package contracts did not reach a fixed point" >&2
fi
exit 1
