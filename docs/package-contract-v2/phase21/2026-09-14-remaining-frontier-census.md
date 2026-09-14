# The 1,532-row frontier, censused (2026-09-14)

Measured after ADR 0103/0104/0105 closed 352 rows. Nothing here is authored.

| group | rows | state |
| --- | ---: | --- |
| `motion-utils` (8 easing consts + `SubscriptionManager`) | **234** | **blocked upstream of any reads premise** |
| `@corvu/utils` `combineStyle` | 74 | census refused, confirmed twice |
| B-4 `createMicrotask` | 62 | rest-parameter spread |
| B-6 `defer` | 62 | captured parameter root |
| `@tanstack/devtools-event-client` `EventClient` | 11 | `implementationUnavailable` after ADR 0105 |

## `motion-utils` is blocked, and not by the premise the plan named

All 234 rows are a **single** artifact case, `9a3a41a5`, reached from three
roots. The depth plan defers them as needing the factory's returned-body
census, and the shape does match that reading:

~~~js
// dist/es/easing/ease.mjs
import { cubicBezier } from './cubic-bezier.mjs';
const easeIn = /*@__PURE__*/ cubicBezier(0.42, 0, 1, 1);
export { easeIn, easeInOut, easeOut };
~~~

But that premise is not what stands in the way. A two-pass census through
`motion-solidjs@0.6.0` (solid 1.9.14, graph lane) emits 239 scaffolds on pass 1
and then **refuses pass 2 outright**:

> Type Facts certification failed for graph node `motion-utils@12.39.0` (.)
> during live graph export-value verification: … does not match its exact
> export subject: **runtime implementation does not match the snapshot-replayed
> export binding**

Two things this settles:

- **The blocker is not version-specific.** It was recorded against
  `motion-solidjs@0.7.0-beta.4`; `0.6.0` reproduces it exactly. Every root that
  reaches this case is affected, so the 234 rows cannot be censused at all
  today — the "unmeasured behind the blocker" note carried since the Tier C
  census is a property of `motion-utils@12.39.0`, not of one consumer.
- **It fires on the re-export barrel.** `dist/es/index.mjs` is
  `export { easeIn, easeInOut, easeOut } from './easing/ease.mjs'` — the export
  binding and the runtime implementation live in different modules. Pass 1
  certifies because an empty corpus weakens every candidate out of the plan;
  pass 2 keeps them, demands Type Facts for them, and the binding mismatch
  surfaces.

So `motion-utils` needs **two** things, in order: the export-binding
resolution, then the returned-body census. It is the largest block on the
frontier and the furthest from closing, and it should not be re-costed as "208
rows of recipes".

## B-4 and B-6 are the two actionable premises

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
- **B-6** refuses on `property-access-unknown-accessor (ElementAccessExpression)`.
  `deps` is parameter 0 — the caller's value, which ADR 0034 already owns — and
  what stops the existing premise reaching it is that the access sits inside a
  callable *nested* in the implementation, so its execution point is not the
  call event.

**B-6's design question, stated rather than assumed.** Two readings close it,
and they are not the same claim:

1. *A captured parameter is still the caller's.* Extends ADR 0034's subject
   root through the nesting. Narrow, and exactly what the plan names.
2. *A form inside a nested callable is not at the call event at all.* The
   producer already states `captured` and `enclosingCallable` on every form, so
   this needs no new fact — but it is a far wider claim, and it is only sound
   where the export does not invoke that callable during the call.

Reading 2 would reach every captured form in the corpus, not just this one, and
the Tier A census already found the same shape in `filterInstance` and
`filterOutInstance`. That reach is the argument for it and the reason it needs
its own review rather than being folded into a 62-row premise.
