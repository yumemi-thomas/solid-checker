# The `tsc` oracle

AGENTS.md carries an absolute rule: **never report what TypeScript already
reports**, judged against the library's *real published typings*. This
directory is the machinery that makes that rule checkable instead of
aspirational.

## Why fixture stubs cannot answer the question

Every semantic fixture in this repository types Solid with a reduced
`solid-js.d.ts`. A stub that is *looser* than the real package invents defects
no real project can produce — and every gate stays green while a rule
duplicates `tsc`. That is not hypothetical: SC3004 and SC9002 survived a full
development cycle because the fixtures typed the Solid 2.0 effect callback as
`apply: (value: T) => unknown`, where `@solidjs/signals` says

~~~ts
export type EffectFunction<Prev, Next extends Prev = Prev> = (
  v: Next, p?: Prev
) => (() => void) | void;
~~~

Against the real type, every spelling those rules proved is a type error.

So the oracle never reads a fixture stub. It installs the packages named in
`packages.json`, at the exact versions this repository audits, and compiles
against those.

That is the **audited release** -- today the `2.0.0-rc.9` triple of
`solid-js`, `@solidjs/signals` and `@solidjs/web` -- and only it. Other
analyzed releases (rc.0-rc.8, under the SC9014 notice) answer some questions
differently, and their typings differ too (rc.0-rc.6 type a store root
`Readonly`, rc.0-rc.5 still declare a one-argument `createEffect`); the
release-sensitive dialect answers and their fixtures carry those, not this
ledger. The checker half of every case must run with **no** SC9014 notice,
which is how the gate refuses an oracle pin that has drifted from the release
the checker audits (`AUDITED_INSTALLATION` in
`rust/crates/solid-dialect/src/solid_2/releases.rs`).

## Files

| File | What it is |
| --- | --- |
| `packages.json` | The audited package versions per dialect, the SRI integrity of each audited tarball, and the `jsxImportSource` each needs. `v2` — the only dialect this build carries — cites `benchmarks/package-contract-v2/phase0/rc9` (the tarball records its integrities come from) and the rc.9 vocabulary review. It does not track `pkg/contracts/bundled/runtime-lock.json`, which is the closure of the historical rc.3 bundled-contract audit and stays at rc.3 with those documents. |
| `rule-cases.json` | The executable half of the redundancy ledger: one case per rule whose positive spelling could plausibly also be a `tsc` error, with the two expectations it must satisfy — `expect` for TypeScript, `checker` for this checker — and a written reason. |

The prose half of the ledger — every rule, its classification, and the actual
`tsc` output as evidence — is in `docs/precision-backlog.md`.

## Using it

~~~sh
# install the audited typings (writes to rust/target/tsc-oracle, a build dir)
bun scripts/tsc-oracle.mjs provision --dialect all

# is this snippet already a type error?
bun scripts/tsc-oracle.mjs check --dialect v2 --code 'import { createEffect } from "solid-js"; createEffect(() => 1, () => 2);'

# enforce every declared case (also runs inside scripts/verify.sh, and as `make tsc-oracle`)
SOLID_CHECKER_BIN="$PWD/rust/target/debug/solid-checker-rust" \
  SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" bun scripts/tsc-oracle-gate.mjs

# what both sides actually say, per case -- for writing a ledger entry
SOLID_CHECKER_BIN="$PWD/rust/target/debug/solid-checker-rust" \
  SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" bun scripts/tsc-oracle-gate.mjs --report
~~~

The gate runs the checker as well as the compiler, so it needs both binaries;
a missing one is a hard failure, not a skip. It accepts only an explicit
`SOLID_CHECKER_BIN` or the fresh debug-build path; it never falls back to the
packaged `bin/solid-checker-rust`. `make tsc-oracle` builds that debug target
without overwriting the packaged binary or rebuilding TypeFacts.

Provisioning needs network access once per version bump. An absent,
version-drifted, or integrity-drifted install (a `bun.lock` entry for an
audited package whose integrity is not the pinned one) is a **hard failure**,
never a skip: a silently skipped oracle is the same trap as a `cargo test` run
without `SOLID_TYPEFACTS_BIN`.

This install is **not** the byte source for the archives the negative rows
quote. Those are pinned per archive in
`rust/crates/solid-dialect/audited-archives.json` and installed by
`make audited-archives-provision` (`scripts/audited-archives.mjs`) under
`rust/target/audited-archives/`, so moving the audited release never moves the
bytes an rc.3 row cites.

## Both sides of a case

A case declares what TypeScript says about its bytes (`expect`) **and** what
this checker says about the same bytes (`checker`). Declaring only the first
was the gap this file used to have: `expect: "silent"` is satisfied just as
well by a rule that reports nothing at all, so a narrowing could walk a rule
to a no-op with every case still green.

Three invariants tie the halves together:

- `removed-because-redundant` requires `checker: "silent"`. That expectation's
  claim is precisely that the checker has stopped reporting here; before the
  gate ran the checker, a case could pin TypeScript's diagnostic while the rule
  went on reporting the same expression.
- `distinct-claim` requires `checker: "reports"` — the mirror image. That
  expectation keeps a finding *because* it says something the type error does
  not, so there has to be a finding for the distinction to be about.
- Every exact dialect catalog rule needs a **keystone**: a case pairing `expect: "silent"`
  with `checker: "reports"` — TypeScript says nothing and the rule still
  speaks. A finding from another dialect's catalog cannot satisfy it. Checked-in
  findings snapshots do not substitute for this executable evidence; coverage
  owns those artifacts with its own fresh-binary gate. Rules whose subject no
  snippet can express are listed, with reasons, in the gate's `EXEMPT` map.

Both strict and loose checker projects use the same exported compiler-option
definition as the TypeScript passes. A reporting expectation pins finding kind
and defaults to exactly one finding; a deliberately multi-subject case must
declare `checkerCount`. When a case has TypeScript errors, the gate inspects
every checker finding on the same source subject, not only the named rule;
case-local `distinctFindings` entries must explain legitimate ownership or
reactivity claims that coexist with a type error.

Cases default to a `.tsx` source. Set `sourceExtension` to `"ts"` when the
claim depends on TypeScript-only grammar that TSX reserves for JSX, such as an
angle-bracket type assertion. The gate accepts only `"ts"` and `"tsx"`.

Set `compilerOptions.verbatimModuleSyntax` when a case depends on whether
TypeScript preserves an otherwise-unused import. The gate applies the same
override to both the compiler and checker project; other per-case compiler
options are rejected until their comparison semantics are reviewed.

A case for a rule that has since been *removed* is kept as a regression record
and is exempt from the keystone rule — there is no rule left to keep alive.

## Adding a rule

Before adding or keeping a rule, write its positive case here with
`expect: "silent"` and `checker: "reports"`, and run the gate. If TypeScript
speaks, the rule is TypeScript's — narrow it to the spellings the type system
does not cover, or do not add it. Better wording, an autofix, "the user may not
run `tsc`", and "the project may not be `strict`" are explicitly not
exceptions; the gate runs a non-`strict` pass precisely so the last one cannot
be argued. If the rule will not fire on the case you wrote, the case is not its
positive spelling — find the one that is, rather than declaring
`checker: "silent"` and moving on.

The one escape hatch is `distinct-claim`: TypeScript reports something, but
about a *different* defect than the finding asserts. It requires the diagnostic
codes and a reason naming what the finding claims that the type error does not.

## Bumping a version

Do not float. Move `packages.json` (versions *and* integrities) together with
the checker's audited release, re-provision, run the gate, and expect
`removed-because-redundant` cases to be the ones that move — those pin the
diagnostics that justified deleting a rule, so if a Solid release loosens a
type the gate fails and the removal gets reconsidered rather than quietly
leaving a hole. A release that *tightens* a type moves the other way: a
`silent` case the rule reports on becomes a `tsc` error, and that is a
duplicate to narrow, never a case to delete.

The rc.3 → rc.9 move (2026-09-27) changed these verdicts:

- `no-direct-mutation`'s root-record case: TS2540 on rc.3, clean on rc.9
  (`Store<T> = T` from @solidjs/signals rc.7), and the rule reports both
  writes -- a `removed-because-redundant` record became a keystone.
- Three `missing-effect-function` cases (a one-argument `createEffect`, a
  one-element spread, the same call under `"use server"`): clean on rc.3 and
  reported, TS2554 on rc.9 and silent, because @solidjs/signals rc.6 removed
  the deprecated one-argument overload. They are `removed-because-redundant`
  records now. The `missing-owner` case beside them dropped its one-argument
  call, which was never its subject and is a type error on rc.9.
- Two `reactive-write-in-owned-scope` store-setter cases (a root body, a
  component body): silent on rc.3, whose `devGuardStoreSetterWrite` exempted a
  root, and reported on rc.9, which dropped the exemption. TypeScript is clean
  on both.
- The three rules that were `EXEMPT` because rc.3 could not express them
  (`until-in-tracked-scope`, `static-dynamic-async-source`,
  `flush-in-action`) now carry a keystone and a negative case each; on rc.3
  the keystones were TS2305/TS2554 or checker-silent.
- `invalid-refresh-target` changed only a message (`Readonly<…>` is gone from
  the printed store type); its code, TS2345, did not.
