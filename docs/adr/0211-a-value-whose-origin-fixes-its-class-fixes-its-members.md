# ADR 0211: A value whose origin fixes its class fixes its members

- Status: accepted and implemented (2026-10-06). Phase 1 of the value-flow
  work on caller-supplied member dispatch.
- Owners:
  - `ValueOrigin`, `SemanticLookup::value_origin` and
    `SemanticLookup::class_method_symbol`
    (`solid-reactive-ir/src/indexes.rs`);
  - the call-site resolution of a callee's parameter-member invocations
    (`solid-reactive-ir/src/interproc.rs`);
  - `BindingFact::initializer_value_kind` (`solid-facts/src/ast/mod.rs`);
  - the demand for an argumentless construction's resolved call
    (`solid-facts-backend/src/demand_plan.rs`).
- Relation: narrows `parameter-member-target-unresolved` (`SC9012`) beyond
  ADR 0190's primitive receivers, and reuses ADR 0209's reassignment check.

## Context

A helper that invokes a member of its parameter leaves the implementation to
each call site:

```ts
function evens(list: number[]) { return list.filter((n) => n % 2 === 0); }
evens([1, 2, 3]);
```

The call site resolves `filter` from the argument. It found no project
implementation, so every such call carried an obligation, even this literal.
On the rc.13 corpus, 695 obligations have this shape, and about 1,200 across
the caller-supplied family.

The declared type cannot settle it, for two reasons:
- **Subclasses:** an Array subclass may override `filter` and still be typed
  `number[]` without a cast.
- **Stores:** a Solid store array is a proxy, and its `filter` reads the
  store.

The value's origin can settle it.

## Decision

1. **A fresh built-in value runs its prototype's members**, and no built-in
   member reads reactive state. A value is a fresh built-in when it is:
   - an array literal;
   - `new` of a reviewed value class: `Array`, `Date`, `Map`, `Set`,
     `WeakMap`, `WeakSet`, `RegExp`, `ArrayBuffer`, `DataView` or a typed
     array, matched by the compiler-selected construct signature
     (`DateConstructor.construct`);
   - a fresh array from `Array.from`, `Array.of`, `Object.keys`,
     `Object.values` or `Object.entries`, called on that global (`List.from`
     builds a subclass under the same declaration), or from `String.split`;
   - an array method that builds by the receiver's species (`map`, `filter`,
     `slice`, `concat`, …) on a value already proven an array, where nothing
     assigns a `constructor` member.

   `Proxy`, `Function`, `Promise` and every event target are left out: their
   members run user code registered elsewhere.
2. **An instance of exactly a project class runs the method that class
   declares.** The instance comes from `new C(…)` with `C` a project class.
   The method must be one plain method in `C`'s own body, not static and not
   an accessor. Its summary is used like any resolved implementation.
   Inherited methods are not followed: the AST carries no heritage.
3. **The origin is followed through `const` bindings only.** A binding
   initializer now carries its runtime kind (`initializer_value_kind`), as an
   argument does. A `let`, a parameter or a property proves nothing.
4. **Nothing applies where the program may replace the member.** This is
   the case when an assignment writes a member of that name or `__proto__`,
   or replaces a prototype or writes one at a dynamic key (ADR 0209's check,
   narrowed by ADR 0213: a named prototype write refuses only its name).
5. **An argumentless construction's resolved call is demanded** (`new
   Date()`). Only the resolved call names the constructor.

## Consequences

- A call site whose argument has one of these origins carries no
  dispatch obligation for that member. A project class's method contributes
  its reads.
- The demand also resolves argumentless constructions written in leaf-owner
  callbacks and their helpers (`new Map()`, `new Set()`). The leaf walk then
  reads them as standard-library calls, as it does any other.
- Assumptions, as for every standard-library row:
  - no program patches a built-in prototype outside an assignment (for
    example through `Object.defineProperty` or `Object.setPrototypeOf`);
  - an element's coercion (`join` calling an element's `toString`) is not
    modeled.
- Still open:
  - arguments that are accessor calls (`items()`), properties (`props.items`,
    `state.items`), parameters and `let` bindings;
  - inherited methods of project classes;
  - multi-segment paths (`list.windows.map`).

## Evidence

- **Fixture** `fixtures/reactive-ir/value-origin-member-dispatch`:
  - clean: an array literal, a `const` array and `slice` of it, `Array.from`,
    `Object.values`, `split` with `map`, `new Date()`, `new Date(0)`,
    `new Set(…)`, and an exact `Counter`;
  - `Live.now`'s read reaches the call site as an `SC1001` uncertifiable
    read. That it runs during the call is proven only through plain calls
    (ADR 0204);
  - `SC9012` stays for a store array, a `let`, an inherited method, a
    reassigned method and a `Proxy`.
- **Coverage:** 180 fixture projects, 956 findings. Only the new snapshot is
  new.
- **rc.13 corpus**, browser host, release binary, against
  `rc13-o-browser.json`:
  - violations unchanged at 285;
  - uncertifiable 3,414 to 3,386: 27 distinct sites removed, none added. These are
    literal and `const` array arguments, and argumentless `new Set()` and
    `new Map()` in leaf-owner callbacks.
