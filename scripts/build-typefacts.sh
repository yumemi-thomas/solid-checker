#!/bin/sh
# Builds the TypeFacts producer from the revision the client crate is pinned to.
#
# The producer and the `typefacts` client verify each other on startup: the
# handshake compares protocol version, schema digest, and build id, and a
# mismatch is a hard failure. Both therefore have to come from one revision,
# so the revision is read out of `rust/Cargo.toml` rather than written twice.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
repo=https://github.com/yumemi-thomas/solid-ts-facts
checkout="${SOLID_TYPEFACTS_CHECKOUT:-$root/.typefacts}"
output="${1:-$root/bin/solid-typefacts}"
build_id="${TYPEFACTS_BUILD_ID:-dev}"

case "$output" in
  /*) ;;
  *) output="$root/$output" ;;
esac

revision=$(
  sed -n 's/^typefacts = .*rev = "\([0-9a-f]\{40\}\)".*/\1/p' "$root/rust/Cargo.toml"
)
if [ -z "$revision" ]; then
  echo "build-typefacts: no typefacts rev pinned in rust/Cargo.toml" >&2
  exit 1
fi

if [ ! -d "$checkout/.git" ]; then
  rm -rf "$checkout"
  git clone --quiet --filter=blob:none "$repo" "$checkout"
fi
if ! git -C "$checkout" cat-file -e "$revision^{commit}" 2>/dev/null; then
  git -C "$checkout" fetch --quiet origin "$revision" || git -C "$checkout" fetch --quiet origin
fi
git -C "$checkout" -c advice.detachedHead=false checkout --quiet "$revision"

mkdir -p "$(dirname -- "$output")"
( cd "$checkout" && go build -ldflags "-X main.buildID=$build_id" -o "$output" ./cmd/solid-typefacts )
echo "build-typefacts: $output at $revision (build id $build_id)"
