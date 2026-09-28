# ADR 0139: What a constructor does with a callable it keeps is a claim the format cannot state yet

- Status: accepted and implemented (2026-09-28). The owner approved the
  proposal below on 2026-09-28; § Implementation records what landed, where it
  is narrower than the proposal and why, and what it measured. Handshake
  protocol 65.
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

## Implementation (2026-09-28)

### Format

`Event::ResultAccess` (`result-access`) is valid only as the trigger and the
execution point of an `invoke` exactly one `callbacks` item names from a bare
parameter, scheduled `external`, `ambient-at-execution` for tracking and owner
with unconstrained requirements, count `trigger` 0..many, unguarded, with no
protocol, inputs, output or resources (`validate_result_access_operation`).
A contract stating one writes `solid-checker:semantic-result-access:v1` first
(`SEMANTIC_RESULT_ACCESS_MARKER`), and a recipe address does per claim; every
other contract and address hashes byte for byte as before, pinned by a golden
vector. The schema, the wire decoder and `semantic-model.md` § callbacks carry
the event.

The denial is **per slot**: a `result-access` item from slot `i` denies every
other retention of slot `i`'s value. A closed domain with only call items from
a slot says nothing about retention, as it never did (ADR 0023). This is
narrower than the proposal's "a domain closed over call and result-access
items denies every other retention", which would have given every certified
document's call-only slots a meaning their census never checked.

### Producer census (Type Facts, handshake protocol 65)

`ExportImplementationTranscript.retainedArguments`, beside `invocation:
construct`, names each constructor parameter the producer found kept
(`retainedArgumentsLocked`): one top-level `this.<key> = p` statement of the
constructor; every other use a direct call in the constructor's own frame;
the key no class element, not `__proto__`, written nowhere else; every other
access of `this.<key>` the callee of a call outside the code a construction
reaches (the constructor less its installed closures and, transitively, every
member whose key reached code names). It answers only for an exact class: no
heritage, static member or nested class; every `this` in the class body the
object of a literal-keyed member access (no escape); no computed member of
`this`; no constructor return value; every member write storing a parameter, a
literal, an object or array literal, a `new` result, a template, a
`void`/`typeof`/`!` expression, or (top-level in the constructor) an installed
function literal -- never an identifier, call result or member read, which
could be a function whose body runs later as a member; and every reference to
the class in the program an export specifier, a `new` callee or a `void`
operand, so nothing augments (`C.prototype.m = …`), extends or passes it on.

Two producer facts widen with it, both true by grammar: a constructor's
completion form is `plain` (it was `unclassified`), and its parameters get the
unwritten-slot identity functions already had, so ADR 0100's call items can be
confirmed in a construction.

The certifier confirms each `result-access` item against the stated fact and
re-derives the parameter half from the transcript's use census: every use is
the stated store or a direct call in the constructor's own frame
(`retained_argument_evidence`). The same fact witnesses the item's positive
families (argument binding, callable path, operation reachability, and the
per-trigger cardinality). A class export's selected signature is its construct
signature. The veto constructs the export with `new` on that signature and
invokes no member of the value (`Observation::ConstructedCallbacks`), so a kept
slot that runs at any time up to the end of the drain is the contradiction.

What the proposal allowed and this does not state: a construction-time
invocation through a member (router-core's `this.update(…)` reaching
`this.getStoreConfig(this)`) -- the call item the census cannot yet follow to
one closure.

### Generator

`solid_facts::ast::retained_constructor_arguments` restates the producer's
rules on Oxc's resolved references for the class an entry name's runtime
binding publishes, and the backend describes each kept parameter as a
`result-access` item when every constructor parameter is kept or unused. It
derives no call item for a class, so a constructor that also calls the kept
parameter (`Primed`) proposes nothing; the census confirms that pair when it is
stated by hand. Re-emission republishes a projected item as itself. A local
caller that forwards its own parameter to a dependency's kept slot does not
inherit the item -- the caller may keep the constructed value anywhere -- and
its slot is opened instead.

### Consumer (ADR 0134 § 6)

`export_names_of_super_argument_obligation` answers after the class rung and
before `fallback-all`, for an obligation in a module-level function `F` (or on
an import binding used only inside it) whose every reference in the package is
an argument `i` of the top-level `super(…)` statement of a module-level class
`C` extending a named import of dependency export `D`, in `F`'s module or
through import bindings used only that way, with no entry name publishing `F`:

| `D`'s accepted `callbacks` for slot `i`, and `C` | Attribution |
| --- | --- |
| closed, `result-access` item, no call item; `C`'s constructor names no member of `this`/`super` | `super-argument-member`: creators' `returns` |
| closed, `result-access` item and a call item, or the constructor names a member | `super-argument-construction`: creators, every domain |
| closed with call items only, no item, a member path, open, absent, degenerate | `fallback-all` |
| `C` declares any member but its constructor, or its instance escapes | `fallback-all` |

The first column's third row deviates from the proposal's "only call items ⇒
construction": without a `result-access` item nothing denies that `D` keeps `F`
somewhere another export later runs it. The creators are the class rung's own
for the `super(…)` call. What `D` does with values `F` returns is `D`'s code,
described by `D`'s contract in its own domains, which is the premise ADR 0134
takes for the instance itself.

### Pinned by

- `contract_semantics::tests::result_access_digest_family_is_separate_and_frozen`
  and `a_result_access_operation_is_validated_to_its_one_shape`;
  `contract_document::tests::a_result_access_item_round_trips_in_its_own_digest_family`;
  `contracts::tests::a_result_access_item_projects_as_a_deferred_row_that_keeps_its_event`;
- `TestAConstructionStatesWhatItKeepsForItsMembers` (Go, 17 rows);
  `session::tests::a_retained_argument_names_one_plain_slot_of_a_construction`;
- `retained_arguments::tests` and `super_argument::tests` (Oxc);
- `contract_certification::tests::the_retained_argument_census_certifies_exactly_what_members_keep`
  (end to end: `Keeper` and `Primed` certify with the veto, seven variants
  withhold by name) and
  `synthesized_vetoes::adversarial_tests::the_constructed_callbacks_module_samples_new_and_never_runs_a_kept_slot`;
- `fixtures/package-contracts/implementation-census-retained-argument` (corpus);
- `scripts/contract-super-argument-attribution.test.mjs` (the consumer rung,
  its construction row and four refusing variants).

Each rule was disabled once to check its pin: the consumer rung (the script's
`kept` and `touching` rows fail), the certifier's retained-argument evidence
and the constructed veto (the end-to-end test fails), and the producer's
escape check (`theInstanceEscapes` fails). The one unpinned rule is the
consumer's refusal to restate a dependency's item on a forwarding caller's
parameter: today a class export's `returns` and owner requirements are never
closed, so an importer of one is opened in every domain by that import and the
restated row could not show.

### Measured on `viviana-ui-main-b005c00a`

A fresh `consumer-environment` run with this build (release binary, network
install): `@tanstack/solid-router` certified with 106 closures (callbacks 88,
creates 34, returns 32), `solid-start-client` 98, `solid-start-server` refused
as before (`node:stream is not a package receipt`). No document in the run
states a `result-access` item: `RouterCore` is not an exact class (its
instance escapes, it hangs imported functions on itself, and its constructor
calls the member that calls the kept key).

Each node replayed against the catalog the graph lane handed it:

| node | `fallback-all` | degenerate exports | exports proposing a closure |
| --- | --- | --- | --- |
| router-core `.` | 2: `createBrowserHistory`, `parseHref` (unchanged) | 71 of 79, `RouterCore` among them | 8: `DEFAULT_PROTOCOL_ALLOWLIST`, `DEV_STYLES_ATTR`, `TSR_DEFERRED_PROMISE`, `defaultSerovalPlugins`, `preloadWarning`, `rootRouteId`, `storageKey`, `trailingSlashOptions` |
| solid-router `.` | 3: `routerStores.js` 9–38 and 40–70 (`getStoreFactory`'s bindings), `route.js` 465 | 91 of 97 | 6, all re-exports: `DEFAULT_PROTOCOL_ALLOWLIST`, `createBrowserHistory`, `createHashHistory`, `createHistory`, `createMemoryHistory`, `rootRouteId` |

`createRouter`, `createFileRoute`, `Link` and `Outlet` propose nothing. The
super-argument rung does not fire on `getStoreFactory`: `RouterCore`'s
`callbacks` is degenerate, and `Router`'s `primeRouterFromRegistry(this)` is an
escape it would refuse anyway. A binary predating ADR 0138 replaying the same
catalog reports 6 root catch-alls; the three `not-found.js` ones are ADR 0138's
(reachability), not this change's -- no attribution marker in either replay
names a super-argument mechanism.

### Router-core's own two catch-alls: no exact rule, nothing changed

`createBrowserHistory` and `parseHref` (`router.js` bytes 1087–1107 and
1109–1118) stay `fallback-all`. Both obstacles were examined for an exact rule:

- **Prototype augmentation** (lines 825–826) is exact for a function-literal
  value -- a member defined at module scope, whose body the reach can read --
  but `_replaceRouteChunk = replaceRouteChunk` hangs an *imported* function on
  the prototype, whose body runs later with `this` the instance, and
  `_refreshRoute`'s literal body hands `this` to `refreshClientRoute`. The same
  holds inside the constructor: `this.loadRouteChunk = loadRouteChunk`. A rule
  admitting only function literals moves neither obligation.
- **The escaping instance** has no exact rule. `this` reaches four package
  helpers (`setupScrollRestoration(this)` at construction, which subscribes and
  adds window listeners holding it; `loadClientRoute`, `preloadClientRoute`,
  `refreshClientRoute`), a global (`self.__TSR_ROUTER__ = this`, readable by any
  code), and the caller's own `getStoreConfig(this)`. "Creators' `returns` and
  construction, never narrower" covers code that runs inside a creator's call
  or on the value it hands out, but not a *different* package export that later
  reaches the instance through module state or the global and runs `update`
  inside its own call; that export would be left unmarked. Proving the helpers
  keep the instance nowhere is an interprocedural retention analysis this
  change does not have. Recorded, not implemented.
