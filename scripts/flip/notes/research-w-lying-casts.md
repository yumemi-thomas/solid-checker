# Lying casts and patched instance methods (research, agent 3)

Code read at `origin/claude/flip-reads` 89f58e233 (exported to `~/EDF/tmp/flipw/casts/src/`). No checker binary was run (hard rule). Every "the checker certifies" below comes from reading the code path. Each one takes the same branch as a held pattern that `open-defects.json` already lists as `false-clean, certified: true`. Chrome and tsc results were measured.

- Probes: `~/EDF/tmp/flipw/casts/cases/<id>/App.tsx`
- Chrome results: `~/EDF/tmp/flipw/casts/chrome.json`
- Census scripts and outputs: `~/EDF/tmp/flipw/casts/scripts/`, `census.json`, `impostors.json`, `stdkey-casts.txt`

## Where the standard method is resolved and judged safe

A call's resolved declaration comes from TypeFacts. If it is a standard-library declaration, it is vetoed only by these three tests:

- **The veto itself:** `rust/crates/solid-reactive-ir/src/indexes/callees.rs:391-419` (`SemanticLookup::resolved_callee_call`), mirrored in the memo context at `rust/crates/solid-reactive-ir/src/base/memo/build.rs:1731-1754`. The three tests:
  - `subclass_veto` (ADR 0683);
  - `patch_veto` under `AnyStandardPatch` (ADR 0781);
  - `unproven_assertion`.
- **The assertion test:** `unproven_assertion` (`base/contributions.rs:879-923`) asks `FileFacts::asserted_call_value` (`rust/crates/solid-facts/src/project.rs:666-688`). That function looks only at:
  - the callee;
  - a member callee's receiver;
  - the initializers of an identifier-binding chain from the receiver (`identifier_binding(declaration)?.initializer?`, project.rs:685).

  A parameter (`run(t as any)` → `p.then`), a field (`new H().p.then`) or a call result (`mk().then`) ends that walk with `None`, so nothing is vetoed.
- **Why `then` is then safe:** `runtime_semantics::argument_behavior` (`runtime_semantics.rs:347-362`) maps `Promise.then` (timing arm at :515) through `FRESH_STACK_SCHEDULERS` (`"Promise.then"` at :82) to `FreshStackCallback`. So `setW(1)` in the callback is placed on a fresh stack and stays silent. Chrome runs it synchronously inside `App` and reports `REACTIVE_WRITE_IN_OWNED_SCOPE`.

## Q1: a narrower answer for lying casts

### Chrome evidence: the lie needs no cast near the receiver, and often no cast at all

All programs pass strict tsc (`scripts/flip/tsc.mjs`: 19 of 19 pass). Chrome runs used `scripts/flip/run.mjs` on rc.21.

| id | how the impostor reaches a `Promise`/`Array`-typed receiver | Chrome | checker, by code reading |
|---|---|---|---|
| base-p3-cast-param / -field / -return | held patterns, copied verbatim | `REACTIVE_WRITE_IN_OWNED_SCOPE` (all 3) | certified (held false cleans) |
| c1-any-param | `const t: any = {then(f){f()}}; run(t)`, **no assertion** | warns | certified (same branch as p3-cast-param) |
| c2-covariance | `const wide: (Promise<void>\|Thenable)[] = ps; wide.push({then…}); ps[0].then(cb)`, **no assertion, no `any`** | warns | certified |
| c3-generic-coerce | `function coerce<T>(x: unknown): T { return x as T }`, the target is a type parameter | warns | certified |
| c4-nested-local | `const h = {p:{then…}} as unknown as {p: Promise<void>}; h.p.then(cb)`: a local cast whose receiver is `h.p` | warns | certified (the receiver is a member, so the walk stops) |
| c5-type-guard | `x is Promise<void>` predicate, **no assertion** | warns | certified |
| c7-instance-then-assign | `const p = Promise.resolve(); Object.assign(p, {then: f => f()}); p.then(cb)`, **no assertion** | warns | certified (`standard_root(p)` is `None`, so no patch) |
| c6-instance-assign | `Object.assign(xs, {slice: () => {setW(1)…}}); xs.slice()` | warns | not derivable: the write sits in a function passed to `Object.assign`, which may read as an unplaced write |
| n1/n2 controls | real `Promise.resolve().then`, real `[1,2].slice` | silent | silent |

So no rule keyed on assertions alone is sound: c1, c2, c5 and c7 have no assertion anywhere, and c3 and c4 have assertions that options (a) and (c) below skip.

### Corpus counts (frozen rc.21, 23 apps)

Syntax-only TypeScript-parser census of app sources. It excludes `node_modules`, `.git`, `dist`, `build`, `.output`, `.vinxi` and `coverage`, but includes tests.

- **Double casts: 991 in 16 of 23 apps.** That is 949 `as unknown as`, 41 `as any as` (one app), 1 `as X as Y`, and 0 `<T><unknown>`. Plain `rg` excluding only `node_modules` gives 954 + 42 = 996; the brief's 974 lies between the two counts.
  - By app: viviana-ui-web 417, app-game 279, kui 79, compass-ui 70, solid-groove 42, expenses-app 31, probus-hk 16, hoxxes-briefing 11, dashiboard-ui 10, every-deck-of-cards 10, jandibat-web 9, skyjtx-website 7, ai-memory-ui 6, donegeon-client 2, inferay 1, jar-hell-web 1.
- **Single `as any`:** 698 (510 app, 188 test). Single non-`const` assertions of other kinds: 8202. Apps with a double cast or `as any`: 19 of 23. Adding `: any` annotations and `x is T` predicates (371 sites in 18 apps): **21 of 23 apps**.
- **Top-level target of the 990 double casts with an `unknown`/`any` middle:**

  | target | count |
  |---|---|
  | named project/dependency type | 440 |
  | DOM/JSX library type | 243 |
  | type literal | 93 |
  | `Record`/`Partial`/… | 78 |
  | container (`T[]`, `Promise<>`, `Map<>`…) | 33 |
  | type parameter | 23 |
  | primitive | 20 |
  | `typeof X` | 18 |
  | indexed access | 16 |
  | function type | 14 |
  | `never` | 3 |
  | other | 9 |

  A target whose top-level type certainly has standard methods (container, library type, `typeof`, function, type parameter, primitive: `String.prototype.slice` counts) covers 351 of 990. Only the 3 `never` casts are certainly method-free. The other 636 need a type walk.
- **Operand of the 990 double casts (syntax):**

  | operand | count |
  |---|---|
  | identifier | 445 |
  | object literal | 150 |
  | member | 94 |
  | call | 91 |
  | **object literal with a standard-method key** | **58** |
  | global (`window`…) | 46 |
  | array literal | 33 |
  | `null`/`undefined` | 32 |
  | primitive literal | 10 |
  | `new` | 10 |
  | function | 9 |
  | `{}` | 4 |
  | other | 8 |

  Only 46 (4.6%: nullish, primitive, `{}`) are safe by syntax alone.
- **Object literals cast with a standard-method key (the p3 shape, written in place): 68.**
  - 62 are in tests, chiefly `{ get, delete } as unknown as KuiApiClient` (53 in kui).
  - 9 target library types: `MediaQueryList` ×5 (`addEventListener`…), `Storage` ×2, `CSSProperties`/`CanvasRenderingContext2D` with a string `filter`.

  Real lies in the corpus are mostly test mocks.

### Options

**0. Today (ADR 0781, local).**
- Premise: a lie is visible on the callee, the receiver, or its binding chain.
- How it fails: p3 ×3, c1–c5 and c7 (Chrome above).
- Reach: —
- Work: —

**B. Blunt: any unproven assertion in the project vetoes every standard call whose receiver is not locally proven.**
- Premise: assertions are the only lie source.
- How it fails: c1 (`: any`), c2 (covariance), c5 (predicate) and c7 (instance write) have no assertion, so it is **unsound** as well as blunt.
- Reach: 16 of 23 apps by double casts; 19 of 23 with `as any`; 21 of 23 once `any` and predicates are added, still unsound.
- Work: about 40 lines. A whole-program memo "some unproven assertion" reuses `unproven_assertion`; no producer change.

**(a) Only casts whose target type has a standard method.**
- Premise: a lie about type T reaches only receivers whose type T contains.
- How it fails: p3-cast-param itself is `as any`, which has every method. c3 targets `T`. c4 targets `{p: Promise<void>}`, a type literal whose *field* is the container. So it must be deep, and must count `any`, type parameters and primitives.
- Reach: shallow covers 351 of 990 casts plus all 698 `as any`. Deep is somewhere between 351 and 987 of 990, unmeasured (the 636 left need types). Every app with a cast keeps at least one.
- Work: a new producer fact, "standard owners reachable from the asserted type" (bounded type walk, protocol bump). IR about 80 lines. Inherits B's unsoundness.

**(b) Only when the operand is not proven to be the target type.**
- Premise: an assertion whose operand type is assignable to the target asserts nothing new; peel `as unknown`/`as any` first.
- How it fails: sound as a filter on B or (a), but inherits their holes (c1, c2, c5, c7). An array literal or a global is safe only for its own methods: `[fake] as unknown as Promise<void>[]` lies in its elements.
- Reach: 46 of 990 safe by syntax. The real reduction needs the typed fact (unmeasured; double casts of an identifier or call make up 536).
- Work: a new producer fact, `isTypeAssignableTo(operand, target)` at each assertion. About 40 lines of IR.

**(c) Cheap value flow from the cast to the receiver (parameter, field, return, JSX prop).**
- Premise: lies travel only from an assertion along modelled flows.
- How it fails: closes p3 ×3 and c4. It cannot see c1, c2, c5 or c7, and must fail closed at every untracked flow (container, store, signal, context, export), which brings back B's reach.
- Reach: not measurable by grep. Every parameter, prop and store receiver in an app with a cast falls back to uncertifiable.
- Work: the largest. Interprocedural flow from assertions into parameters, fields and returns, over the `value_origin_assuming` machinery (`indexes/primitive_proof.rs:232`) and ADR 0366's holder walk. About 600 lines or more.

**(d) (own) Member-keyed impostor census.** A standard `C.m` loses its type-only proof while the program holds any site that may put a function at key `m` on an object that is not a real `C`. The sites are:
- object-literal members;
- class instance members;
- writes and definers of `m`;
- computed keys, unknown-key writes, `Object.fromEntries` and Proxy `get` traps whose key domain may hold `m`.

A receiver keeps the proof by a value-origin proof (array literal, `new C` or a static factory of an unpatched global, an `async` call for `Promise`).

- Premise: only code can put a function under a method name, and dependency objects typed as a standard container are that container (ADR 0300 style). A project lie about a *dependency* object (`obs as unknown as number[]`) is caught by adding "an unproven assertion whose operand type declares `m` outside the default library" to `census(m)`.
- How it fails: one residual, stated as a premise: a dependency value typed `any`/`unknown` that holds an impostor and that project code casts or passes on. No corpus instance was found. It catches every Chrome case above (each has a `then`/`slice` literal or write), and catches c2 without needing a lie marker.
- Reach: these counts are for the members the runtime tables give a timing. "Possibly callable" means a method, accessor, or non-literal value, excluding tests.

  | member | apps (sites) |
  |---|---|
  | `then` | 1 (app-game: vendored gsap `Animation.then` ×2 and a math example; plus 1 in a test) |
  | `catch` | 1 (compass-ui) |
  | `finally` | 0 |
  | `forEach` | 1 |
  | `map` | 4 |
  | `filter` | 6 |
  | `find` | 3 |
  | `sort` | 5 |
  | `every` | 2 |
  | `some` | 1 |
  | `reduce` | 1 |
  | `slice` | 2 |
  | `addEventListener` | 3 (another 7 apps in tests) |

  The accessor members are wide: `get` 13 apps, `entries` 10, `set` 8, `at` 7. Unknown keys open `census(*)` unless typed: 344 computed members with possibly-callable values in 13 apps, and Proxy `get` traps in 6 apps (app code). A 25-site sample was nearly all `const` or enum keys, or non-function values, so a producer key domain and callability would remove most. Rows are unmeasured (this needs a release sweep).
- Work:
  - solid-facts: one new per-file fact, "keys a file may define with a callable value" (literal members; Proxy and `fromEntries` sites). Class elements, assignment writes and definers already exist. About 100 lines plus a schema bump.
  - IR: a whole-program memo modelled on `compute_objects_assigned_into` / `own_member_may_be_replaced` (`indexes/member_values.rs:208-300`, which already covers writes, unknown keys and reflective definitions), about 150 lines. The hook at callees.rs:391 and build.rs:1731 is about 30 lines. A promise origin for the receiver exemption is about 80 lines (`ValueOrigin`, `indexes.rs:368`, deliberately excludes Promise).
  - Producer: none for phase 1 (unknown keys stay open). Phase 2 adds the key literal domain (`primitive_literal_candidates_of_type`, `type_facts.rs:519`, as a new demand) and value callability, one protocol bump.
  - Fixtures: about 8 (p3 trio, c1, c2, c5 and c7 positive; no impostor negative; impostor of another name shadowed; class member; computed-key wrapper).
  - Total about 500–700 lines.

### Recommendation (Q1)

**Adopt (d), the member-keyed impostor census, and keep ADR 0781's local veto beside it.**

- **Why (d):** it is the only option that closes the three held p3 patterns *and* the Chrome-confirmed siblings that no option keyed on assertions can see (c1 `any`, c2 covariance, c5 predicate, c7 instance write).
- **Narrower where it matters:** for `then`, the member p3 dispatches, it vetoes in 1 app of 23, against 16 to 21 for B.
- **Order of work:**
  1. The timing members (`then`, `catch`, `finally`, the array iteration methods, `addEventListener`) and `slice`. These are the fuzzer's shapes and also `net/prop-std-method-tracked`.
  2. The accessor members (`get`, `set`, `has`…). They are wide and need the receiver exemption extended to `new Map()` bindings, as ADR 0683 already suggests.
  3. Producer key domains, to shrink `census(*)`.
- **Until a stage lands:** its members stay listed in docs/open-work.md next to the existing open-work.md:280 entry.
- **Blocker name for the uncertifiable:** "standard `C.m` not proven: the program defines `m` at <site>".

## Q2: patched instance methods

### Today's behaviour

- **Where the global-key patch is raised:** `globalThis[key] = v` with a key no fact reads raises `standard_patch("", "*")`. The computed-write loop is at `base/contributions.rs:760-783` and the definers at :785-811 (`Object.assign(window, api)` with a non-literal `api` also gives `"*"`).
- **Why instance methods are spared:** `patch_veto` (`contributions.rs:847-870`) lists `("", "*")` only for functions and constructors (`owner.is_empty() || owner.ends_with("Constructor")`, :865-866). An instance method gets `(Container, "*")` instead (:868). The doc comment at :841-846 and docs/open-work.md:281 record the limit; vetoing every method was measured at +751 rows.
- **A second, larger gap:** `FileFacts::standard_root` (`solid-facts/src/project.rs:693-734`) names a root only by text.
  - It returns `None` when the chain's root identifier has any declaration (:710-711).
  - It returns `None` for anything not spelled `G.prototype`, a bare name, or a global (:727-732).
  - Every assignment and definer in `standard_patches` (:739-812) goes through it, so a prototype reached any other way **raises no hazard at all**.

### Chrome evidence (rc.21; all pass strict tsc; all report `REACTIVE_WRITE_IN_OWNED_SCOPE` in `render`; controls n1/n2 silent)

Each patch fires once at the `App` call (an `armed` flag), so Vite and Solid keep the real method.

| id | patch | hazard today (code reading) | checker, by code reading |
|---|---|---|---|
| g1-key-proto-then | `(globalThis as any)[k].prototype[m] = syncThen` (k, m computed at run time) | none: `standard_root("(globalThis as any)[k].prototype")` is `None` | certified |
| g4-assign-key-proto | `Object.assign((globalThis as any)[k].prototype, {[m]: syncThen})` | none (same root) | certified |
| g6-key-global-alias | `const C = (globalThis as any)[k]; C.prototype.then = syncThen` | none: root `C` has a declaration (:710) | certified |
| g3-getproto-then | `Object.getPrototypeOf(Promise.resolve()).then = syncThen` (a **static key**, no unknown key) | none | certified |
| g2-key-ctor-then | `(globalThis as any)[k] = class extends Real { static resolve() { return {then(f){f()}} } }`, then `Promise.resolve().then(cb)` | `("", "*")`: vetoes `PromiseConstructor.resolve`, **not** `Promise.then` | certified (the open-work.md:281 limit, now confirmed in Chrome) |
| g5-key-proto-slice | `(globalThis as any)[k].prototype[m]` on `Array`/`slice`, and the patch itself calls `setW` | none | not derivable (the write sits in the patching function) |
| c7-instance-then-assign | `Object.assign(p, {then})` on a real promise instance | none: root `p` → `Promise.resolve()` → `None` | certified |

So yes, a real program can do this, and the unknown-key global write (g2) is the smaller half. Corpus instances of the larger half, in app code:

- `viviana-ui-web/packages/solidaria/src/interactions/createInteractionModality.ts:222`: `Reflect.defineProperty(windowObject.HTMLElement.prototype, "focus", …)`. The root `windowObject` is a local, so no hazard.
- `app-game/packages/spector/backend/utils/originFunctionHelper.ts:33`: `object.prototype[originFunctionName] = …` (`object: any`).

Sites that raise `("", "*")` in the corpus: 4.

- `solid-groove/src/audio/testAudioContext.ts:37`: the key comes from an `as const` tuple, so its domain is finite.
- `jar-hell-web/web/design/support.js:1901` and `solid-groove/docs/design/support.js:1831`: `Object.assign(window, api)` where `api` is a `const` object literal with static keys, so finite.
- `solid-groove/src/audio/AudioRuntime.singleContext.test.ts:36`: the key comes from `Object.entries(...)` and is `string`, so open.

### Recommendation (Q2)

Do **not** extend `("", "*")` to every instance method. Do three narrow things:

1. **Name the key first.**
   - What: for a computed global write, demand the key's literal domain (`primitive_literal_candidates_of_type`, type_facts.rs:519). For `Object.assign(global, src)`, demand `src`'s declared keys. Raise `("", K)` for each `K`; `patch_veto`'s fourth entry, `standard_patch("", container)` (:860-863), already vetoes K's instance methods (the g2 shape).
   - Corpus effect: 3 of the 4 sites become finite.
   - Work: one producer demand and about 40 lines in `standard_patches`.
2. **Only for a key that stays open, extend the stop to instance methods.** Add `("", "*")` to the instance entry of `patch_veto` (one line, :865-869).
   - Corpus effect: 1 site, a solid-groove test file. Rows are unmeasured; open-work.md:281 attributes the +751 to a test helper and a build plugin, and these 4 sites are the only `("", "*")` sources in the corpus.
3. **Close the `standard_root` gap.** This is the bigger soundness hole: g1, g3, g4, g6 and c7, and two corpus app files.
   - Rule: when `standard_root` gives `None` for an assignment's object or a definer's target, decide by the object's type. Raise `(Owner, key)` for each standard owner that `library_types` (type_facts.rs:933) reports. This also covers instance own-property writes (c7: `Promise`, `then`).
   - When the type is `any` or `unknown`, raise a wildcard owner `("*", key)`, or `("*", "*")` for an open key. Only spector's `object: any` reaches the full wildcard in the corpus.
   - Work:
     - producer: a `LIBRARY_TYPE_ANY` sentinel in `library_types_at`, since `any` has no library name today (about 5 lines, protocol bump);
     - IR: a new demand capability on the write object or definer target and two `patch_veto` entries (about 60 lines);
     - no new AST facts;
     - fixtures: g1, g3, g4, g6 and c7 positive; a project-class prototype write negative (ecsy `Component.prototype[key]` must stay silent because it is typed as a project class); an alias variant.

The fuzzer needs shapes for g1, g3, g4, g6, c7 and c1, c2, c5 (closing a defect requires `make fuzz-focus` rediscovery).

## Summary of costs

- **Q1 (d):** about 500–700 lines of Rust, one facts schema bump, no producer change for phase 1 (one later for key domains), about 8 fixtures. Reach on the timing members: 1 to 6 apps per member (`then`: 1). Rows unmeasured.
- **Q2:** about 100 lines of Rust, one producer protocol bump (key domain demand and `any` sentinel), about 6 fixtures. Steps 1–2 reach 1 open-key site; step 3 reaches 2 corpus app files and spector's wildcard.
- **Not run:** no checker binary, cargo, make or fuzz runs. Row counts need a release sweep on a build that includes `e40429f5e`.
