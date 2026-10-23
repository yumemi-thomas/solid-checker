#!/bin/sh
# Builds the repository-owned Type Facts producer.
#
# The producer and the `typefacts` client verify each other on startup: the
# handshake compares protocol version, schema digest, and build id, and a
# mismatch is a hard failure. The adjacent stamp binds the ignored binary to a
# digest over the producer sources, the client crate, schemas, Cargo.lock (which
# pins tsrs to an exact revision), the tsrs revision, and `rustc -vV`, plus the
# linked build id. The producer is a separate Cargo workspace
# (apps/solid-typefacts, release profile, fat LTO) with its own target
# directory; the result is copied to bin/.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
output="${1:-$root/bin/solid-typefacts}"
build_id="${TYPEFACTS_BUILD_ID:-dev}"

case "$output" in
  /*) ;;
  *) output="$root/$output" ;;
esac

identity=$(node "$root/scripts/typefacts-source-identity.mjs" --build-id "$build_id")
digest=$(printf '%s' "$identity" | sed -n 's/.*"sourceDigest":"\([0-9a-f]\{64\}\)".*/\1/p')
if [ -z "$digest" ]; then
  echo "build-typefacts: could not compute local source identity" >&2
  exit 1
fi

# Several make targets depend on this producer. The exact local manifest and
# build id make repeated calls cheap while ensuring any relevant source move
# invalidates the ignored binary. Set TYPEFACTS_REBUILD=1 to force a rebuild.
stamp="$output.buildinfo"
if [ "${TYPEFACTS_REBUILD:-0}" != "1" ] &&
   [ -x "$output" ] &&
   [ -f "$stamp" ] &&
   [ "$(cat "$stamp")" = "$identity" ]; then
  echo "build-typefacts: $output already at source $digest (build id $build_id)"
  exit 0
fi

mkdir -p "$(dirname -- "$output")"
temporary="$output.tmp.$$"
temporary_stamp="$stamp.tmp.$$"
trap 'rm -f "$temporary" "$temporary_stamp"' EXIT HUP INT TERM
toolchain=$(sed -n 's/^channel *= *"\([^"]*\)".*/\1/p' "$root/apps/solid-typefacts/rust-toolchain.toml")
(
  cd "$root/apps/solid-typefacts"
  TYPEFACTS_BUILD_ID="$build_id" RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-$toolchain}" \
    cargo build --release --locked --bin solid-typefacts
)
cp "$root/apps/solid-typefacts/target/release/solid-typefacts" "$temporary"
printf '%s' "$identity" > "$temporary_stamp"
mv "$temporary" "$output"
mv "$temporary_stamp" "$stamp"
trap - EXIT HUP INT TERM
echo "build-typefacts: $output at source $digest (build id $build_id)"
