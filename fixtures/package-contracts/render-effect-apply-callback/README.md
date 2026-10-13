# `createRenderEffect`'s apply has no execution word

`execution: "deferred"` promises a consumer the export has **not** invoked the
callback by the time it returns; `execution: "inline"` promises it has. The
generator used to publish `deferred` for a callback reaching
`createRenderEffect`'s apply, because the dialect's contract word for that slot
deferred to the attribution word, which is `Deferred`.

The bytes say otherwise. `effect()` in `@solidjs/signals@2.0.0-rc.3` ends
`recompute(node, true); !options?.defer && (node._type === EFFECT_USER ||
options?.schedule ? node._queue.enqueue(…) : runEffect(node))`
(`dist/dev.js:5285-5289`, `dist/prod/core/effect.js:16-17`; rc.9
`dist/dev.js:1612-1620`), and a render effect is not `EFFECT_USER`. A Node probe
on rc.3 and rc.9, dev and prod, records `compute,apply,returned` for the plain
two-argument call with no owner, in a root, in a memo compute, in a render-effect
compute and in a component body under `render`.

`inline` is not true either. The same call leaves the first apply for later when
`defer` (skipped) or `schedule` (queued) is set, when the compute returns a
promise or reads a source that is still pending (probed: the apply runs after
the source settles), and on rc.9 when the first pass was staged into a live
transaction (`dist/dev.js:1617`). Every later run comes from the flush. So the
slot has no word, and the export's `callbacks` domain stays open.

| Export | `callbacks` | Why |
| --- | --- | --- |
| `renderApplyWrapper` | no row, domain open | `handle` is called from the apply; was `invoke`, `queued`, `ambient-at-execution` |
| `renderApplyForwarded` | no row, domain open | the same slot with the parameter passed directly, which is the primitive-slot branch of the generator; was the same `queued` row |
| `effectApplyWrapper` | `invoke`, `queued`, `ambient-at-execution` | negative: `createEffect` queues every apply, so `deferred` holds |
| `renderComputeWrapper` | `invoke`, `same-stack`, `tracked` | negative: the compute slot is unchanged |

The same first run decides the `reads` domain. It runs during the call with
the caller's listener current (probed: a memo whose compute creates the render
effect re-runs when a signal read only in its apply changes), so a read there
is a read the export performs.

| Export | `reads` | Why |
| --- | --- | --- |
| `renderApplyRead` | `read-0` (`same-stack`), domain open like any direct read | was `reads: []` with the domain **closed**, a negative claim over a read the call performs |
| `effectApplyRead` | `[]`, closed | negative: the apply reads after the call returns |

Each export also carries the render effect's own owner requirement, and declines
`creates` for its effect constructor as `dialect-silent`, exactly as
`callback-untracked-wrapper` does for `createEffect`.

## Stub faithfulness

`node_modules/solid-js/index.d.ts` copies `createRenderEffect`, both
`createEffect` overloads and every type they name verbatim from
`@solidjs/signals@2.0.0-rc.3` `dist/types/signals.d.ts`, which `solid-js`
re-exports as `typeof coreRenderEffect` and `typeof coreEffect`. The only
omission there is `solid-js`' optional `ssrSource` augmentation of
`EffectOptions`, which no export passes. `createSignal` keeps only its
plain-value overload, with its types verbatim and the `$REFRESH` brand as a
local `unique symbol`; it only supplies the signal the read exports read.
`index.ts` type-checks clean against this stub and against the real
`solid-js@2.0.0-rc.3` and `2.0.0-rc.9` installs.
