# ADR 0209: A method of an exact instance is followed in a leaf scope

- Status: accepted and implemented (2026-10-06). Step 3 of the 2026-10-06
  plan (calls in leaf-owner callbacks).
- Owners:
  - `exact_instance_method`, and the `ExactThis` context through
    `helper_forbidden_operations` and `function_forbidden_operations`
    (`solid-reactive-ir/src/cleanup.rs`);
  - `class_for_symbol` and `member_name_may_be_reassigned`
    (`solid-reactive-ir/src/indexes.rs`).
- Relation: extends the leaf-scope helper walk (ADR 0179, ADR 0192).

## Context

A call in a leaf owner's callback (`onSettled`, `createTrackedEffect`) must
create no primitive, register no cleanup and flush nothing. The checker
follows a plain call into a project helper's body. A call it cannot follow
leaves an `SC9012` obligation. There are 738 of them on the rc.13 corpus.

Sized by callee, about 160 are method calls on class instances, many of them
workspace source (app-game's `ogl`, `gsap` and `bitecs` copies):

```ts
const camera = new Camera(gl);
onSettled(() => { camera.perspective({ aspect: width() / height() }); });
```

The walk looked the callee up by its whole span, where a member call has no
symbol, so every method call stopped it.

## Decision

1. **A method call is followed when its receiver's class is exactly known**:
   - the receiver is a `const` identifier bound directly to `new C(…)`, `C`
     resolves to a project class, and the method TypeScript resolves is
     declared in `C`;
   - or the receiver is `this` inside such a method, which was entered
     through an instance whose class is exactly `C`. Here TypeScript
     resolves on `C`, so an inherited method is exact too.
2. **The class is the class.**
   - The class symbol's only declaration is a class declaration at the name
     of a class in an analyzed file. A class merged with an interface or a
     namespace has several declarations, so it is not exact.
   - Through a binding, the method must be declared in `C` itself. An
     annotation can widen the binding's type to a superclass:
     `const d: Base = new Derived()` resolves `d.run` to `Base.run`, while
     `Derived.run` runs.
3. **Nothing reassigns it.** No assignment in the project writes a member
   of that name (`x.name = …`, or a literal key `x["name"] = …`), and none
   writes through a `prototype`.
   - Amended by ADR 0213: a named write through a prototype
     (`C.prototype.name = …`) refuses only that name. A write that replaces
     a prototype, or writes one at a dynamic key, still refuses every name.
   - A dynamic-key write onto a class instance is left out. Under the
     published declarations it is a type error unless the class has an index
     signature or the write casts. Every resolved member call here rests on
     the same trust.
4. **The method's body is walked like a helper's.** Its forbidden operations
   are reported with the call as their site, and a call it cannot follow
   keeps the obligation.
5. **A call in a nested function's parameter list is not body code**
   (`written_directly_in`, ADR 0204). The helper walk used to count it.

## Consequences

- A forbidden operation in a method of an exact instance becomes a proven
  `leaf-owner-forbidden-call` violation, like one in a helper.
- Still uncertifiable:
  - an instance a caller supplies, a `let`, or one held in a property;
  - an annotation that widens the class;
  - inherited methods through a binding;
  - any project that writes the method's name or a prototype;
  - methods of package classes, whose bodies are not in the program.

## Evidence

- **Fixture** `fixtures/reactive-ir/leaf-scope-exact-method`:
  - `ExactInstance` is clean;
  - `ExactInstanceForbidden` is an `SC3001` violation (`onCleanup` in
    `Leaky.start`);
  - the supplied, widened and `let` instances stay `SC9012`.
- **Coverage:** 178 fixture projects, 938 findings. Only the new snapshot is
  new.
- **rc.13 corpus**, browser host, release binary, against
  `rc13-k-browser.json`:
  - violations unchanged at 285;
  - uncertifiable 3,382 to 3,380: `inferay`'s `AppLayout` and
    `solid-groove`'s `useShortcuts` leave.
- **Why so few.** The class-instance calls the sizing counted mostly reach,
  one call deeper, a method of an object held in a property:
  `Camera.perspective` calls `this.projectionMatrix.fromPerspective(…)`.
  Such an object is not provably exact, so the walk stops there. That code
  imports nothing from Solid at all. Proving that of a module's import graph
  would cover these calls whatever their receiver; that is not decided.
