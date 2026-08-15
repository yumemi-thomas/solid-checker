---
name: upstream-parity
description: Investigate solid-checker divergences from eslint-plugin-solid — the parity corpus, deviations.json ledger, and the upstream-faithfulness rule for upstream_compat heuristics. Use when parity fails, when a heuristic in upstream_compat looks wrong, or when a rule's behavior differs from upstream.
---

# Upstream parity work

The parity corpus is every valid/invalid case from eslint-plugin-solid
**0.14.5** at upstream commit **`6d3bc311`** (2025-11-18): 465 cases across 19
rules, extracted as data into `fixtures/upstream-parity/upstream-cases.json`
by `scripts/extract-upstream-cases.mjs`. `jsx-uses-vars` is deliberately
absent and its rule deliberately never fires (see
`docs/rules/v1/jsx-uses-vars.md`).

## Running

~~~sh
SOLID_CHECKER_BIN="$PWD/rust/target/debug/solid-checker-rust" \
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" node scripts/parity.mjs
~~~

Use the fresh debug binary after Rust source changes; the checked-in `bin/`
binary may be stale. `--update` rewrites
`fixtures/upstream-parity/deviations.json` from the run — only after the
non-updating run has shown that every changed case is an intentional,
explainable divergence.

## The deviations ledger

Every divergence from upstream must be declared in
`fixtures/upstream-parity/deviations.json` (schemaVersion 2): one entry per
case with a `status` and a `reason` that names the semantic ground for the
divergence (e.g. "the checker reports a conditional return only when the
condition is proven reactive; upstream reports the shape regardless"). The
status vocabulary and its tables are documented in
`fixtures/upstream-parity/README.md`. An undeclared divergence is a parity
failure, not an acceptable delta.

## The upstream-faithfulness rule (trap)

Code under `rust/crates/solid-reactive-ir/src/upstream_compat/` ports upstream
heuristics **byte-faithfully**. Several look like bugs and are not — proven
examples: the `on*` third-character-alphabetic event-handler test, `on:` /
`oncapture:` duplicate folding, and ASCII-only case tests. Before "fixing"
anything there:

1. Read the actual upstream source at the pinned revision:

   ~~~sh
   gh api "repos/solidjs-community/eslint-plugin-solid/contents/<path>?ref=6d3bc311" --jq .content | base64 -d
   ~~~

2. If upstream does the same thing, the heuristic is a faithful port — leave
   it, and record the oddity in `docs/precision-backlog.md` if it is not
   already there.
3. If the checker deliberately does better (evidence-backed precision), keep
   the divergence and declare it in `deviations.json` with the semantic
   reason.
4. Only if the port genuinely mismatches upstream is it a bug to fix — with a
   focused fixture pinning the corrected behavior.

Never resolve a parity failure by weakening a proof, adding blanket trust, or
regex-matching what upstream resolves semantically. Moving the upstream pin
itself is a reviewed dependency change (docs/monorepo.md), not a debugging
step — re-extract with `scripts/extract-upstream-cases.mjs` and re-review, do
not hand-edit `upstream-cases.json`.
