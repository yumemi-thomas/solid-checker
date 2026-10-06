# ADR 0217: Leaf scopes reach helpers, namespaces and more host callbacks

- Status: accepted and implemented (2026-10-06). Drafted by a research agent
  from the leaf-scope census (`rust/target/research/leaf-census.md`), then
  integrated and measured here.
- Owners:
  - the demand for argumentless calls in local helpers a leaf callback reaches
    (`solid-facts-backend/src/demand_plan.rs`, `lib.rs`);
  - exact namespace-import member calls in the leaf walk
    (`solid-reactive-ir/src/cleanup.rs`, `indexes.rs`);
  - five host timing rows (`solid-reactive-ir/src/runtime_semantics.rs`).
- Relation: extends ADR 0179, 0192, 0209 and 0210.

## Decision

1. **Argumentless calls in reached helpers are resolved.** A local helper that
   a leaf callback calls is walked, and its argumentless member calls
   (`text.trim()`, `new Date().getTime()`) now get the resolved call that
   tells a standard-library member from anything else. Before, only calls
   written inside the leaf callback had it.
2. **A namespace import's member is its export.** `utils.f()`, where `utils`
   is the namespace import binding itself, follows the exactly resolved
   exported function. The function must be a declaration or a `const` with a
   direct function initializer. A computed member, an alias of the namespace,
   a mutable export, or a wrapped export keeps the obligation.
3. **Host timing rows**, each matched by its compiler-selected declaration
   and cited to its specification:
   - `PromiseConstructor.construct` argument 0 is an inline callback. ECMA-262
     `Promise(executor)` step 10 calls the executor before the constructor
     returns.
   - `NodeList.forEach` and `NodeListOf.forEach` argument 0 are inline
     callbacks (Web IDL iterable methods install `Array.prototype.forEach`).
   - The global `addEventListener` argument 1 and `MediaQueryList.addListener`
     argument 0 are deferred listeners, like every other default-library
     listener (ADR 0210: synchronous dispatch is not modeled).
   - `MediaSession.setActionHandler` argument 1 is a fresh-stack callback.
     Media Session queues a task before it invokes the handler.

The draft also proposed keeping the two new listener rows open unless their
callback is proven clean. That is stricter than ADR 0210 treats every other
listener, so it was left out to keep one dispatch policy.

## Consequences

- A forbidden operation in a `Promise` executor or a `NodeList.forEach`
  callback in a leaf scope is a proven violation.
- Still open:
  - imported helpers' own argumentless calls (the demand is local);
  - a callback wrapped in a type assertion and passed to an inline host
    callback (`WrappedNodeListCallback`);
  - the executor's `resolve`/`reject` parameters.

## Evidence

- **Fixtures:**
  - `leaf-scope-helper-demand`, `leaf-scope-host-timing` and
    `leaf-scope-namespace-export` are new, each with positives, negatives
    and shadowing, alias, computed and mutable cases;
  - in `leaf-scope-host-callbacks`, `UnauditedCallback` (a `Promise` executor
    calling `onCleanup`) is now a proven violation.
- **Coverage:** 188 fixture projects. Only these move.
- **rc.13 corpus**, browser host, release binary, against
  `rc13-z-browser.json` (ADR 0216):
  - violations unchanged at 195;
  - uncertifiable 3,422 to 3,397, with 25 removed and none added: media
    session handlers, `matchMedia` listeners, global listeners, local
    helpers and namespace calls in leaf callbacks.
