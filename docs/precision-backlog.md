# Precision backlog

The analyzer's known approximations, recorded so each is a decision with an
owner rather than a rediscovery. Items live here when a fix is a *design
change* — it would move findings broadly and needs its own fixture-gated
change — as opposed to the bounded corrections that land as ordinary fixes.

Direction legend: **FN** — misses real defects; **FP** — reports correct
code; **Both** — either, depending on the code.

## The `tsc` redundancy ledger (audited 2026-08-17)

AGENTS.md carries an absolute rule — never report what `tsc` reports, judged
against the library's *real published typings*. This section is that rule
applied to every rule in both catalogs, once, with evidence.

**How the evidence was produced.** `scripts/tsc-oracle.mjs` compiles a snippet
against packages installed from `fixtures/tsc-oracle/packages.json` at the
versions this repository audits — `solid-js@1.9.14` for the 1.x catalog (the
version `pkg/contracts/bundled/solid-v1/solid-js.json` was generated from) and
`solid-js`/`@solidjs/signals`/`@solidjs/web`@`2.0.0-rc.0` for the 2.0 catalog
(the versions in `pkg/contracts/bundled/runtime-lock.json`). It never reads a
fixture stub. Two passes run, `strict` and non-`strict`, because "the project
may not be `strict`" is a distinction a ledger entry has to state even though
the absolute rule refuses it as an exception. TypeScript 5.9.3.

**The parity ledger is held to its own claims, by span.**
`scripts/parity-tsc-ownership.mjs` compiles the *same bytes* `scripts/parity.mjs`
lints — the materialization is shared through `scripts/lib/upstream-cases.mjs` so
the two cannot describe different source — and reads the run artifact parity now
writes (`rust/target/upstream-parity/findings.json`: every finding with the byte
span the checker reported). Both of its directions gate, and both compare spans:

- A deviation declared **`status: "typescript-owned"`** must sit on a case
  TypeScript reports **in the case's own code**, not in the imports the harness
  prepends and not via an incidental untyped-corpus error. 45 verified.
- No **finding may share a span** with a TypeScript diagnostic unless the
  difference in claims is written down. 30 span matches are declared
  `ACKNOWLEDGED` with the difference named, and `PENDING_NARROWING` is **empty** —
  both duplicates it caught have been narrowed away (below).

`typescript-owned` is a new status, and splitting it out of `policy` was the fix
for a flaw in the first version of this check. Plain `policy` predates the whole
audit and covers *any* deliberate difference from upstream — a name-only allowlist
this checker does not carry, a stricter demand for an explicit `untrack`. Those
make no claim about TypeScript, so demanding a diagnostic for them was asking for
evidence of a claim they never made. Twelve entries stay `policy`; the 45 this
audit wrote are `typescript-owned` and now verifiable. Verified failing by
relabelling one entry.

The "does not type-check" count printed under `--report` remains a lower bound on
corpus cleanliness, not a statement about ownership — 73% is easy to misquote,
and only the span-matched list below it is an ownership claim.

### Duplicates the span comparison caught, both now narrowed

Found by the span comparison, each suppressed in `PENDING_NARROWING` with a
pointer here rather than left to fail:

**Landed 2026-08-17: `v1/jsx-no-duplicate-props`'s `children`-prop-plus-JSX-children
pair.** TS2710 is *"'children' are specified twice. The attribute named 'children'
will be overwritten."* — word for word the finding's claim, in **both** passes and
on components as well as intrinsic elements. (An earlier draft of this entry said
strict-only; that was a misreading — the strict-only diagnostic in this family is
TS2783, for the attribute-then-spread duplicate.) Only that exact pair is covered:
`innerHTML` with `textContent`, and `innerHTML` with JSX children, draw no
diagnostic at all, so a set including either still reports — the finding then
asserts more than TS2710 does even where TS2710 also fires. Pinned by
`eslint-compat`: the two surviving conflicts report, and the children pair is a
negative on both an intrinsic element and a component.
**Landed 2026-08-17: `v1/no-innerhtml`'s `dangerouslySetInnerHTML` arm** (upstream
cases 09, 10, 11). TS2322 *"Property 'dangerouslySetInnerHTML' does not exist"* and
the finding *"The dangerouslySetInnerHTML prop is not supported; use innerHTML
instead"* are the same claim. Narrowed to components, where props are whatever the
component declares and TypeScript is silent; the `innerHTML` arm is untouched
because `innerHTML` is a declared Solid prop and every claim about it is
independent. Pinned by `upstream-divergences`'s `ReactMarkupProp` — the silent
intrinsic, the reported component with its rewrite fix, and the reported component
whose extra object entry leaves no unambiguous rewrite.

Both are landed, and the ownership gate's confirmed-duplicate list is empty.

**The gate.** `scripts/tsc-oracle-gate.mjs` enforces
`fixtures/tsc-oracle/rule-cases.json` in `scripts/verify.sh` and as
`make tsc-oracle`. A rule whose positive case is also a `tsc` error fails CI, and
a removal justified by a diagnostic fails CI if that diagnostic ever disappears.
Verified in both directions.

It also enforces **completeness**: every rule in either catalog must have a case,
or an `EXEMPT` entry in the gate script saying why no snippet can express its
subject (the package-contract family, whose subject is a third-party artifact;
`execution-map-incomplete`, unreachable from real source by construction; the
server-surface and SSR rules, which need a rendering mode rather than a type; and
`v1/jsx-uses-vars`, which has no diagnostic of its own). That is what turns the
absolute rule from documentation into a mechanism: a new rule cannot be merged
without its positive spelling and the oracle's verdict on it. Verified by
deleting a case and watching the gate name the rule.

**Why this was invisible for a full cycle.** Every fixture stubs Solid with a
reduced `solid-js.d.ts`, and two of those stubs were *looser* than the real
package in exactly the place a rule's proof depended on:

| Stub said | Real package says |
| --- | --- |
| `apply: (value: T) => unknown` | `EffectFunction<Prev, Next extends Prev = Prev> = (v: Next, p?: Prev) => (() => void) \| void` |
| `onSettled(callback: () => unknown)` | `onSettled(callback: () => void \| (() => void))` |
| `createTrackedEffect(callback: () => unknown)` | `createTrackedEffect(compute: () => void \| (() => void), options?)` |
| `refresh(target: unknown)` | `refresh<T>(target: Refreshable<T>)`, where `Refreshable<T> = T & { readonly [$REFRESH]: any }` |
| `affects(target: unknown, key?: PropertyKey)` | `affects(target: Accessor<unknown> \| Store<object>)` / `affects<T extends object>(target: Store<T>, key: keyof T)` |

Each loosening manufactured a defect no real project can produce, and every
gate stayed green while the rule duplicated `tsc`. The proof-bearing
signatures are now byte-faithful in the fixtures that exercise them
(`solid2-precision`, `leaf-owner`, `execution-phases`, `eslint-plugin-corpus`,
`engine/eslint-reactivity-v2`, `package-callback-producer`); where a stub stays
deliberately loose (`static-api`, `static-api-unresolved`, whose subject *is*
the malformed call) the stub now says so in a comment naming the real signature
and asserting that no surviving rule's proof depends on the looseness.

### The general mechanism: TypeScript does not check hyphenated JSX attribute names

Found on 2026-08-17 by `scripts/parity-tsc-ownership.mjs`, and it is the boundary
of every "this attribute is TypeScript's" argument above.

TypeScript exempts a JSX attribute whose **name contains a hyphen** from the
excess-property check entirely — a deliberate allowance for HTML's own hyphenated
custom attributes. Verified against `solid-js@1.9.14`: `data-x`, `my-prop`,
`on-foo`, `html-For`, and the namespaced `class:mt-10` are all accepted on a
`<div>`, while `myProp` is TS2322. The *duplicate-name* check (TS17001) is
syntactic and is **not** exempt, so it still fires on `on-foo` written twice.

Three of the narrowings above were written per element rather than per name and
lost findings to this. All three now ask
`upstream_compat::jsx_name_is_type_checked` before staying silent:

- **SC8012** — `<div class:mt-10={true} />` and its shorthand are upstream's own
  cases 04 and 05. They were declared `status: "policy"` on the grounds that
  TypeScript reports them; it does not. Restored, and the two deviations removed.
- **SC8001** — `<div onFoo-bar="a" />` has an alphabetic third character, so the
  rule looks at it, and the name is never type-checked. Its static-value and
  ambiguous-name arms are restored for any hyphen-bearing name.
- **SC1005** — `<div data-count={count} />` is the one native-attribute value
  position that survives: the accessor is stringified into the attribute and no
  type objects.

All four shapes are pinned in `fixtures/reactive-ir/eslint-compat` and in the
oracle gate's `silent` cases.

### Removed: eight rules, 72 findings

Every one is a **violation the type system already reports on the same code**,
or an **obligation whose whole domain the type system closes**. The first seven
are 2.0-catalog rules, so they left `scripts/parity.mjs` untouched; the eighth,
`v1/imports`, is a 1.x rule and its seven upstream cases are declared
`status: "policy"`.

| Code | Rule | Findings | Why |
| --- | --- | --- | --- |
| SC3004 | `invalid-cleanup-return` | 29 | Every spelling is TS2345/TS2322 against `EffectFunction`'s `(() => void) \| void` return |
| SC9002 | `cleanup-return-unresolved` | 18 | Its whole domain was the *legality* of a returned value, which the same type closes |
| SC7003 | `invalid-refresh-target` | 6 | `Refreshable<T>` is the source brand as a type; every invalid target is TS2345 |
| SC7003 | `invalid-affects-target` | 2 | Same, against `Accessor<unknown> \| Store<object>` |
| SC7004 | `affects-keys-on-accessor` | 2 | A key on an accessor target selects the one-argument overload; the key is TS2345 |
| SC9003 | `refresh-target-unresolved` | 3 | Asked whether the target carries the brand — a question the type answers |
| SC9003 | `affects-target-unresolved` | 3 | Same |
| SC8002 | `v1/imports` | 9 | Its one condition — the named module does not export the name — is exactly TS2305's; audited later, see below |

#### SC3004 `invalid-cleanup-return`

`tsc --noEmit`, real `2.0.0-rc.0` typings, **both** passes (so no `strict`
argument is available):

~~~
sc3004.tsx(5,29) TS2345: Argument of type '(value: number) => number' is not assignable to
  parameter of type 'EffectFunction<number, number> | EffectBundle<number, number>'.
    Type 'number' is not assignable to type 'void | (() => void)'.
sc3004.tsx(6,29) TS2345: ... (explicit `return value + 1`)
sc3004.tsx(7,29) TS2345: ... (`() => makeCount()`, a returned call)
sc3004.tsx(8,29) TS2322: ... (`() => teardown.count`, a member return)
~~~

The legal spellings — `() => teardown.dispose`, `() => () => {}`,
`() => undefined`, `() => {}` — are **accepted**. The type does not merely
reject more than the rule did; it draws the same line. `(() => void) | void` is
a union, not bare `void`, so return-value-ignoring assignability does not apply.

#### SC9002 `cleanup-return-unresolved`

The obligation had four sources. Three are TypeScript's, and the fourth is not
a defect:

- **mixed union / `unknown`** — TS2345 and TS2322 in both passes.
- **a non-callback second argument** (`undefined`, `null`, `5`, `"apply"`, and
  1.x-style value threading) — TS2345 in both passes.
- **an unconstrained generic return** — TS2345.
- **`any`, and an unresolved wrapper callback** — `tsc` is silent, and that is
  not licence to report. Absence of a type error because the type is `any` is
  *missing evidence*, not proof (AGENTS.md's own trap list). More decisively:
  when the program type-checks, TypeScript has *proven* the returned value
  legal, so an obligation asserting uncertainty about its legality is noise
  about code the type system has cleared. The ownership consumer needs no
  finding for this — it simply does not get a "cleanup was handed over" fact,
  which is correctly modeled as an absent proof.

One of those unresolved-callback obligations was worse than noise: three of the
18 sat on `createEffect(compute, { effect, error })`, which is the **supported
`EffectBundle` form**. `tsc` accepts it; the checker raised an obligation on
idiomatic code. `rule_quality_process.rs` pins all of these at 0 so a
reintroduction fails there.

#### SC7003 / SC7004 / SC9003 — the refresh and affects target family

This family was hidden by the same mechanism, one layer deeper: the fixtures
type `refresh(target: unknown)`, while `@solidjs/signals` brands its
refreshable sources in the type system —

~~~ts
export type Refreshable<T> = T & { readonly [$REFRESH]: any };
export declare function refresh<T>(target: Refreshable<T>): void;
export declare function affects(target: Accessor<unknown> | Store<object>): void;
export declare function affects<T extends object>(target: Store<T>, key: keyof T): void;
~~~

so *every* shape the rules proved invalid is a type error, in both passes:

~~~
p3.tsx(11,11) TS2345: '() => number' is not assignable to 'Refreshable<() => number>'.
                        Type '() => number' is not assignable to '{ readonly [$REFRESH]: any; }'.
p3.tsx(12,11) TS2345: '{}' ... Property '[$REFRESH]' is missing
p3.tsx(13,11) TS2345: a plain accessor `target` ... '[$REFRESH]' is missing
p3.tsx(14,11) TS2345: `state.user`, a store child record ... '[$REFRESH]' is missing
p3.tsx(15,11) TS2345: `affects(signalGet())` — 'number' is not assignable to
                        'object | Accessor<unknown>'
p4.tsx(14,11) TS2345: `refresh(valueFormStore)` — only the derived forms are branded
p4.tsx(6,17)  TS2345: `affects(memo, "name")` — '"name"' is not assignable to 'unique symbol'
~~~

And the valid targets — `refresh(memo)`, `refresh(signalGet)`, `affects(memo)`,
`affects(state)`, `affects(state, "user")`, `affects(state.user, "name")` — all
type-check. Same line, both directions. Zero-argument and over-long calls are
TS2554 by arity.

**What was kept.** `static_api.rs` still records the `refresh(...)` *write* that
SC2001 `reactive-write-in-owned-scope` consumes; only the target diagnostics
went, and the control flow that skips a malformed call still skips it, so a call
`tsc` rejects records no write. SC2001 is unchanged at 36 findings.

**Do not amputate the runtime half.** The same applies to cleanup: SC3004's
consumer is gone, but `cleanup.rs::function_returns_cleanup` and the
`CleanupReturnStatus` classification behind it are load-bearing for SC4002 and
SC4004, which assert *ownership and disposal* — facts no type expresses. The
`callResultDomain` and member-return work still serves them; `diagnostics_process.rs`
pins that a returned call producing a `number` hands over no cleanup while one
producing a function does.

### Correction 2026-08-17: three rules were mis-classified as fully redundant

An earlier pass of this ledger listed `v1/event-handlers`, `v1/no-react-specific-props`,
and `v1/style-prop` as **proven redundant, removal specified**. That was wrong,
and the mistake is worth recording because it is the mirror image of the
fixture-stub trap: each was probed on *one* arm — an unknown attribute name on
an intrinsic element — and the verdict was generalised to the whole rule. Read
against each rule's actual upstream domain (`fixtures/upstream-parity/upstream-cases.json`),
all three have an arm TypeScript does not cover, so all three are **partially
redundant** and belong in the table below. None was deleted.

What the full probe found (real `1.9.14` typings, both passes):

| Spelling | `tsc` | Whose claim |
| --- | --- | --- |
| `<div onclick={fn} />` | silent — `onclick` *is* a declared prop | SC8001's canonical-casing advice |
| `<div ondblclick={fn} />`, `<div onDblClick={fn} />` | silent — both declared | SC8001's ambiguous-name advice |
| `<div {...{ onClick: fn }} />` | silent | SC8001's `warnOnSpread` arm |
| `<div onClIcK={fn} />`, `<div oncLICK={fn} />`, `<div onDoubleClick={fn} />`, `<div ondoubleclick={fn} />`, `<div only={fn} />`, `<div onLy="s" />` | TS2322 "Property does not exist" | TypeScript's |
| `<Pascal className="x" />`, `<Pascal htmlFor="x" />`, `<Pascal key={1} />` with permissive props | silent | SC8011 — and upstream's cases 4, 8, 9 are exactly these |
| `<Strict className="x" />` where the component declares `{ class?: string }` | TS2322 | TypeScript's |
| `<div className="x" />`, `<div htmlFor="x" />`, `<div key={1} />` | TS2322 | TypeScript's |
| `<div style="font-size: 10px; missing-value: ;" />` and every other string-valued `style` | silent — string styles are legal in 1.x | SC8017's string arms, including the malformed-CSS claim |
| `<div style={{ "-webkitAlignContent": "center" }} />` | silent — the `` [key: `-${string}`] `` index signature absorbs it | SC8017 |
| `<div style={{ fontSize: 10 }} />`, `{{ COLOR: "x" }}`, `{{ unknownStyleProp: "x" }}` | TS2561/TS2353 | TypeScript's |
| `<div style={{ "margin-top": -10 }} />` | TS2322 against `MarginTop<…>` | TypeScript's |
| `<div css={{ … }} />` (a configured extra style prop) | TS2322 | TypeScript's |

The narrowing each needs is the same question — *is this attribute name declared
on this element's attribute type* — which is a type fact the checker does not
have. The implementable approximation, per rule, is in the table below.

### Narrowed 2026-08-17: five rules, partially redundant, now scoped

Each keeps the arm no type answers and drops the arm TypeScript already
reports. Every one is pinned in **both** directions by
`fixtures/tsc-oracle/rule-cases.json` — a `removed-because-redundant` case for
the dropped arm and a `silent` case for the surviving one — so neither half can
move without failing CI. Each rule's upstream cases that stopped firing are
declared `status: "policy"` in `fixtures/upstream-parity/deviations.json`, every
entry naming its own diagnostic.

| Code | Rule | Dropped, and why | Kept, and why |
| --- | --- | --- | --- |
| SC8011 | `v1/no-react-specific-props` | `className`/`htmlFor`/`key` on an **intrinsic** element — TS2322 each. The `key` arm was intrinsic-only and went entirely. | The same spellings on a **component**, upstream's own cases 04 and 08. A component's props are whatever it declares, so the key is permitted on a permissive one and a type error on `{ class?: string }` — the answer genuinely depends on the component. |
| SC8017 | `v1/style-prop` | The object-key arms on an intrinsic element: camelCase (TS2561 with the kebab suggestion), an unknown key (TS2353), a unitless number for a length (TS2322 against `MarginTop<…>`), and a configured extra style prop (TS2322 on the attribute). | Every **string-valued** `style`, legal in 1.x, including the two claims no type can make — a declaration with a missing value, and a value that is not CSS. Plus any `-`-prefixed key on any element: `` [key: `-${string}`] `` absorbs it, so `-webkitAlignContent` is silent (upstream's case 02). Plus any key on a component. |
| SC8001 | `v1/event-handlers` | Every **type-checked unknown** `on*` name in every value form including the boolean shorthand (TS2322), and every mis-cased or non-standard spelling — `onClIcK`, `oncLICK`, `onDoubleClick`, `ondoubleclick` are not declared under any casing. Also the static-value arm on a standard declared handler: no static value is assignable to `EventHandlerUnion`. | The readability rename for a **declared** spelling: 1.x declares each handler as both `onClick` and `onclick`, so `onclick` and `ondblclick` type-check (upstream's cases 02 and 12). Every arm on a **hyphenated tag**: `<my-widget />` is TS2339 against stock typings, so a project using one declared it itself, commonly permissively. Hyphenated attribute names such as `on-foo`, which TypeScript deliberately skips but the compiler still lowers. And `warnOnSpread`, which type-checks while Solid does not attach the handler. |
| SC8003 | `v1/jsx-no-duplicate-props` | **Identically spelled** duplicates, by origin pair: two attributes are TS17001, an attribute then a spread is TS2783 (`strict` pass only, which the rule does not accept as an exception), two keys in one spread object are TS1117. | Two **differently spelled** props the compiler folds into one slot — `onClick`/`onclick` both become the delegated `el.$$click` write, `attr:title`/`title` share the template attribute slot. Plus the two identical-name orders TypeScript leaves alone: a spread then an attribute (upstream's case 02) and two different spread objects. Plus every child-content conflict — no type relates `children`, JSX children, `innerHTML`, and `textContent`. |
| SC8012 | `v1/no-unknown-namespaces` | Every namespaced prop on an **intrinsic** element — TS2322. Solid resolves namespaces through mapped types over user-augmentable interfaces plus individually declared `on:*` events, so an unrecognised prefix has nothing to land on. This covered the `style:`/`class:` steer too: **neither prefix is declared at all**, a real gap in Solid's published typings given the 1.x compiler supports both. | The same on a **component**, upstream's cases 06 and 07. Props are a plain object, TypeScript is silent, and the claim — the compiler special-cases namespaces only on DOM elements it lowers directly, so the prop arrives inert — is one no type makes. |
| SC1007 | `expected-function-got-expression` | The **call-result** arm on a normal declared handler. Both its triggers land on TS2322 at the same attribute: an expression *proven non-callable* is exactly what TypeScript rejects, and a *proven-accessor call* is rejected whenever the accessor's value is not callable (`onClick={count()}` with `count: Accessor<number>`). Deliberately **not** kept for the one spelling TypeScript permits — an accessor holding a function, `onClick={handler()}` — because there the finding would be wrong: a JSX attribute expression is a tracked read, so that handler does update. | The **reactive-handler-read** arm: a callable handler read out of reactive props or store state. TypeScript is silent, and the claim is a timing one — a native listener receives its function value once during DOM setup, so reading it through reactive props freezes the initial handler. Also the hyphenated native `on*` arm TypeScript deliberately declines to check: proven non-callable/non-array values are violations and mixed runtime shapes are uncertifiable. |
| SC1005 | `uncalled-accessor` (both catalogs) | Three of its six value positions, in both dialects: a native JSX attribute (TS2322 — an accessor is never assignable to a DOM attribute type), a class object value (TS2322 against 2.0's `Record<string, boolean>`), a computed property access (TS2538). This removed the last consumers of the dialect's `class_object_values_are_truthiness_coerced` and `native_children_attribute_invokes_functions` predicates, which went with them. | The positions TypeScript **permits**, and the most common real spellings of the bug: a string-concatenation binary operand (`"hello " + label` renders the accessor's source text), a unary operand (logical-not and the numeric coercions `-`/`+`/`~`, all clean against the published typings), and a template-literal interpolation. |

### Narrowed 2026-08-17: `no-direct-mutation`, in the 2.0 catalog only

2.0's `createStore` returns a shallowly `Readonly` proxy, so a write to a
**root** record property is already a type error against
`@solidjs/signals@2.0.0-rc.0`, for both spellings:

~~~
mut.tsx(4,29) TS2540: Cannot assign to 'count' because it is a read-only property.  // state.count = 1
mut.tsx(5,29) TS2540: Cannot assign to 'count' because it is a read-only property.  // state.count++
~~~

**Solid 1.x is the opposite**, and this is why the predicate is asked of the
dialect rather than assumed: its `createStore` returns a *mutable* store type,
and the same four writes produce **no diagnostic at all** against
`solid-js@1.9.14`. The 1.x rule is fully independent and untouched — carrying the
2.0 answer across the seam would have silenced it wrongly, which is exactly the
failure AGENTS.md warns about.

Three shapes survive in 2.0, each a write TypeScript accepts and the runtime
drops:

- **A nested record's property.** The readonly-ness stops at the top level, so
  `profile.user.name = "Grace"` type-checks.
- **A cast.** `(profile as { count: number }).count = 1` erases the readonly.
  This one constrains the implementation: `member_root` resolves *through* the
  cast, so comparing the written member's object against the resolved root span
  alone would have handed this case to a diagnostic that does not exist. The
  narrowing therefore requires the object to be a bare **identifier**.
- **A props member**, which is not readonly at all.

Coverage 526 → 524; the three root writes went and two of the surviving shapes
were added as fixture cases, because the fixture had none. The finding *count*
happens to land on four either way, so `diagnostics_process.rs` asserts each
surviving spelling and each dropped one **by span** — a count alone cannot tell
this narrowing from a regression that dropped the wrong three.

### Closed 2026-08-18: SC1005 no longer overlaps arithmetic diagnostics

The structural fact now retains binary `+` expressions with a string literal on
one side, unary logical-not, and the unary numeric coercions. Numeric and
bitwise **binary** operators reject a function operand in TypeScript — `count +
1` is TS2365, `count - 1`, `count * 2`, and `count | 0` are TS2362 — and are no
longer SC1005 positions, which removes the former `count + 1` duplicate.
Concatenations whose string behavior would require a resolved operator
signature remain outside the violation claim rather than being guessed from
source text.

The **unary** operators were dropped in the same pass on the assumption that
they behave like their binary counterparts. They do not: probed against
solid-js@1.9.14 through `scripts/tsc-oracle.mjs`, `-f`, `+f`, and `~f` on a
function value are clean in *both* the strict and loose passes, exactly like
`!f`. Dropping them removed a real, TypeScript-silent defect class (a coerced
accessor is silently `NaN`) and lost upstream parity case
`reactivity__invalid__21`, whose deviation could not be declared
`typescript-owned` because `scripts/parity-tsc-ownership.mjs` proved TypeScript
reports nothing there. They are restored under
`CoerciveOperandKind::NumericCoercion`, pinned by a `expect: "silent"` /
`checker: "reports"` oracle case and by
`fixtures/reactive-ir/uncalled-accessor-v2`, whose `TypeScriptOwnedOperators`
case now holds only the binary spellings.

### Independent — keep

Grouped by why no type can express the claim. `tsc` was confirmed silent on a
positive case for each entry marked ✓; the rest assert a runtime, timing, or
provenance fact with no type surface at all.

- **Reactivity and timing** — SC1001 `strict-read-untracked`, SC1002
  `reactive-read-after-await` ✓, SC1006 `untracked-derived-function`, SC9011
  `reactive-source-uncaptured`, SC5004 `v1/no-async-tracked-scope` ✓
  (`createMemo(async …)` type-checks: 1.x `EffectFunction` returns `Next`
  freely and 2.0's `ComputeFunction` admits `PromiseLike`), SC5001/SC5002/SC5003/SC5005
  (the pending-async and Loading-boundary family). *When* a read happens
  relative to a tracking scope is not a property of its type.
- **Ownership and disposal** — SC4001 `no-owner-effect`, SC4002
  `no-owner-cleanup` ✓ (`onCleanup(() => {})` is a well-typed `Disposable`),
  SC4003 `no-owner-boundary`, SC4004 `no-owner-settled-cleanup` ✓ (returning a
  real cleanup from an unowned `onSettled` is perfectly typed; the claim is that
  nothing will dispose it), SC3001/SC3002/SC3003 (the leaf-scope rules).
- **Write phase and transactions** — SC2001 `reactive-write-in-owned-scope`,
  SC2002 `action-called-in-owned-scope`, SC2003 `no-direct-mutation`, SC2004
  `resolve-in-reactive-scope`. Which scope is active at a call is not typed.
- **Compiler lowering** — SC8008 `v1/no-innerhtml` ✓ (`innerHTML` is a declared
  prop and a string is its declared type), SC8004 `v1/jsx-no-script-url` ✓
  (`href` is `string`; the claim is about the scheme the string carries),
  SC8019 `no-implicit-draggable` ✓ (`draggable={false}` is well typed; the claim
  is what the rc.0 runtime does with `false`), SC8020 `valid-jsx-nesting` ✓
  (`<p><div /></p>` type-checks), SC8007 `v1/no-array-handlers` ✓ — the case that
  proves the JSX family is not uniformly redundant: `EventHandlerUnion` includes
  `BoundEventHandler`, so `onClick={[handler, 1]}` is **legal** per Solid's own
  types.
- **API shape that survives its own signature** — SC7001
  `missing-effect-function` ✓: the single-argument `createEffect(compute)`
  overload still exists in rc.0, deprecated and typed `never`, so the call
  type-checks and the claim "this effect never runs" is the checker's alone.
  Cast-hidden non-callable values survive in both dialects as well, including
  `.ts` angle-bracket assertions and a cast-hidden non-callable `effect` field
  in the required `{ effect, error }` bundle. Raw invalid arguments, including
  nullish values accepted only with `strictNullChecks` disabled, are excluded
  because the strict published-type pass reports them.
  Missing facts now remain explicit rather than becoming silent gaps. A
  spread argument hides the call's arity, so `missing-effect-function` reports
  an uncertifiable obligation; in 2.0 `no-owner-effect` does the same when
  allocation depends on the hidden tuple length. Recovering a proof needs
  tuple-arity facts the fact layer does not carry. Unknown or `any` callback
  values are also uncertifiable, as is a nullable callback hidden by the
  runtime-transparent `!` wrapper, while compiler-proven callable identifiers retain a proven
  violation path. A `"use server"`
  directive is a framework and bundler convention that no core package reads;
  both published server entries neutralise client-runtime claims (1.9.14 uses a
  no-op; 2.0.0-rc.0 routes through `serverEffect`). Otherwise-reporting effect
  and ownership cases under the directive are therefore uncertifiable until a
  project/compiler fact proves which entry executes. Oracle cases pin both the
  uncertain directive path and undirected controls.
  SC7002 `sync-node-received-async`, SC7005 `http-response-after-flush`,
  SC7006/SC7007 (the server surface) likewise assert runtime behavior. SC5005
  and SC7005 now distinguish a visible server-render entry (violation) from
  an absent entry (uncertifiable); absence is not treated as proof of CSR
  because the entry may live in a different tsconfig or package.
- **Syntax and style, no type surface** — SC1003 `v1/no-destructure` ✓ /
  `component-props-destructure`, SC1004 `v1/components-return-once` /
  `component-returns-conditionally`, SC8002 `v1/imports`, SC8006
  `v1/jsx-uses-vars`, SC8009 `v1/no-proxy-apis` ✓ (a legal import; the claim is
  target-runtime Proxy support; explicit type imports are proven erased and
  runtime-referenced imports are proven executing, while unused value imports
  are uncertifiable because `verbatimModuleSyntax` changes their emit; direct
  Proxy calls require the exact standard-library declaration; and `mergeProps`
  reports a violation only for a proven function source, certifies only exact
  plain literals, and keeps every possible callable/`$PROXY` source
  uncertifiable without identifier-name heuristics), SC8010
  `v1/no-react-deps` ✓
  (`createEffect(fn, [dep])` type-checks — the array is 1.x's `Init` value),
  SC8013 `v1/prefer-classlist`, SC8014 `v1/prefer-for`, SC8015
  `v1/prefer-show`, SC8016 `v1/self-closing-comp` ✓, SC8018
  `prefer-component-syntax`, SC6001 `primitive-in-directive-application`.
- **Provenance and contracts** — SC9001, SC9005, SC9006 (the package-contract
  family). A missing contract is a statement about analyzability, not about a
  type.
- **SC8005 `v1/jsx-no-undef`** — kept, with a caveat worth recording. Its
  surviving domain is an unknown `use:` name (unresolved JSX tags are
  TypeScript-owned and remain checker-silent). Against the published typings *alone*,
  `use:autofocus` is TS2322, because `JSX.Directives` ships empty and is meant
  to be augmented. In a project that has augmented it — the documented, intended
  usage — `tsc` is silent, and the checker's claim (no lexical *value* binding
  exists for that name) is a different question from whether the *type* was
  declared. Independent, but a narrowing candidate if the two ever collapse.

### Fixed 2026-08-17: `solid-1x-sources` had been running the 2.0 dialect

The documented `.gitignore` trap, live in the repository.
`fixtures/reactive-ir/solid-1x-sources/node_modules/solid-js/` existed as an
**empty directory** — no `package.json`, no `.gitignore` exception, nothing
tracked — so dialect selection found no 1.x version and fell back to the 2.0
default. The fixture whose entire stated purpose is "the reactive-source
factories 1.x has and 2.0 does not" had never exercised the 1.x catalog.

What it was actually asserting: six `package-contract-export-missing`
obligations, because `createComputed`, `createDeferred`, `createSelector`, and
`createResource` are not in the 2.0 contract; plus a spurious
`missing-effect-function` and `no-owner-effect` on 1.x's single-argument
`createEffect`; plus one of the 18 SC9002 obligations the cleanup-return removal
dropped, which was this artifact rather than a real obligation.

What it asserts now: thirteen findings, every one a 1.x source factory's
untracked read — `createSignal`, `createMemo`, `createResource`,
`createDeferred`, `createSelector`, `createMutable`, `For`, `Index` — exactly the
"evidence that the source was discovered at all" its comment claims, plus
`v1/no-proxy-apis` on the store import, `v1/no-async-tracked-scope`, and
`v1/reactive-read-after-await`.

The stub and its `.gitignore` exception lines are now tracked together, which is
the only form of this fix that survives CI.

### Withdrawn: `import-location` is not a fixture defect

An earlier pass of this ledger recorded
`fixtures/reactive-ir/import-location`'s `import { createSignal, createMemo }
from "solid-js/store"` as a defect, on the grounds that it is TS2305 and no real
project compiles it. That reading was wrong: importing a name from the wrong
module **is** `v1/imports`'s subject, so the case is deliberate and correct.

It did raise a live question, and auditing it removed an eighth rule.
`v1/imports` (SC8002) fired on exactly one condition — the module named in the
import does not export the name — which is exactly TS2305's condition:

~~~
imp.tsx(1,10) TS2305: Module '"solid-js/web"' has no exported member 'createEffect'.
imp.tsx(2,10) TS2305: Module '"solid-js"' has no exported member 'render'.
imp.tsx(3,15) TS2305: Module '"solid-js/store"' has no exported member 'Component'.
~~~

Both passes, and value and type positions alike. The second arm I assumed it had
does not exist: a name exported by *both* modules returns early, so the style
preference upstream expresses for `import { Show } from "solid-js/web"` was never
reported here — verified silent, and pinned as such. Its module-rewrite autofix
was genuinely useful, and offering an autofix is explicitly not an exception.
**Removed**, 9 findings; `Dialect::export_modules` and the generated per-subpath
export index remain, still consumed by the contract layer.

## Compiler-faithful heuristics (verified against the 1.x compiler, do not "fix")

These were flagged as suspect eslint-plugin-solid ports and have now been
verified against the **pinned 1.x compiler**
(`solid-1x-compiler@79b9b637`, byte-faithful to
`babel-plugin-jsx-dom-expressions@0.40.7`) — the parity target is Solid's own
behavior, not upstream's quirks. Each entry below matches the compiler, which
is why it stays.

- **`on*` event-name detection** (`upstream_compat/shared_reactivity.rs`,
  `solid1x_attributes.rs`): the compiler's attribute lowering treats *every*
  `on`-prefixed DOM prop as an event (`plan.key.starts_with("on")`,
  `to_event_name` = the suffix lowercased), so `once`/`only` genuinely become
  listeners for events `ce`/`ly` when function-valued, and statically-valued
  ones are frozen into the template as plain attributes — exactly what
  `v1/event-handlers` reports. Upstream's `/^on[a-zA-Z]/` is *narrower* than
  the compiler; the checker deliberately uses the compiler's `startsWith`
  boundary instead, so a statically frozen `on-foo` receives SC8001 while a
  dynamic non-callable value is handled by SC1007's TypeScript-unchecked
  handler branch. A callable `on-foo` remains clean because it is a real
  listener for the distinct `-foo` event.
- **ASCII-only element-name case classification**
  (`upstream_compat/mod.rs::is_lowercase_led`): Babel's `isCompatTag` is
  `/^[a-z]/`, so a non-ASCII-led tag compiles as a component reference. The
  checker matches the compiler.
- **Static `innerHTML` without children is silent**
  (`no-innerhtml`, `allowStatic` default) and **single-line
  whitespace-only children block `self-closing-comp`** — configurable
  stylistic leniencies matching upstream's option defaults; neither can
  produce a false positive.

## Resolved: Solid 2.0 precision corrections 2026-08-17

**Read with the ledger above.** Where an entry below describes SC3004 or SC9002
as reporting something, that consumer is gone; the *classification* work it
describes survives because SC4002/SC4004 need it. The entries are kept as
written rather than rewritten, because they record how the runtime value domain
came to be trusted — which is still true — and rewriting them would erase the
reason the removal was safe.

- **SC7007 inline arguments and serializer identity** (`server_rules.rs`,
  `demand_plan.rs`): compiler library-type facts are now demanded for every
  non-spread server-function argument, so `save(new Date())` no longer falls
  through a variable-only gap. `configureServerFunctionsClient` must resolve
  to the exact `@solidjs/web/server-functions` declaration; a local same-named
  function with a `serializeArgs` property cannot silence the project. A valid
  exact configuration call with a dynamic options object produces an
  uncertifiable SC7007 until `serializeArgs` presence is closed. The remaining
  top-level fact boundary no longer creates silent nested false negatives:
  arguments outside the deliberately proven JSON-safe primitive set now carry
  an uncertifiable transport obligation, including object graphs, arrays,
  aliases, unions, arbitrary numbers, spreads, and missing facts. Invalid calls
  remain TypeScript-owned through an exact call-validity gate.

- **Synchronous standard callbacks after `await`** (`static_rules.rs` and
  `runtime_semantics.rs`): SC1002's accessor-call *and* member-read proofs now
  continue into a function written directly in an exact built-in
  `Array`/`ReadonlyArray.prototype.filter` call after a dominating await.
  Callability is sampled at the argument, not the callee, and the callback must
  be the literal argument for a proven SC1002 —
  `filter(makePredicate(fn))` and an `async` callback instead produce SC9012,
  preserving the unknown synchronous callback extent as an explicit proof
  obligation. Promise callbacks and project-defined or shadowed methods are
  outside this exact built-in behavior; unresolved package callbacks remain
  package-contract obligations.
- **Cleanup returns classified from the runtime value domain** (`cleanup.rs` and
  `demand_plan.rs`): identifier returns are demanded with TypeFacts'
  `runtimeValueDomain` and classified from it rather than from rendered type
  text, at exactly the peeled span the classifier resolves the entity at (so
  `return (value)` and `return value as Cleanup` work like the bare form).
  `CleanupReturnStatus` now separates "proven a function" from "proven legal but
  not a function", so a proven-`undefined` return can no longer make a callback
  look like one that returns a cleanup. Mixed legal domains, `unknown`, `any`,
  and generics are not legality findings (the removed SC9002 was TypeScript's
  job), but an unowned `onSettled` callback that may return a cleanup now keeps
  an uncertifiable SC4004 owner obligation instead of treating that cleanup as
  absent.
- **Static member cleanup returns** (`cleanup.rs` and `demand_plan.rs`): member
  return spans now receive the same exact `runtimeValueDomain` demand as
  identifier returns. A proven static function member is accepted as a
  cleanup and a proven primitive member was SC3004 (removed; now simply "not a
  cleanup"). A *mixed* union
  (`(() => void) | number`), `any`, and a computed member were SC9002 (removed),
  because their runtime property value is not closed by an exact dispatch
  proof. An **optional** member (`maybe?: () => void`) is legal but does not
  prove a cleanup on every execution; when owner safety depends on it, SC4004
  is uncertifiable. Verified against the pinned producer for all four
  spellings.
- **`runWithOwner` Owner identity** (`owners.rs` and `solid-dialect`): the
  supplied-owner proof now accepts only a compiler-resolved `Owner` type whose
  declaration and origin match the active dialect export table. Re-exported
  aliases are accepted; a user-local `Owner` lookalike and unresolved values
  remain conditional. This removes the rendered-type-name match without
  changing the nullable-owner fail-closed behavior.
- **Assignment target reads** (`solid-facts/src/ast` and
  `AstFacts::is_plain_assignment_target`): normalized facts distinguish plain
  assignment from compound/update reads, and only the member that *is* the
  written target is exempted, so a computed key or destructuring default inside
  a target stays an SC1001/SC1002 read.
- **Owner-backed settled cleanup** (`owners.rs`): the owner requirement pass now
  gates only the duplicate SC4002 for an inline owner-backed `onSettled` callback,
  and only when the callback is the literal argument; SC3001, genuinely unowned
  SC4002, and unowned returned-cleanup SC4004 remain distinct. Indirect,
  exported, and unresolved cases stay conservative.
- **The lexical leaf pass requires an exact callback and its synchronous
  extent** (`cleanup.rs`). The leaf-scope rules (SC3001/SC3002/SC3003) used to
  fire for a primitive written lexically *anywhere* inside the leaf-owner
  argument, so `onSettled(wrap(() => { onCleanup(fn) }))` reported SC3001 even
  though `wrap` may stash the callback and run it out-of-band, where no leaf
  scope exists and the call does not throw. The pass now demands the same two
  containment facts the dynamic-extent path already did: a literal or exact
  in-project callback exposes its body, and the call must sit in that
  callback's own synchronous extent (`direct_callback_contains`). An exact
  identifier callback is followed transitively, so forbidden operations keep
  their SC3xxx identity and an exact safe body is certified. A wrapper call,
  callback-returning call, or unresolved helper cannot support a specific
  violation claim and now produces SC9012 `uncertifiable` instead of silent
  failure. Known accessors, setters, actions, primitives, and exact
  standard-library calls discharge that walk, preventing false uncertainty on
  ordinary signal operations. The genuinely unowned SC4002 and the unowned
  returned-cleanup SC4004 are unaffected, as are the settled call-site gates.
  Pinned by `fixtures/reactive-ir/solid2-precision` and the opaque/exact pairs
  in `fixtures/reactive-ir/leaf-owner/`.

### Remaining approximations from this slice

- **Resolved 2026-08-17, returned calls are classified from the call result**
  (`cleanup.rs::returned_call_domain`, `demand_plan.rs`). The TypeFacts
  interface change this needed has landed: `callResultDomain`
  (`solid-ts-facts` `559c9031`, ADR 0013) matches a call-like node against the
  demand's exact start *and end* bytes and classifies the checker type there
  with the same runtime value-domain classifier, so the callee a call shares a
  start byte with can never be the subject. `cleanup_return_status` now feeds
  that domain to the existing `domain_cleanup_return_status`, which closes both
  directions the old callee probe produced: `return makeCount()` where
  `makeCount(): number` is SC3004 rather than silent (**FN** closed), and the
  unowned `onSettled(() => { return makeCount(); })` no longer reports SC4004
  as though a cleanup were handed over (**FP** closed). `handlers[i]()` is
  classified from its own signature rather than from a fact about `handlers`,
  which was the hazard that kept the value domain off call spans before.
  Both return spellings are covered: an expression-bodied arrow records its
  return on the function fact, so `returned_callees` now chains
  `functions[].expression_return` and `() => make()` is demanded exactly like
  `return make()`. Absent (no exact call-like node) and `unknown` (checker
  error or recovery type) remain fail-closed, as does a callee whose
  `resolvedCall` is not `Valid`. Pinned by
  `fixtures/reactive-ir/solid2-precision`'s `ReturnedCallCleanupReturns` and
  the two module-level `onSettled` returned-call cases.
  Across the corpus this discharged 30 SC9002 obligations and proved 13 SC3004
  returns — all of them a call producing a primitive where a cleanup is
  expected, such as `createEffect(() => 1, () => read())` and
  `createEffect(() => count(), () => untrack(() => count()))`.
  `callability` is no longer demanded at returned-call spans. Cleanup was its
  only consumer, and demanding it there is not merely dead: callability is read
  through `smallest_contained_callability`, which selects the smallest entity
  *contained* in the queried span, so a callability fact on an expression-bodied
  arrow's own returned call (`(post) => post.includes(id())`) sits inside the
  callback-argument span and outranks the arrow. That answered "is
  `post.includes(id())` callable" (no) where `inline_standard_callback` asked
  "is this argument a callable callback", which silently withdrew the
  `Array#filter` synchronous proof and with it SC1002 on the accessor read —
  visible only when the callback body *is* the returned call, since a binary
  body has no returned callee. The result domain is invisible to that lookup.
- **Evidence-backed divergence from upstream, `no_direct_mutation`**: with
  compound-assignment and update facts, the shared port now reports
  `store.count++` on a props/store member, which eslint-plugin-solid 0.14.5
  (commit `6d3bc311`) misses — its props branch tests for an ESTree
  `AssignmentExpression`, and `++` is an `UpdateExpression`. The compound form
  and an accessor binding's `++` are both parity-correct (upstream reports them
  via `AssignmentExpression` and `reference.isWrite()` respectively). No upstream
  case exercises any of these spellings, so parity stays green and there is no
  `deviations.json` entry to attach; pinned by
  `fixtures/reactive-ir/v1-reactivity`'s `MutatesInPlace`.

## Resolved: false negatives closed 2026-08-16

- **Leaf-owner rules follow the dynamic extent through exact helpers**
  (`cleanup.rs::helper_forbidden_operations`). `onCleanup`/`flush`/primitive
  creation in a project function's *synchronous extent* (body minus nested
  function bodies) throws when the function is called from a leaf scope; the
  call site in the leaf callback is flagged, naming the helper
  (`LeafOwnerOperation::via`). Resolution is the exact TypeScript identity,
  transitive with a cycle guard. Remaining boundaries, deliberate: an
  unresolved/ambiguous/package callee contributes nothing (package behavior
  is the contract surface's), IIFEs inside a helper count as nested bodies
  (silent), and helper calls written inside nested functions within the leaf
  callback are not the leaf's synchronous extent (silent, correct).
  The leaf callback must also be a **function literal written directly in the
  owner's callback argument**, or an exact in-project function reference:
  `createTrackedEffect(makeCallback())` evaluates its factory under the
  enclosing owner *before* any leaf scope exists, and
  `createTrackedEffect(wrap(() => …))` hands the arrow to an opaque wrapper
  that decides what the owner receives. Neither is a violation proof, so both
  are SC9012 obligations rather than silent false negatives.
  `fixtures/reactive-ir/leaf-owner/` pins the `onCleanup`, `flush`, and
  primitive positives, the transitive hop, exact safe and defective
  references, both literal spellings, the nested-body and event-handler
  negatives, and both opaque argument forms.
  Cost, accepted: the helper traversal is redone per call site rather than
  memoized by callee symbol. Depth is capped at 8 with a cycle guard and the
  walk only starts for a non-primitive call inside a leaf callback, so the
  fan-out is small; memoizing it is open work.
- **`draggable={false}` on draggable-by-default elements** (2.0 catalog).
  The rc.0 runtime removes the attribute on `false` (RFC 07's remove half),
  and removal selects `auto`, which is draggable on `img` and `a[href]` —
  flagged with the `draggable="false"` fix hint; 1.x stringifies
  (`draggable="false"` works) and is deliberately unaffected. The `a` default
  needs a **proven-present** final `href` write — a JSX string or the bare
  spelling. A spread-carried/final spread value may not be there, and a dynamic
  `href={expr}` is removed by the runtime when `expr` is nullish, after which
  the anchor is *not* draggable by default; both are now explicit
  **uncertifiable** obligations rather than silent false negatives. A later
  static href remains proven, while an anchor with no href and no spread is
  proven non-draggable in auto state and stays clean. Every other element and
  the string draggable spelling stay clean too.
  Pinned in the backend `jsx-correctness` fixture for both dialects,
  including the dynamic-`href` anchor.

## Resolved: upstream quirks that contradicted the compiler

- **`on:`/`oncapture:` duplicate folding is gone** (2026-08-16,
  `solid1x_syntax.rs::duplicate_slot`). Upstream folds `onClick`/`onclick`/
  `on:click`/`oncapture:click` onto one name and reports runtime-legal pairs
  as duplicates. The compiler lowers `on:evt` to a bubble
  `addEventListener`, `oncapture:evt` to a capture `addEventListener`, and a
  non-delegated plain `on*` to one listener per occurrence — all attach, so
  none of those pairs is dead code. `v1/jsx-no-duplicate-props` now reports
  event-shaped names only for proven single-winner slots: the delegated
  `el.$$event = handler` property write (later-wins) and the statically
  valued template attribute (first-wins, shared with `attr:`). No upstream
  parity case pins the folding, so the corpus is unaffected;
  `fixtures/reactive-ir/eslint-compat` pins both directions.
  The slot model is **DOM lowering, so it applies to intrinsic elements
  only**. A component's props are a plain object the compiler never lowers:
  there the slot is the key as written, so `<MyComp onSave={a} onSave={b} />`
  and `<MyComp on:click={a} on:click={b} />` are real later-wins duplicates
  (the slot model would have silenced both), while `onClick`/`onclick` and
  `attr:title`/`title` are distinct keys.
  The static-value half is a *node-kind* test matching the compiler's inline
  branch (`StringLiteral`/`NumericLiteral`): `{0x10}` and `{1_000}` freeze,
  `{-1}`/`{+1}`/`{NaN}`/`{Infinity}` do not.

  `v1/event-handlers` (SC8001, `solid1x_attributes.rs`) now uses the same
  compiler node-kind predicate, so `{-1}`, `{+1}`, `{NaN}`, and `{Infinity}`
  are dynamic while radix and separator numeric literals remain static. The
  shared static-string resolver still covers upstream's proven string locals
  and literal concatenations. No upstream corpus case separates the two
  spellings, so parity is unaffected and there is no deviations entry to
  attach.

  Adding the node-kind predicate was not sufficient on its own: a source-text
  arm (`text(span).parse::<f64>().is_ok() || static_string(..)`) survived in
  the same disjunction and decided the answer first, so `{-1}` and `{NaN}`
  still reported until it was removed. The diagnostic asserts Solid "will treat
  the value as an attribute", which is only true of the frozen forms, so the
  text arm was making a false claim rather than a conservative one. Pinned by
  `fixtures/reactive-ir/eslint-compat`'s `onClick={-1}`/`onClick={NaN}` pair
  (now clean) alongside the `onFoo="a"`/`onFoo="b"` static duplicates (still
  reported).

  **Closed 2026-08-18: non-frozen, non-callable handler values.** On a normal
  declared handler such as `onClick`, `onClick={-1}`, `onClick={NaN}`, and
  `onClick={someNumber}` are TS2322 and remain checker-silent. The uncovered
  case is a hyphenated attribute such as `on-event`, which TypeScript
  deliberately declines to check even though the compiler lowers every native
  `on` prefix as a listener. SC1007 now reads the exact runtime value domain and
  array shape there: a proven non-callable, non-array value is a violation; a
  callable/non-callable union, `any`, or an unresolved array/bound-pair shape is
  uncertifiable; a callable or absent handler is certified. Type assertions are
  peeled before classification. Real-typings oracle cases pin violation,
  uncertainty, and safe controls in both dialects.

## Audited remaining `TypeDescriptor.text` consumers 2026-08-17

**No consumer decides anything from `TypeDescriptor.text` any more** (verified
2026-08-18). Every remaining hit either labels a message or is a doc comment; the
two that made proof decisions were replaced by facts, below. The audit is kept
because it is what made the replacements findable:

- `interproc.rs` uses `text` only to label an unknown-callback diagnostic and
  generated contract stub; it does not make a proof decision.
- `solid1x_structure.rs` and the array branch of `solid1x_attributes.rs` asked
  a type-shape question (array/tuple versus a callable value) by matching
  descriptor text, because the Type Facts schema had no structural array-shape
  fact. **Resolved 2026-08-18** — see the `arrayShape` entry below.
- `shared_reactivity.rs` does not: its remaining `text` use is not a
  type-shape test.
- `server_rules.rs` asked whether a transport type has a rich serialization
  member (`Date`, `Map`, `Set`, typed arrays, and so on). **Resolved
  2026-08-18** — see the `libraryTypes` entry below. Its one remaining use of
  `text` quotes the author's type in the message; the decision never reads it.

Two consumers reintroduced a text decision while the SC1007 handler domain and
the SC7007 transport domain were being widened, and both are closed again:

- `shared_reactivity.rs::unchecked_handler_value_proof` certified an absent
  handler by matching `"null" | "false"`. `type Falsy = false` renders as
  `Falsy`, so the identical runtime value was a *proven violation* through an
  alias and silent as a literal. The runtime value domain cannot separate them
  — `null` and `false` both arrive as `may_be_other` — so the proof now comes
  from the AST: the literal written at the attribute, or the initializer of an
  immutable binding the reference resolves to.
- `server_rules.rs::argument_is_proven_json_safe` matched `"string" |
  "boolean" | "true" | "false" | "null"` for JSON safety, with the same alias
  asymmetry in the other direction — a spurious obligation on `type Name =
  string`. Type Facts exposes no structural primitive-kind fact, so the
  certification is now `null`, a constant string, a finite constant number, and
  an AST-proven primitive/nullish literal. **Remaining approximation:** a
  declared `string`/`boolean` *identifier* is no longer certified and produces
  an explicit SC7007 obligation. That is the fail-closed direction, and it is
  uniform across spellings, but it is a real precision loss until a structural
  primitive fact exists.

## Resolved: static attribute values are a fact, not a rendered type 2026-08-17

Bumping `typefacts` for the call-result domain also brought that revision's
node-selection change ("Classify complete demanded expressions"): a demand
resolves to the complete expression at its span rather than the deepest node at
its start byte. That is the correct subject, and it exposed a consumer
heuristic that had been right only by accident.

`upstream_compat::literal_string_type` recovered a static attribute string by
parsing `TypeDescriptor.text` for a rendered literal type, decoding JSON-style
escapes by hand. For `innerHTML={"a" + "b"}` the old selection typed the
leading `StringLiteral`, so a literal type appeared and the value read as
static; the complete `BinaryExpression` widens to `string`, so the same test
called a static value dynamic and `v1/no-innerhtml` reported it (upstream's own
case asserts it is valid under `allowStatic`).

The fix is the fact the migration guide asks for, not a repaired heuristic:
`constantValue` (`solid-ts-facts` `fc739a6c`, ADR 0014) is demanded at the
exact attribute-value span and accepted only as a present `kind: "string"`.
The producer folds literals, substitution-free templates, transparent
wrappers, unary signs, same-kind binary `+`, and compiler-resolved immutable
declarations (`const`, `readonly`, enum members), bounded by a depth limit and
a declaration cycle guard. Absence is "not proven constant", so a dynamic
value stays uncertifiable rather than guessed.

This is a precision *gain* in both directions, not only an FP fix:
`v1/jsx-no-script-url` now proves the scheme in
`href={"java" + "script:alert(1)"}`, which no literal type ever described, and
a `const`-referenced value is static wherever it was declared. Pinned by
`fixtures/reactive-ir/upstream-divergences`'s `FoldedMarkup` and `ScriptUrls`;
parity returns to fully green at 421/465, and
`no-array-handlers__valid__10`'s declared deviation was removed because the
complete-expression selection types its unresolved `SafeArray` cast correctly
and it no longer deviates.

Deliberately **not** folded into the producer: the *node-kind* tests. The 1.x
compiler inlines an attribute into the template on a `StringLiteral`/
`NumericLiteral` branch, so `jsx-no-duplicate-props` must keep asking what was
written rather than what it evaluates to — `{"a" + "b"}` is not inlined. The
`v1/event-handlers` inconsistency recorded above was that same syntactic
question and is now closed; see the note under the duplicate-props entry.

## Resolved: array/tuple shape is a fact, not a rendered type 2026-08-18

Two consumers decided "is this an array or a tuple?" by matching
`TypeDescriptor.text` against `[`, `readonly `, `Array<`, `ReadonlyArray<`, and a
trailing `[]`. Both were fail-closed, so both were false negatives, and text
could not settle the question in two independent ways:

- **An alias renders as its own name.** `type Handlers = [(data: number, event:
  MouseEvent) => void, number]` renders as `Handlers` and matched no prefix.
- **A trailing `[]` is ambiguous.** An array of functions (`((n) => void)[]`) and
  a function returning an array (`() => string[]`) render identically, which is
  why the screen also had to consult `callability` — and even then it could not
  see through the alias.

The fix is the fact, not a repaired heuristic: `arrayShape` (`solid-ts-facts`
`ce4c772`, ADR 0015) classifies the type at the exact demanded expression span
with the checker's own `isArrayOrTupleType` predicate over the real union
constituents. `array` requires every constituent to be an array or tuple;
`notArray` requires none to be, and is a positive claim so the negative is usable
as proof; `mixed` and `unknown` are proven states that prove neither side.
Absence stays fail-closed. `expression_descriptor` and `expression_callability`
had no other callers and were removed with the screen.

Closed false negatives, measured by A/B against the text screen on
`fixtures/reactive-ir/array-shape-v1`:

- `v1/no-array-handlers` now reports an aliased tuple, a doubly-aliased tuple,
  and a call returning one — all previously silent.
- `v1/prefer-for` now offers the `<For each>` autofix when the `.map` receiver is
  an *alias* for an array (`type Rows = string[]`). The alias hole had been
  withholding a correct rewrite, which is the same defect reaching a second rule.

### Narrowed in the same pass: `no-array-handlers`' `on:` arm

Writing the fixture against the real typings (`scripts/tsc-oracle.mjs`) showed
the fixture stub had been hiding a duplicate — the trap `fixtures/tsc-oracle`
exists for. `solid-js@1.9.14` types the two handler spellings differently:

~~~ts
type EventHandlerUnion<T, E, EHandler> = EHandler | BoundEventHandler<T, E, EHandler>;
interface BoundEventHandler<T, E, EHandler> { 0: (data: any, ...e: Parameters<EHandler>) => void; 1: any }

type EventHandlerWithOptionsUnion<T, E, EHandler> = EHandler | EventHandlerWithOptions<T, E, EHandler>;
interface EventHandlerWithOptions<T, E, EHandler> extends AddEventListenerOptions { handleEvent: EHandler }
~~~

`onEvent` accepts `BoundEventHandler`, an interface with members `0` and `1`, so
a `[handler, data]` tuple is legal and only this rule can object to it. `on:event`
has **no** bound arm, so every array and every tuple there is already `TS2322`:

~~~
Type '[(data: number, event: MouseEvent) => void, number]' is not assignable to
type 'EventHandlerWithOptionsUnion<HTMLDivElement, MouseEvent, ...>'
~~~

in both the strict and non-strict passes. The `on:` arm was removed under the
absolute rule; upstream case `no-array-handlers__invalid__03` is now a declared
`typescript-owned` deviation, verified by `scripts/parity-tsc-ownership.mjs`.
Parity moved 373/465 → 372/465 with 102 deviations, all declared.

### Closed 2026-08-18: the plain-array duplicate, via `tupleShape`

A **plain array** on `onEvent` has no `0`/`1` members either, so it was `TS2322`
too — confirmed for `((event: MouseEvent) => void)[]`, `ReadonlyArray<...>`,
`any[]`, and `unknown[]` — and the rule reported it anyway.

`arrayShape` could not settle it, by construction: it reports `array` for a plain
array and a tuple alike, because both of its consumers wanted the union of them.
The condition that is genuinely this rule's is not "array or tuple" but **"a
tuple with both numbered slots whose first can be called with `(data, event)`"**
— exactly what `BoundEventHandler` accepts and `tsc` therefore permits.

`tupleShape` (`solid-ts-facts` `b9d1a8e`, ADR 0016) supplies it: fixed slot count,
whether a rest tail follows, and the first slot's callability *and minimum
arity*, present only when the type at the exact span is itself a tuple. The rule
now fires on that and nothing else. `tsc` names each removed shape's reason
precisely, which is how the partition was checked:

~~~
((event: MouseEvent) => void)[]      missing the following properties: 0, 1
[number, number]                     Types of property '0' are incompatible
[(event: MouseEvent) => void]        Property '1' is missing
[1, 2, 3]                            Type 'number' is not assignable to (data: any, e) => void
[(a, b, c) => void, number]          Target signature provides too few arguments
~~~

The last row is the arity residual, closed by an amendment to ADR 0016.
`elementZero` says the slot is callable, which does not settle whether it can be
*invoked* with the two arguments Solid passes: `BoundEventHandler` types slot 0
as `(data: any, ...e: Parameters<EHandler>) => void`, and `EventHandler` takes
one parameter. A handler requiring three is callable, and not callable here.
Adding `elementZeroMinimumParameters` took the fixture's SC8007 count from 6 to 5.

Against the fixture, `handler-cases.tsx` now holds every SC8007 and produces
**zero** `tsc` diagnostics, while `clean-cases.tsx` produces eight and **zero**
findings. The rule and the type checker partition the space exactly.

**Contextual typing is what makes this work, and it is load-bearing.** An array
literal written where the expected type has numbered members acquires fixed
slots; the same literal in an unconstrained position stays a plain array. So
`tupleShape` absent *plus* an array literal written here means no bound-handler
type applies at this attribute — the project's JSX typings are not checking it,
and the rule is the only thing that can speak, which is the boundary
`jsx_name_is_type_checked` already draws. The syntactic fallback is kept for
exactly that case.

The consequence is that `fixtures/reactive-ir/array-shape-v1` had to stop using a
permissive `IntrinsicElements` index signature: its stub now carries the real
`EventHandlerUnion`/`BoundEventHandler` signatures, because a looser stub erases
the contextual typing the rule depends on and the fixture would stop exercising
its own path. The upstream parity corpus still uses a permissive harness, so
three cases upstream reports are now declared `typescript-owned`
(`no-array-handlers__invalid__03`, `__05`, `__07`), each verified against the
real typings by `scripts/parity-tsc-ownership.mjs`. Parity 372/465 → 370/465,
104 deviations, all declared.

### Closed 2026-08-18: unions of tuples

A union had no `tupleShape` at all — the fact answered only for a type that was
itself a tuple — so `Handlers | OtherHandlers` and the very common optional
`Handlers | undefined` both failed closed. `arrayShape` reported `mixed` for the
optional form, which proves neither side, so the rule went silent on values that
are a bound pair whenever they are anything.

The fact now answers for a union with its constituents' **meet**: the slots they
all have, a rest tail only if all carry one, callable only if all are, and the
largest argument requirement among them. What it reports therefore holds
whichever constituent the value turns out to be. A single non-tuple constituent
voids the answer, and nullish constituents are skipped because they carry no
structure — presence stays `runtimeValueDomain`'s question. The payload and Wire
table schema are unchanged; this widened when the fact is emitted, not its shape.

`tsc` agrees on every boundary, which is how it was checked: `H1 | H2` and
`H1 | undefined` are silent (ours, now reported), while `H1 | number[]`,
`H1 | [number, number]`, and `H1 | [(a,b,c) => void, number]` are each TS2322
(the type checker's, still silent here).

### Closed 2026-08-18: mixed handler shapes and runtime presence

A union that mixes a bound pair with a **plain function** (`Handlers |
((e: MouseEvent) => void)`) has no `tupleShape`: one constituent is not a tuple,
so no violation holds for every runtime value. It no longer disappears. The
handler expression now also demands `runtimeValueDomain`; callable/non-callable
unions, `any`, and generic shapes produce an explicit **uncertifiable** SC8007.
An unresolved import remains silent because TS2307 owns that source.

The same audit found two adjacent proof errors:

- `Handlers | undefined` has a common tuple shape, but a tuple is not present on
  every execution. With `strictNullChecks` disabled TypeScript erases the
  undefined constituent before the runtime-domain fact is computed, and the IR
  does not receive that compiler option. A violation is therefore reserved for
  a structurally proven runtime array (an inline literal or immutable local
  array initializer); type-only pair evidence remains uncertifiable.
- A TypeScript assertion was treated as a safety voucher. JSX facts now record
  whether a wrapper is a runtime type escape, and the rule demands/classifies
  the peeled runtime expression. An asserted array is a violation, an asserted
  function is safe, and an unresolved runtime shape is uncertifiable.

The focused fixture now pins proven violations, proven-safe controls,
TypeScript-owned invalid tuples/arrays, and every uncertifiable branch. The
real-typings oracle carries strict and non-strict cases for a pair/function
union, an optional pair, an asserted array, and an asserted-function control.

### Closed 2026-08-18: `rich_transport_member`, via `libraryTypes`

`server_rules.rs` asked whether a server-function argument is one of a few
runtime types JSON cannot carry (`Date`, `Map`, `Set`, `RegExp`, a typed array).
It answered by splitting `TypeDescriptor.text` on top-level `|`/`&`, stripping a
`[]` suffix, and matching the head against a name list. It was the second
`TypeDescriptor.text` consumer named alongside `array_like_type`, and it was
deliberately left alone when `arrayShape` landed on the grounds that its question
was open-ended.

That framing was wrong, and worth recording. The open-ended question is "does
this object graph contain a non-JSON-safe member" — a recursive walk needing a
cycle guard, a depth bound, and a nesting policy. But that is not the question
this rule asks. It asks only about the **top level**, by deliberate design ("an
unproven rich type is not a proven throw"), and a bounded top-level question is
exactly the kind a fact can answer.

`libraryTypes` (`solid-ts-facts` `3d51c40`, ADR 0017) answers it: the sorted set
of standard-library type names the type at the exact span is built from at its
top level — itself, its union and intersection members, and one array-element
unwrap. A name is recorded only when the resolved symbol is declared in a
default-library file. The rule keeps its own list of which names matter, and that
a lone `Uint8Array` has a natural HTTP encoding; the fact carries no policy.

Three defects text could not avoid, all closed:

- **An alias renders as its own name.** `type Stamps = Date[]` matched nothing,
  declared locally or imported from another module. Measured on
  `fixtures/reactive-ir/server-function-rich-args`: the text walk found 6
  findings, the fact finds 8.
- **`Array<Date>` and `Date[]` are the same runtime value** and only the second
  matched.
- **A user-declared type could match a global's name.** `Map` from the project's
  own code is now excluded by its declaration file, not hoped away by spelling.

The fact's nesting boundary remains unchanged, but it no longer becomes a
checker blind spot: `Boxed = { title: string; when: Date }` produces an explicit
uncertifiable SC7007 because the complete object graph is not proven JSON-safe.
`tsc` cannot duplicate any of this — the argument's type matches its parameter's
type by construction, so no diagnostic is possible; the claim is entirely about
runtime transport.

## Closed 2026-08-18: unresolved generic member dispatch is explicit

Generic member calls now have three outcomes instead of a silent fail-closed
branch. One exact implementation contributes its summary; a finite set of
implementations contributes a summary only when every candidate has the same
reactive-read behavior; missing or divergent behavior produces SC9012
`reactive-dispatch-unresolved` as an `uncertifiable` finding.

The explicit obligation covers parameter-member substitution at each call
site, computed calls whose TypeScript call is valid, and direct member dispatch
on parameters of exported helpers with unseen callers. A compiler-proven
tracked JSX call is safe regardless of which implementation reads, and an
exact standard-library method is not open dispatch, so neither produces an
export obligation. Member calls nested in returned, assigned, or scheduler
callbacks are not mistaken for direct export behavior: the existing callback
execution contract owns those paths. These distinctions keep JSX collection
helpers and higher-order adapters certifiable while refusing a falsely empty
summary for a helper whose own execution directly depends on an unseen
implementation.

`fixtures/reactive-ir/interprocedural-methods-v2/` pins all three finite-set
outcomes: an exact reactive object is a proven SC1001 read, reactive candidates
with equivalent summaries remain a proven read, and reactive/inert candidates
produce SC9012. It also pins valid computed dispatch as SC9012 and a direct
export boundary. `fixtures/reactive-ir/v1-reactivity/` pins the same shared
obligation under the v1 identity. The real-typings oracle carries keystones for
both dialects and TS2349 negative controls; invalid calls remain TypeScript's
job and never receive SC9012.

## Design-change candidates (open)

### `execution-map-incomplete` (SC9004) moved to producer integrity

Both dialect compilers emit every `jsx-expression` operation together with a
same-span region or callback role in every decision arm, and
`CompilerFacts::classifies` matches by span containment — so
`uncovered_jsx_expressions()` is empty by construction. The former SC9004
project rule could therefore describe only externally produced or partial
compiler facts, not a defect in analyzed source. It was removed from both
catalogs on 2026-08-20 and retained as a producer-integrity requirement. If a
third compiler adapter lands, its adapter tests must prove the same totality.

### Shorthand property values follow exact project-local exports

TypeScript projects a shorthand property's *own* symbol at `{ pathname }` --
never the referenced value binding's -- so no TypeFacts entity, reference, or
declaration fact at that span identifies the value. The binder that builds the
Oxc AST facts does resolve that exact reference, and its answer is now carried
on `ObjectPropertyFact::shorthand_binding`; `interproc.rs`
(`binding_initializer`, `named_accessor`) reads the declaration from it instead
of matching the spelling within the enclosing function. That is scope-exact, so
the previous block-scoping hole is closed in both directions.

The cross-file gap is now closed for named and default relative imports: a
shorthand whose binder declaration is an import specifier follows the relative
specifier to the exporting file — exact ESM resolution against the analyzed
file set, never the filesystem — and matches that file's exported declaration
in the accessor map exactly as the same-file arm does
(`interproc.rs::imported_accessor`). Named re-exports, default re-exports,
export-all chains, and cycles are followed with a cycle guard. What remains
unavailable for an exact structured-property claim is listed below. Each
exported shorthand now produces SC9012 instead of disappearing; the generated
summary still omits the unproven property rather than inventing a reactive leaf:

- **an ambiguous relative specifier.** `./values` can name `values.ts`,
  `values.tsx`, or `values/index.ts`, and which one a bundler picks depends on
  resolution settings this pass does not model. When more than one project
  file matches, `relative_module_file` returns `None` rather than taking the
  first one enumerated — file order is not evidence, and a proven accessor
  claim sourced from the wrong module would be worse than no claim. **Pinned**
  by the fixture's `ambiguousShorthand` (`ambiguous.ts` +
  `ambiguous/index.ts`, both exporting the accessor).
- **bare and path-mapped specifiers**, which the resolver rejects outright
  (it only walks `./` and `../` against the analyzed file set, never the
  filesystem or `tsconfig` `paths`).
- **globals/unresolved bindings.** A namespace import is an exact non-reactive
  namespace object and remains certified without SC9012. An unresolved export
  cycle is TypeScript-owned (TS2303), so it receives no checker finding.

What the fixture pins today is the same-file resolution set
(`scopedShorthand`, `unprovenShorthand`, `shadowedShorthand`,
`writtenShorthand`), the cross-file named-import join
(`importedAccessorShorthand`), the ambiguity bail (`ambiguousShorthand`), a
nondeterministic import set (`importedShorthand`, `namespaceShorthand`,
`bareImportShorthand`, `pathMappedShorthand`, `cyclicReexportShorthand`),
the default/named/export-all joins (`defaultReexportShorthand`,
`namedReexportShorthand`, `exportAllShorthand`), and a global
(`globalShorthand`).

The same fixture asserts the five unresolved exported shorthands explicitly:
ambiguous relative resolution, bare import, path mapping, and global binding.
Exact local non-reactive values and namespace objects remain certified without
SC9012.

The shared `solid_facts::resolve_relative_module_path` helper now answers
"which file does this relative specifier name" for both
`interproc.rs::relative_module_file` and the backend's
`resolve_relative_export`. It is lexical, project-local, and returns no
answer when extension/index candidates are ambiguous.

## Partially resolved design changes

- **`v1/jsx-no-undef` now fails closed on missing semantic facts.** It reports
  unresolved `use:` names only when the structural binder proves that no
  lexical binding exists. Unresolved JSX tags, including dotted roots, are
  TypeScript-owned (TS2304) and checker-silent rather than a second diagnostic.
  The old auto-import helpers remain test coverage for the upstream formatting
  logic, not a blanket semantic allowlist.
- **Unknown callback helpers remain contract obligations.** Exact TypeScript
  call facts now enrich the obligation and diagnostic with package,
  entrypoint, export/function, callback parameter index/type, required
  execution mode, and an editable schema-v1 contract stub. Standard-library
  behavior and project/package contracts can discharge it; unknown execution
  remains refused until an explicit contract proves it.

## Resolved design changes

- **Shorthand property values are resolved by the binder, not by spelling.**
  The value binding at `{ pathname }` is named by
  `ObjectPropertyFact::shorthand_binding` -- the declaration Oxc's scope tree
  chose for that exact reference -- so a same-spelled binding in a sibling
  block neither substitutes for the intended one nor makes it ambiguous. This
  replaced a spelling match scoped to the enclosing function, which both lost
  a provable structured return whenever any sibling block reused the spelling
  and, worse, could certify an accessor the shorthand never named. A shorthand
  the binder leaves unresolved carries no fact and proves nothing. The
  remaining cross-file gap is listed above.

- **Invoking a parameter's member is resolved per call site.** A function that
  calls `reader.read(value)` on its own parameter makes no claim about which
  implementation runs — that belongs to each caller. The owner records the
  obligation (`invoked_parameter_members`: parameter index and property), and
  `interprocedural_reads` resolves it against the argument actually passed at
  each site, the way `invoked_parameters` already substitutes a directly
  invoked parameter. One exact object contributes its reads; several exact
  objects contribute their common summary only when those summaries are
  equivalent. A missing or divergent implementation produces SC9012 rather
  than contributing no read. This replaces both the old pooled answer (which
  contaminated an exact site with an ambiguous sibling) and the later silent
  omission at the ambiguous site.

- **Callee resolution is exact and conservative.** Parenthesized, `as`,
  `satisfies`, and non-null wrappers are peeled through a shared AST fact
  helper. Resolved call declarations identify member callees when TypeScript
  provides them; static members can use their exact property entity, while a
  TypeScript-valid computed member such as `handlers[i]()` produces SC9012
  instead of inheriting `i`, inheriting `handlers`, or disappearing. An invalid
  computed call remains silent because TypeScript owns it.
- **Summary discovery covers method, alias, and returned-value branches.**
  Class/object methods, returned closures, conditional aliases, destructured
  function properties, and exact object spreads retain their canonical
  symbols. Direct generic calls and resolved structural member calls propagate
  summaries only through the dispatch proof described above; a finite
  unresolved dispatch remains explicitly uncertifiable through SC9012.
- **Transparent TypeScript wrappers are peeled at equality gates.** The
  shared helper is used by map/callback discovery, Solid 1.x structure gates,
  and shared reactivity function matching, with AST and fixture coverage for
  parentheses, `as`, `satisfies`, and non-null assertions.
- **Namespace-imported JSX primitives use dialect vocabulary.** `<Solid.For>`,
  `<Solid.Show>`, and `<Solid.Repeat>` resolve only when the namespace import
  is from a dialect-owned module and the member is in that dialect's export
  vocabulary. The namespace and named-import twins are pinned by
  `fixtures/reactive-ir/namespace-import-v2/`.
- **Component identity conventions are dialect-owned.** JSX call sites,
  direct JSX returns, and exact compiler-resolved Solid component aliases prove
  component identity. Solid 1 does not promote upstream's uppercase-name
  shortcut to proof: capitalization makes component identity **uncertifiable**
  until a JSX call site or exact component type selects the execution model.
  This uncertainty propagates through ownership, props, reads, destructuring,
  conditional returns, handler reads, and mutations. Solid 2's
  direct-JSX-return convention remains dialect-owned. The shared reactive core
  contains no hard-coded proven-component casing rule. Intrinsic-tag case
  checks remain syntax-only.
- **Dialect-owned type origin is no longer enough to register a source.** The
  dialect classifies exact exported aliases as component, accessor/resource,
  signal, store, setter, or store setter; user-local lookalike aliases and
  unrelated Solid types do not become accessors.
- **Unclassified function spans are `Unknown`.** Explicit compiler-untracked
  regions and other semantic proofs become `UntrackedRendering`; AST-proven
  module evaluation is its own one-shot role. Unknown reads/writes are not
  projected as violations.
- **Owner-shape recognition is AST-backed.** Binding immutability, array
  slots, call initializers, returned functions, and arrow kind now come from
  facts rather than scanning source bytes.
- **Compiler-established ownership is trace-backed.** Default compiler effect
  reruns emit typed owned regions without changing generated code. Custom
  wrappers make no claim; component and runtime-callback ownership still comes
  from exact TypeFacts identity and package contracts.
