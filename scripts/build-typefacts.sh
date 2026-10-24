#!/bin/sh
# Builds the repository-owned Type Facts producer.
#
# The producer and the `typefacts` client verify each other on startup: the
# handshake compares protocol version, schema digest, and build id, and a
# mismatch is a hard failure. The adjacent stamp binds the ignored binary to a
# digest over the producer sources, the client crate, schemas, Cargo.lock (which
# pins tsrs to an exact revision), the tsrs revision, and `rustc -vV`, plus the
# linked build id and any non-release profile. The producer is a separate Cargo
# workspace (apps/solid-typefacts, release profile, fat LTO) with its own target
# directory; the result is copied to bin/.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
output="${1:-$root/bin/solid-typefacts}"
build_id="${TYPEFACTS_BUILD_ID:-dev}"

case "$output" in
  /*) ;;
  *) output="$root/$output" ;;
esac

# Inner-loop builds may ask for the thin-LTO `bench-dev` profile, which skips
# the fat-LTO relink (about 60 s per producer change, ADR 0365). The profile is
# part of the identity, so a `bench-dev` producer never passes for a release
# one: `make verify` builds release, and replaces it.
profile="${TYPEFACTS_PROFILE:-release}"
case "$profile" in
  release|bench-dev) ;;
  *)
    echo "build-typefacts: TYPEFACTS_PROFILE must be release or bench-dev" >&2
    exit 2
    ;;
esac

identity=$(TYPEFACTS_PROFILE="$profile" node "$root/scripts/typefacts-source-identity.mjs" --build-id "$build_id")
digest=$(printf '%s' "$identity" | sed -n 's/.*"sourceDigest":"\([0-9a-f]\{64\}\)".*/\1/p')
if [ -z "$digest" ]; then
  echo "build-typefacts: could not compute local source identity" >&2
  exit 1
fi

# Several make targets depend on this producer. The exact local manifest and
# build id make repeated calls cheap while ensuring any relevant source move
# invalidates the ignored binary. Set TYPEFACTS_REBUILD=1 to force a rebuild.
# An inner-loop request is also met by the release build of the same sources,
# which `make verify` leaves behind: same facts, only slower to link.
stamp="$output.buildinfo"
if [ "${TYPEFACTS_REBUILD:-0}" != "1" ] && [ -x "$output" ] && [ -f "$stamp" ]; then
  current=$(cat "$stamp")
  if [ "$current" = "$identity" ] || { [ "$profile" = bench-dev ] &&
     [ "$current" = "$(TYPEFACTS_PROFILE=release node "$root/scripts/typefacts-source-identity.mjs" --build-id "$build_id")" ]; }; then
    echo "build-typefacts: $output already at source $digest (build id $build_id)"
    exit 0
  fi
fi

# Worktrees share built producers. The binary is a function of the identity
# alone (sources, lockfile, toolchain, build id, profile), so a producer another
# worktree built for this identity is the same build; the copy gets this
# checkout's stamp like a fresh build would. TYPEFACTS_SHARED_CACHE names the
# directory; empty disables it, and CI, which caches through actions/cache,
# leaves it off.
if [ -n "${CI:-}" ]; then
  shared="${TYPEFACTS_SHARED_CACHE-}"
elif [ -n "${XDG_CACHE_HOME:-}" ] || [ -n "${HOME:-}" ]; then
  shared="${TYPEFACTS_SHARED_CACHE-${XDG_CACHE_HOME:-$HOME/.cache}/solid-checker/typefacts}"
else
  shared="${TYPEFACTS_SHARED_CACHE-}"
fi
key=$(printf '%s' "$identity" | { sha256sum 2>/dev/null || shasum -a 256; } | cut -c1-64)
entry="$shared/$key"

mkdir -p "$(dirname -- "$output")"
temporary="$output.tmp.$$"
temporary_stamp="$stamp.tmp.$$"
staging=""
trap 'rm -f "$temporary" "$temporary_stamp"; [ -z "$staging" ] || rm -rf "$staging"' EXIT HUP INT TERM
# TYPEFACTS_REBUILD=1 means "do not trust what is there", the shared entry too:
# it builds, and the build replaces the entry.
if [ "${TYPEFACTS_REBUILD:-0}" != "1" ] && [ -n "$shared" ] && [ -x "$entry/solid-typefacts" ] && [ -f "$entry/identity" ] &&
   [ "$(cat "$entry/identity")" = "$identity" ]; then
  cp "$entry/solid-typefacts" "$temporary"
  source_note="from the shared cache $entry"
else
  toolchain=$(sed -n 's/^channel *= *"\([^"]*\)".*/\1/p' "$root/apps/solid-typefacts/rust-toolchain.toml")
  (
    cd "$root/apps/solid-typefacts"
    TYPEFACTS_BUILD_ID="$build_id" RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-$toolchain}" \
      cargo build --profile "$profile" --locked --bin solid-typefacts
  )
  cp "$root/apps/solid-typefacts/target/$profile/solid-typefacts" "$temporary"
  source_note="built ($profile)"
  if [ -n "$shared" ]; then
    # Published whole or not at all: a directory renamed into place. A second
    # worktree that published the same key first wins, which is the same bytes.
    mkdir -p "$shared"
    staging=$(mktemp -d "$shared/.staging.XXXXXX")
    cp "$temporary" "$staging/solid-typefacts"
    printf '%s' "$identity" > "$staging/identity"
    if [ "${TYPEFACTS_REBUILD:-0}" = "1" ]; then
      rm -rf "$entry"
    fi
    # `mv` onto an existing directory would move into it, so publish only into
    # an absent entry.
    if [ ! -e "$entry" ]; then
      mv "$staging" "$entry" 2>/dev/null || :
    fi
    rm -rf "$staging"
    staging=""
  fi
fi
printf '%s' "$identity" > "$temporary_stamp"
mv "$temporary" "$output"
mv "$temporary_stamp" "$stamp"
trap - EXIT HUP INT TERM
echo "build-typefacts: $output at source $digest (build id $build_id), $source_note"
