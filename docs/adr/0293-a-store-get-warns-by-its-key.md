# ADR 0293: A store Get warns by its key

- Status: accepted and implemented (2026-10-13).
- Owners:
  - `Dialect::store_key_may_skip_strict_read` (the key premise; `Solid2`
    answers `key == "then"`);
  - `local_access::store_head_get` (the head-Get decision), consulted by the
    direct store member loop in `local_access.rs` and by the store-member
    summary contributions in `interproc.rs`;
  - the `STORE_KEY_UNDECIDED` wording in `projection.rs`.
- Extends the rc.13 strict-key premise (`Dialect::store_key_warns_strict_read`,
  already required by the async-prefix and incoming-prop proofs) to the store
  Gets that were witnesses before that premise existed.

## Context

Both of these are `tsc --noEmit` clean against published rc.13 (`strict`,
`skipLibCheck: false`), and both were proven `SC1001` violations whose hint
says "Solid warns STRICT_READ_UNTRACKED here in dev":

```tsx
const [q] = createStore({ then: () => {}, data: 1 });
q.then();                  // component body
const key = Symbol("user");
const [s] = createStore({ [key]: 1 });
const value = s[key];      // component body
```

Neither warns. The rc.13 store trap (`@solidjs/signals` `dist/dev.js`
6660-6675) warns only when `typeof key === "string" && key !== "then"`;
rc.9's gate (6229-6235) is the same. A promise resolution probes `.then`, and
that is not a read the user wrote. rc.3's gate (7662) has no `then` clause.

The gate does not test ownness or value type. An own function member
(`q.refetch()`) reads the slot through `serveDataKey`, subscribes under
tracking and warns in a component body, so function members stay violations
(research `store-fn-and-typed-accessor.md`, runtime probes).

`store_key_warns_strict_read` already answered this for rc.13, but only new
proofs asked it. It is gated on the audited release, so it answers `false`
for every key under rc.3 and rc.9. Using it at the direct Get would have
removed every store violation for those releases.

## Decision

1. A new dialect question, `store_key_may_skip_strict_read(key)`, asks the
   refusal half: may a Get of this known string key skip the warning other
   string keys give? `Solid2` answers `key == "then"` in every vocabulary.
   That is exact for rc.9 and rc.13 and over-withholds for rc.3. The trait
   default answers `true`, so an unaudited dialect proves no direct store Get.
2. A store member chain's warning is its **head** Get's: it runs first, on
   the store proxy. `store_head_get` walks to the head and reads its key the
   way the existing paths do (the parser's cooked key for a literal computed
   member, the property text otherwise). The result is:
   - a known string key the dialect does not exempt: **warns**. The chain is
     reported as before, whatever follows (`q.nested.then`);
   - an exempt key heading a single Get: **exempt**, no read;
   - no known string key (a symbol, a dynamic expression), or an exempt head
     followed by another Get (`q.then.value`, which warns only if the value
     is a store proxy): **undecided**.
3. The direct store member loop in `local_access` drops an exempt Get. An
   undecided Get in a role that reports an untracked read is an `SC9012`
   obligation with the `store-key-undecided:` context, never an `SC1001`.
4. Interprocedural store-member contributions (`direct_reference_contributions`)
   keep only warning heads. Exempt and undecided Gets are withheld there:
   a summary row has no obligation channel, and a row would become a proven
   read at the caller.

Not changed:
- `no-destructure` (`SC1003`) on `const { then } = q`. Its claim is that the
  binding is frozen, which holds for `then` too.
- Store spreads, async reads (`SC5001`), and setter rules.

## Consequences

- A symbol key is undecided, not exempt. No fact gives the key expression's
  type, so `q[userKey]` is an obligation, though rc.13 never warns there.
- A template literal key (`` sessions[`draft:${id}`] ``) is undecided, though
  it is a string that is never `then`.
- A summary through a helper that reads an undecided key is now silent where
  it was an uncertifiable `SC1001`. That is a recall loss.
- An alias (`const alias = q; alias.then()`) is not followed by the store
  analysis at all, so it stays silent.

## Evidence

- Fixture `reactive-ir/store-strict-read-keys`: 6 `SC1001` violations
  (`OwnFunction`, `ForwardedFunction` through ADR 0287's proxy, `DataMember`,
  `KnownComputedKey`, `NestedThen`, `ClosureData`). `ThenRead`,
  `ComputedThen`, `ForwardedThen` and `ClosureThen` have no `SC1001`. 4
  `SC9012` obligations (`SymbolRead`, `UnknownComputedKey`, `ThenHeadChain`,
  `SymbolHeadChain`). Before the change, all 8 were `SC1001` violations
  (`ClosureThen` through the closure's summary).
  `tsc --noEmit` is clean against the stub and against installed rc.13.
- Coverage: `async-call-prefix` moved. `StoreProtocolKeys`' `then` default
  (an uncertifiable `SC1001`) is gone, its symbol and dynamic defaults are
  `SC9012` instead of uncertifiable `SC1001`, and the three
  `…Body` summary rows are withheld.
- rc.13 corpus, release binary, browser host: violations unchanged in base
  mode (562, identical rows) and with `--dependency-source '*'` (560,
  identical rows). Uncertifiable 3,737 → 3,735 and 3,288 → 3,286, all in
  derp-media-server: three summary rows over template-literal and dynamic
  keys of its `sessions` store are withheld, and `sessions[key]` in
  `hermes-session-store.ts:441` is a new `SC9012` obligation.

## Review round 1

Two silent losses, now uncertifiable results:
- **An exempt `then` Get erased its backing property's code.** The trap
  exempts its own warning, then evaluates `src[key]`, and rc.13 serves an
  accessor key through `Reflect.get`, so a `get then() { return count(); }`
  in the store's literal runs its read in the component body. A `then` Get
  is now withheld only when `store_get_decision` proves the property runs
  nothing: the store is a `const [store] = createStore(literal)` in the same
  file whose setter slot is absent or never referenced, the literal is closed
  (data properties only), and `then` is absent from it or holds a primitive,
  object or array literal, or a function literal with no call, member,
  reference, spread, tagged template, JSX or `this`/`super`/`import`
  (`function_literal_runs_nothing`). No file may write a member named `then`,
  write through a prototype, or spell `defineProperty`, `defineProperties`
  or `__defineGetter__` (rc.13's store clone preserves accessors, so a store
  write is not assumed to install data). Otherwise the Get is undecided:
  `SC9012` where it is written, an undecided summary row in a helper. The
  cross-file digest binds the project-wide vetoes.
- **Undecided keys vanished from helper summaries.** `function read() {
  return q[key]; }` called in a component body was silent. A summary row now
  carries `undecided`: it propagates like any row, is an uncertifiable read
  at every caller (sorted after a proven row of the same symbol, so
  `q.data + q[key]` stays a violation), and no contract publishes it as a
  read or a returned accessor.

Fixture: `store-strict-read-keys` gains `Review.tsx`: `GetterThen`,
`UnprovenThenBacking`, `CalledReadingThen`, `WrittenStoreThen` are
`SC9012`; `ThenData`, `AbsentThen` stay silent; `HelperUndecidedKey` and
`ClosureUndecidedKey` are uncertifiable `SC1001`; `HelperMixedKeys` stays a
violation. `ForwardedThen` (a store declared in `hook.ts`) is now `SC9012`.
`async-call-prefix`'s `StoreProtocolKeys` moved: its literal has a computed
key, so `thenBody` is an uncertifiable `SC1001` and `thenDefault` an
`SC9012`, and `symbolBody`/`dynamicBody` are uncertifiable `SC1001` again.
rc.13 corpus, release binary, browser host: violations unchanged in both
modes (562, identical rows); uncertifiable 3,735 → 3,738 and 3,284 → 3,287,
the three derp-media-server `sessions` rows this ADR had withheld.
