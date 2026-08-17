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

## Files

| File | What it is |
| --- | --- |
| `packages.json` | The audited package versions per dialect, plus the `jsxImportSource` each needs. `v1` tracks `pkg/contracts/bundled/solid-v1/solid-js.json`; `v2` tracks `pkg/contracts/bundled/runtime-lock.json`. |
| `rule-cases.json` | The executable half of the redundancy ledger: one case per rule whose positive spelling could plausibly also be a `tsc` error, with the expectation it must satisfy and a written reason. |

The prose half of the ledger — every rule, its classification, and the actual
`tsc` output as evidence — is in `docs/precision-backlog.md`.

## Using it

~~~sh
# install the audited typings (writes to rust/target/tsc-oracle, a build dir)
node scripts/tsc-oracle.mjs provision --dialect all

# is this snippet already a type error?
node scripts/tsc-oracle.mjs check --dialect v2 --code 'import { createEffect } from "solid-js"; createEffect(() => 1, () => 2);'

# enforce every declared case (also runs inside scripts/verify.sh, and as `make tsc-oracle`)
node scripts/tsc-oracle-gate.mjs

# what does the oracle actually say, per case -- for writing a ledger entry
node scripts/tsc-oracle-gate.mjs --report
~~~

Provisioning needs network access once per version bump. An absent or
version-drifted install is a **hard failure**, never a skip: a silently skipped
oracle is the same trap as a `cargo test` run without `SOLID_TYPEFACTS_BIN`.

## Adding a rule

Before adding or keeping a rule, write its positive case here with
`expect: "silent"` and run the gate. If TypeScript speaks, the rule is
TypeScript's — narrow it to the spellings the type system does not cover, or
do not add it. Better wording, an autofix, "the user may not run `tsc`", and
"the project may not be `strict`" are explicitly not exceptions; the gate runs
a non-`strict` pass precisely so the last one cannot be argued.

The one escape hatch is `distinct-claim`: TypeScript reports something, but
about a *different* defect than the finding asserts. It requires the diagnostic
codes and a reason naming what the finding claims that the type error does not.

## Bumping a version

Do not float. Update `packages.json`, re-provision, run the gate, and expect
`removed-because-redundant` cases to be the ones that move — those pin the
diagnostics that justified deleting a rule, so if a Solid release loosens a
type the gate fails and the removal gets reconsidered rather than quietly
leaving a hole.
