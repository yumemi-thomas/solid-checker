#!/bin/sh
set -eu

revision=46e038a1554cdac58b0a2f04cde735f010508061
owned_root=false
cleanup() {
  if [ "$owned_root" = true ]; then rm -rf "$root"; fi
}
trap cleanup EXIT
if [ -n "${SOLID_PRIMITIVES_CORPUS:-}" ]; then
  root=$SOLID_PRIMITIVES_CORPUS
else
  root=$(mktemp -d "${TMPDIR:-/tmp}/solid-primitives-corpus.XXXXXX")
  owned_root=true
fi

if [ ! -d "$root/.git" ]; then
  git clone https://github.com/solidjs-community/solid-primitives.git "$root"
fi

if [ -n "$(git -C "$root" status --porcelain)" ]; then
  echo "Solid Primitives corpus checkout has local changes: $root" >&2
  exit 1
fi

git -C "$root" fetch origin next
git -C "$root" checkout --detach "$revision"
pnpm --dir "$root" install --frozen-lockfile
pnpm --dir "$root" build
node scripts/prepare-solid-primitives-corpus.mjs "$root"

SOLID_CHECKER_BIN=$(pwd)/bin/solid-checker-rust \
  scripts/generate-solid-primitives-contracts.sh "$root"
SOLID_CHECKER_BIN=$(pwd)/bin/solid-checker-rust \
  scripts/validate-solid-primitives-contracts.sh "$root"
