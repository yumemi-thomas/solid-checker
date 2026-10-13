# ADR 0251: Solid Primitives batch 7

- Status: accepted and implemented (2026-10-08), in two slices (7a, 7b).
- Owners: the twelve specs below, their probe pairs, and two ledger cases.
- Relation: the first wave of the second census
  (`rust/target/research/census-2/CENSUS.md`, items A-closures, A-timeout,
  A-discarded and A-upload). Drafted read-only in
  `rust/target/research/batch-7a/` and `-7b/`; current vocabulary only.

## Decision

- **7a, constructor censuses completed:**
  - `createEventStack`;
  - `createClipboard`;
  - `createPreventScroll`;
  - `createKeyHold` and `useKeyDownList`;
  - `createMutationObserver`;
  - `until`;
  - `createTimeoutLoop`.

  All 52 probe pairs of the seven packages pass in Chrome on rc.13.
- **7b, factories whose returned value callers may discard:**
  - `createElementSize`;
  - `createMs`;
  - `createPureReaction`;
  - `repeat` and `mapRange`;
  - `keyArray`.

  Function returns use `returned-callable` with the graph omitted (ADR 0249).
  `createElementSize` returns an object of four `unknown` members: its
  `createStaticStore` getters are lazy, and describing them needs the lazy
  getter extension. All probe pairs pass.
- **The two `upload` `createDropzone` ledger cases are removed.**
  `@solid-primitives/upload@1.0.0-next.4` cannot load on rc.13. Its root
  imports `createDropzone` (`upload/dist/createDropzone.js:5`), which imports
  `drag-drop`, whose `dist/context.js:3` imports `solid-js/web`, a subpath
  rc.13 does not export (`solid-js/package.json:30-108`). No upload entry
  avoids it. With no runtime behavior to check, a misuse expectation is
  meaningless. The checker reports the import as uncertifiable, which is
  right.

## Consequences

- Primitives ledger, browser: 88 of 112 report correctly (was 73 of 114).
  None: 7 of 113. No correct twin on any host has a violation.
- rc.13 corpus:
  - primitive import declarations still raising SC9005 drop from 24 to 21 of
    43;
  - no violation is added or lost;
  - uncertifiable rises by 26, almost all from 30 `createElementSize` member
    reads that are now `reactive-dispatch-unresolved` obligations instead of
    two import notices. The lazy getter extension is what turns those into
    proven results.
