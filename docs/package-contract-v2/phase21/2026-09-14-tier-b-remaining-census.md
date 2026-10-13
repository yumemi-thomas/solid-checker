# Tier B after ADR 0103: B-2 is dead, B-3 is not a dialect change (2026-09-14)

Measured against the re-pinned 1,736-row `reads` frontier, after ADR 0103
closed 148 rows. Nothing here is authored.

## The frontier, re-ranked

| group | rows | state |
| --- | ---: | --- |
| `motion-utils` easing consts (8 exports × 26) | **208** | deferred by the plan; factory returned-body census |
| B-3 `sharedConfig` (`createHydratableSignal`, `createHydrateSignal`) | **124** | actionable, see below |
| Tier C classes | 117 | construct-signature selection; veto question open |
| `@corvu/utils` `combineStyle` | 74 | census refused, confirmed twice |
| B-4 `createMicrotask` | 62 | `iteration-protocol (SpreadElement)` |
| B-6 `defer` | 62 | `property-access-unknown-accessor (ElementAccessExpression)` |
| B-2 `defaultEquals`, `tryOnCleanup` | 124 | **zero gainable — do not build** |

## B-2 is zero gainable, for two different reasons

The depth plan describes B-2 as one premise, "a dependency-export alias". Its
two exports do not share a shape, and neither is reachable.

~~~js
// @solid-primitives/utils@6.4.1
import { getOwner, onCleanup, /* … */ equalFn } from "solid-js";
export const defaultEquals = equalFn;                       // :16
export const tryOnCleanup = isDev
    ? fn => (getOwner() ? onCleanup(fn) : fn)
    : onCleanup;                                            // :121
~~~

- **`defaultEquals` (62 rows)** *is* the plan's shape: an alias of a dependency
  export, refusing with `implementationUnavailable` because the alias hop lands
  on `solid-js`'s declaration file. A premise that composed the dependency's
  claim would close it — except the dependency states nothing to compose.
  Measured on the real corpus, `solid-js@1.9.14` closes exactly two exports,
  `Match` and `onCleanup`, and **both only in `returns`**. It publishes no
  `reads` closure at all, and none for `equalFn` in any domain. There is
  nothing for an alias premise to inherit.
- **`tryOnCleanup` (62 rows)** is not an alias. It is a conditional over two
  branches, one of them a locally declared arrow that calls `getOwner()` and
  `onCleanup(fn)`. Deciding its `reads` means deciding both branches, and both
  end in the same missing `solid-js` claims.

This is § 43's bottom-up rule, measured: **nothing at this layer moves until
the dialect tier publishes positive `reads` closures.** Building B-2 first
would have produced a correct premise with no row behind it — the same outcome
the `./immutable` case warned about in the Tier A census, and the reason that
census exists.

## B-3 is a certifier review, not a dialect-seam change

`createHydratableSignal` refuses on one node:

~~~js
if (sharedConfig.context) {        // dist/index.js:166
~~~

> `reads-census premise required: the property-access-unknown-accessor form
> (PropertyAccessExpression) … states no reviewed subject root, so whose value
> it reads is undecided`

`sharedConfig` is an imported `solid-js` binding, so the producer refuses it as
a module binding. The plan files this as "a dialect data object … dialect seam
change — both dialect crates move together".

ADR 0103 suggests a cheaper and better-separated shape, and it is the one that
just closed 148 rows: **the producer states, the certifier reviews.** The
producer already has the vocabulary — `SubjectRootDerivation` carries
`default-library`, `own-class`, `parameter-result` and the rest, so this adds
one derivation naming the imported module and member. Deciding that
`solid-js`'s `sharedConfig` is a data object whose property read is not a
reactive source is then a reviewed table in the certifier, beside
`REVIEWED_DEFAULT_LIBRARY_ALIASES`, and no dialect crate moves.

The split matters for the same reason it did in ADR 0103: the producer cannot
tell `sharedConfig.context` from any other imported-object property read,
because the syntax is identical. Which modules and members are data is a
reviewed act.

**Unmeasured, and the thing to check first:** the census reports only the
*first* refusal, so whether `sharedConfig.context` is `createHydratableSignal`'s
only blocker is not established. The function also calls `createSignal`,
`onMount` and the caller-supplied `update()`. Two facts argue the calls are
fine — `createSharedRoot`, `createSingletonRoot`, `createBranch` and
`createDisposable` all reach `solid-js` primitives and all measured *decidable*
in pass 2 — but `update()` is caller-supplied code invoked at call time, and
nothing here proves the `reads` census accepts it under ADR 0034. Lift the
subject root, re-run the census, and read the next refusal before writing the
certifier arm.
