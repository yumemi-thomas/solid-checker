# ADR 0267: conditional captured dictionary lookup

- Status: accepted and implemented (2026-10-09).
- Owners: the authored `capturedLookup` vocabulary (model, validation,
  canonical digest and recipe binding, certification refusal, wire, schema,
  authoring citations and probe binding); its consumer in contract
  resolution; the `@solid-primitives/i18n@3.0.0-next.4` spec.

`translator(dict)` returns `t`; calling `t("hello")` reads the dictionary
accessor and returns the own string under that key. Read in a component body,
that read is untracked, and Chrome on rc.13 warns STRICT_READ_UNTRACKED. The
checker could not see through `t`: the captured dictionary call and its
dynamic Get stayed an open dispatch obligation.

An authored spec can now state a conditional captured lookup: the returned
callable reads its captured dictionary in the caller's execution context and
selects an own string key. The consumer accepts only:

- a dictionary made by a fresh native `createSignal` with an object-literal
  initializer, no options, and its setter discarded;
- exact binder references, with no escape of the dictionary or of `t`;
- construction and every direct `t(...)` call in the same synchronous lexical
  scope, with no callback-supplied scope, async or generator scope, deferred
  use, or extra argument;
- an exact own string key present in the literal, with no default.

Everything else stays `reactive-dispatch-unresolved`: missing, inherited,
numeric-invocation or dynamic keys; getters, spreads, computed keys,
`__proto__` and proxies; custom resolvers or handlers; selected functions or
objects; mutable sources and wrappers.

## Premise and channels outside it

The proof consumes ADR 0266's standard runtime configuration explicitly: it is
granted only when `permits_proof()` holds, and a visible veto still demotes the
violation at projection. Channels outside ADR 0266 were checked separately:

- **Hydration:** an object initializer bypasses `hydrateSignalLike`. The
  review ran the real rc.13 modules under an id-seeded root with six
  hydration payloads (missing, object, callable, async-iterable, throwing
  `load`, throwing `has`); each warned exactly once.
- **Transition comparators:** the source's setter is never written, so no
  transition can stage it.
- **Diagnostics listeners:** the warning is emitted before any listener runs.
  This is not a no-throw claim.

The recipe also refuses positively identified uses of ten rc.13 exports that
expose nodes or alter reporting: `latest`, `isPending`, owner and observer
exposure, the diagnostic footer, snapshot controls and the root error hook.
This does not widen ADR 0266.

## Review

Two adversarial rounds. Round 1's blockers were exactly ADR 0266's three
channels. Round 2 found no accepted program without the warning, and one
major issue, fixed here: the cross-file proof digest now binds the resolved
lookup instances, so a `latest` import added in another file invalidates
cached graph and local-access fragments. Landing also fixed two missing
struct-literal fields, a clippy item order and a test expectation.

## Measured consequences

- Fixture `package-captured-lookup-consumer`: the body read is a violation and
  the tracked read is clean. Every refused shape is uncertifiable, as is the
  namespace-created dictionary (predicted a violation; the conservative
  direction).
- All three i18n pairs pass in Chrome on rc.13.
- Primitives ledger, browser: 99 of 111 report correctly (was 98):
  `translator`'s argument read. No correct twin has a violation.
- rc.13 corpus: no violation moved. app-game's one `translator` user no longer
  stops at the import; its dynamic-key calls are dispatch obligations (+8, -2
  uncertifiable sites).
