# ADR 0268: standard hydration host

- Status: accepted owner decision; implemented (2026-10-09).
- Amends: ADR 0266, standard runtime configuration premise.
- Owners: shared Dialect question, Solid 2 declaration/read catalog, Reactive
  IR's positive visible veto and existing project-wide projection.

## Decision

Add a fourth clause to **standard runtime configuration**: **the hydration
host is Solid's standard serializer runtime**. No custom `sharedConfig.load`,
`has`, or other hydration host callback may inspect owners or retain computations.
This is a conditional assumption, like unpatched built-ins and ADR 0266's three
existing clauses. It is Assumed by default and Vetoed only on positive source
evidence. There is no Unknown state; missing evidence never vetoes.

The standard host means the published Solid 2 rc.13 core hydration machinery,
DOM renderer hydration/bootstrap, and server renderer's default Seroval-backed
serialization, operating on the ordinary serializer payload/registries. Standard
`enableHydration`, `hydrate`, `render`, `renderToString` and `renderToStream`
entries are permitted. This is not a no-hydration premise, a proof of host
authentication, or a restriction on ordinary supported payload values.

SolidStart and @solidjs/web dependency installation of this standard runtime is
covered by definition. A dependency installing a custom host is outside the
premise, like ADR 0266's other dependency configuration: the project must ensure
the condition. Dependencies, excluded sources, bootstrap globals, custom SSR
serializer/plugin injection, reflective/dynamic installation, HMR and prior
configuration are not authenticated or exhaustively scanned here. In particular,
this object-export veto does not analyze `renderToStream`'s serializer option or
Seroval plugin bodies. Their behavior remains a separate proof obligation.

## Motivation and runtime authority

Research `rust/target/research/union-premise/NOTES.md` and
`rust/target/research/rebase-polled/NOTES.md` demonstrates owner inspection,
retained computation replay and callable substitution through custom `has/load`
under id-seeded roots, without any of ADR 0266's original configuration channels.
The new clause excludes that custom host conditionally; it promotes no recipe,
timer contract, callback-completeness claim or browser admission.

Let D be
`rust/target/primitives-checkpoint/misuse/set-union-top-level-read/node_modules/`.
All citations below are retained published rc.13 bytes under D.

- `solid-js/dist/solid.js:40-44,182-200,461-465,703-767` holds the mutable host
  and invokes has/load during hydration. `:908-976` installs standard id/state
  callbacks and hydrating/done descriptors; development adds the peek callback
  at `solid.dev.js:956` and hydration verification at `:136`.
- `@solidjs/web/dist/web.js:1353-1435` enables hydration and installs completed,
  events, load, has, gather, asset loading, fragment cleanup, registry and
  boundary capture. The standard load/has are `:1371-1372`: payload lookup and
  membership in `globalThis._$HY.r`. This runtime reads claimRoots at `:1535`.
- `@solidjs/web/dist/server.js:444-475` builds the default Seroval Serializer
  for `_$HY.r`; `:1471-1533,1570,1791,1945-2018` installs the SSR context and
  serialization. Custom plugins and the streaming serializer option exist;
  their behavior is not granted standard-host authority by a closed claim.
- Full optional member shapes: `solid-js/types/internal.d.ts:83-138` and
  server context `types/server/shared.d.ts:24-98`. Every member write is subject
  to the veto, including future/undeclared names reached through an exact host.

## Exact positive veto

Add `RuntimeConfigurationApi::HydrationHost` and exact name-node identities:

- `solid-js/types/internal.d.ts:138`, bytes **7472..7484**;
- `solid-js/types/server/shared.d.ts:98`, bytes **4285..4297**.

The public internal subpath uses the former. Server re-exports and
`@solidjs/web/types/server.d.ts:218` resolve back to the declared object; no new
identity for their source spelling is inferred. The main solid-js rc.13 typings
and client web typings do not export sharedConfig. @solidjs/signals owns no
sharedConfig or hydration-host setter. Standard enableHydration is an installer,
not a custom-host setter, and must not itself veto.

Reuse ADR 0266's exact entity span, canonical binder alias, single declaration,
and existing owning-manifest alias/linked-root normalization. Runtime import
bindings, exact namespace members and named re-exports identify the host.
After positive identification, any runtime reference vetoes unless positively
classified as harmless. This catches any member write (including computed,
compound, destructuring, loop and delete), alias, destructure, Object.assign,
Object.defineProperty, escape, object/member transport or callback invocation.
No Object/Reflect spelling is trusted: their host argument itself vetoes.
The scanner asserts no installed callback; a veto withholds certification.

The dialect permits direct static **reads** of hydrating/done, whose standard
getters return boolean data, and truthiness-only tests of the object or context:
if/ternary tests, left operand of &&, and logical negation, including transparent
TypeScript wrappers. Context extraction, nested context reads/calls, other
members and computed reads veto conservatively. Mutation facts exclude writes
before applying the data-read exception. Every binder reference must be harmless;
a guard elsewhere cannot conceal a write or escape. Type-only imports and typeof
type queries are erased; runtime typeof of the object still vetoes. Same-named
locals, ambiguous declarations, missing/unresolved symbols and unqueried paths
add no evidence and preserve Assumed.

The existing once-per-build scan, retained-program identity, cache binding and
projection are reused. Vetoed projects retain rule/code/primary location, receive
uncertifiable wording and veto evidence, and lose fixes. Assumed projection is
byte-identical. No shared version switch, new finding rule,
contract document, receipt, producer demand or knowledge state is introduced.
The review correction adds exact simple write-target spans to structural facts
(facts schema 53), distinguishing writes from key/default reads.

## Acceptance and limits

The patch adds 22 focused process cases to standard-runtime-configuration, plus
exact catalog and syntax/binder unit regressions. All 22 sources pass TypeScript
5.9.3 strict/noEmit against published rc.13 typings, skipLibCheck=false.
The default fixture config excludes cases, preserving its baseline snapshot.

Measured: zero existing fixture findings moved, and no rc.13 corpus
violation or uncertifiable site moved. The only existing runtime fixture
source mentioning sharedConfig, rc9-reexport-gap-aliased, uses the undeclared
rc.9 main export, so it has no catalog identity.

**Known recall limit.** The published rc.13 `solid-js` typings do not export
`sharedConfig` from the main entry, though the runtime does. `import {
sharedConfig } from "solid-js"; sharedConfig.load = ...` therefore has no
exact identity and stays Assumed. That is missing evidence, which never
vetoes; closing it needs a reviewed exact runtime-export join or a published
typing fix, never name matching. Imports from `solid-js/internal` and the
`@solidjs/web` server declarations do resolve and veto.

## Review

One adversarial round, no blockers. The major issue is fixed here: a
harmless `hydrating`/`done` read inside the key or default of an unrelated
assignment target (`out[host.done ? 1 : 0] = 1`) was mistaken for a host
write and would have demoted the whole project. Exact simple write targets
are now an AST fact (facts schema 53), and the scan compares exact target
spans. A clippy item-order risk in the dialect file was also fixed. Loop
tests (`while (host.context)`) are not exempted and still veto.

Hydration entry/success, standard payload behavior, owner lifetime/replay,
transition/history, comparators, diagnostics/attribution and callback completion
remain independent obligations. Unknown configuration channels stay outside the
assumption; their silence is never a closed proof of standard-host behavior.
