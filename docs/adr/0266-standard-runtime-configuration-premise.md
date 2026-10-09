# ADR 0266: standard runtime configuration premise

- Status: accepted owner decision; implemented (2026-10-09).
- Owners: the Dialect question, Solid 2's exact declaration catalog, and
  Reactive IR's visible veto, retained-program identity and projection.

The checker adopts the explicit conditional premise **standard runtime
configuration**: the analyzed program runs with no `enableExternalSource`
configuration, no installation of any `DEV.hooks` callback (`onOwner`,
`onGraph`, `onUpdate`, `onStoreNodeUpdate`), and no `OBSERVE.exclude` marking.
This is the same class of assumption as unpatched built-ins in ADRs 0167,
0190, 0211 and 0262. Absence of a source veto is not a proof of absence.

There are exactly two states: **Assumed** (default) and **Vetoed(site)**.
Unresolvable references, missing symbols/declarations/import attestations,
synthetic intrinsics, alias cycles and ambiguous/merged identities do not
veto or demote anything. They are outside the premise, as are dependency
internals, excluded host code, reflective patching, bundler injection and
prior/HMR configuration. No third knowledge state or incomplete census exists.

## Runtime authority

Retained published `2.0.0-rc.13` bytes are under
`rust/target/primitives-checkpoint/misuse/static-store-createStaticStore-primed-top-level-read/node_modules/`.

- `@solidjs/signals/dist/dev.js:238-260` installs external-source
  configuration; `:217-227` hands the original compute to its factory and
  replaces execution with `source.track`; `dev-shared.js:5650` wires it.
  `dist/types/core/external.d.ts:1-14` declares the factory/configuration.
- `@solidjs/signals/dist/dev-shared.js:413,572-581` exposes the mutable hooks.
  Their invocations are owner creation `:3440,5648`, graph registration
  `:755` (before numeric-signal construction returns, `dist/dev.js:2352-2354`),
  queue-flush tail `:2152`, and store write diff `dist/dev.js:5806,5813,5817`.
- `@solidjs/signals/dist/dev-shared.js:524-526` implements exclusion;
  `:630-650` suppresses diagnostic delivery/reporting. Its declaration is
  `dist/types/core/dev.d.ts:323-338`.

These falsifiers motivated earlier strict-read, push-signal and hook research.
Neither primitive return values nor unpatched built-ins remove them.
The public typings admit the fixture uses; the checker reports no type error.

## Positive visible veto

Use only existing Oxc binder and Type Facts evidence over analyzed source.
Resolve exact queried spans, follow canonical binder aliases, require one
declaration, and ask the selected Dialect for the exact declaration-file and
name-node span. Never recognize a source identifier by spelling, module text,
smallest contained symbol, guessed member dispatch or suffix catch-all.

The Solid 2 rc.13 identities (package-relative name-node byte spans) are:

- `solid-js/types/index.d.ts`: OBSERVE `2480..2487`, DEV `2531..2534`;
- `@solidjs/signals/dist/types/index.d.ts`: OBSERVE `1010..1017`, DEV `1148..1151`;
- `@solidjs/signals/dist/types/core/dev.d.ts`: OBSERVE `22529..22536`, DEV `22966..22969`;
- `@solidjs/signals/dist/types/core/external.d.ts`: enableExternalSource `457..477`.

Named re-export spellings and namespace members follow these identities,
including solid-js's re-export of signals' installer. The published web type
surface supplies no additional DEV/OBSERVE/enableExternalSource export
identity; references reached through a barrel still require the exact listed
declaration. Existing owning-manifest attestations normalize npm aliases and
linked roots; a canonical declaration without a catalog identity is simply
outside this premise. Installed runtime bytes are not authenticated by this
model (CONTEXT.md, built-in runtime model).

A positively identified **call** of enableExternalSource vetoes. Merely
importing or reading its function value does not. Any positively identified
runtime value reference to DEV or OBSERVE vetoes, including Solid.DEV,
aliasing, destructuring, Object.assign/defineProperty, mutation and escape.
One exception: a named import whose every reference only tests truthiness
(`if (DEV)`, a ternary test `DEV ? a : b`, or the left operand of
`DEV && f()`) does not veto, since none of these can reach a hook or
exclusion. Any other reference in the project, such as `DEV.hooks` inside
that same `if`, still vetoes. Unrelated-member inspection and other value
reads veto: the veto is small and conservative instead of a mutation/escape
analysis. A veto is withholding certification, not asserting that a hook was
installed.

Measured: before the exception, the rc.13 corpus's only veto was app-game's
`if (DEV)` dev-only guard, which demoted 39 violations in `apps/web`.

Type-only imports and typeof **type queries** never veto. Existing import
runtime_referenced evidence excludes erased uses; recorded type-query spans
exclude namespace members in those queries. Runtime typeof DEV is a value
reference and does veto. Intrinsics, unrelated packages and dynamic loads
alone have no effect. No whole-program reference demand, namespace escape
veto, unrelated-symbol reference fetching or devtools denylist is added.

Veto scope is project-wide, including analyzed unimported setup sources.
Every projected violation becomes uncertifiable, keeps its rule/code/primary
location, replaces assertive wording, drops fixes, and carries the veto site
as evidence and a related location. Existing uncertifiable findings stay
unchanged. Assumed projection is a no-op: all finding bytes remain identical.
The premise is scanned once per build entry and included in retained-program
identity. Cross-file setup edits advance existing fact-generation/source
identity and recompute the veto; even a same-generation Assumed/Vetoed
transition refuses the retained hit. No content-hash guarantee for
same-generation edits beyond the existing fact identity is claimed.

## Limits

Unresolvable/reflective configuration, unknown dynamic loads, alias/member
flows for which the existing analysis supplies no exact export identity,
linked transitive declarations without owning evidence, dependency installers
and excluded files remain outside the explicit assumption. A project using
those channels must ensure standard configuration itself. Missing evidence
does not turn this runtime premise into a closed semantic claim.

This premise grants no transition/history authority, hydration entry/success,
no-throw guarantee, callback completeness, lifetime or lookup preservation.
Diagnostics/attribution listeners, snapshots, error hooks, comparators and
descriptor traps still need separate handling where a proof depends on them.
No research promotion or package-contract claim lands here.

## Measured consequences

- Twenty process scenarios with byte-faithful rc.13 typings: every veto case
  (calls, renamed calls, `DEV.hooks` assignment, aliases, destructuring,
  helper writes, namespace members, casts, `OBSERVE.exclude`, escape) turns
  the fixture's strict-read violation uncertifiable. Every control keeps it:
  homonyms, type-only imports and typeof queries, an unrelated package,
  intrinsics, dynamic imports, a mere read of `enableExternalSource`, and
  truthiness-only `DEV` tests.
- All 218 existing fixtures: zero findings moved.
- Primitives ledger: unchanged (98 of 111 browser).
- rc.13 corpus: no violation or uncertifiable site moved. Without the
  truthiness exception, app-game's `if (DEV)` guard vetoed `apps/web` and
  demoted 39 violations.
- Review: one major fixed before landing. The outer diagnostic cache now
  includes the premise state, so a same-generation change in the evidence
  cannot serve a stale result.

## Rejected: a third, Unknown state

The first draft withheld the premise whenever its census was incomplete. The
triggers included intrinsics without declarations (`undefined`,
`globalThis`), namespace imports, unrelated packages and dynamic imports.
Built, it demoted violations in 109 of 218 fixtures, including a plain
`import { createSignal } from "solid-js"`. Missing evidence is outside this
premise, as it is for unpatched built-ins; it never demotes.
