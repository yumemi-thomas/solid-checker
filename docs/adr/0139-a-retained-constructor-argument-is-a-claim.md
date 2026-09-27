# ADR 0139: What a constructor does with a callable it keeps is a claim the format cannot state yet

- Status: **proposed — owner decision required** (2026-09-28). Nothing here is
  implemented. It changes the contract format, so the rule of AGENTS.md applies:
  every producer, consumer, bundled contract, fixture, proof sidecar, receipt and
  gate moves together, or none does.
- Date: 2026-09-28
- Relation: follows ADR 0134 (class attribution) and ADR 0135 (helper reach). It
  extends ADR 0023 (retention is not invocation) and ADR 0105 (a class export is
  constructed).

## Context

After ADRs 0132 to 0137, `@tanstack/solid-router@2.0.0-rc.8`'s root node in the
`viviana-ui-main-b005c00a` environment still has 5 obligations that mark every
export. The replay is exact: the observed graph-lane node is regenerated
against the dependency catalog it was handed. Two of the five sit at the
`createNonReactiveMutableStore` and `createNonReactiveReadonlyStore` import
bindings in `routerStores.js` (bytes 9–38 and 40–70). Their only uses are in the
private arrow `getStoreFactory`. That arrow's only use is an argument:

```js
// solid-router dist/esm/router.js
var Router = class extends RouterCore {
  constructor(options) {
    super(options, getStoreFactory);
    if (!(isServer ?? this.isServer)) primeRouterFromRegistry(this);
  }
};
```

When `getStoreFactory` runs, and inside whose call, depends on what
`@tanstack/router-core@1.171.22`'s `RouterCore` constructor does with it. The
published bytes (`dist/esm/router.js`) do two things with it:

```js
constructor(options, getStoreConfig) {                    // line 85
  …
  this.update = (newOptions) => {                         // line 97
    …
    if (!this.stores && this.latestLocation) {            // line 137
      const config = this.getStoreConfig(this);           // line 138
      …
  this.getStoreConfig = getStoreConfig;                   // line 625
  this.update({ defaultPreloadDelay: 50, …, ...options, … }); // line 626
  if (!(isServer ?? typeof document === "undefined")) self.__TSR_ROUTER__ = this; // line 638
}
```

- The constructor **calls it during construction**, through
  `this.update(…)` → `this.getStoreConfig(this)`. The call is guarded by
  `!this.stores && this.latestLocation`.
- The constructor **keeps it** as a public, writable instance property, and
  every later `update()` call can invoke it. solid-router makes such a call
  itself, `router.update({...})` in `RouterContextProvider`
  (`RouterProvider.js:13`), and wraps it in `Solid.runWithOwner(null, …)`. So
  the stores `getStoreFactory` creates (`createRoot`/`createMemo`) can be
  created there, with no owner.

### Why router-core's contract says nothing about `RouterCore`

The accepted router-core document gives `RouterCore` and `BaseRoute` the
degenerate summary `{"call":{}}`, and 71 of its 79 exports get the same.
Nothing refused: the node's refusals file is empty, and none of its 774
unresolved claims comes from a census or a recipe. They are attribution
sentinels. Replaying router-core's own node shows two `fallback-all`
obligations in router-core's `router.js`. Both are `@tanstack/history` import
bindings:

- `createBrowserHistory` (bytes 1087–1107, `unknown-contract-claims:ownerRequirements`);
- `parseHref` (bytes 1109–1118, `returns,ownerRequirements`).

Both are used only inside closures that `RouterCore`'s constructor creates.
ADR 0134's class rung refuses them, for two independent reasons:

- **§ 3.** `RouterCore.prototype._replaceRouteChunk = …` and
  `RouterCore.prototype._refreshRoute = …` (lines 825–826, the non-production
  HMR augmentation) are references to the class that are neither an export, a
  `new` callee, nor a heritage.
- **§ 5.** The instance escapes: `self.__TSR_ROUTER__ = this`,
  `setupScrollRestoration(this)`, `loadClientRoute(this, opts)`.

Both obligations therefore mark every export in all five domains, `callbacks`
included.

### What the format can say today

- **Construction.** It is modelled: a class export's `call` summary is its
  construction (ADR 0105).
- **Called during construction.** It is expressible as a `callbacks` item
  `{from: {arg: 1}}` whose operation is
  `at: {event: call, schedule: same-stack}`. The census proves such an item
  only in the export's own frame, outside every nested callable (ADR 0100
  rules 4 to 6). `RouterCore`'s call is `this.getStoreConfig(this)`: a member
  callee, inside a closure, reached through `this.update`. So even the
  construction half cannot be certified.
- **Kept.** It cannot be stated at all. ADR 0023 makes storage *open*
  `callbacks` knowledge. The census walks calls only, and "retaining a
  callable … on a returned object is not one" (`census_callbacks_domain`). A
  closed `callbacks` therefore does not deny that a stored argument is invoked
  later. No item says the argument is kept only by the value the call returns.

So the fact ADR 0134 would need cannot be written into a contract: invoked here
⇒ construction; kept by the instance ⇒ creators' `returns`. That is a missing
claim form, not a missing census or recipe.

## Proposal (for the owner)

The smallest additive form is **one execution-point event**.

1. **Format.** Add `result-access` to the `event` vocabulary, valid only on an
   `invoke` named by a `callbacks` item `from` a bare parameter, with
   `schedule: external`, `ambient-at-execution`, count `trigger 0..many`, and
   no guard. Its meaning: *this invocation stores the callable only in the
   value it returns (for a construction, the instance), and the callable runs
   later, on the stack of code that reaches it through that value.* A
   `callbacks` domain closed over `call` items and `result-access` items then
   denies every other retention: a registry, a module-level variable, a queued
   task, an external event. Such a document hashes in a digest family of its
   own, and a decoder that predates the event refuses the document. This is the
   same precedent as `protocol` (Decision 2026-09-24), so every other contract,
   and every recipe address, stays unchanged.
2. **Producer census** (Type Facts). A parameter qualifies when each of its
   uses in the constructor's own frame is either an ADR 0100 direct call, or a
   store `this.<literal key> = p`. Across the class body and every
   `C.prototype.<m> = …` in the module, `<key>` must also be written nowhere
   else, and every read of `this.<key>` must be the callee of a call whose
   arguments do not include the callable. Anything else refuses by name. A
   construction-time invocation reached through a member (RouterCore's case)
   is published as a `call` item only once the census can follow `this.m` to
   exactly one closure. Until then, `result-access` without a `call` item is
   stated only where no member read of the key runs at construction.
3. **Consumer (ADR 0134 § 6).** A private function whose every reference is
   argument `i` of `super(…)` in a module-level class `C` whose heritage is an
   import of dependency export `D` reads `D`'s accepted summary:

   | `D`'s answer | Attribution |
   | --- | --- |
   | `callbacks` closed, only `call` items from `i` | construction of `C` |
   | only `result-access` from `i` | `returns` of `C`'s creators |
   | both | construction |
   | open, absent, degenerate | `fallback-all` |

   It also refuses when `C` declares any member other than its constructor,
   because an override changes what the base's `this.m(…)` runs.

## What it would move, and what it would not

Measured on the replay: 5 catch-alls, unchanged, and 91 of 97 root exports
degenerate. The proposal alone moves none of them. `RouterCore`'s `callbacks`
is also opened by router-core's own `createBrowserHistory` obligation, which
runs during construction, so it closes only after those two obligations are
attributed exactly. That needs one of two changes:

- ADR 0134 accepts `C.prototype.m =` augmentation, together with an instance
  that escapes during construction only;
- `@tanstack/history` closes `ownerRequirements` for `createBrowserHistory`
  and `parseHref`.

## A gap found in ADR 0134 while reading this

`classify` returns `InstanceMember` for a closure the constructor creates
(`this.update = (…) => …`), and for any method. It does so even when the
constructor itself calls that member (`this.update(…)`, line 626). ADR 0134 § 2
then opens only the creators' `returns`, but the obligation runs during the
creator's own call. The premise of § 2, "not during the exporting call", does
not hold for that shape. The narrowing is unsound in the direction that
publishes a closed domain, so it is recorded here for the owner of ADR 0134.

Fixed by the amendment of 2026-09-28 in ADR 0134.
