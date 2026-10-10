# ADR 0215: The value-flow proofs survive review

- Status: accepted and implemented (2026-10-06).
- Owners:
  - `SemanticLookup::member_name_may_be_reassigned`, `binding_written`,
    `class_instance_is_exact`, `class_method_symbol` and
    `function_value_is_current` (`solid-reactive-ir/src/indexes.rs`);
  - the leaf-scope walk (`solid-reactive-ir/src/cleanup.rs`);
  - the pass-through discharges (`solid-reactive-ir/src/attribution.rs`);
  - `ClassFact::heritage` and `ClassFact::elements`
    (`solid-facts/src/ast/mod.rs`).
- Relation: corrects ADR 0209 to ADR 0214 after an adversarial review that
  found reachable counterexamples in each.

## Context

A source review of ADRs 0209 to 0214 (`rust/target/research/review-0209-0214.md`)
constructed type-correct programs for which those proofs were wrong:

- **False positives:**
  - a generator called in a leaf scope was walked as if its body ran;
  - a static method written on its own line could stand for an instance
    method;
  - a constructor returning another object made the walk report the class's
    method.
- **Wrong clean results:**
  - computed writes (`C.prototype[key] = f`) escaped the reassignment veto;
  - the pass-through discharge skipped the veto for a direct origin;
  - writes to an updater's previous value, or destructuring and loop-head
    writes to a parameter, went unseen;
  - a reassigned `let` function, or a call-initialized binding, was read as
    its initializer;
  - `thenable.then()` with no callback completed vacuously;
  - an invoked function's defaults were not walked;
  - a local class named `Array` passed for the global;
  - `split` was trusted with a protocol separator;
  - a memo's `loadingValue` option was ignored;
  - aliased props went unchecked.

## Decision

1. **Writes are found wherever they are.**
   - A computed write records its literal key, or the string of a `const`
     key.
   - Any other computed write onto a prototype refuses every member.
   - A binding counts as written when any assignment target or loop head
     contains a reference to it, other than as a member's object.
2. **The member veto holds for every origin**, including the direct one in
   the pass-through discharge.
3. **An exact class instance has the class's own members.** New class facts
   carry `extends` and every body element: its kind (method, getter, setter,
   field, constructor), whether it is static, and its key. A class is exact
   for a member when, through the whole resolved chain:
   - no constructor returns a value;
   - no field, parameter property or computed element can define that member
     on the instance.
   An unresolved `extends` proves nothing. `class_method_symbol` selects the
   one plain, non-static method by these facts, not by source text.
4. **A function value must be current.** It is a declaration nothing
   rebinds, a method, or the direct initializer of a `const`. This holds for
   helper calls in a leaf scope, callback arguments, and helper returns used
   as origins.
5. **Leaf scopes:**
   - calling a generator runs none of its body;
   - an invoked function's own defaults are walked, and an operation there, or
     a call there the walk cannot follow, keeps the obligation;
   - `PromiseLike.then` always keeps it;
   - an object-literal argument with an accessor or method keeps it.
6. **Origins:**
   - `Array.from`/`Object.keys` need the receiver to be the global itself;
   - `split` needs a primitive separator;
   - a memo is followed only in its bare one-argument form;
   - an updater's previous value is assumed only when nothing writes it.
7. **Props** may only ever be the object of an unwritten member read. An
   alias, spread, argument or write refuses the proof.

Two findings of the review are kept, as documented limitations:

- **A forbidden operation that may not run is still a violation.**
  `[].forEach(() => onCleanup(…))` in a leaf scope reports the cleanup, as a
  conditional `onCleanup` in a helper always has. The rule reports code that
  can perform the operation.
- **Synchronous dispatch and reflective definition are not modeled.**
  `el.dispatchEvent(e)` running a listener in the leaf scope, and
  `Object.defineProperty(c, "run", …)`, are as ADR 0210 and ADR 0211 state.

## Consequences

- Several proofs are now narrower, and the corpus shows the cost. app-game
  contains spector's `object.prototype[name] = …` on a parameter, so every
  member name is refused there, and the removals ADR 0209 to 0214 made in
  that program return.
- A per-class veto for `Component.prototype[key]` (a receiver that resolves
  to one class) would recover some of them. It is not decided.

## Evidence

- **Fixture** `fixtures/reactive-ir/value-flow-review` (a closed program):
  - each of the review's reproducers now keeps `SC9012`, except the
    generator method, which is now clean;
  - the previous release binary certified seven of the App cases clean and
    reported the generator as a violation.
- **Coverage:** 184 fixture projects, 999 findings. Only the new snapshot is
  new.
- **rc.13 corpus**, browser host, release binary, against
  `rc13-v-browser.json` (ADR 0214):
  - violations unchanged at 285;
  - uncertifiable 3,299 to 3,368: 70 added, 1 removed. Of the 70, 61 are
    app-game sites whose earlier removal rested on the prototype hole; the
    rest are call-initialized helpers and narrower member and parameter
    proofs.

## Round 2

A second review of this ADR's own fix (`rust/target/research/review-0215.md`)
found nine variants, eight of them fixed here:

1. **Methods.** A function nested in a method inherited its `method_name`.
   The method exemption of `function_value_is_current` now covers only a
   class element's or object literal method's own function.
2. **Destructuring and loop-head member writes** (`[c.run] = …`,
   `for (c.run of …)`). The name census reads every leaf member inside an
   assignment target or loop head. The object of another written member
   (`C.prototype` in `C.prototype.m = f`, also under a cast) is read, not
   written.
3. **Computed prop reads** (`props["items"]`) are not followed.
4. **Redeclarations.** `var xs = …` over a parameter, and a second
   `function f`, write the binding. Any same-named declaration in the scope
   now counts.
5. **Destructured parameter defaults** run on entry, for generators too.
   The parameter-list walk covers every call in the parameter patterns.
6. **A defaulted updater parameter** is not assumed to hold the previous
   value.
7. **Calls through `props`** (`props.replace()`) hand `props` to user code
   as `this`, so they refuse the prop proof.
8. **A getter object bound to a `const`** and named as an argument is
   checked like a literal.

Kept as a trust boundary: a dynamic-key write that is not spelled through a
prototype, such as `p[key] = f` with `key: keyof C` through a prototype
alias. It is the reflective-write class `Object.defineProperty` belongs to.
Vetoing every dynamic write would refuse every `xs[i] = v`.

- **Fixture:** `value-flow-review/Leaf2.tsx` and `App2.tsx`, ten cases, all
  `SC9012`. The previous binary certified each of them clean.
- **rc.13 corpus**, against `rc13-w-browser.json`: violations unchanged at
  285; uncertifiable 3,368 to 3,370 (kui's `BarChart` and `LineChart` call a
  function through `props`).
