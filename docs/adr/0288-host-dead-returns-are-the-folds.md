# ADR 0288: Host-dead return sites are the fold's; the tuple veto is narrowed to prototype writes

- Status: accepted (2026-10-13). No engine change.
- Owner: fixtures `reactive-ir/host-dead-return`, `host-dead-return-server`
  and `host-dead-return-wildcard`. The behavior they pin is owned by
  `fold_emission_host_constants` (`solid-facts-backend/src/main.rs`, applied to
  package files by ADR 0284 § 5) and `accessor_provenance::Resolver::tuple_slot`
  (ADR 0282).

## Context

ADR 0282 deferred "slice 2": return sites that are dead under a stated host.
Under `--dependency-source @solid-primitives/raf` (ADR 0284), the rc.13 corpus
loses two app-game violations that the raf contract had reported:
`packages/three-examples/renderer.tsx:32` and
`packages/math-examples/rapier-2d-physics-engine/index.tsx:80`
(`createRAF`'s `start()` in a component body). `createRAF` begins with
`if (isServer) return [() => false, noop, noop];`. The working hypothesis was
that this second return made ADR 0282's single-unconditional-return rule
refuse the factory.

## Findings

1. **The `isServer` return is not what refuses.** ADR 0284 § 5 already folds
   a selected package's files under the declared host before facts are built:
   the guard becomes `if (!1) ;`, so the factory has one return.
   `host-dead-return` proves it. `createRAF`'s shape and its default-export
   alias are `SC1001` under a browser host. The published raf bytes, in a
   scratch copy with and without a pnpm symlink layout, are `SC1001` too.
2. **The wall is ADR 0282's wildcard veto.** `tuple_slot` refuses every array
   pattern in the program while `MemberStability::all` is set. In app-game it
   is set by `setValues((draft) => Object.assign(draft, next))` in
   `apps/abr-viewer/src/features/brush-detail/BrushDetailEditable.tsx`.
   That file belongs to another app of the workspace, but `apps/web`'s
   program reaches it. An instrumented run saw
   the veto at every one of the program's 3,354 `tuple_slot` entries.
   `host-dead-return-wildcard` pins this: one `Object.assign` with an open
   source in the project silences the proven `Loop`.

## Decision

1. **No IR rule for host-dead returns.** For package code, the fold is the
   one place where a host constant decides a branch, and it decides it before
   any fact exists. A second decision in the IR would duplicate it and could
   disagree with it.
2. **Project files stay unfolded**, as ADR 0284 § 5 states. So slice 2 is
   still open for project code: a project factory's `if (isServer) return
   [...]` keeps two return sites (`ProjectLoop`). The same holds for ADR
   0276/0280's every-completion walk over a project helper's `if (isServer)
   return;`, the vendored timer's case.
   - Project code under an SSR rendering runs on both hosts, so a fold by
     target alone would be wrong there.
   - A project-file constant would have to be authenticated by the
     inferred-host machinery. That is a separate slice.
3. **The wildcard veto is narrowed for tuples** (lead decision, 2026-10-13,
   after this ADR's measurement).
   - **No other code holds the array.** It is the factory's fresh literal,
     created at the `return` and destructured by the call's own binding, so a
     write to an ordinary object can never reach it. Destructuring can change
     only through `Array.prototype` or `%ArrayIteratorPrototype%`.
   - **What still vetoes:** a reflective write (`Object.assign`,
     `Reflect.set`, `defineProperty`, …) whose target is spelled through a
     `prototype`, or is a spread, in any analyzed file, package code
     included. `host-dead-return-prototype` pins it.
   - **Outside the premise:** a direct
     `Array.prototype[Symbol.iterator] = f` has a non-literal computed key.
     A prototype reached through an alias is outside it too. Both are
     outside ADR 0273's premise, under the standard runtime configuration
     premise that intrinsics are not patched.
   - **Every other wildcard write** (ADR 0273's `all`) no longer refuses a
     tuple. app-game's `setValues((draft) => Object.assign(draft, next))` is
     one.

## Measurement of the premise (experiment only, not committed)

A release binary with `tuple_slot`'s `all` check removed, rc.13 corpus,
`--browser`:
- dependency mode (11 packages): 559 → 560 violations, 3,556 → 3,564
  uncertifiable.
  - The added violation is `renderer.tsx:32:3`, a true positive: `Renderer`
    is a component and calls `start()` in its body; `start` reads `running()`
    as its first statement, untracked. The contract run reported the same
    site.
  - `rapier-2d-physics-engine/index.tsx:80:5` became uncertifiable
    ("a callback written in TestCube … not proven to invoke it"), not a
    violation. `TestCube` is declared inside `lazy(async () => …)`; the
    contract path proved the site, and the transported path does not. This
    is not diagnosed.
  - The other new uncertifiable rows are `reactive-dispatch-unresolved` at
    `paint-core/src/composition/PaintApplication.tsx:34`–`39` and
    `tanki/index.tsx:241`: newly resolved tuple slots whose callees cannot be
    selected.
- base mode: 562 → 562 violations, 3,731 → 3,737 uncertifiable.

So the premise buys one true positive and no false positive in this corpus.
The second raf site needs the `lazy` difference as well.

## Consequences

- **Coverage:** four projects are added.
  - `host-dead-return` has 4 `SC1001` violations, and `-server` has none.
  - `-wildcard` has 1 `SC1001` once the veto is narrowed.
  - `-prototype` has none.

  No other snapshot moved.
- **Corpus with the narrowed veto** (rc.13, release binary, `--browser`):
  - base: 562 violations (unchanged), 3,731 → 3,737 uncertifiable;
  - dependency mode, 11 packages: 560 → 561 violations;
  - dependency mode, `*`: 560 → 561 violations.

  The added violation is raf's `renderer.tsx:32:3`, a true positive reported
  in contract mode too. The second raf site is uncertifiable, as measured
  above.
- ADR 0282's "createRAF itself stays silent … slice 2" is true only for base
  mode, where raf is bound by its contract and not analyzed.
- Still refused, silent:
  - a live return under a folded control test
    (`if (!isServer) { …; return [...] }`, `ClientLoop`);
  - a condition the host constant takes part in without deciding it
    (`isServer || flag`);
  - `isServer` read through a namespace import;
  - a ternary `return isServer ? a : b`, because the fold reads `if`
    statements only;
  - every project-file guard.
