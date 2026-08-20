# Rule catalog reduction plan

**Status: implemented.** Completed on 2026-08-20 in the 31 green commits
specified by §10. The resulting catalogs contain 18 Solid 1.x rows and 26
Solid 2.0 rows: 28 concepts / 44 external keys. The compatibility registry
contains the plan's 31 retired and 19 aliased external identities. The
replacement ownership gate covers 284 cases, and all 465 migration-ledger rows
have a final disposition. The public contract schema is unchanged. The full
`make verify` handoff gate, including coverage, ownership, TypeScript oracle,
performance, package, manifest, and bundled-contract checks, passed before this
completion record was committed.

**Date:** 2026-08-20

---

## 1. Purpose, governing test, and how to review

The catalogs hold **79 per-dialect rows** covering **54 distinct concepts**. This
plan reduces that to **28 concepts / 44 rows / 44 external keys**, by deleting
rules that are provably wrong, duplicate TypeScript, duplicate another rule,
never fire, or — the largest category — **describe legal behavior that resembles
a mistake**.

### The governing test

From `AGENTS.md`:

> Report a violation only when semantic facts and the execution model prove it.
> When a required fact, symbol, call target, package contract, or compiler
> behavior is missing, fail closed or produce an explicit uncertifiable result.

and:

> **If `tsc` reports a diagnostic for the same code against the library's real
> published typings, this checker must not report it.**

A **violation** must assert that semantic facts plus the execution model
establish runtime misbehavior. The following do not qualify, and are deleted or
reclassified wherever they appear:

- a **legal seed or previous value** that resembles a React dependency array;
- a **deliberate override** — a spread followed by a same-named attribute;
- an **intentional custom attribute** on a hyphenated element the project
  augmented itself;
- a **readability preference** (canonical casing, self-closing syntax, component
  syntax);
- a **timing-dependent hazard** whose side of the race cannot be proven — that is
  an `uncertifiable` finding, not a violation at warning severity;
- a **component-forwarding assumption** — that a prop handed to a component is
  inert, renamed, or passed to the DOM;
- **generic web-platform validation** — CSS property names, HTML nesting,
  injection sinks.

No rule is retained to reach a target catalog size. Three rules that fail the
test are retained on an explicit owner decision, and §7 moves them behind an
opt-in preset so the **default** configuration stays proof-backed.

### Review order

1. **§3 Evidence register.** Every deletion of a shipping rule rests on a row
   there, each with a file:line citation or a reproducible probe. Re-run the
   probes; do not accept the transcripts.
2. **§8 replacement ownership gate.** The one piece of genuinely new machinery.
   If its case manifest cannot carry what the upstream corpus carries, §10's
   sequencing breaks.
3. **§6.2 aliases** and **§5.2 code policy** — where damage is silent: a lost
   `enabled: false` makes suppressed diagnostics reappear.
4. **§10 execution order** — specifically the green-per-commit argument.
5. **§11 arm audit** and **§12 correction log**.

### Confidence vocabulary

- **Verified** — checked against source, published typings, a pinned compiler
  revision, or a runtime probe, with the citation given.
- **Judgment** — follows from a verified fact plus a decision in §2. The
  alternative is named.

### Reproducing the evidence

```bash
make tsc-oracle-provision
```

populates `rust/target/tsc-oracle/v1/node_modules` (`solid-js@1.9.14`) and
`rust/target/tsc-oracle/v2/node_modules` (`solid-js`, `@solidjs/web`,
`@solidjs/signals` all `2.0.0-rc.0`). `rust/target/` is gitignored, so those paths
exist only after provisioning. All probes are read-only and were run from a
scratch directory.

Compiler claims cite the revisions pinned in `rust/Cargo.toml`:
`dom-expressions-compiler` rev `b0965a934120d238dfefbc04274f5e6c9c55387f` (2.0)
and `solid1-dom-expressions-compiler` rev
`79b9b63721c59b0acfd72348438bbb6e090ec81c` (1.x), under
`~/.cargo/git/checkouts/`.

---

## 2. Decisions — all settled

| ID | Decision | Answer |
| --- | --- | --- |
| **D1** | Is eslint-plugin-solid parity a product goal? | **No.** Keep upstream's *name* for a rule we implement; implement only rules that are genuinely necessary. |
| **D2** | Do style/preference rules belong? | **Keep `prefer-for`, `prefer-show`, `prefer-classlist`** — behind an opt-in preset (§7), not in the default config. |
| **D3** | Merge granularity | **One rule per defect class.** Message variants carry the specificity. |
| **D4** | Do non-Solid rules belong? | **No.** Generic security, HTML-validity, CSS-validity, and target-compatibility rules leave the catalog. |
| **D5** | Fail-closed-only rules | **Internal.** A rule that cannot fire with the bundled fact producer becomes a producer-integrity error. |
| **D6** | Is the v1 catalog first-class or a migration path? | **Migration path.** `untracked-derived-function` (SC1006) is therefore deleted from **both** dialects, after the SC1001 chain-following fix (§10 step 5). |
| **D7** | External rule keys: keep the `v1/` namespace or collapse? | **Keep `v1/`.** Collapse is orthogonal to catalog reduction and conflicts with ESLint's flat registry and static `meta.docs.url`. Removed from this sequence; recorded as a follow-up design project (§13). |
| **O1** | Does `(1.x only)` in D2 attach to `prefer-classlist` alone? | **Yes, `prefer-classlist` alone.** `prefer-for` and `prefer-show` are ported to 2.0. |
| **O2** | Merge-family disable transfer | **Canonical aliases** for `missing-owner` and `package-contract-incomplete` (accepting family-wide widening, documented); **declared break** for `leaf-owner-forbidden-call`, `pending-async-unsuspendable-read`, and the SC5003/SC5005 merge. |
| **O3** | Does a CSS-name-validity claim survive D4? | **No.** The v1-only `style-prop` (SC8017) is deleted and is never ported to v2. |
| **O4** | Does `jsx-no-undef` keep its upstream name? | **Yes.** Only the `use:` arm survives; fix the documentation, not the name. |
| **O5** | Are the three preference rules acceptable in the default config? | **No — opt-in preset.** They stay in the catalog, default to disabled, and are enabled by dialect-safe `preferences-v1` / `preferences-v2` configs (§7). |

### D8 — `no-implicit-draggable` (SC8019): deleted from both dialects

**Settled.** The rule is deleted from the v1 and v2 catalogs. Both external
identities (`v1/no-implicit-draggable`, `no-implicit-draggable`) are retired.
Grounds are recorded in §3.10; the deletion lands in step 8.

The v1 arm reported the boolean shorthand on every element, including `<img>` and
`<a href>` where `auto` *is* draggable and the shorthand does exactly what the
author asked. Correcting it would have required inverting the guard **and**
adding a standard-HTML-element census to keep custom and unknown elements silent
— new machinery for one narrow claim about an HTML enumerated attribute's
serialized state. Under D4 that is web-platform validation, not reactivity,
ownership, execution phase, timing, compiler lowering, or package runtime
behavior. The v2 arm's literal-`false` claim is sound in isolation, but it is the
same web-platform claim and does not survive D4 either.

### What D7 = keep-`v1/` removes from this plan

- No dialect field on diagnostic snapshots, `Finding`, or `RuleMetadata`.
- No change to `schema/solid-reactivity.schema.json` — that document is titled
  *"Solid Reactivity Package Contract"* and describes package contracts, not
  diagnostics; its `schemaVersion` stays 1.
- No separate diagnostic schema.
- No dynamic `(dialect, name)` ESLint registry or per-file dialect router. The
  existing flat `plugin.rules` keyed by rule name keeps working, and the existing
  `catalog.namespace` trigger keeps forcing v1 analysis for `v1/`-prefixed rules
  (`packages/cli/eslint.cjs:311-314`).
- v1 rules keep dialect-specific documentation URLs (`docs/rules/v1/`) and
  dialect-specific `rule-options.json` keys.
- **Internal per-dialect rows and external ESLint keys are the same number: 44.**

---
## 3. Evidence register

Every load-bearing claim, with how to check it.

### 3.1 `createReaction` is not a leaf owner in Solid 1.x — verified

**Affects:** deleting `v1/cleanup-in-forbidden-scope` (SC3001) and
`v1/primitive-in-leaf-owner` (SC3002).

The repository asserts `Primitive::CreateReaction => &[(0, CallbackOwner::Leaf)]`
(`rust/crates/solid-dialect/src/solid_1x.rs:285`), and both rules' pages state
the consequence: the cleanup "will never run", primitives are "never tracked into
the graph and never disposed".

**Mechanism (1.9.14).** `runComputation` sets `Listener = Owner = node`
(`dist/dev.js:748`), so the callback runs owned by the reaction's own
computation. `updateComputation` calls `cleanNode(node)` before every run
(`:730`); `cleanNode` runs `node.cleanups` and disposes `node.owned`. `onCleanup`
pushes onto `Owner.cleanups` (`:508`).

**Probe** (invalidation triggered from *outside* `createRoot`, so the callback
runs through the real scheduler path — a synchronous first invocation would run
under the root's owner and give a false reading):

```
v1 reaction#1: owner=PRESENT
  effect(gen1) tick=0
  cleanup(gen1) RAN
-- setTick(9): which effects respond? --
-- dispose --
(no warnings)
```

`getOwner()` is present, the `onCleanup` ran, and the child effect did not
respond to `setTick(9)` because `cleanNode` had disposed it. Both premises are
false.

**Solid 2.0 differs and the repository has 2.0 right.** `solid_2.rs:384` answers
`CallbackOwner::None`; probed on `@solidjs/signals dist/dev.js`:

```
v2 reaction#1: owner=NULL
-- dispose --
WARN [NO_OWNER_CLEANUP] onCleanup called outside a reactive context will never be run
```

### 3.2 Solid 1.x directive and `ref` application keep their owner — verified

**Affects:** deleting `v1/primitive-in-directive-application` (SC6001).

1.x applies directives through `use(fn, element, arg) { return untrack(() => fn(element, arg)); }`
(`solid-js/web/dist/dev.js:328`), and 1.x's `untrack` clears only `Listener`,
never `Owner` (`dist/dev.js:474-483`). The spread `ref` path routes through the
same `use` inside a `createRenderEffect` (`web/dist/dev.js:314`) — still an
owner. Probed:

```
directive apply: owner=PRESENT
  directive effect sees tick=0
-- setTick(1) --
  directive effect sees tick=1
-- dispose --
  directive onCleanup RAN
```

**2.0 is genuinely unowned**, so the v2 rule is correct and stays:
`@solidjs/web dist/dev.js:592` is `runWithOwner(null, () => applyRef(resolved, element));`.

**Secondary defect in the same rule.** v1's `creates_directive_owner`
(`solid_1x.rs:749`) lists `CreateSignal`, `CreateStore`, `CreateMutable`, and
`creation_registers_work`'s `_ => true` arm
(`rust/crates/solid-reactive-ir/src/directives.rs:167`) makes them unconditional
violations — contradicting the same file's `cleanup_rule` comment
(`solid_1x.rs:659`): *"State factories are absent: a function passed to
`createSignal`, `createStore`, or `createMutable` is stored as data in 1.x and
registers no work in the surrounding owner."* The shipped fixture
(`fixtures/reactive-ir/no-owner-v1/App.tsx`,
`<div ref={element => createSignal(element)}>`) is wrong twice over.

### 3.3 `untracked-derived-function` reports the same runtime failure as `strict-read-untracked` — verified (2.0), judgment (1.x)

**Affects:** deleting `untracked-derived-function` (SC1006).

`strictRead` is a module-level ambient flag (`@solidjs/signals dist/dev.js:2900`)
set by `untrack(fn, strictReadLabel)` (`:2934`) and restored in a `finally`. It
is dynamic, not lexical, so any read anywhere in the synchronous stack below a
component's window trips `warnStrictReadUntracked` (`:183`, called at `:3104`).
Probed with the real component wrapper:

```
A direct read  -> 0
B helper read  -> 0
--- warnings ---
1. [STRICT_READ_UNTRACKED] Reactive value read directly in <DerivedButDiscarded> will not update…
2. [STRICT_READ_UNTRACKED] Reactive value read directly in <DerivedButDiscarded> will not update…
total: 2
```

Two warnings, identical text. `docs/rules/strict-read-untracked.md` calls SC1001
"the static counterpart of Solid's dev-mode `STRICT_READ_UNTRACKED` warning", so
this is already its subject matter. SC1006 anchors on the *function name* rather
than the read, which is why the redundancy never showed as a co-located span.

A second, independent problem: module scope.

```
module-scope helper read -> 0
warnings after module scope: 0      <- runtime silent
transitive read -> 0
warnings after transitive: 1        <- runtime warns
```

SC1001 deliberately stays silent at module scope in 2.0 — *"A deliberate
module-scope snapshot is legal, undiagnosed Solid."* SC1006's 2.0 gate names
module scope as evidence, reporting what the runtime does not diagnose and its
sibling declines to report.

**The 1.x half is weaker.** Solid 1.x has no `STRICT_READ_UNTRACKED` warning, so
v1's SC1001 is a static claim rather than a runtime mirror, and the redundancy
argument rests on chain-following alone. **D6 is settled as "migration path"**,
so the rule is deleted from both dialects once the SC1001 chain-following fix
lands in the same commit (§10 step 5).

### 3.4 `no-react-deps` reports a legal seed value — verified

**Affects:** deleting `no-react-deps` (SC8010). It was previously retained as
v1-only on the grounds that 2.0's typings reject the shape. That is true and
irrelevant: the v1 finding is not a defect.

**The second argument is not ignored.** 1.x declares
`createEffect<Next, Init = Next>(fn: EffectFunction<Init | Next, Next>, value: Init, options?)`.
`value` is the effect's initial previous value, threaded to the callback as its
parameter. The repository documents this as idiomatic — `solid_1x.rs:697`:
*"1.x declares its effect callbacks as `EffectFunction<Prev, Next> = (v: Prev) => Next`
and threads the return value to the next run, so `createEffect(prev => prev + 1, 0)`
is idiomatic accumulation."* An array is a perfectly legal seed.

**The trigger is syntactic intent inference.**
`rust/crates/solid-reactive-ir/src/upstream_compat/solid1x_structure.rs:75-79`:

```rust
let source = text(file, argument.span).trim();
let looks_like_deps = source.starts_with('[')
    || binding_initializer(context, file, argument.span)
        .is_some_and(|(_, _, initializer, _)| initializer.trim_start().starts_with('['));
```

It fires on any second argument whose source text begins with `[`. No semantic
fact about the value's role is consulted.

**The message is factually wrong.** `:82-84` says the argument *"does nothing in
Solid except get silently ignored — or, for `createMemo`, get mistaken for the
equality comparator it actually is."* Neither clause holds:
`createEffect(fn, value)` threads `value` into the callback, and
`createMemo(fn, value, options)` takes the initial value second with `equals`
in the third `options` argument.

**And the offered fix is unsafe while declared safe.** `:89-101` emits
`applicability: "safe"` with an edit that deletes the argument *and* its leading
comma. For `createEffect(prev => prev.concat(x()), [])` that changes the
first-run input from `[]` to `undefined` — a behavior change, and for that body a
crash.

Nothing here proves misbehavior. `createEffect(fn, [seed])` is legal, working
1.x code that resembles a React habit. **Delete.**

### 3.5 `event-handlers` retains no arm that proves a defect — verified + judgment

**Affects:** deleting `event-handlers` (SC8001) from v1; cancelling its planned
2.0 port.

The rule has four arms. One is factually false (§3.6). The other three fail the
governing test:

| Arm | Status |
| --- | --- |
| **Canonical-spelling rename** — a *declared* lowercase alias (`onclick`, `ondblclick`) should be camelCase | Its own page concedes the point: *"`onclick` and `ondblclick` type-check, so the remaining objection is **readability**."* A readability preference, not a defect. 1.x declares 139 lowercase `on*` aliases; both spellings attach the same listener. |
| **Static value on a hyphenated tag** — `<my-widget onclick="doThing()">` frozen into the template as a plain attribute | The lowering is **verified** (§3.5a) but the finding is not a defect. A hyphenated tag exists only because the project augmented `JSX.IntrinsicElements` itself; a static `on*` attribute on a custom element is how inline handlers are written in HTML, and the element may read the attribute deliberately. Intent, not proof. |
| **Ambiguous name on a hyphenated tag** — an unrecognised `on*` name | Explicitly ambiguous by the page's own description: *"an unrecognised `on*` name is **ambiguous** between a word beginning 'on' and a misspelled handler."* Ambiguity is not proof. |
| **`warnOnSpread`** | False in both dialects — §3.6. |

**Delete, and do not port.** The static-value arm is the only one with a verified
mechanism, and it is intent-based, so §1 forbids porting it to v2.

**§3.5a — the lowering, for the record.** The 2.0 compiler's `classify_plan`
(`packages/compiler/src/dom/attrs.rs:232`) makes no exception for `on*`:
`PlanValue::Literal`, `Expression::StringLiteral`, and
`Expression::NumericLiteral` all reach `PlanDisposition::Inline`, which appends
to the template string; `BooleanLiteral(true)` yields a bare attribute. The
`starts_with("on") → dom_event_statements` branch (`:354`) sits inside
`lower_runtime_attribute`, i.e. the `Runtime` path, which a static literal never
takes. The compiler's own comments name the case twice (`:188`, `:674`, *"a
folded `on*`… lands here"*). So 2.0 does still freeze a static `on*` into the
template — the mechanism is real; the objection is not.

### 3.6 `warnOnSpread` is false in both dialects — verified

`assignProp` — what `spread()`/`assign()` calls per prop — has an unconditional
`on*` branch in both runtimes: `@solidjs/web@2.0.0-rc.0 dist/dev.js:1392` and
`solid-js@1.9.14 web/dist/dev.js:458`. Probed against 1.9.14 with a stub node:

```
1.9.14  spread onClick -> ATTACHED via delegated $$click
```

The arm's premise — that Solid attaches listeners only from attributes its
compiler can see — is false. It is off by default, so this is a latent rather
than live false positive. It dies with the rule (§3.5).

### 3.7 `no-array-handlers` reports a supported runtime form — verified

**Affects:** deleting `no-array-handlers` (SC8007).

**The bound pair is first-class and intentional.**
`@solidjs/web@2.0.0-rc.0 dist/dev.js:508-518`:

```js
function addEvent(node, name, handler, delegate) {
  if (delegate) {
    if (Array.isArray(handler)) {
      node[`$$${name}`] = handler[0];
      node[`$$${name}Data`] = handler[1];
    } else node[`$$${name}`] = handler;
  } else if (Array.isArray(handler)) {
    const handlerFn = handler[0];
    node.addEventListener(name, handler[0] = e => handlerFn.call(node, handler[1], e));
  } else node.addEventListener(name, handler, typeof handler !== "function" && handler);
}
```

Dispatch reads the data back (`:1436`). 1.9.14 is array-aware at
`web/dist/dev.js:462`. A correctly matched tuple dispatches successfully in both.

**The rule cannot tell matched from mismatched.** Its facts
(`solid1x_attributes.rs:904`) are slot presence and slot-0 *arity*:

```rust
if !(tuple.has_slot(0) && tuple.has_slot(1) && tuple.element_zero_accepts(2)) {
    return ArrayHandlerStatus::Safe;
}
return if runtime_array_origin_is_proven(context, file, span) {
    ArrayHandlerStatus::Violation
} else { ArrayHandlerStatus::Uncertain };
```

No fact compares slot 1's type with slot 0's first parameter. And such a fact
would be vacuous against the declaration: `BoundEventHandler[0]` is
`(data: any, ...e: Parameters<EHandler>) => void` (`jsx.d.ts:164-176`), so a
comparison against `any` can never fail.

**Its own flagship example is a matched pair.**
`docs/rules/v1/no-array-handlers.md` gives as *incorrect*:

```tsx
type SaveHandler = [(data: Record, event: MouseEvent) => void, Record];
const click: SaveHandler = [save, record];
<button onClick={click}>Save</button>
```

Slot 0 takes `(data: Record, …)`; slot 1 is `Record`. Correctly matched, and it
dispatches. The documented defect is a false positive. **Delete.**

### 3.8 Both compilers pass component props through with exact keys — verified

**Affects:** deleting `no-react-specific-props` (SC8011),
`no-unknown-namespaces` (SC8012), `no-innerhtml` (SC8008), and `style-prop`
(SC8017); restricting `jsx-no-duplicate-props` (SC8003).

After the 2026-08-17 `tsc` narrowing, the *component* arm is the only surviving
domain of several of these rules, and that arm assumes a component prop is inert,
renamed, invalid, or forwarded to the DOM.

2.0, `packages/compiler/src/shared/component_props.rs:141`:

```rust
pub(crate) fn component_property<'a>(
    ctx: &impl ComponentPropContext<'a>, span: Span, name: &str,
    value: Expression<'a>, needs_getter: bool,
) -> ObjectPropertyKind<'a> {
    if needs_getter { ctx.object_getter_property(span, name, value) }
    else { ctx.object_property(span, name, value) }
}
```

The key is the attribute's `name`, verbatim. Namespaced keys become literal
`"ns:name"` — `shared/component.rs:93-95`:

```rust
oxc_ast::ast::JSXAttributeName::NamespacedName(name) => {
    format!("{}:{}", name.namespace.name, name.name.name)
}
```

The pinned 1.x compiler is byte-identical in that branch
(`shared/component.rs:99-101`). No `Aliases`, `getPropAlias`, `className`,
`htmlFor`, or `dangerously*` special-casing appears in either shared component
path.

**Consequence.** `<Panel className="x" />`, `<Panel style={{ fontSize: 10 }} />`,
`<Panel on:click={fn} />`, `<Panel dangerouslySetInnerHTML={h} />` all hand the
component an ordinary prop with that exact key. A component may **intentionally
consume** any of them. Nothing in the checker's facts proves forwarding, and
nothing proves inertness.

| Rule | Intrinsic arm | Component arm | Result |
| --- | --- | --- | --- |
| `no-react-specific-props` SC8011 | TS2322 — TypeScript's | false premise | **delete** |
| `no-unknown-namespaces` SC8012 | TS2322 — TypeScript's | false premise | **delete** |
| `no-innerhtml` SC8008 | TS2322 for the React prop | false premise | **delete** (§3.9) |
| `style-prop` SC8017 | partly TypeScript's | false premise | **delete** (below) |
| `jsx-no-duplicate-props` SC8003 | survives, gated | removed | **restrict** (§3.11) |

**`style-prop` is deleted outright (O3: a CSS-name-validity claim does not survive D4).** Removing the component arm
leaves exactly one candidate: a `-`-prefixed key on an intrinsic element.
`CSSProperties` carries `` [key: `-${string}`]: string | number | undefined ``,
so `tsc` accepts `-webkitAlignContent`, `-webkit-align-content`, and `-fooBar`
alike, and 2.0 lowers object keys through `nodeStyle.setProperty(s, v)`
(`@solidjs/web dist/dev.js:552`), which silently no-ops on an unrecognised
property. The drop is real — but the claim being made is **"this is not a valid
CSS property name."** That is generic CSS-name validation, not reactivity,
ownership, execution phase, timing, compiler lowering, or package runtime
behavior. D4 puts it out, consistently with removing `jsx-no-script-url` (generic
injection) and `no-proxy-apis` (generic target policy). The remaining arms go
independently: camelCase keys on intrinsic elements are TS2561, and
"a string `style` is replaced wholesale rather than patched in place" is a
granularity preference, not a defect.

### 3.9 `no-innerhtml` has no surviving Solid-specific domain — verified + judgment

| Arm | Verdict |
| --- | --- |
| `dangerouslySetInnerHTML` on an intrinsic element | Already TypeScript's; the page records the 2026-08-17 narrowing (TS2322). |
| `dangerouslySetInnerHTML` on a component | **False premise** (§3.8). The page claims it "arrives as an inert attribute"; it arrives as an ordinary prop. |
| `innerHTML` as an injection surface | Generic XSS advice. **D4** puts it out. |
| `innerHTML` conflicting with JSX children | The content-overwrite defect, owned by `jsx-no-duplicate-props` (§3.11). **D3**: one defect class, one rule. |
| "the value is not actually markup" | Generic HTML-validity advice. **D4** puts it out. |

Nothing is left that is both Solid-specific and proven. **Delete**, and its
`allowStatic` option with it.

### 3.10 `no-implicit-draggable` — deleted from both dialects — verified

**Affects:** deleting SC8019 from v1 and v2 (§4.1), retiring both external
identities.

**The v1 arm was inverted.** `draggable` is an enumerated attribute whose keywords
are `"true"` and `"false"`; the bare shorthand lowers to `draggable=""`
(`PlanValue::None` → `PlanDisposition::Inline(None)` → `append_bare_attribute`,
pinned 1.x compiler `dom/attrs.rs:315-325`), and `""` selects the invalid-value
default `auto`. On `<img>` and `<a href>` `auto` **is** draggable, so
`<img draggable />` does exactly what the author asked. On every other element
`auto` is not draggable, so `<div draggable />` is the only defective shape.

The implementation had it backwards and under-gated:
`rust/crates/solid-reactive-ir/src/static_rules.rs:158-165` hardcodes
`DraggableSpelling::Shorthand => DraggableDefault::Yes`, so the guard never fires
and the shorthand is reported on *every* element. `docs/rules/v1/no-implicit-draggable.md`
uses `<img draggable />` as its worked "Incorrect" example — the one element where
the shorthand is harmless. And simply inverting the guard would not have been
enough: `element_defaults_to_draggable` returns `No` for anything that is not
`img` or `a` (`:210-212`), so an inverted guard would report
`<my-widget draggable />`, whose behavior the checker cannot know.

**Why deletion rather than correction (D4).** A corrected arm would need a
standard-HTML-element census to separate "standard element, not draggable under
`auto`" from "custom or unknown element". None exists in the Rust crates:
`is_lowercase_led` (`upstream_compat/mod.rs:77-79`) only separates intrinsic from
component, and `DOMElements` appears solely as an export name in the contract
tables. That is new machinery built for a single claim about an HTML enumerated
attribute's serialized state — web-platform validation, which D4 puts out,
consistently with removing `style-prop` (CSS-name validity),
`jsx-no-script-url` (injection), `no-proxy-apis` (target policy), and
`valid-jsx-nesting` (HTML nesting).

**The v2 arm goes with it.** Literal `false` removing the attribute and selecting
a draggable `auto` is a real, probed lowering fact, but the claim it makes is the
same web-platform one. Keeping v2 alone would leave a rule whose two dialects
assert different halves of an HTML attribute's state machine, for no reactivity,
ownership, or timing consequence.

### 3.11 `jsx-no-duplicate-props`: one arm is a deliberate override, two are proven — verified + judgment

**Affects:** restricting SC8003.

The rule
(`rust/crates/solid-reactive-ir/src/upstream_compat/solid1x_syntax.rs:52`)
carries three arms, and they must be judged separately.

**(a) Exact duplicate keys across a spread boundary — remove.** After the
`tsc` narrowing, the residue is a spread followed by a same-named attribute, and
two different spread objects (`:144-146`). Both are the canonical override
idiom — `<div {...defaults} class="override" />` — which works exactly as
written; the rule's own hint concedes the mechanism: *"JSX keeps only the last
value written, so an earlier occurrence is dead and a later one silently wins."*
Later-wins is the *point*. Deliberate override is not a defect (§1). Remove this
arm.

**(b) Differently-spelled keys folded into one slot — keep, intrinsic only,
v1 only.** `onClick` and `onclick` both lower to the delegated `$$click` write;
`attr:title` and `title` share one static template slot. Two distinct, legal,
type-checked properties collapse and one is silently discarded, with no plausible
authorial intent and no `tsc` diagnostic. That is a proven silent loss. It is a
DOM-lowering behavior, so it is **intrinsic-only** — component keys pass through
verbatim (§3.8) and nothing folds. In 2.0 the arm has **no domain**: no lowercase
`on*` aliases and no `attr:` namespace, and `prop:value` versus `value` are two
different slots that both apply.

**(c) Content competition — keep, intrinsic only.**
`ChildProperties = Set(["innerHTML", "textContent", "innerText", "children"])`
(`@solidjs/web dist/dev.js:32`), and the compilers route every member to a
runtime property write while JSX children live in the template. Two content
sources on one intrinsic element therefore cannot both survive: each member is a
whole-content assignment. **The proof does not depend on knowing which wins** —
sharing one content slot establishes that at least one authored source is
discarded. On a component these are four ordinary props the component may combine
however it likes, so the arm is intrinsic-only. This arm ports to 2.0
(`innerHTML` is declared, `jsx.d.ts:889`).

**Net:** SC8003 survives with (b)+(c) in v1 and (c) only in v2.

### 3.12 `http-response-after-flush` is a hazard, not a violation — verified

**Affects:** changing SC7005's finding **kind**, not deleting it.

Every runtime premise checks out at the cited lines of
`@solidjs/web@2.0.0-rc.0 dist/server.js`: `httpStatus` gates on
`!response.committed` (`:2901`), `httpHeader` likewise (`:2935`), the single
commit point is `stub.committed = true` (`:2635`), and the client exports are
literal no-ops (`dist/dev.js:1921-1922`). The failure is silent — no throw, no
warning, no log — and no type can express it.

**But the page states plainly that the timing cannot be proven:** *"The drop is
**conditional**: the runtime still applies the write whenever the boundary
settles before the head commits… A static rule cannot prove which side a given
request lands on."*

Under §1 and the precision contract, an unprovable side of a race is an
`uncertifiable` result, not a violation issued at warning severity. Severity and
kind are independent axes, and the page currently uses the severity axis to
express doubt that belongs on the kind axis — `CONTEXT.md` defines finding kind
as *"**violation** (the analyzer proved the code misbehaves at runtime)"* versus
*"**uncertifiable** (a proof obligation the analyzer could not resolve)"*.

**Action:** report SC7005 as `uncertifiable`. Note the certification
consequence, which is a real behavior change: per
`docs/rules/README.md`, certification status is computed from a finding's *kind*,
so an `uncertifiable` SC7005 fails `--certify` until the head decision is moved
above the boundary, marked `deferStream: true`, or the rule is disabled. The
existing uncertifiable arm (rendering mode not visible) is unaffected.

### 3.13 Four undeclared double-reports — verified

`docs/rules/README.md` declares "one defect class, one rule" with one declared
exception. Scanning `fixtures/findings-snapshots/` for findings on byte-identical
spans:

| Pair | violation + violation | violation + uncertifiable |
| --- | --- | --- |
| `cleanup-in-forbidden-scope` + `no-owner-cleanup` | **4** | 1 |
| `pending-async-untracked-read` + `strict-read-untracked` | **3** | 1 |
| `component-returns-conditionally` + `strict-read-untracked` | **5** | 6 |
| `missing-effect-function` + `no-owner-effect` | **4** | 2 |

Only violation+violation is a breach. Two of these pairs dissolve on their own:
the first when v1 SC3001 is deleted and v2's is merged, the fourth when
SC4001–SC4004 merge into `missing-owner`. The remaining two need explicit
suppression: SC5001 should suppress SC1001 on the same read, and SC1004 should
own the condition read it reports.

### 3.14 `jsx-no-undef`'s tag arm is dead code — verified

`rust/crates/solid-reactive-ir/src/upstream_compat/solid1x_undef.rs` contains
exactly one `violations.push`, at line 57, inside the `use:` loop. The second
loop walks JSX tag names, applies guards, and terminates in `let _ = name;` — an
explicit no-op with the comment *"`EntitySymbols` contains only proven semantic
bindings. Its absence is deliberately uncertifiable."*

So the rule does not report undefined JSX tags. The defects are ~130 lines of
dead code and a page
(`docs/rules/v1/jsx-no-undef.md`) that opens *"Reports undefined JSX component
and `use:` directive names"* and spends most of its text on behavior that does
not exist. (For the record: real `tsc` 5.9.3 against 1.9.14 reports
`TS2304: Cannot find name 'Missing'` for `<Missing />`, so if the arm were live
it would duplicate TypeScript.)

The surviving `use:` arm is proven: TypeScript does not bind the local-name node
of a namespaced JSX attribute, and Oxc's semantic binder supplies an explicit
positive/negative result rather than an absence.

### 3.15 `valid-jsx-nesting`: the runtime covers it, better — verified + judgment

The 2.0 compiler emits expected tag names into the hydration walkers whenever the
build is hydratable and dev (`packages/compiler/src/dom/children.rs:917`):

```rust
if !self.hydratable || !self.dev {
    return self.child_walk_expression(span, parent, index);   // plain .firstChild
}
// else emit _$getFirstChild(parent, "div") / _$getNextSibling(prev, "div")
```

and the runtime checks them (`@solidjs/web dist/dev.js:1256`), warning
`"Hydration structure mismatch: expected <div> as first child of …"` with
`describeSiblings` (`:1273`) rendering the actual DOM. `getNextSibling` has the
same check at `:1264`; `getNextElement` adds a root tag check at `:1223-1228`.
For `<p><div>{x()}</div></p>` the browser hoists the `div` out, `p.firstChild` is
null, and the runtime names the exact node — a diagnostic a static rule cannot
match, because a static rule can only *predict* the parser.

**Why "the runtime warns" is decisive here and not generally.** Mirroring dev
warnings is the house pattern (SC1001, SC5001, the owner family). What separates
this rule is the cost: mirroring `STRICT_READ_UNTRACKED` reuses the owner graph
the checker must build anyway, while mirroring the parser needs a new subsystem —
the stack of open elements, three scope lists, the *special* category, the form
pointer, foster parenting, the adoption agency algorithm — serving one rule, at
`error` severity, with three known gaps (the page declares SVG breakout and
`option`/`select`; this audit adds a fourth: the page cites the WHATWG "in body"
insertion mode while Solid parses in "in template" mode, `t.innerHTML` on a
`<template>` — 1.x `web/dist/dev.js:212`, 2.0 `@solidjs/web dist/dev.js:364`).
And the claim is HTML validity, which D4 puts out.

Upstream — a pure syntax linter that could express this trivially — deliberately
does not ship it; `html-validate` and the Nu validator are maintained
implementations of the same spec.

**One slice is genuinely uncovered.** `getNextMarker` (`dist/dev.js:1237`) walks
siblings for `$`/`/` comment markers with no structural check, so dynamic
children in table/select context — where the parser foster-parents the markers
out of the table — hydrate silently wrong. Optional follow-up (§11), not part of
this plan.

**Asymmetry worth noting.** Solid 1.x has no structural hydration check at all —
no `getFirstChild(node, expectedTag)`, no "Hydration structure mismatch";
`getNextElement` only throws on a missing hydration key
(`web/dist/dev.js:372-382`). The plan still deletes it there, on cost and D4
grounds rather than redundancy.

### 3.16 v1 SC2001 fires on component-body writes — verified

**Affects:** restricting SC2001's v1 domain.

`rust/crates/solid-facts-backend/tests/dialects_process.rs:246-258` pins the
behavior over `tests/fixtures/semantic-component-identity/App.tsx`:

```rust
for (dialect, expected) in [
    ("solid-v2", vec![typed_offset]),                // setCount(1)
    ("solid-v1", vec![typed_offset, compat_offset]), // setCount(1) AND setCount(2)
] { … assert_eq!(writes, expected, "wrong component identity in {dialect}"); }
```

```tsx
const [count, setCount] = createSignal(0);
const lowercase: Component = () => { setCount(1); return null; };   // component body
function UppercaseFactory() { setCount(2);                          // v1 also reports this
```

So v1 reports component-body writes and reports **more** of them than v2.
`fixtures/reactive-ir/write-scope/App.tsx` shows the same shape for `setCount`,
`setAliased`, and `setNamespaced` inside `Counter`; that fixture has no
`node_modules`, so it resolves the v2 default dialect, where the finding is
correct — 2.0's dev guard genuinely throws
(`@solidjs/signals dist/dev.js:3171`, hit incidentally while writing an
unrelated probe).

**In 1.x this is a false positive.** A component body is not a tracked scope — it
runs once — and 1.x ships no write guard.
`docs/rules/v1/reactive-write-in-owned-scope.md` concedes it: *"Solid 1.x
tolerates these writes at runtime."* The genuine 1.x hazard is a write during a
**tracked computation**, which is the only shape the existing v1 fixtures
exercise (`engine__eslint-reactivity-v1.json`:
`createMemo(() => { setCount(count() + 1); … })`).

**Action:** restrict the v1 rule to genuinely tracked scopes — memo compute,
effect and render-effect compute, tracked JSX. v2 unchanged. This is a source
change *and* a test change, since `dialects_process.rs` pins the current
behavior.

### 3.17 `RETIRED_RULES` does not transfer a disable — verified

`rule-options.json` names are matched **exactly** —
`rust/crates/solid-reactive-ir/src/upstream_compat/solid1x_options.rs:265-270`:

```rust
/// enabled unless the project document explicitly disables their exact
/// external name.
pub fn is_enabled(&self, rule: &str) -> bool {
    !self.disabled.contains(rule)
}
```

`RETIRED_RULES` only keeps old configuration **loadable** —
`rust/crates/solid-facts-backend/src/dialect.rs:26-28`: *"Accepting a retired id
is **not** demoting the rule or hiding it behind an option … disabling it is a
no-op. Only the stale key is tolerated."* The check is a plain equality scan
(`:67-72`), consumed at `diagnostics.rs:1408`.

So any rename, name unification, namespace collapse, or merge leaves a project's
existing `enabled: false` pointing at a name nothing declares. The outcomes are:

1. name in `RETIRED_RULES` → the disable becomes a **silent no-op**, and
   previously suppressed diagnostics **reappear**;
2. name absent → **unknown rule name fails the whole analysis** (the deliberate
   typo protection).

Neither is what the project asked for, and a notice does not change either. §6.2
specifies the canonical aliases that preserve the semantics, and where an alias
is deliberately omitted.

**Inline suppressions are a separate, unpreservable channel.** The checker has no
suppression-comment mechanism of its own; suppression happens through ESLint's
`// eslint-disable-next-line solid-checker/<name>`, keyed by the plugin rule key.
No alias mechanism repairs those. The migration note must list old → new keys.

### 3.18 The parity corpus carries 465 cases and underwrites a gate — verified

D1 removes parity as a *product goal*; it does not make the corpus valueless.
`fixtures/upstream-parity/upstream-cases.json` holds **465 cases** across 19
upstream rules — 236 `valid`, 229 `invalid`. Each `invalid` case carries `code`,
`errors`, `messageIds`, `options`, `output`, and `tsOnly`: positive, negative,
**option**, **count**, **fix**, and **TypeScript-overlap** coverage.

| rule | total | | rule | total | | rule | total |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `reactivity` | **121** | | `jsx-no-undef` | 22 | | `no-react-deps` | 15 |
| `no-destructure` | **41** | | `components-return-once` | 20 | | `imports` | 14 |
| `event-handlers` | 28 | | `no-react-specific-props` | 20 | | `jsx-no-script-url` | 10 |
| `style-prop` | 28 | | `no-array-handlers` | 19 | | `prefer-for` | 10 |
| `self-closing-comp` | 25 | | `no-unknown-namespaces` | 19 | | `prefer-show` | 7 |
| `no-innerhtml` | 17 | | `no-proxy-apis` | 16 | | **TOTAL** | **465** |
| `prefer-classlist` | 17 | | `jsx-no-duplicate-props` | 16 | | | |

It also underwrites a gate: `scripts/parity-tsc-ownership.mjs:45` imports
`CORPUS, caseId, corpusCases, materialize` from `./lib/upstream-cases.mjs`, and
`make tsc-ownership` depends on `parity` (`Makefile:56-57`). That is the
span-level TypeScript-ownership gate — the mechanism that catches a rule drifting
back into `tsc`'s territory, the exact failure the absolute rule exists to
prevent. Deleting the corpus deletes the gate.

**Cases belonging to retained rules, to migrate:** `reactivity` (121, feeding
SC1001–SC1007), `no-destructure` (41), `components-return-once` (20),
`jsx-no-duplicate-props` (16, minus the removed spread-duplicate arm),
`prefer-classlist` (17), `prefer-for` (10), `prefer-show` (7), and the `use:`
subset of `jsx-no-undef` (22 total). The rest belong to deleted rules and are
dropped with an explicit count.

§8 specifies the replacement gate that must be operational before this corpus
is retired, and §10 sequences the migration.

### 3.19 The ESLint adapter constrains naming, and the contract schema is not a diagnostic schema — verified

**(1) The registry is one flat object keyed by rule name.**
`packages/cli/eslint.cjs:365-374`:

```js
const plugin = { meta: {…}, rules: { certification }, configs: {} };
for (const catalog of Object.values(manifests)) {
  for (const entry of catalog.rules) {
    plugin.rules[entry.name] = reportingRule(entry, catalog);
  }
}
```

`v1/strict-read-untracked` and `strict-read-untracked` are distinct keys today.
Two dialects sharing an unprefixed name would collide, and `manifests` is
iterated in discovery order (`rules-solid-v1.json` before
`rules-solid-v2.json`), so the v2 entry would overwrite the v1 entry. The same
collision would hit `docsUrlsByRule` (`:359-363`), a `Map` built by `flatMap`
over both catalogs, silently routing v1 documentation links to v2 pages. **This
is why D7 keeps the `v1/` namespace** — the flat registry and the static
`meta.docs.url` baked into each rule object (`:297`) both assume one key per
rule.

**(2) The namespace is what forces v1 analysis.** `eslint.cjs:311-314`:

```js
const forced =
  catalog.namespace && !configuration(context).dialect
    ? contextWithDialect(context, catalog.dialect)
    : context;
```

`rules-solid-v1.json` has `namespace: "v1"`; `rules-solid-v2.json` has
`namespace: ""`. A non-empty namespace is the trigger pinning a standalone v1
rule to `--dialect solid-v1`. Keeping the prefix keeps this working with no
change.

**(3) `schema/solid-reactivity.schema.json` is not a diagnostic schema.** It is
titled **"Solid Reactivity Package Contract"**, with top-level properties
`schemaVersion, package, compilerFactsProtocol, artifacts, summaries,
entrypoints, evidence`. The precision contract pins its `schemaVersion` at 1.
**This plan does not touch it.** Snapshots keep their existing
`{status, findings}` root.

**(4) External keys and internal rows are different things — and under D7 they
are equal.** The `v1/` prefix is an external ESLint rule-key convention; the
internal catalogs (`rust/dialects/solid-v{1,2}/rules/src/rules.rs`) and the
shipped manifests (`packages/cli/lib/rules-solid-v{1,2}.json`) are per-dialect
because each dialect owns its own severity, domain, and options for a shared
concept (SC1004 is a warning in v1, an error in v2). Because the prefix is
retained, each internal row has exactly one external key: **44 and 44** (§9).

---
### 3.20 The preference preset has no path to the native checker today — verified

**Affects:** §7, which must specify a complete enablement interface rather than
an ESLint config entry.

Four facts, all in shipping code:

1. **`RuleOptions` stores only a disabled set** —
   `rust/crates/solid-reactive-ir/src/upstream_compat/solid1x_options.rs:191-194`:

   ```rust
   pub struct RuleOptions {
       disabled: BTreeSet<String>,
       pub solid1x: Solid1xRuleOptions,
   }
   ```

   There is no representation for "explicitly enabled", so a default-disabled
   rule cannot be distinguished from an unset one.

2. **`is_enabled` receives only a name** (`:268-270`), so it cannot consult a
   catalog default:

   ```rust
   pub fn is_enabled(&self, rule: &str) -> bool { !self.disabled.contains(rule) }
   ```

3. **The adapter passes no enablement information to the spawned checker.**
   `packages/cli/eslint.cjs:127-135` builds the argv as
   `[...commandArgs, "--project", project, "--format", "json"]`, then appends
   `--dialect` and `--contract` only. An ESLint preferences config entry
   changes which ESLint rules run; it cannot reach into the analysed project's
   `.solid-checker/rule-options.json`, and it sends nothing to the checker.

4. **The snapshot cache key omits it** (`:111`):
   `JSON.stringify({ command, commandArgs, project, contracts, dialect })`.
   Even once a channel exists, two configurations differing only in preset would
   share one cached snapshot.

**Consequence.** With the three preferences default-disabled and no channel, the
native checker suppresses their findings before the ESLint preference rules could
report them, and the preset would silently do nothing. §7 specifies the
tri-state override, the catalog default, the CLI flags, the cache-key change, and
the tests that hold each link.

### 3.21 TypeScript ownership is already per-finding, not per-case — verified

**Affects:** §8.1's manifest shape.

`scripts/parity-tsc-ownership.mjs` keys its acknowledgements by
`<case id>:<rule>`, stated in the comment at `:111-115` and built at `:223`:

```js
const key = `${id}:${finding.rule}`;
if (ACKNOWLEDGED[key] || PENDING_NARROWING[key]) continue;
```

And the table genuinely needs that granularity — one upstream case carries two
entries for two different rules, with two different justifications:

```js
"no-array-handlers__invalid__05:v1/no-array-handlers": ARRAY_HANDLER,
"no-array-handlers__invalid__05:v1/event-handlers":    CORPUS_ARTEFACT,
```

A single source file routinely produces findings from several rules whose
relationships to TypeScript differ: one may be `checker-only`, another a
`distinct-claim` against an overlapping TS2322. A case-wide class cannot express
that, so the replacement manifest attaches ownership to each expected finding.

Note also that the existing gate has **two** acknowledgement tables —
`ACKNOWLEDGED` (deliberate distinct claims) and `PENDING_NARROWING` (overlaps
awaiting a fix). The replacement collapses both into the per-finding
`distinct-claim` class with a required justification; an overlap that is *not*
yet justified simply fails the gate, which is the behavior `PENDING_NARROWING`
was deferring.

---
## 4. Rule-by-rule actions

Names follow §6. SC codes are **labels, not identities** (§5.2).

### 4.1 Delete outright (16)

| Rule | Code | Present in | Grounds | Evidence |
| --- | --- | --- | --- | --- |
| `untracked-derived-function` | SC1006 | both | Reports the same runtime failure as SC1001 at a different anchor; its 2.0 module-scope gate reports what the runtime does not diagnose. D6 = migration path, so it goes from both. Lands with the SC1001 chain-following fix. | §3.3 |
| `no-implicit-draggable` | SC8019 | both | v1 arm inverted and under-gated; correcting it needs a standard-element census for a web-platform claim D4 excludes, and the v2 arm makes the same claim. | §3.10 |
| `no-async-tracked-scope` | SC5004 | v1 | Reports a shape. An async tracked scope with no post-`await` reactive read misbehaves in no way; the proven defect is the post-`await` read, owned by SC1002. | judgment |
| `event-handlers` | SC8001 | v1 | No arm proves a defect: canonical-casing is readability by its own admission, static values on hyphenated tags and unrecognised `on*` names are intent and explicit ambiguity, `warnOnSpread` is false. | §3.5, §3.6 |
| `jsx-no-script-url` | SC8004 | v1 | D4 — generic injection-sink rule. | judgment |
| `jsx-uses-vars` | SC8006 | v1 | Documented as never firing. | verified |
| `no-array-handlers` | SC8007 | v1 | The bound pair is an intentional supported runtime form; no fact distinguishes a matched pair from a mismatched one; its own flagship example is matched. | §3.7 |
| `no-innerhtml` | SC8008 | v1 | Component arm false; injection and HTML-validity arms out under D4; content-overwrite arm owned by SC8003 under D3. | §3.9 |
| `no-proxy-apis` | SC8009 | v1 | D4 — a project target-compatibility policy, unprovable from source. | judgment |
| `no-react-deps` | SC8010 | v1 | An array is a legal 1.x seed threaded into the callback; the trigger is `source.starts_with('[')`; the message is factually wrong; the `"safe"` fix changes callback input. | §3.4 |
| `no-react-specific-props` | SC8011 | v1 | Intrinsic arm is TS2322; component arm rests on a forwarding assumption both compilers contradict. | §3.8 |
| `no-unknown-namespaces` | SC8012 | v1 | Same shape — namespaced component keys are delivered verbatim. | §3.8 |
| `self-closing-comp` | SC8016 | v1 | Formatting. | judgment |
| `style-prop` | SC8017 | v1 | Component arm false; the one intrinsic residue is CSS-name validation, which D4 excludes; camelCase is TS2561; the string-form arm is a granularity preference. | §3.8 |
| `prefer-component-syntax` | SC8018 | both | Its own page: *"works at runtime … enforces a convention."* | verified |
| `valid-jsx-nesting` | SC8020 | both | The 2.0 runtime covers it with a better diagnostic at a fraction of the cost; the claim is HTML validity, which D4 excludes. | §3.15 |

### 4.2 Delete from v1 only — probed false premises (3)

The 2.0 copies are correctly grounded and stay, so these become v2-only.

| Rule | Code | Grounds |
| --- | --- | --- |
| `cleanup-in-forbidden-scope` | SC3001 | §3.1 |
| `primitive-in-leaf-owner` | SC3002 | §3.1 |
| `primitive-in-directive-application` | SC6001 | §3.2 |

Root cause for the first two is one line: `solid_1x.rs:285`.

**Not folded in.** Solid 1.x does have a genuine unowned-leaf case, but it is a
different primitive: `createRoot` with a **zero-arity** callback runs under the
shared `UNOWNED` sentinel (`dist/dev.js:162`) and `createComputation` skips child
registration for it (`:798`). Probed:

```
zero-arity root: owner=present     <- getOwner() is TRUTHY (it is UNOWNED)
one-arity root: owner=present
-- dispose one-arity --
one-arity cleanup RAN              <- the zero-arity cleanup never ran
```

No arity modelling exists in `owners.rs` or `solid_1x.rs`. New analysis
capability, recorded in `docs/precision-backlog.md` (§13) — not a reason to
retain SC3001/SC3002, whose premise is a different primitive.

### 4.3 Correct or restrict domain (3) — all retained

| Rule | Code | Change | Grounds |
| --- | --- | --- | --- |
| `reactive-write-in-owned-scope` | SC2001 | **v1 only:** restrict to genuinely tracked scopes — memo compute, effect and render-effect compute, tracked JSX. Remove the component-body and uppercase-named-function arms. v2 unchanged. | §3.16. Also update `dialects_process.rs:246-258`, which pins the current behavior. |
| `jsx-no-duplicate-props` | SC8003 | Remove the exact-duplicate-across-spread arm (deliberate override). Gate the folding arm and the content-competition arm to **intrinsic elements**. Folding is v1-only; content competition is shared. | §3.11 |
| `http-response-after-flush` | SC7005 | Report as **`uncertifiable`**, not a violation at warning severity. | §3.12. Behavior change: an uncertifiable finding fails `--certify`. |

### 4.4 Move internal (1)

| Rule | Code | Grounds |
| --- | --- | --- |
| `execution-map-incomplete` | SC9004 | D5 — its own page: *"an ordinary fresh project analysed with the bundled producer should not reach SC9004."* A producer-integrity assertion. |

### 4.5 Remove dead code (1)

| Rule | Code | Change |
| --- | --- | --- |
| `jsx-no-undef` | SC8005 | Delete the dead tag loop and its orphaned `#[cfg(test)]` helpers; rewrite the page to describe only the `use:` arm, which is proven (§3.14). Keeps its upstream name (O4). |

### 4.6 Port to 2.0 (3)

Each verified against the *published* 2.0 typings, not a fixture stub.

| Rule | Code | 2.0 evidence |
| --- | --- | --- |
| `prefer-for` | SC8014 | `For` present (`solid_2.rs:58`); `Index` removed (`rust/dialects/solid-v2/rules/src/lib.rs:628`). Fix hint must read `<For keyed={false}>`. |
| `prefer-show` | SC8015 | `Show` present (`solid_2.rs:80`). |
| `jsx-no-duplicate-props` content-competition arm | SC8003 | `innerHTML` declared (`jsx.d.ts:889`); `ChildProperties` is the same four-member set (`@solidjs/web dist/dev.js:32`). The folding arm has **no 2.0 domain** — no lowercase `on*` aliases, no `attr:`. |

`prefer-for` and `prefer-show` are ports of preference rules (O1, D2); both land
in the opt-in preset (§7), not the default config.

### 4.7 Keep, dialect-restricted (2)

| Rule | Code | Restriction | Grounds |
| --- | --- | --- | --- |
| `jsx-no-undef` | SC8005 | v1 only | `use:` directives do not exist in 2.0 (no `Directives` interface in `jsx.d.ts`). |
| `prefer-classlist` | SC8013 | v1 only | O1. `classList` absent from 2.0; `class` takes `ClassValue` including `Record<string, boolean>` (`jsx.d.ts:227-234`). |

### 4.8 Keep unchanged (14)

`strict-read-untracked` SC1001 *(plus the chain-following fix)*,
`reactive-read-after-await` SC1002, `no-destructure` SC1003,
`components-return-once` SC1004, `uncalled-accessor` SC1005,
`reactive-handler-frozen` SC1007 *(renamed)*,
`action-called-in-owned-scope` SC2002, `no-direct-mutation` SC2003,
`resolve-in-tracked-scope` SC2004 *(renamed)*, `missing-effect-function` SC7001,
`sync-computation-received-async` SC7002 *(renamed)*,
`server-function-module-directive` SC7006, `server-function-rich-argument`
SC7007, `reactive-source-uncaptured` SC9011 and `reactive-dispatch-unresolved`
SC9012 *(both uncertifiable by design)*.

---

## 5. Merges and the SC-code policy

### 5.1 The five merges (D3)

| New rule | Absorbs | v1 Δ | v2 Δ | Code retained | Disable transfer (O2) |
| --- | --- | --- | --- | --- | --- |
| `leaf-owner-forbidden-call` | SC3001, SC3002, SC3003 | — (deleted, §4.2) | −2 | **SC3001** | declared break |
| `missing-owner` | SC4001, SC4002, SC4003, SC4004 | −2 | −3 | **SC4001** | canonical alias (widens to the family) |
| `pending-async-unsuspendable-read` | SC5001, SC5002 | — | −1 | **SC5001** | declared break |
| `async-outside-loading-boundary` | SC5003, SC5005 | — | −1 | **SC5003** | declared break |
| `package-contract-incomplete` | SC9001, SC9005, SC9006 | −2 | −2 | **SC9005** | canonical alias (widens to the family) |

**Why each is sound.** `leaf-owner-forbidden-call`'s three members share a
byte-identical scope predicate, wrapper/factory handling, and SC9012 fallback,
and all three throw; SC3001 keeps its safe fix. `missing-owner`'s four members
are one class — an owner-requiring operation with no owner — and SC4001 already
carries per-finding severity, so SC4004's error tier needs no new machinery.
`pending-async-unsuspendable-read`'s two members are both "a pending async read
in a scope that cannot suspend". SC5005 is SC5003 plus two facts and a severity
bump, the escalation pattern SC4001 already expresses inside one rule; merging
also erases the SC5004/SC5005 numbering scar. `package-contract-incomplete`'s
three members are three granularities of one gap whose fix is always "write or
install a contract entry"; SC9006's generated JSON stub becomes a message
variant.

The merged leaf-owner rule also settles a naming inconsistency: SC3001/SC3003
said "forbidden scope" while SC3002 said "leaf owner" for the identical scope.

### 5.2 SC codes are labels, not identities

`rust/dialects/solid-v1/rules/src/rules.rs:15-19`:

> A rule that shares a concept with the Solid 2.0 dialect keeps its `SC` code …
> **Codes are labels, not identities: the variant is the identity.**

- The `Rule` enum variant is the identity; the name is the external handle; the
  code is a portable label guaranteeing only that one *concept* keeps one code
  across dialects.
- **A merge reuses a member's code** without claiming the merged rule "is" that
  former rule.
- **No new codes are minted, and no legacy codes are retained behind one name.**
  Each family keeps the code in §5.1 and the *message variant* identifies which
  sub-defect fired. Retaining several codes under one name would make the code a
  second identity — what the architecture denies — and break the
  one-concept-one-code invariant.

| Surface | Effect |
| --- | --- |
| Findings | A former SC4002 finding reports `SC4001` / `missing-owner` with a cleanup-specific message. `Finding.id` changes for every merged-away member. |
| Manifests | `packages/cli/lib/rules-solid-v{1,2}.json` regenerate via `SOLID_RULES_UPDATE=1 cargo test -p solid-v{1,2}-rules`; a plain run fails on drift (`the_shipped_manifest_is_the_catalog`). |
| Snapshots | Every `fixtures/findings-snapshots/*.json` touching a merged family changes `rule` **and** `code`, in the merge commit. |
| Docs | One page per merged rule; retired members' pages deleted. `every_rule_has_a_documentation_page` and the ≥400-byte guidance test must still pass. |
| Suppressions | Code changes do not help — both channels key on the **name** (§6.2). |
| Downstream consumers | Anything grepping `SC3002`, `SC3003`, `SC4002`, `SC4003`, `SC4004`, `SC5002`, `SC5005`, `SC9001`, `SC9006` stops matching. Permitted (codes are labels), user-visible, so it goes in the release notes. |

SC7005 keeps its code and name while changing **kind** (§4.3). Kind is not part
of the identity, but it changes `--certify` outcomes, so it also goes in the
release notes.

---

## 6. Naming and compatibility

### 6.1 Naming (D1)

> One concept, one name across dialects. Where upstream has that rule, keep
> upstream's name. Parity as a whole is not a goal.

Because D7 keeps the `v1/` prefix, "one name" means one *unprefixed stem*: the v1
external key is `v1/<stem>` and the v2 key is `<stem>`.

**Two stems must unify:**

| Code | Today | Unified stem |
| --- | --- | --- |
| SC1003 | `v1/no-destructure` + `component-props-destructure` | **`no-destructure`** |
| SC1004 | `v1/components-return-once` + `component-returns-conditionally` | **`components-return-once`** |

`component-returns-conditionally` is the better name and it loses, because
`components-return-once` is upstream's. SC1004's severity split (warning in v1,
error in v2) is deliberate adoption policy and survives as per-dialect metadata.

**Stems locked by D1**, after §4.1: `components-return-once`,
`jsx-no-duplicate-props`, `jsx-no-undef`, `no-destructure`, `prefer-classlist`,
`prefer-for`, `prefer-show`.

**Renames (3):**

| Code | From | To | Grounds |
| --- | --- | --- | --- |
| SC1007 | `expected-function-got-expression` | `reactive-handler-frozen` | An upstream *message id* that no longer describes the rule. |
| SC2004 | `resolve-in-reactive-scope` | `resolve-in-tracked-scope` | The guard is `getObserver()` — a tracked scope. |
| SC7002 | `sync-node-received-async` | `sync-computation-received-async` | "Node" is internal graph vocabulary. |

### 6.2 Compatibility — two channels, both handled

`RETIRED_RULES` keeps old configuration **loadable**; it does **not** transfer a
disable (§3.17). Both compatibility channels must be addressed separately.

#### Channel 1 — `.solid-checker/rule-options.json`

New machinery: `RULE_ALIASES: &[(&str, &str)]`, old external name → new external
name, consulted during rule-options parsing alongside `RETIRED_RULES`.

**Sequencing rule, and it is a correctness requirement, not a style choice.**
The machinery lands in an **isolated commit with an empty mapping table**. Every
individual alias is then added **atomically with the rename, merge, or
name-unification commit that creates its target** — steps 17 (merges) and 18
(renames and unifications). Activating an alias early
canonicalizes an old disabled key onto a name no current finding uses, which
silently *re-enables* the diagnostics the project had suppressed — the exact
failure the aliases exist to prevent.

Symmetrically, **every deleted rule receives its `RETIRED_RULES` entry in the
same commit as its deletion**, so no intermediate commit rejects a
previously-valid configuration as an unknown name.

#### External-identity audit — count names, not concepts

Every removed **external identity** needs a permanent entry, in the commit that
removes it. Concepts are not the unit: a rule present in both dialects has two
external names.

| Source | External names removed | Destination |
| --- | --- | --- |
| §4.1 deletions — 4 both-dialect concepts (`untracked-derived-function`, `prefer-component-syntax`, `no-implicit-draggable`, `valid-jsx-nesting`) × 2 | 8 | `RETIRED_RULES` |
| §4.1 deletions — 12 v1-only concepts | 12 | `RETIRED_RULES` |
| §4.2 v1-only deletions (`v1/cleanup-in-forbidden-scope`, `v1/primitive-in-leaf-owner`, `v1/primitive-in-directive-application`) | 3 | `RETIRED_RULES` |
| §4.4 SC9004 → internal (`v1/execution-map-incomplete`, `execution-map-incomplete`) | 2 | `RETIRED_RULES` |
| §5.1 `leaf-owner-forbidden-call` (v2) — declared break | 3 | `RETIRED_RULES` |
| §5.1 `pending-async-unsuspendable-read` (v2) — declared break | 2 | `RETIRED_RULES` |
| §5.1 `async-outside-loading-boundary` (v2) — declared break; the merged rule keeps SC5003's name, so only `ssr-client-source-outside-loading-boundary` goes | 1 | `RETIRED_RULES` |
| §5.1 `missing-owner` — alias policy; v1 3 names + v2 4 names | 7 | `RULE_ALIASES` |
| §5.1 `package-contract-incomplete` — alias policy; v1 3 + v2 3 | 6 | `RULE_ALIASES` |
| §6.1 renames and unifications | 6 | `RULE_ALIASES` |
| **Total** | **50** | **31 retired · 19 aliased** |

Two points the concept-level view hides:

- **Moving SC9004 internal removes catalog identities.** It is not a deletion in
  §4.1, but both its external names disappear from the manifests, so both need
  retirement entries — otherwise a project that had disabled
  `execution-map-incomplete` fails to load.
- **A merged rule with a *new* name retires every member's name, including the
  member whose code it keeps.** `leaf-owner-forbidden-call` keeps SC3001's code
  but not its name, so `cleanup-in-forbidden-scope` is retired in v2 alongside
  `primitive-in-leaf-owner` and `flush-in-forbidden-scope`.
  `async-outside-loading-boundary` is the one exception: it keeps SC5003's name
  as well as its code, so only the absorbed member's name is retired.

Per-step totals, for the execution table:

| Step | Removes | External identities |
| --- | --- | --- |
| 5 | `untracked-derived-function` both dialects | 2 |
| 6 | the three §4.2 v1-only deletions | 3 |
| 8 | `v1/no-implicit-draggable`, `no-implicit-draggable` | **2** |
| 9 | `v1/no-array-handlers` | 1 |
| 10 | `v1/no-react-deps`, `v1/event-handlers` | 2 |
| 11 | `v1/no-react-specific-props`, `v1/no-unknown-namespaces`, `v1/no-innerhtml`, `v1/style-prop` | 4 |
| 13 | 5 v1-only deletion keys + `v1/prefer-component-syntax` + `prefer-component-syntax` + `v1/execution-map-incomplete` + `execution-map-incomplete` | **9** |
| 14 | `v1/valid-jsx-nesting`, `valid-jsx-nesting` | **2** |
| 17 | five merges: 3 + 2 + 1 retired, 7 + 6 aliased | 19 |
| 18 | renames and unifications | 6 |
| **Total** | | **50** |

**One-to-one aliases (no semantic change):**

| Old name | New name | Lands with |
| --- | --- | --- |
| `expected-function-got-expression`, `v1/expected-function-got-expression` | `reactive-handler-frozen`, `v1/reactive-handler-frozen` | the SC1007 rename |
| `resolve-in-reactive-scope` | `resolve-in-tracked-scope` | the SC2004 rename |
| `sync-node-received-async` | `sync-computation-received-async` | the SC7002 rename |
| `component-props-destructure` | `no-destructure` | the SC1003 unification |
| `component-returns-conditionally` | `components-return-once` | the SC1004 unification |

Because D7 keeps the `v1/` namespace, there are **no** `v1/foo → foo` collapse
aliases in this plan.

**Merge-family aliases (O2, semantic — they widen).**

- `no-owner-effect`, `no-owner-cleanup`, `no-owner-boundary` (v1 and v2) plus
  `no-owner-settled-cleanup` (v2) → `missing-owner`. **7 aliases.**
- `package-contract-export-missing`, **`package-contract-missing`**, and
  `package-contract-callback-missing` → `package-contract-incomplete`, in both
  dialects. **6 aliases.** `package-contract-missing` is *not* exempt: the merged
  rule keeps SC9005's *code* but takes a **new name**, so the old name stops
  being declared and needs its own alias exactly like the other two. (The
  alternative is to keep `package-contract-missing` as the canonical merged name,
  which would drop those two aliases and leave 48 removed identities; this plan prefers
  the new name because it describes the merged concept, and pays the two
  aliases.)

Disabling one former member now disables the whole family; the migration note
must say so.

**The one merge that keeps its name.** `async-outside-loading-boundary` retains
both SC5003's code *and* its name, so only the absorbed member
(`ssr-client-source-outside-loading-boundary`) is retired. Every other merge
takes a new name and therefore retires or aliases **all** of its members' names,
including the member whose code survives.

**Declared breaks (O2).** `cleanup-in-forbidden-scope`,
`primitive-in-leaf-owner`, `flush-in-forbidden-scope`,
`pending-async-untracked-read`, `pending-async-forbidden-scope`,
`ssr-client-source-outside-loading-boundary` get `RETIRED_RULES` entries and **no
alias**. **Existing disables for these six stop applying, and any diagnostics
they were suppressing will reappear.** Every one is named in the release notes.

**Deleted rules** get `RETIRED_RULES` entries only — there is no successor, and
their disables correctly become no-ops.

#### Channel 2 — explicit ESLint configuration keys

A project may name a rule directly, e.g.
`"solid-checker/v1/expected-function-got-expression": "error"`. Renaming the
manifest entry removes that key from `plugin.rules`, and ESLint fails on an
unknown rule in an explicit config — this is a *separate* break from channel 1,
and `RULE_ALIASES` does not touch it.

**Policy for the three renames and two unifications:** retain a **deprecated
delegating entry** in `plugin.rules` under each old key for one minor release.
The delegating entry carries `meta.deprecated: true` and
`meta.replacedBy: ["<new key>"]`, and its `create` forwards to the new rule's
implementation. `plugin.configs.*` reference only the new keys. `eslint.cjs`
builds `plugin.rules` from the manifests (`:371-374`), so the delegating entries
come from an explicit `DEPRECATED_RULE_KEYS` table in the adapter, not from a
catalog row — the catalogs stay clean.

**Policy for merges and deletions:** **breaking**, no delegating entry. A merged
or deleted rule has no single successor with the same domain, and silently
forwarding `no-owner-boundary` to `missing-owner` in an explicit config would
change severity for three other defect classes. Named in the release notes.

#### Inline disable comments

`// eslint-disable-next-line solid-checker/<key>` cannot be preserved by any
mechanism here — the checker has no suppression-comment channel of its own
(§3.17), and ESLint surfaces a stale directive only when
`reportUnusedDisableDirectives` is enabled. **The migration note must list every
old → new key** so projects can grep. This applies to renames, unifications,
merges, and deletions alike.

### 6.3 D7 — resolved: keep the `v1/` namespace

Collapsing the external key namespace is orthogonal to catalog reduction, and
§3.19 shows it conflicts with the adapter's flat `plugin.rules` registry and with
the static `meta.docs.url` baked into each rule object. It is **removed from this
implementation sequence** and recorded as a follow-up design project (§13).

---

## 7. The preference preset (O5) — the enablement interface

Three rules survive without proving a defect: `prefer-classlist` (SC8013, v1),
`prefer-for` (SC8014), `prefer-show` (SC8015). D2 retains them; O5 keeps them out
of the default configuration.

**The naive design does not work**, for the reasons in §3.20: `RuleOptions` stores
only a disabled set, `is_enabled` receives only a name, the ESLint adapter passes
no enablement information to the spawned checker, and the snapshot cache key does
not include any. An ESLint config entry alone would leave the native
checker suppressing the findings before the ESLint rules could report them.

**One *semantic* enablement interface**, owned by `solid-reactive-ir` and
consumed by everything else: no default logic in the adapter, the backend, or
individual rules. It is reached by three of four entry points — CLI, ESLint, and
the daemon. **WASM is explicitly out of scope** (§7.3), because it has no
rule-options channel of any kind today.

### 7.1 Tri-state overrides, catalog defaults, and the dialect-owned lookup

```rust
pub enum RuleOverride { Unset, Enabled, Disabled }

pub struct RuleOptions {
    overrides: BTreeMap<String, RuleOverride>,   // replaces `disabled: BTreeSet<String>`
    requested_presets: BTreeSet<String>,
    requested_rules: BTreeSet<String>,
    pub solid1x: Solid1xRuleOptions,
}
```

`RuleMetadata` gains `default_enabled: bool` (true for all 25 proof-backed and
uncertifiable rules, false for the three preferences) and
`presets: &'static [&'static str]` (`["preferences"]` for the three, empty
otherwise).

**The filtering point needs metadata it cannot currently reach.** Today it is one
line — `rust/crates/solid-facts-backend/src/diagnostics.rs:394`:

```rust
findings.retain(|finding| identity.rule_options.is_enabled(&finding.rule));
```

`is_enabled` has only a name. To consult `default_enabled` and preset membership
without hard-coding rule names and without duplicating metadata onto every
`Finding`, the **dialect** answers by name. `crate::dialect::Dialect` (a struct of
function pointers, `dialect.rs:115`) gains one row beside the existing
`package_contract_finding`:

```rust
pub rule_metadata: fn(&str) -> Option<RuleMetadata>,
```

Each dialect implements it from its own catalog — `Rule::ALL` and
`Rule::metadata()` already exist, so it is a name lookup over the catalog the
dialect already owns. The filter becomes:

```rust
// Result-returning: a finding whose rule the selected catalog does not declare is
// a catalog/producer-integrity error, never an enabled finding.
fn retain_enabled(
    dialect: &Dialect,
    options: &RuleOptions,
    findings: &mut Vec<Finding>,
) -> Result<(), BackendError> {
    let mut unknown: Vec<String> = Vec::new();
    findings.retain(|finding| match (dialect.rule_metadata)(&finding.rule) {
        Some(meta) => options.is_enabled(&finding.rule, meta.default_enabled, meta.presets),
        None => {
            unknown.push(finding.rule.clone());
            false            // fail closed: never published
        }
    });
    if unknown.is_empty() { return Ok(()); }
    unknown.sort_unstable();
    unknown.dedup();
    Err(BackendError::UnknownRuleIdentity { dialect: dialect.id, rules: unknown })
}
```

**Fail closed, two ways, pick one.** Either the result-returning pass above — the
finding is dropped *and* the analysis fails with an explicit
`UnknownRuleIdentity` naming the dialect and the offending names — or an earlier
exhaustive validation that makes the lookup infallible: assert at startup that
every identity the IR can emit under this dialect resolves in its catalog, and
then take `RuleMetadata` by value rather than `Option`. This plan prefers the
result-returning pass, because the IR's identity surface is not enumerable from
one place today and a startup assertion would have to be maintained by hand.

Either way an unknown rule is **never published**. Silently enabling it would
publish a finding no catalog can name, no documentation page describes, and no
`rule-options.json` key can disable.

**Regression test** (`contract-process`): drive the filter with a synthetic
finding whose `rule` is absent from the selected catalog; assert the analysis
returns `UnknownRuleIdentity` naming that rule, and that no snapshot is emitted.
A companion assertion covers the benign direction — every identity in
`Rule::ALL` for both dialects resolves through `rule_metadata`.

with enablement decided in exactly one place:

```rust
pub fn is_enabled(&self, rule: &str, default_enabled: bool, presets: &[&str]) -> bool {
    match self.overrides.get(rule) {
        Some(RuleOverride::Disabled) => false,          // explicit disable always wins
        Some(RuleOverride::Enabled)  => true,           // explicit enable
        None | Some(RuleOverride::Unset) => {
            default_enabled
                || self.requested_rules.contains(rule)
                || presets.iter().any(|p| self.requested_presets.contains(*p))
        }
    }
}
```

**Precedence, highest first:** explicit `disabled` in `rule-options.json` →
explicit `enabled` in `rule-options.json` → `--enable-rule` → `--preset` →
catalog `default_enabled`.

### 7.2 Cache identity — two independent caches, both must key on enablement

There are **two** caches on the path, and `DiagnosticIdentity` protects only one
of them. Getting this wrong is silent: a request with a preset would be answered
from a cached snapshot in which the preference findings were already filtered
out.

#### 7.2.1 Retained diagnostic analysis — `DiagnosticIdentity`

`DiagnosticIdentity` (`diagnostics.rs:121-137`) already carries
`rule_options: RuleOptions`, with the comment that rule options are re-read every
analysis so an edited document invalidates a retained diagnostic. That property is
exactly what this feature needs — **provided the requested presets and rule
overrides are folded into `RuleOptions` before the identity is constructed.**

So this half is a sequencing requirement, not a new field: the transport values
are merged into `RuleOptions.requested_presets` and `.requested_rules` at the same
point the on-disk document is parsed, before `DiagnosticIdentity` is assembled.

#### 7.2.2 Daemon answer cache — `CachedAnswer`, and it needs new fields

**`DiagnosticIdentity` does not protect this cache**, because the daemon can
answer without ever reaching `DiagnosticSession::analyze`.
`daemon.rs:616-623`:

```rust
let Some(cached) = &state.last else { return Ok(None) };
let current = contract_files(state, &cached.modules, &check.contract_paths)?;
Ok(cached.snapshot_if_current(state.session.generation(), &check.contract_paths, &current))
```

and `daemon_cache.rs:12-32`:

```rust
pub(crate) struct CachedAnswer {
    pub(crate) generation: u64,
    pub(crate) explicit: Vec<String>,
    pub(crate) modules: Vec<String>,
    pub(crate) contract_files: Vec<ContractFile>,
    pub(crate) status: Arc<str>,
    pub(crate) body: Arc<[u8]>,
}

pub(crate) fn snapshot_if_current(&self, generation, explicit, contract_files) -> Option<CachedSnapshot> {
    (self.generation == generation && self.explicit == explicit && self.contract_files == contract_files)
        .then(|| (Arc::clone(&self.status), Arc::clone(&self.body)))
}
```

It returns the already-serialized `(status, body)` on a match of generation,
explicit contract paths, and contract files only. Enablement state is invisible
to it.

**Required changes:**

| Component | Change |
| --- | --- |
| `CheckRequest` (`daemon.rs:41-45`, today `{project_id, contract_paths}`) | Add `presets: Vec<String>` and `enable_rules: Vec<String>`, both `#[serde(default)]` so existing clients keep working |
| Normalization | Both are **sorted and deduplicated on receipt**, before they are stored or compared, so `["preferences"]` and `["preferences","preferences"]` — and any two orderings of a multi-element set — produce one cache entry |
| `CachedAnswer` | Store the **normalized** `presets: Vec<String>` and `enable_rules: Vec<String>` alongside `generation`, `explicit`, and `contract_files` |
| `snapshot_if_current` | Take both as parameters and compare them; a difference in either **misses** the cache even when generation, explicit paths, and contract files are all unchanged |
| Merge point | The same normalized values are folded into `RuleOptions` before `DiagnosticIdentity` is built (§7.2.1), so a daemon miss produces a correctly-keyed retained analysis rather than reusing a differently-configured one |

Both requirements stand independently. The answer cache guards the serialized
response; `DiagnosticIdentity` guards the retained analysis behind it. Fixing one
without the other leaves a stale path.

**Daemon-level tests** (`backend-process`):

| Test | Asserts |
| --- | --- |
| Preset change misses the answer cache | Two checks at the same generation, same contract paths and files, differing only in `presets`, produce two distinct responses; the second reports `cacheHit: false` |
| Enabled-rule change misses the answer cache | Same shape, differing only in `enableRules` |
| Order-independent normalization shares one entry | `["a","b"]` then `["b","a"]` hits the cache (`cacheHit: true`); a duplicate-laden list normalizes to the same key |
| Absent fields are backward-compatible | A `CheckRequest` with neither field behaves exactly as today and shares a cache entry with an explicit empty list |
| Retained analysis is separately keyed | A preset change that misses the answer cache also misses the retained `DiagnosticIdentity`, rather than re-serializing a stale analysis |

### 7.3 Entry points

| Entry point | Transport | Status |
| --- | --- | --- |
| CLI | `--preset <name>`, `--enable-rule <name>`, both repeatable | **supported** |
| ESLint adapter | adapter options `preset: string[]` / `enableRule: string[]`, appended to the spawned argv beside `--project`, `--format`, `--dialect`, `--contract` (`eslint.cjs:127-135`); both added to the snapshot cache key (`:111`), sorted so key identity is order-independent | **supported** |
| Daemon / retained session | `CheckRequest` carries normalized `presets` / `enableRules`; they are compared by `CachedAnswer::snapshot_if_current` **and** folded into `RuleOptions` before `DiagnosticIdentity` — both caches keyed (§7.2) | **supported** |
| WASM (`CheckRequest`) | — | **out of scope — see below** |

**WASM is explicitly out of scope, and the reason is pre-existing.**
`packages/wasm/index.d.ts:16-28` declares
`CheckRequest { projectId, dialect?, generation, sources, typeFacts }`. There is
no rule-options channel of any kind: the WASM build states it "cannot inspect a
node_modules tree", and nothing in `packages/wasm/` reads
`.solid-checker/rule-options.json`. So WASM today honours **no** per-rule
configuration at all — not presets, not `enabled: false`, nothing.

This plan therefore does **not** claim a universal enablement interface. It
claims one *semantic* interface (§7.1) reached by three of four entry points. The
WASM adapter behaves as if no rule options were supplied, which means the three
preference rules never fire there — consistent with their default, and unchanged
from today's behavior for every other rule's options. Extending `CheckRequest`
with `ruleOptions` / `presets` / `enableRules` is recorded as a follow-up (§13);
until then `packages/wasm/README.md` must state the limitation.

### 7.4 ESLint configs must be dialect-safe

**One `preferences` config cannot work.** A `v1/`-prefixed rule key forces v1
analysis — `catalog.namespace && !configuration(context).dialect` triggers
`contextWithDialect(context, catalog.dialect)` (`eslint.cjs:311-314`). A single
config listing both `v1/prefer-show` and `prefer-show` would force v1 analysis
for the v1 keys while the unprefixed keys leave dialect selection to detection,
so on a v2 project the v1 keys would spawn a second, wrong-dialect analysis and
the v2 keys would report from the right one.

Two configs, mirroring the existing per-dialect `v1` / `v2` configs:

| Config | Rule keys | Adapter settings |
| --- | --- | --- |
| `preferences-v1` | `solid-checker/v1/prefer-classlist`, `solid-checker/v1/prefer-for`, `solid-checker/v1/prefer-show` | `preset: ["preferences"]` |
| `preferences-v2` | `solid-checker/prefer-for`, `solid-checker/prefer-show` | `preset: ["preferences"]` |

There is deliberately no combined `preferences` config: composing
`preferences-v1` with a v2 project is a configuration error the user should make
explicitly, not one a convenience alias hides.

**`--preset preferences` stays dialect-neutral** on the native side. The preset
name is resolved against the *selected* catalog's `presets` metadata, so the same
flag enables three rules under v1 and two under v2 with no dialect-specific
spelling. Only the ESLint rule-key surface needs the split, because only it is
namespaced.

**The generated manifests transport the catalog decision to the adapter.** Each
rule entry gains `defaultEnabled` and `presets`, generated from the Rust
`RuleMetadata`; the adapter does not maintain a second list of preference rule
names. It derives all ESLint behavior from those fields:

- `meta.docs.recommended` is
  `entry.defaultEnabled && !entry.uncertifiable`, so an opt-in preference is not
  advertised as recommended;
- the ordinary `v1` / `v2` configs include only `defaultEnabled` rule keys;
- `preferences-v1` / `preferences-v2` select entries whose `presets` contain
  `"preferences"` and attach the matching adapter setting from the table above.

**An explicitly enabled ESLint rule must work without a second hidden switch.**
Configuring `"solid-checker/prefer-show": "warn"` is itself an opt-in. Before any
`Program` listener runs, every enabled reporting rule has already registered in
the existing per-file `ownedRules` set (`eslint.cjs:15-22`). `loadSnapshot`
therefore unions the configured `enableRule` values with the active
default-disabled rule names from that set, sorts and deduplicates them, appends
one `--enable-rule` argument per name, and includes the normalized list in the
snapshot cache key. Multiple explicitly enabled preferences still share one
native analysis. Proof-backed rules need no injected flag because their catalog
default is already enabled.

### 7.5 Required tests

| Test | Location | Asserts |
| --- | --- | --- |
| Preferences absent by default | `contract-process` | No preset, no rule-options → no SC8013/SC8014/SC8015 finding; `--certify` passes on a project whose only issues are preferences |
| Explicit enable in rule-options | `ir-lib` + `contract-process` | `{"v1/prefer-show": {"enabled": true}}` alone emits the finding, with no preset |
| ESLint preset reaches the native checker | `npm test --prefix packages/cli` | `preferences-v1` produces SC8015 findings, asserted against the spawned argv containing `--preset preferences`, not only the reported output |
| Explicit disable beats the preset | `contract-process` | `--preset preferences` plus `{"v1/prefer-show": {"enabled": false}}` emits nothing |
| Preset participates in cache identity | `contract-process` | Two analyses of one project differing only in `--preset` do not share a retained result |
| One run shared | `npm test --prefix packages/cli` | Two enabled rules in one config cause exactly one spawn; changing the preset list causes a second |
| `preferences-v2` does not force v1 | `npm test --prefix packages/cli` | Linting a v2 project with `preferences-v2` spawns exactly one analysis, with no `--dialect solid-v1` |
| WASM limitation is honest | `npm test --prefix packages/wasm` | A `CheckRequest` produces no preference findings, and the documented limitation is asserted rather than silently assumed |
| Explicit ESLint rule is an opt-in | `npm test --prefix packages/cli` | Enabling only `solid-checker/prefer-show` emits SC8015 and the shared spawn receives `--enable-rule prefer-show` without a separate adapter option |
| Dialect configs preserve the default | `npm test --prefix packages/cli` | The generated `v1` / `v2` configs omit every `defaultEnabled: false` entry; composing the matching preference config adds only that dialect's preference keys |
| ESLint recommendation metadata follows the catalog | `npm test --prefix packages/cli` | All three preferences expose `meta.docs.recommended: false`; proof-backed default-enabled rules remain recommended unless uncertifiable |
| Manifest metadata stays generated | `contract-process` + manifest validation | `defaultEnabled` and `presets` in both shipped manifests equal the owning Rust catalog metadata; no adapter-maintained preference-name list exists |

### 7.6 Release-note consequence

A project relying on `prefer-show` or `prefer-for` firing under `recommended` or
a dialect config stops seeing those findings until it adds `preferences-v1` or
`preferences-v2`. Directly enabling an individual ESLint preference rule remains
a valid opt-in and needs no additional adapter setting. This is intentional and
must be listed by rule name.

---

## 8. The replacement ownership gate

Ordinary reactive-IR fixtures plus findings snapshots do **not** preserve what the
upstream corpus carries. A snapshot records `rule`, `code`, `kind`, `severity`,
`path`, `start`, `end`, and a fix count — not expected *absence*, not per-case
rule options, not expected fix *output*, and not the relationship between a
checker finding and a `tsc` diagnostic at the same span. The corpus's 465 cases
carry all of those (§3.18), and `make tsc-ownership` is built on them.

**Ownership is per finding, not per case** (§3.21). One source file routinely
produces findings from several rules whose relationships to TypeScript differ.

### 8.1 Case manifest format

New artifact `fixtures/ownership-cases/cases.json`, plus a README describing the
three ownership classes.

```jsonc
{
  "schemaVersion": 1,
  "cases": [
    {
      // Stable, never reused — not even after a case is deleted.
      "id": "no-destructure/positive/007",
      "dialect": "solid-v1",

      // Exact materialization inputs. The runner writes prelude + text verbatim.
      // caseBytes is DERIVED as [len(prelude), len(prelude) + len(text)] in
      // UTF-8 bytes and is never written by hand.
      "source": {
        // Extension only. The runner derives the on-disk path from `id` (§8.3),
        // so grouped cases cannot collide.
        "extension": ".tsx",
        "prelude": "import { createSignal } from \"solid-js\";\n",
        "text": "const C = (props: { a: string }) => { const { a } = props; return <p>{a}</p>; };\n"
      },

      // Per-case options, merged into a synthetic rule-options.json for the run.
      "ruleOptions": { "v1/no-destructure": { "enabled": true } },
      "presets": [],
      "enableRules": [],

      "expect": {
        // Exactly these findings, in any order. An unlisted finding fails.
        "findings": [
          {
            "rule": "v1/no-destructure",
            "code": "SC1003",
            "kind": "violation",
            "severity": "error",
            // Span relative to source.text, by marker. See §8.2.
            "span": { "marker": "{ a } = props" },
            "fix": { "behavior": "none" },
            "typescript": { "ownership": "checker-only", "diagnostics": [] }
          }
        ],
        // Rules or families that must produce nothing for this case.
        "absent": []
      }
    },

    {
      // A TypeScript-owned case: the checker must be silent AND the diagnostic
      // must be present. Both halves are asserted.
      "id": "no-react-specific-props/typescript-owned/001",
      "dialect": "solid-v1",
      "source": {
        "extension": ".tsx",
        "prelude": "",
        "text": "const C = () => <label className=\"field\">Email</label>;\n"
      },
      "ruleOptions": {},
      "presets": [],
      "enableRules": [],
      "expect": {
        "findings": [
          {
            "rule": "v1/no-react-specific-props",
            "code": "SC8011",
            // No kind/severity: this describes a finding that must NOT exist.
            "span": { "marker": "className=\"field\"" },
            "typescript": {
              "ownership": "typescript-owned",
              "diagnostics": [
                { "code": "TS2322", "span": { "marker": "className" } }
              ]
            }
          }
        ],
        "absent": []
      }
    },

    {
      // A genuine negative: nothing owns this span, in either direction.
      "id": "no-destructure/negative/002",
      "dialect": "solid-v1",
      "source": {
        "extension": ".tsx",
        "prelude": "",
        "text": "const C = (props: { a: string }) => <p>{props.a}</p>;\n"
      },
      "ruleOptions": {},
      "presets": [],
      "enableRules": [],
      "expect": {
        "findings": [],
        // A negative case names the rule or family that must stay absent.
        "absent": [
          { "rule": "v1/no-destructure",
            "reason": "the prop is read through a member access, which stays subscribed" }
        ]
      }
    }
  ]
}
```

**`typescript.ownership` — three classes.** `not-typescript` is deliberately
**absent**: the absolute rule applies to code that contains type errors just as
much as to code that does not, and upstream's `tsOnly` flag only records that
upstream ran the case under a TypeScript parser. It never exempts a case from
ownership checking.

| Class | Requires | Gate failure |
| --- | --- | --- |
| `checker-only` | No `tsc` diagnostic overlaps this finding's span | An overlapping diagnostic fails — the rule has drifted into TypeScript's territory |
| `typescript-owned` | The checker finding is **absent**, and every listed `tsc` diagnostic is **present** at its span | A checker finding at that span fails; a missing expected diagnostic also fails |
| `distinct-claim` | Both present at overlapping spans, with a **required** finding-specific `justification` naming what the checker asserts that the diagnostic does not | A missing or empty justification fails |

`diagnostics` is **verified, not decorative**: each entry is
`{ "code": "TS2322", "span": { "marker": "…" } }`, and the runner asserts the
diagnostic exists at that resolved span. A `checker-only` finding carries an
empty list and the runner asserts no overlap.

For a `typescript-owned` expectation the entry lives in `expect.findings` with
`kind`/`severity` omitted — it describes a finding that must not exist — and its
`rule` names what must stay silent.

This replaces `deviations.json`'s three tables and
`parity-tsc-ownership.mjs`'s `ACKNOWLEDGED` and `PENDING_NARROWING` maps: an
intentional overlap becomes a `distinct-claim` finding entry with its
justification attached to the finding, at the same granularity the existing gate
already needs (§3.21).

### 8.2 Spans: markers, never hand-computed offsets

Absolute byte arithmetic is not maintainable by hand. A span is written one of two
ways, both relative to `source.text`:

| Form | Example | Resolution |
| --- | --- | --- |
| Marker | `{ "marker": "{ a } = props" }` | The runner finds the marker in `text`. **Rejected if it occurs more than once** unless `"occurrence": <1-based>` is given. |
| Explicit text range | `{ "textRange": [44, 57] }` | UTF-8 byte offsets **within `text`**, for spans no substring names cleanly |

The runner converts either form to a full-file offset by adding
`len(prelude)` in UTF-8 bytes, because checker spans and `tsc` spans are both
byte offsets. For the first example above, mechanically:

| Quantity | Value |
| --- | --- |
| `len(prelude)` | 41 |
| `len(text)` | 81 |
| derived `caseBytes` | `[41, 122]` |
| marker `{ a } = props` in `text` | `[44, 57]` |
| resolved file span | `[85, 98]` |

**UTF-8 is in the proof set, not an afterthought.** Character index and byte
index diverge the moment a case contains non-ASCII source — for
`const label = () => "café";\nconst C = () => <p>{label}</p>;\n`, the marker
`{label}` is at character 47 and byte **48**. At least two proof-set cases carry
non-ASCII source, one with the marker *after* the non-ASCII text, so a
char-vs-byte confusion fails the gate rather than silently shifting a span.

### 8.3 Runner

New `scripts/ownership-gate.mjs`, new target `make ownership-gate`.

1. **Validate the manifest** before running anything: every `id` unique; every
   marker resolvable and unambiguous (or explicitly disambiguated); every
   `distinct-claim` finding carrying a non-empty justification; every negative
   case naming at least one `absent` rule or family.
2. **Group** by `(dialect, ruleOptions, presets, enableRules)` so the checker runs
   once per distinct configuration.
3. **Materialize** each group into a temporary project seeded from
   `rust/target/tsc-oracle/<dialect>`, writing `prelude + text` verbatim plus a
   synthetic `.solid-checker/rule-options.json`. **Each case gets its own file**,
   at a path derived from its stable `id` — `/` and other path-unsafe characters
   replaced by `__`, `source.extension` appended — so
   `no-destructure/positive/007` materializes as
   `cases/no-destructure__positive__007.tsx`. Grouped cases therefore never
   collide, and a case's on-disk name is stable and greppable. Manifest
   validation rejects two cases whose derived paths collide.
4. **Run the checker** with `SOLID_CHECKER_BIN` / `SOLID_TYPEFACTS_BIN`, passing
   `--preset` / `--enable-rule` from the group (§7.2).
5. **Run `tsc --noEmit` on the identical bytes**, in both the strict and
   non-strict passes.
6. **Resolve every span** to full-file byte offsets (§8.2).
7. **Compare, per finding:**
   - each `expect.findings` entry with a `kind` matches exactly one checker
     finding by `(rule, code, span)`, and its `kind`/`severity` agree;
   - each entry classified `typescript-owned` matches **no** checker finding;
   - every checker finding inside `caseBytes` is claimed by exactly one entry —
     an unlisted finding fails;
   - each `absent` rule or family produced nothing inside `caseBytes`;
   - `fix.behavior` agrees (`none` / `safe` with an expected post-fix `text` /
     `unsafe`), and when a fix output is given, applying the safe fixes yields it;
   - overlap with `tsc` diagnostics is computed **within `caseBytes`** and judged
     against the finding's ownership class, and every listed expected diagnostic
     is present at its resolved span.
8. **Report** a per-case, per-finding pass/fail table; non-zero exit on any
   failure.

Diagnostics outside `caseBytes` are prelude noise and are ignored, which is what
makes a shared prelude safe.

### 8.4 Migration phases — no coverage gap

The retirement condition is explicit and checkable:

> The parity comparison may be retired only when **every still-active
> retained-rule case is covered by `make ownership-gate`**, asserting that
> case's **current** behavior. No case may lose coverage because its expectation
> is scheduled to change later.

| Phase | Commit | What moves |
| --- | --- | --- |
| **1 — build** | step 1 | Format, runner, `make ownership-gate`, and a ~24-case proof set spanning all three ownership classes, all six coverage dimensions, both dialects, both span forms, and at least two non-ASCII cases. `make parity` and `make tsc-ownership` untouched and green. |
| **2 — migrate everything retained** | step 2 | **Every** case belonging to a retained rule, encoded with its **current** behavior and **current** expectation — including cases this plan will later change. A case destined to become negative is migrated **as positive**, because that is what the code does today. Both old gates still green. |
| **3 — retire** | step 3 | The parity comparison stops being a gate. Permitted only because phase 2 left no retained-rule case uncovered. `make tsc-ownership` is superseded by `make ownership-gate`. |
| **4 — flip atomically** | steps 5–20 | Each semantic commit updates or flips its own cases in the same commit: `outcome` positive → negative, spans, counts, fix outputs, and ownership classes. Nothing is migrated ahead of the behavior it describes, and nothing lags behind it. |
| **5 — drop the corpus** | step 21 | `upstream-cases.json` and `deviations.json` deleted. |

**Deleted-rule cases** are *not* migrated in phase 2. They retire in the same
commit that deletes their rule (steps 5, 6, 8–14), each one dropped explicitly
rather than by omission.

**The 465-case reconciliation** is a checked-in ledger,
`fixtures/ownership-cases/migration-ledger.json`, one row per upstream case:

```jsonc
{ "upstreamCase": "no-destructure__invalid__05",
  "disposition": "pending",              // "pending" | "migrated" | "dropped"
  "movedIn": null,                       // null while pending; "step-2", "step-9", … once resolved
  "ownershipCaseId": null,               // required when migrated
  "reason": null }                        // required when dropped
```

**Staged validation, so the ledger is usable from step 1 rather than only at the
end.** `make ownership-gate` enforces, at every stage:

| Rule | From |
| --- | --- |
| The ledger contains **exactly** the 465 upstream case identities — no additions, no omissions, no duplicates | step 1 |
| Every row is `pending`, `migrated`, or `dropped` | step 1 |
| A `migrated` row has a non-null `movedIn` and an `ownershipCaseId` that resolves to a real case | step 1 |
| A `dropped` row has a non-null `movedIn` and a non-empty `reason` | step 1 |
| A `pending` row has `movedIn`, `ownershipCaseId`, and `reason` all null | step 1 |
| Every retained-rule case is `migrated` | **step 3** (the parity-retirement condition) |
| **Zero** `pending` rows | **step 21** (corpus retirement) |

Step 1 seeds all 465 rows as `pending`. Step 2 flips every retained-rule row to
`migrated`. Steps 5–20 flip the rows their commit affects — a deleted rule's
cases to `dropped` with a reason, a changed case's `ownershipCaseId` to its new
entry. **No commit may leave a row it touched still `pending`**, and step 21
refuses to delete the corpus while any row is. "When did each case move" is
answerable from the repository rather than from commit archaeology.

---
## 9. Arithmetic

| Quantity | Now | After |
| --- | --- | --- |
| **Distinct concepts** | 54 | **28** |
| **Internal per-dialect rows** (catalogs + shipped manifests) | 79 (42 v1 + 37 v2) | **44** (18 v1 + 26 v2) |
| **External ESLint keys** | 79 | **44** |

Internal rows and external keys are equal because D7 retains the `v1/` prefix:
each per-dialect row has exactly one external key.

The three preference rules (§7) are **included** in all three counts. Moving them
behind a preset changes their default, not their membership.

### Derivation from the action tables

Today: 25 shared + 17 v1-only + 12 v2-only = 54.

| Movement | Shared | v1-only | v2-only |
| --- | --- | --- | --- |
| start | 25 | 17 | 12 |
| §4.1 deleted outright — 4 shared (SC1006, SC8018, SC8019, SC8020), 12 v1-only | −4 → 21 | −12 → 5 | — |
| §4.2 SC3001/SC3002/SC6001 move from shared to v2-only | −3 → 18 | — | +3 → 15 |
| §4.4 SC9004 → internal | −1 → 17 | — | — |
| §5.1 `missing-owner` absorbs SC4002, SC4003 (shared) and SC4004 (v2-only) | −2 → 15 | — | −1 → 14 |
| §5.1 `leaf-owner-forbidden-call`: 3 v2-only members → 1 | — | — | −2 → 12 |
| §5.1 `pending-async-unsuspendable-read`: 2 → 1 | — | — | −1 → 11 |
| §5.1 `async-outside-loading-boundary`: 2 → 1 | — | — | −1 → 10 |
| §5.1 `package-contract-incomplete` absorbs SC9001, SC9006 | −2 → 13 | — | — |
| §4.6 ports move SC8003, SC8014, SC8015 from v1-only to shared | +3 → **16** | −3 → **2** | — |
| **final** | **16** | **2** | **10** |

16 + 2 + 10 = **28**. Rows: v1 = 16 + 2 = **18**; v2 = 16 + 10 = **26**;
total **44**. Keys = rows = **44**.

**Enablement split:** 21 proven + 4 uncertifiable = **25 enabled by default**,
plus **3 opt-in preferences** (§7) = 28.

`style-prop` and `no-implicit-draggable` appear in no column: the v1-only
`style-prop` is deleted and never ported to v2, while `no-implicit-draggable` is
deleted from both dialects (§4.1).

### The 28, enumerated

*Shared (16):* `strict-read-untracked` SC1001 · `reactive-read-after-await`
SC1002 · `no-destructure` SC1003 · `components-return-once` SC1004 ·
`uncalled-accessor` SC1005 · `reactive-handler-frozen` SC1007 ·
`reactive-write-in-owned-scope` SC2001 *(v1 restricted)* · `no-direct-mutation`
SC2003 · `missing-owner` SC4001 *(merged)* · `missing-effect-function` SC7001 ·
`jsx-no-duplicate-props` SC8003 *(restricted; v1 = folding + content, v2 =
content only)* · `prefer-for` SC8014 *(preset)* · `prefer-show` SC8015
*(preset)* · `package-contract-incomplete` SC9005 *(merged)* · `reactive-source-uncaptured`
SC9011 · `reactive-dispatch-unresolved` SC9012

*v1-only (2):* `jsx-no-undef` SC8005 · `prefer-classlist` SC8013 *(preset)*

*v2-only (10):* `action-called-in-owned-scope` SC2002 ·
`resolve-in-tracked-scope` SC2004 · `leaf-owner-forbidden-call` SC3001
*(merged)* · `pending-async-unsuspendable-read` SC5001 *(merged)* ·
`async-outside-loading-boundary` SC5003 *(merged)* ·
`primitive-in-directive-application` SC6001 ·
`sync-computation-received-async` SC7002 · `http-response-after-flush` SC7005
*(uncertifiable)* · `server-function-module-directive` SC7006 ·
`server-function-rich-argument` SC7007

### The optional rule

`prefer-class-object` (§13) is a **v2-only** analogue of `prefer-classlist` —
2.0 removed `classList`, so the v1 rule cannot share its implementation or its
domain. Adding it yields **29 concepts, 45 rows** (v1 18 + v2 27), **45 keys**.
It is *not* a shared entry; a genuinely shared rule would add two rows and yield
46.

---
## 10. Execution order

**23 execution steps comprising 31 commits.** Steps 17 and 18 are each five
commits (one per merge, and one per rename or unification); every other step is
one commit. Use `.claude/skills/green-commits/SKILL.md` for the slicing.

**Green-per-commit invariants**, enforced by the ordering below:

- the replacement ownership gate is **operational and proven** before the parity
  comparison is retired (steps 1–3);
- **every retained-rule case is under the new gate at its current expectation
  before retirement** (step 2), so no case is unguarded between step 3 and the
  semantic commit that later changes it;
- a case's expectation is **flipped in the commit that changes its behavior**,
  never earlier and never later;
- every external identity removed gets its `RETIRED_RULES` or `RULE_ALIASES`
  entry, and its `migration-ledger.json` row, in the same commit (§6.2 audit);
- `RULE_ALIASES` machinery lands with an **empty table** (step 4); each alias is
  activated **atomically with the identity change that creates its target**;
- each deleted rule's `RETIRED_RULES` entry lands **in its deletion commit**;
- manifests, focused snapshots, rule pages, and migration-note lines travel with
  the change that requires them;
- **no namespace collapse anywhere** (D7);
- expensive verification runs **only at handoff** (step 23).

Provision and build once. A stale `bin/solid-checker-rust` makes every coverage
run look like a no-op:

```bash
make tsc-oracle-provision
cargo +1.97 build --manifest-path rust/Cargo.toml
```

Every coverage or gate run needs both
`SOLID_CHECKER_BIN="$PWD/rust/target/debug/solid-checker-rust"` and
`SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts"`.

| # | Commit | Carries | Focused checks |
| --- | --- | --- | --- |
| 1 | Ownership-gate format + `scripts/ownership-gate.mjs` + `make ownership-gate` + ~24-case proof set: all three ownership classes, all six coverage dimensions, both dialects, both span forms, ≥2 non-ASCII (§8.1–8.3) | `fixtures/ownership-cases/{cases.json,README.md,migration-ledger.json}` | `make ownership-gate`; ledger seeded with all 465 rows as `pending`; `make parity` and `make tsc-ownership` still green |
| 2 | Migrate **every** retained-rule case at its **current** behavior and expectation — including cases later steps will change (a future negative is migrated as positive) | manifest entries + ledger rows marked `step-2` | `make ownership-gate`; both old gates still green; ledger covers every retained-rule case |
| 3 | Retire the parity **comparison** as a gate — permitted only because step 2 left no retained-rule case uncovered (§8.4) | `Makefile`, `scripts/`; `make tsc-ownership` superseded | `make ownership-gate` with the **all-retained-migrated** stage enabled: every retained-rule row is `migrated` with a resolvable `ownershipCaseId` |
| 4 | `RULE_ALIASES` machinery, **empty table** — no behavior change | `solid1x_options.rs`, options tests | `ir-lib`, `contract-process` |
| 5 | Fix SC1001 chain-following; delete SC1006 from both dialects | **2 `RETIRED_RULES` entries**, snapshots, flipped cases + ledger rows, both pages | `ir-lib`, coverage compare, `make ownership-gate` |
| 6 | The three probed v1 false-premise deletions (§4.2): `v1/cleanup-in-forbidden-scope`, `v1/primitive-in-leaf-owner`, `v1/primitive-in-directive-application` — **live wrong findings, highest urgency** | **3 `RETIRED_RULES` entries**, `solid_1x.rs:285`, `creates_directive_owner`, snapshots, flipped cases + ledger rows, three v1 pages | `ir-lib` → `contract-process` → coverage compare, `make ownership-gate` |
| 7 | Restrict v1 SC2001 to tracked scopes | `dialects_process.rs:246-258`, positive fixtures (memo/effect/render-effect compute, tracked JSX), negative fixtures (component body, event handler, `onMount`, plain helper), changed cases, v1 page | `ir-lib`, `contract-process`, coverage compare, `make ownership-gate` |
| 8 | Delete `no-implicit-draggable` from **both** dialects | **2 `RETIRED_RULES` entries**, snapshots, flipped cases + ledger rows, both pages deleted | `ir-lib`, coverage compare, `make ownership-gate` |
| 9 | Delete `no-array-handlers` | **1 `RETIRED_RULES` entry**, snapshots, flipped cases + ledger rows | `ir-lib`, coverage compare, `make ownership-gate` |
| 10 | Delete `no-react-deps` and `event-handlers` (the false `warnOnSpread` arm dies with the latter) | **2 `RETIRED_RULES` entries**, options-schema entries, flipped cases + ledger rows | `ir-lib`, coverage compare, `make ownership-gate` |
| 11 | Delete the component-arm rules: `no-react-specific-props`, `no-unknown-namespaces`, `no-innerhtml`, `style-prop` | **4 `RETIRED_RULES` entries**, options-schema entries, flipped cases + ledger rows | `ir-lib`, coverage compare, `make ownership-gate` |
| 12 | SC8003: remove the spread-duplicate arm; gate folding and content competition to intrinsic elements | fixtures on **both** element kinds, flipped cases + ledger rows, page | `ir-lib`, coverage compare, `make ownership-gate` |
| 13 | Remaining deletions — `no-async-tracked-scope`, `jsx-no-script-url`, `jsx-uses-vars`, `no-proxy-apis`, `self-closing-comp` (5 v1-only keys), `prefer-component-syntax` (both dialects) — plus SC9004 → internal (both dialects) and `jsx-no-undef` dead-code removal | **9 `RETIRED_RULES` entries** (§6.2 audit), flipped cases + ledger rows, pages | `ir-lib`, coverage compare, `make ownership-gate` |
| 14 | Delete `valid-jsx-nesting` — same file as step 8, so sequence them | **2 `RETIRED_RULES` entries** (`v1/valid-jsx-nesting`, `valid-jsx-nesting`), scope-boundary machinery removal, flipped cases + ledger rows | `ir-lib`, coverage compare, `make ownership-gate` |
| 15 | SC7005 → `uncertifiable` | snapshots, page, release-note line | `ir-lib`, `contract-process`, coverage compare |
| 16 | Two double-report suppressions: SC5001 suppresses SC1001; SC1004 owns its condition read | snapshots | coverage compare, non-updating run first |
| 17 | The five merges (§5.1), **one commit each**, largest first (`missing-owner` is 4→1 in v2) | per merge: its `RULE_ALIASES` entries (7 + 6) or `RETIRED_RULES` declared break (3 + 2 + 1) — **19 identities total**, snapshots with new `rule` **and** `code`, regenerated manifests, one merged page, retired pages deleted, flipped cases + ledger rows, release-note lines | `ir-lib`, `contract-process`, coverage compare, `SOLID_RULES_UPDATE=1` regen |
| 18 | Name unification (SC1003, SC1004) + the three renames (§6.1), **one commit each** | per change: its `RULE_ALIASES` entry (**6 total**), its `DEPRECATED_RULE_KEYS` delegating entry, regenerated manifests, page moves, flipped cases + ledger rows, release-note lines | `contract-process`, `node scripts/dialect-manifests.mjs validate`, `npm test --prefix packages/cli` |
| 19 | The three ports (§4.6): `prefer-for`, `prefer-show`, SC8003's content arm | new v2 fixtures, manifests, pages, new ownership cases | `add-fixture` each; `make tsc-oracle`; coverage compare; `make ownership-gate` |
| 20 | Preference preset (§7): tri-state `RuleOverride`; `default_enabled` + `presets` on `RuleMetadata` and generated `defaultEnabled` + `presets` manifest fields; `Dialect::rule_metadata` function-pointer row; the `diagnostics.rs:394` filter rewrite; presets merged into `RuleOptions` **before** `DiagnosticIdentity`; `--preset` / `--enable-rule` on CLI and daemon requests; **`CachedAnswer` + `snapshot_if_current` keyed on normalized presets/enableRules**; adapter options + **snapshot cache-key change**; active default-disabled ESLint rules forwarded as `--enable-rule`; ordinary dialect configs filtered by `defaultEnabled`; `meta.docs.recommended` derived from catalog metadata; `preferences-v1` and `preferences-v2` configs; `packages/wasm/README.md` limitation note; three pages | manifests, the twelve §7.5 tests, the five §7.2.2 daemon tests, the §7.1 unknown-identity regression test | `ir-lib`, `contract-process`, `npm test --prefix packages/cli`, `npm test --prefix packages/wasm` |
| 21 | Delete `upstream-cases.json` and `deviations.json` | ledger asserted complete | `make ownership-gate` with the **zero-pending** stage enabled: 465 rows, none `pending`, every `migrated`/`dropped` row carrying valid completion metadata |
| 22 | Documentation sweep: README rule tables and counts, the migration note (old → new keys, codes, the six declared breaks, the preset change), `docs/precision-backlog.md` entries with probe transcripts and `tsc` output | — | rule-page tests |
| 23 | Handoff | — | `make verify` |

### Why each intermediate commit is green

- **Steps 1–3** never change checker behavior, so all three gates hold
  throughout. Step 2 migrates every retained-rule case *at today's behavior* —
  including the ones later steps will flip — so step 3 retires the old
  comparison with **zero** loss of regression coverage. A case scheduled to
  become negative is guarded as a positive until the commit that makes it
  negative.
- **Steps 5–20** each pair a behavior change with the fixture, snapshot,
  manifest, ownership-case, and ledger edits that describe it. No commit leaves a
  case asserting an outcome the code does not produce, and none leaves a removed
  external name without a retirement or alias entry.
- **Deleted-rule cases** are dropped in their rule's deletion commit with an
  explicit `dropped` ledger row and a reason — never by omission, which is what
  makes the 465-row zero-pending assertion enforceable at step 21.
- **Step 4's empty table** means the alias mechanism exists before any alias is
  needed, while no mapping yet points at an absent target. Steps 17 and 18 add
  each mapping in the same commit as its target's creation, so no intermediate
  state canonicalizes a disabled key onto a name nothing declares.
- **`RETIRED_RULES` entries in the deletion commits** (5, 6, 8–14) mean no
  intermediate commit rejects a previously-valid `rule-options.json` as an
  unknown name.
- **Steps 8 and 14 both edit `static_rules.rs`** — step 8 removes SC8019's
  draggable logic, step 14 removes `valid-jsx-nesting`'s scope-boundary and
  element machinery. Neither depends on the other, so the ordering is
  convenience: doing 8 first keeps each diff smaller.
- **Step 16 before step 17** because the merges change which co-located pairs
  exist; suppressing first keeps both commits' snapshot diffs readable.
- **Step 20 last among behavior changes** so the preset is applied to a settled
  rule set.

### Focused checks

```sh
# ir-lib
cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-reactive-ir --lib

# contract-process
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" cargo +1.97 test \
  --manifest-path rust/Cargo.toml -p solid-facts-backend \
  --test contracts_process --test dialects_process

# coverage compare (non-updating first, always)
SOLID_CHECKER_BIN="$PWD/rust/target/debug/solid-checker-rust" \
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" node scripts/coverage.mjs

# the replacement gate
make ownership-gate
```

`make verify`, `make tsc-oracle`, and `make parity` are the expensive runs. Only
`make verify` is deferred to step 23; `make tsc-oracle` runs at the three steps
that move a rule-arm boundary (8, 12, 19), and `make parity` only at steps 1–3
while it is still a gate. `make ownership-gate` is cheap enough to run on every
commit from step 1 onward, and does, because it is the only gate left after step
3.

**Handoff (step 23) asserts all of:** `make verify`; `make ownership-gate` green
with the ledger at 465 rows; `node scripts/dialect-manifests.mjs validate`;
`jq empty schema/solid-reactivity.schema.json` **unchanged** from the base commit;
`npm test --prefix packages/cli` including the twelve §7.5 enablement tests; and no
`v1/`-prefixed key removed without a `RETIRED_RULES` or `RULE_ALIASES` entry.

### Per-step rules

- **Snapshot updates travel with the code that moved the findings** — the same
  commit, never the thematically nearest one. Always run the non-updating
  coverage compare first and confirm each change is intended before `--update`.
- **Steps 5, 6, 7, 8, 17 move `fixtures/reactive-ir/dialect-solid-1x` and
  `dialect-solid-2`.** That pair deliberately pins where the dialects differ;
  read its fixture comments before editing.
- **Steps 8, 12, 19: write every case against the published typings and run
  `tsc --noEmit`** before trusting it. This is the trap that produced the §3.2
  false positive.
- Run only one Cargo process at a time; parallel Cargo contends for the build
  lock.

---

## 11. Arm-level audit

**Proven** means semantic facts plus the execution model establish runtime
misbehavior. Legal seeds, deliberate overrides, intentional custom attributes,
readability preferences, and timing-dependent hazards do not qualify.

### 11.1 Proven violations — 21 rules, 22 dialect rows

| Rule | What is proven |
| --- | --- |
| SC1001 `strict-read-untracked` | Read outside every tracking scope; mirrors `STRICT_READ_UNTRACKED` (dynamic flag verified §3.3) |
| SC1002 `reactive-read-after-await` | Tracking has already ended at the read |
| SC1003 `no-destructure` | Props unwrapped into frozen locals — compiler lowering |
| SC1004 `components-return-once` | The branch is evaluated once, so later updates cannot re-select it |
| SC1005 `uncalled-accessor` | Accessor stringified or coerced in three `tsc`-permitted value positions |
| SC1007 `reactive-handler-frozen` | Handler installed once during setup; or a proven non-callable hyphenated `on-*` value |
| SC2001 (v2) `reactive-write-in-owned-scope` | Dev guard throws (`@solidjs/signals dist/dev.js:3171`) |
| SC2001 (v1, restricted §4.3) | Write during a tracked computation re-triggers the graph that produced it |
| SC2002 `action-called-in-owned-scope` | Dev guard throws |
| SC2003 `no-direct-mutation` | The write is dropped — subscribers are not notified |
| SC2004 `resolve-in-tracked-scope` | Probe-confirmed throw on an active observer |
| SC3001 `leaf-owner-forbidden-call` | All three variants throw in a leaf scope |
| SC4001 `missing-owner` | Registration has no owner; mirrors `NO_OWNER_*` |
| SC5001 `pending-async-unsuspendable-read` | The scope cannot suspend; mirrors the runtime throw |
| SC5003 `async-outside-loading-boundary` | No `Loading` boundary dominates the read |
| SC6001 (v2) `primitive-in-directive-application` | `runWithOwner(null, …)` verified §3.2 — the creation is never disposed |
| SC7001 `missing-effect-function` | Assertion-defeated non-callable first argument |
| SC7002 `sync-computation-received-async` | `sync: true` contradicts an async callback |
| SC7006 `server-function-module-directive` | The client build loses the export |
| SC7007 `server-function-rich-argument` | The default transport throws at the call site |
| SC8003 `jsx-no-duplicate-props` (folding, content) | Two differently-spelled props fold into one lowering slot, or two content sources share one content slot — at least one authored value is discarded (§3.11) |
| SC8005 `jsx-no-undef` (`use:` arm) | Oxc's lexical binder returns an explicit negative TypeScript cannot supply |

22 rows, 21 rules — SC2001 is listed once per dialect.

### 11.2 Preferences — retained, moved behind the preset (§7) — 3 rules

| Rule | Why it is not a violation |
| --- | --- |
| SC8013 `prefer-classlist` | A `class` helper call works correctly; `classList` patches individual classes instead of reassigning the string. A granularity preference. |
| SC8014 `prefer-for` | `.map` as a JSX child renders correctly. `<For>` preserves DOM identity per item — a real update-behavior difference, not a defect. |
| SC8015 `prefer-show` | Its own page: *"Solid's compiler already handles the reported JavaScript forms correctly."* |

These are off by default and enabled by `plugin.configs["preferences-v1"]` /
`plugin.configs["preferences-v2"]` (§7.4). Nothing in
this plan defends them as proven.

### 11.3 Uncertifiable by design — 4 rules

| Rule | Obligation |
| --- | --- |
| SC7005 `http-response-after-flush` | Which side of the shell-flush race a request lands on cannot be proven (§3.12) |
| SC9005 `package-contract-incomplete` | A package boundary the analysis cannot see through |
| SC9011 `reactive-source-uncaptured` | A reactive source flowing into an undescribed callee |
| SC9012 `reactive-dispatch-unresolved` | Exact runtime dispatch not established |

21 + 3 + 4 = **28**, matching §9. Default-enabled: 21 + 4 = **25**.

---

## 12. Correction log

Claims this audit made and later withdrew. These are where its judgement has
already failed.

| Claim | Asserted | Corrected to | Root cause |
| --- | --- | --- | --- |
| `jsx-no-undef` | reports JSX tag names, duplicating TS2304 | the tag loop is dead code (§3.14) | read the page, not the `violations.push` sites |
| `valid-jsx-nesting` | both paths parse, so no mismatch; then, the hydration motive is right | delete: the runtime covers it better (§3.15) | missed that `ssr()` is string concatenation; then had not found `getFirstChild(node, expectedTag)` |
| `event-handlers` | do not port; then, port the static-value arm | delete the rule (§3.5) | judged the *mechanism* real without asking whether the *finding* was a defect |
| `no-array-handlers` | "highest-value port" | delete (§3.7) | treated a supported runtime form as a defect |
| component-only arms | ported four rules on their component arms | delete all four (§3.8) | never read either compiler's component path |
| `no-innerhtml` | port two arms | delete (§3.9) | the same component-forwarding assumption |
| `no-react-deps` | keep v1-only — 2.0's typings reject it | delete (§3.4) | correct about 2.0, never asked whether the v1 finding was a defect |
| `style-prop` | port; then narrow to one intrinsic arm | delete (§3.8) | kept a CSS-name-validity claim D4 excludes |
| `no-implicit-draggable` | "a probed lowering fact — do not cut"; then "correct the v1 domain" | **deleted from both dialects** (§3.10) | trusted the page's worked example over the enumerated-attribute semantics; then proposed a standard-element census — new machinery for a web-platform claim D4 excludes |
| SC7005 | proven violation at warning severity | `uncertifiable` (§3.12) | used the severity axis for doubt that belongs on the kind axis |
| SC8003 | keep exact-duplicate detection across spreads | remove that arm (§3.11) | did not recognise later-wins override as the idiom it is |
| merge suppressions | "silently widens what a disable turns off" | the disable is silently **lost** (§3.17) | assumed prefix matching; `is_enabled` is exact set membership |
| SC codes | "stable identities" | labels, not identities (§5.2) | contradicted `rules.rs:15-19` |
| parity corpus | retire it under D1; then, migrate into ordinary fixtures | fixtures cannot carry it — a purpose-built manifest and gate are required (§8) | assumed snapshots preserved absence, options, fix output, and span overlap |
| D7 | "bookkeeping"; then a design change needing a snapshot field | keep the `v1/` namespace; no schema or snapshot change at all (§6.3) | never read `eslint.cjs`; then proposed a field on the package-contract schema |
| alias sequencing | one machinery commit, then the renames | machinery with an **empty table**, each alias atomic with its target (§6.2) | an early alias canonicalizes onto a name nothing declares, re-enabling suppressed diagnostics |
| SC2001 v1 | "must verify" | resolved: it fires on component bodies (§3.16) | did not search the process tests |
| ownership classification | one class per case, plus a `not-typescript` escape | **per finding**, three classes, no escape (§8.1) | the existing gate already keys acknowledgements `<case id>:<rule>` and carries two different justifications for one case (§3.21); and `tsOnly` records how upstream *parses* a case, not an exemption from the absolute rule |
| migration phasing | migrate only unchanged-expectation cases before retiring parity | migrate **every** retained-rule case at current behavior, then flip atomically (§8.4) | left changed retained-rule cases unguarded between parity retirement and their semantic commits |
| the preference preset | one `plugin.configs.preferences` entry | dialect-split configs plus a full enablement interface — tri-state overrides, catalog default, dialect-owned metadata lookup, generated adapter metadata, direct-rule activation, CLI flags, daemon answer-cache keys, twelve tests (§7) | `RuleOptions` stores only a disabled set, and the adapter sends the checker no enablement state, so the native run would suppress the findings first (§3.20) |
| retirement entries | counted per concept ("`RETIRED_RULES` ×6") | counted per **external name** — 50 identities, 31 retired, 19 aliased (§6.2 audit) | a both-dialect rule has two names, and moving SC9004 internal removes two catalog identities without being a §4.1 deletion |
| manifest spans | hand-written absolute byte offsets | derived `caseBytes`, `text`-relative markers, runner-resolved (§8.2) | the worked example was arithmetically wrong: the prelude is 41 bytes and the file 122, so `[43,121]` was impossible |

**Surviving every pass:** the Solid 1.x `createReaction` ownership correction
(§3.1), the Solid 1.x directive-owner correction (§3.2), and the falsity of
`warnOnSpread` in both dialects (§3.6). Each was re-verified twice, the second
time with a corrected probe methodology (invalidation triggered outside the
root).

---

## 13. Follow-ups, deliberately outside this plan

None of these gates the implementation above.

- **External key namespace collapse (former D7(b)).** A separate design project.
  It needs a dialect-aware registry, a replacement for the `catalog.namespace`
  dialect-forcing trigger, per-dialect documentation routing, and a decision
  about `meta.docs.url`, which is static per rule object today. Orthogonal to
  catalog reduction.
- **WASM rule-options transport.** `CheckRequest` carries no rule-options
  channel at all (§7.3), so the WASM adapter honours no per-rule configuration —
  a pre-existing limitation this plan documents rather than introduces.
  Extending it with `ruleOptions` / `presets` / `enableRules` would let WASM join
  the enablement interface.
- **Zero-arity `createRoot` / the `UNOWNED` sentinel** (§4.2). New analysis
  capability; `docs/precision-backlog.md`.
- **`foster-parented-dynamic-child`.** The one slice of `valid-jsx-nesting` the
  2.0 runtime does not check: `getNextMarker` (`dist/dev.js:1237`) has no
  structural check, so dynamic children in table/select context hydrate silently
  wrong.
- **`prefer-class-object`.** A v2-only preference analogue of `prefer-classlist`;
  would belong in the preset. Arithmetic in §9.
- **A tuple-mismatch fact for event handlers** (§3.7). Re-adding
  `no-array-handlers` would require comparing slot 1 against the *concrete*
  handler's declared first parameter.
- **A published diagnostic-snapshot schema.** Only if the snapshot format must be
  externally versioned. It would be a new artifact, never an extension of
  `schema/solid-reactivity.schema.json`.
- **SC1005 `uncalled-accessor` re-audit.** Its 2026-08-17 narrowing was read but
  not independently re-probed.
- **`reactive-source-uncaptured` and `reactive-dispatch-unresolved`.** Retained
  as-is; not audited in depth.
