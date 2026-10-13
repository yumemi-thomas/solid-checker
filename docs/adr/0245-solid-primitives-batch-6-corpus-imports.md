# ADR 0245: Solid Primitives batch 6, for the corpus's imports

- Status: accepted and implemented (2026-10-08).
- Owners: the specs listed below and their probe pairs.
- Relation: steered by what the two rc.13 corpus apps that use primitives
  (app-game, readingroom) actually import. Drafted read-only in
  `rust/target/research/primitives-batch-6a/` and `-6b/`.

## Decision

- A new spec for `page-utilities@3.0.0-next.0` `createPageVisibility`.
  - It is audited against `next.0`'s own bytes, not carried over from
    `next.2`.
  - Its identity comes from an isolated install whose dependency environment
    matches the corpus copy, including `event-listener@3.0.0-next.3`.
- Domains closed with the current format:
  - `clipboard` `writeClipboard` and `newClipboardItem`;
  - `event-bus` `createEventBus`;
  - `event-listener@3.0.0-next.3` `makeEventListenerStack`, `preventDefault`
    and `stopPropagation`;
  - `resize-observer` `makeResizeObserver`;
  - `utils` `asArray`.
- All probe pairs pass in Chrome on rc.13.
- Not shipped:
  - `drag-drop@0.1.0-next.0` cannot load on rc.13: its public entry imports
    `solid-js/web`. app-game uses a patched copy, which would need its own
    spec.
  - `context`, `jsx-tokenizer` and `storage` could only take partial claims
    that leave the import notice in place.
  - `scroll` `createScrollPosition` was drafted and passed its probes, but it
    is **withdrawn**. Closing its reads exposed six false `strict-read-untracked`
    violations in app-game's `solid-virtual`. That code reads the store's `y`
    only inside an object getter passed to `merge`, which rc.13 keeps lazy
    (`@solidjs/signals/dist/dev.js:4443-4500`). The checker bug is under
    investigation, and the claim waits for its fix.
- Ten rows need the returned-function format extension
  (`rust/target/research/format-extension-2/`), and eighteen need other
  extensions. Even the ten need companion shapes, such as the callable
  `.clear` member of `debounce`/`throttle`.

## Consequences

- rc.13 corpus: primitive import declarations still raising SC9005 drop from
  32 to 28 of 43. No violation is added or lost.
- Browser ledger unchanged at 71 of 114. No correct twin has a violation.
