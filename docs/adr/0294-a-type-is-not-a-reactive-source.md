# ADR 0294: A type is not a reactive source

- Status: accepted and implemented (2026-10-14).
- Owners:
  - `source_discovery::typed_implementation` and `TypedSource` (what a typed
    value holds);
  - `LiteralMemberSlots::closed_values` (the property a Closed member read
    observes);
  - the `TYPE_ONLY_SOURCE` obligations in `local_access.rs` (calls, the
    effect-apply descriptor shortcut, store Gets) and `static_rules.rs`
    (reads after an await), worded in `projection.rs`.
- Amends ADR 0286: its kept literal roots are now proven by what the literal
  stores, and its `External` case (an interface member read through an
  Unknown receiver) is an obligation.
- Removes the type-only interprocedural summary stage
  (`discover_typed_accessors`) and its per-file cache.

## Context

`@solidjs/signals@2.0.0-rc.13` `dist/types/signals.d.ts:55` declares
`export type Accessor<T> = () => T;`. A constant function satisfies it and
reads nothing:

```tsx
const obj: { orientation: Accessor<string> } = { orientation: () => "h" };
const value = obj.orientation(); // component body
```

This is `tsc --noEmit` clean and warns nothing at runtime. Three paths made
it a proven `SC1001` violation from the type alone:

1. Source discovery's type-only fallback put every entity typed with a
   dialect alias (`Accessor`, `Signal`, `Store`) into `accessors` and
   `source_kinds`, with the alias as its declaration. Every consumer then
   treated it as a proven source.
2. The effect-apply shortcut in `local_access` reported a no-argument call
   in effect apply whose callee Type Facts typed `Accessor`, with no symbol
   entry at all.
3. `interproc::discover_typed_accessors` added a summary read for every
   call in a helper whose callee was typed `Accessor`, so a caller of
   `function parameterRead(get: Accessor<string>) { return get(); }` was
   reported even when it passed `() => "h"`.

The research (`rust/target/research/store-fn-and-typed-accessor.md`) found
nine false violations with a faithful ambient `declare module "solid-js"`
stub. Against the published packages Type Facts gives the alias no origin
module (`declarationModule` searches only ambient module ancestors), so the
fallback is not reached there today. Fixing that later must not switch the
unsound premise on.

## Decision

1. **The type never supplies the reactive-read premise.** The fallback no
   longer writes `accessors` or `source_kinds` from a type. For each typed
   entity (Accessor, Signal or Store role, past the existing refusals) it
   judges what the value holds:
   - a proven source of the same kind, reached through an unwritten `const`
     binding (and `const` aliases of it), a tuple slot of an unwritten
     `const` array pattern bound from one exact synchronous project
     function every return of which writes one source there, or a member
     whose every read is Closed (ADR 0286) over project literals that all
     store it: the typed value is a **proven root**, with that source's
     name and declaration;
   - a function literal (an object or array literal for `Store`) held
     directly or returned by every return of one exact factory: the
     implementation is **visible**, and its own body decides wherever the
     analysis resolves the call;
   - anything else (a parameter, a destructured parameter, an import, a
     `declare`d value, a class field, an Unknown receiver): a **typed
     candidate** with unknown implementation.
   Visible and unknown candidates go into `typed_sources`, never into
   `accessors`, so no summary or propagated root can launder them.
2. **A typed candidate with unknown implementation**, read in a role that
   reports an untracked read (a call located like an accessor read, a store
   Get, a call after an await in a tracked async computation), is an
   `SC9012` obligation with the `type-only-source` context. It is never an
   `SC1001` or `SC1002`.
3. **The effect-apply shortcut** is the same premise: it is an `SC9012`
   obligation, and silent when the callee's typed entry has a visible
   implementation.
4. **The type-only summary stage is removed.** A parameter call is decided
   per call site by the existing invoked-parameter path, which follows the
   argument.

## Consequences

- A visible function literal that reads a signal is reported only where the
  analysis follows the call into that function. If it does not (a member
  holding an arrow, for example), the read is silent. That is a false
  negative, never a claimed one.
- An `Accessor<T>` parameter of an exported or unenumerable function is an
  obligation in its effect apply or after-await body, where it was a
  violation.
- A typed callee inside a helper (`function h() { opaque(); }`) no longer
  gives its callers a summary row, so it is silent at those callers, not an
  obligation.
- A class field, a destructured parameter and an imported receiver are not
  judged, so they are obligations even when the project wrote a plain
  function there.
- Setter and store-setter type roles keep the old fallback. They feed write
  rules, not read premises.
- Removed public timing fields: `typed_accessor_reused_files`,
  `typed_accessor_recomputed_files`, `interprocedural_typed_accessors`
  (and their session-bench keys).

## Evidence

- Fixture `reactive-ir/typed-accessor-roots` (ambient stub byte-faithful to
  `signals.d.ts:55`): 8 plain cases silent, 3 violations by value flow
  (`AliasedSignal`, and the existing `ReturnedSignal` and
  `ParameterSignal`), 5 `SC9012` obligations (`DeclaredValue`,
  `ClassMember`, `DestructuredProp`, `DeclarationMember`,
  `EffectApplyWritten`). Before, every case but `PlainUntyped` was a
  violation. `tsc --noEmit` is clean against the stub and against installed
  rc.13. Against installed rc.13 the checker reports the same as before (the
  fallback is not reached).
- Coverage moves, all from the type-only premise:
  - `engine/eslint-reactivity-v2`: `effect-apply-member`,
    `effect-apply-parameter` (`SC1001`) and `after-await-member`,
    `after-await-parameter` (`SC1002`) become `SC9012`. Each is an exported
    `install` taking an `Accessor<number>` parameter.
  - `reactive-ir/eslint-plugin-corpus`: the same three `install` shapes.
  - `reactive-ir/literal-member-handoff`: `External` becomes `SC9012`; the
    four kept roots stay violations, now by value flow.
  - `reactive-ir/async-call-prefix`: `TypeOnlyInputs`' uncertifiable row on
    `const read: Accessor<number> = () => 1` is gone.
  - `reactive-ir/provider-array-provenance`: `TypeOnly` and
    `TypeOnlyHelper`'s uncertifiable summary rows are gone.
  - `tuple-slot-provenance`'s `AccessorSlot` stays a violation through the
    tuple-slot judge.
- rc.13 corpus, release binary, browser host, against the ADR 0293 commit: base
  mode 562 violations / 3,735 uncertifiable and dependency mode 560 / 3,286,
  every row identical. The research's census found no corpus finding resting
  on the fallback, and this confirms it.
