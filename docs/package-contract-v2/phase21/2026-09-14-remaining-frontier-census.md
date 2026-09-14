# The 1,532-row frontier, censused (2026-09-14)

Measured after ADR 0103/0104/0105 closed 352 rows. Nothing here is authored.

| group | rows | state |
| --- | ---: | --- |
| `motion-utils` (8 easing consts + `SubscriptionManager`) | **234** | the returned-body census; the blocker *is* the premise |
| `@corvu/utils` `combineStyle` | 74 | census refused, confirmed twice |
| B-4 `createMicrotask` | 62 | rest-parameter spread |
| B-6 `defer` | 62 | **not the plan's shape** — B-5 plus a join |
| `@tanstack/devtools-event-client` `EventClient` | 11 | `implementationUnavailable` after ADR 0105 |

## `motion-utils` is blocked by the premise the plan named, not by something in front of it

*Corrected 2026-09-14, later the same day. The first version of this section
blamed the re-export barrel and concluded that `motion-utils` needed "the
export-binding resolution first, then the returned-body census". That was a
reading of the source, not a measurement, and it is wrong: there is no separate
resolver bug, and the two are one piece of work.*

All 234 rows are a **single** artifact case, `9a3a41a5`, reached from three
roots. A two-pass census through `motion-solidjs@0.6.0` emits 239 scaffolds on
pass 1 and refuses pass 2 outright:

> Type Facts certification failed for graph node `motion-utils@12.39.0` (.)
> during live graph export-value verification: … does not match its exact
> export subject: **runtime implementation does not match the snapshot-replayed
> export binding**

The blocker is not version-specific: it was recorded against
`motion-solidjs@0.7.0-beta.4`, and `0.6.0` reproduces it exactly, so it belongs
to `motion-utils@12.39.0` rather than to one consumer.

### What actually refuses

A focused producer test over the package's own shape — a factory in one module,
a `const` bound to its result in a second, a re-export barrel as the entry —
answers it in one line:

~~~
declaration name="easeIn" path=cubic-bezier.js
~~~

`easeIn` is bound in `ease.mjs`. The producer follows the value to the arrow
`cubicBezier(…)` **returned**, which is written in `cubic-bezier.mjs`. The
binding check in `census_implementation_subject` requires the stated
declaration's path to end with the snapshot's runtime binding path, and those
two files are different, so it refuses.

**The barrel is not the cause.** `ease.mjs` and `cubic-bezier.mjs` are
different files with or without it, so the mismatch fires either way, and the
resolver's re-export handling is not implicated. `motion-utils` is idiomatic
ESM throughout — this is a checker limitation, not a package defect, and
nothing here is an upstream issue to file.

### What that means for the cost

The blocker *is* the premise. A `const` bound to a factory's call result has no
declaration at its own binding site, and supplying one is exactly what the
returned-body census (ADR 0035/0096 territory) would do. There is no resolver
fix to land first and no two-stage estimate: one premise, 234 rows, and the
binding check is where its absence surfaces rather than an extra obstacle in
front of it.

## B-4 is actionable; B-6 is not a premise

~~~js
export function createMicrotask(fn) {            // B-4, 62 rows
    let calls = 0, args;
    return (...a) => {
        (args = a), calls++;
        queueMicrotask(() => --calls === 0 && fn(...args));   // :4711..4718
    };
}

export function defer(deps, fn, initialValue) {  // B-6, 62 rows
    const isArray = Array.isArray(deps);
    return prevValue => {
        if (isArray) {
            for (let i = 0; i < deps.length; i++) input[i] = deps[i]();  // :3427..3435
        } else input = deps();
    };
}
~~~

- **B-4** refuses on `iteration-protocol (SpreadElement)`. `fn(...args)`
  spreads a binding whose every assigned value is the rest array the engine
  built for `...a`, so its iterator is `Array.prototype[Symbol.iterator]` and
  the spread reaches no user code. The premise is a subject root for a
  rest-parameter alias.
- **B-6 does not exist as the plan describes it.** *(Measured 2026-09-14.)*
  The plan calls `defer` a captured-parameter-root case. Probed directly, the
  producer states **two** forms for it, and `deps[i]` — the supposed problem —
  already roots at parameter 0:

  ~~~
  [0] ElementAccessExpression captured=true root=""          refusal="local-binding-written"   <- input[i] = …
  [1] ElementAccessExpression captured=true root="parameter" param=0                           <- deps[i]
  ~~~

  The blocker is `input[i]`, where `input` is a local binding assigned either
  `Array(deps.length)` or `deps()` — an engine allocation joined with a
  caller-rooted call result. That is **B-5** territory (a default-library call
  result root) plus a two-arm join over a local binding, which is ADR 0093's
  shape one level out. A premise built to B-6's description would close
  nothing.

The design question this section originally posed — whether a captured
parameter is still the caller's, or whether a form inside a nested callable is
not at the call event at all — turned out to be moot for `defer`, because the
captured parameter already roots. It remains a real question for the
`filterInstance`/`filterOutInstance` shape the Tier A census found, and should
be asked there, against measured rows, rather than here.

## Corrected again, 2026-09-14 (later): the transcript is complete, and the fact is on the wire

The section above concluded that "the blocker *is* the premise … there is no
resolver fix to land first and no two-stage estimate: one premise, 234 rows",
and costed it as the returned-body census, ADR 0035/0096 territory. A fixture
against the real shape — JS, not TypeScript, because `motion-utils` ships `.mjs`
and the two answer differently — says otherwise.

~~~
plain   (an arrow bound directly)     complete=true  implDecl=ease.js
easeIn  (bound to a factory's result) complete=true  implDecl=cubic-bezier.js
~~~

**`easeIn`'s implementation transcript is complete.** No open reasons, a
selected signature, a resolved declaration, a control-flow census. There is no
missing fact and nothing for a returned-body census to supply. `query_name` is
`"easeIn"` and matches the runtime export. The *only* thing that refuses is the
path comparison in `census_implementation_subject`:

~~~rust
let actual_path = declaration.location.path;         // …/cubic-bezier.js
let expected_suffix = format!("/{}", runtime_path);  // …/ease.js
if implementation.query_name.as_ref() != runtime_export
    || !actual_path.ends_with(&expected_suffix)
~~~

And the transcript already carries what distinguishes this from a producer
resolving to some unrelated declaration:

| field | value |
| --- | --- |
| `declaration` | `VariableDeclaration easeIn` in **ease.js** — the binding |
| `callablePaths` | `ArrowFunction` in cubic-bezier.js, via `FunctionDeclaration cubicBezier` |
| `implementation.declaration` | that same `ArrowFunction` |

`callable_paths` is not a new fact: the checker consumes it already, in
`require_export_callable_paths_closed` and the root-resolution machinery.

So the shape is what the earlier section said, and the **cost is not**. This
looks like the ADR 0107 pattern a second time — a binding check whose scope is
narrower than the facts available, rather than a census that needs building. The
architecture already separates the two bindings: `transcript.declaration` is
checked against the snapshot's *declaration* (typings) binding, and
`implementation.declaration` against its *runtime* binding. `easeIn` passes the
first and fails the second, because a `const` bound to a call result runs a body
declared in another file of the same package.

**Not yet built, deliberately.** Relaxing a binding check is the one kind of
change that can weaken certification silently, and the soundness condition —
what exactly ties the implementation to the binding, and what keeps a
dependency's declaration out — needs its own argument rather than being
inferred from one fixture. What is established here is that the 234 rows are
not blocked on a missing fact, and that costing them as a returned-body census
was wrong.
