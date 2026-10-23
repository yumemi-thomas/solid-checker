/**
 * `solid-js/internal/container-trace` — the client half of the container tier
 * at the slot border (DR-2 case 3): a server projection crosses a
 * serialization boundary as its TRACE and materializes back into a live local
 * projection. NOT public API (the `solid-js/internal` namespace): consumed by
 * `@solidjs/web/frames`' traces tier (`@solidjs/web/frames/trace`), which the
 * frames client loads lazily — the server announces the tier when it
 * serializes a trace, and an un-announced marker in a record's args starts
 * the load itself (frames savings pass §2) — so the store engine
 * (`createProjection` and everything `@solidjs/signals/store` drags in, ~8 KB
 * brotli) stays out of a server-component page that never meets one.
 *
 * Its own dist entry, not a member of `solid-js`: that build is one flat
 * module, so any binding in it that references the engine welds the engine to
 * the eager chunk the moment anything imports the binding — a lazy
 * `import("solid-js/internal")` would split off a facade and leave the engine
 * where it was. This module reaches `createProjection` through
 * `@solidjs/signals` (per-module files the app bundler can assign to this
 * chunk), bundles its own copy of the store hydration adapter
 * (store-hydration.ts — see there: reading the eager slot through the main
 * entry pinned the adapter into every server-component page's entry chunk),
 * and takes the patch protocol and the adapter's helpers from `solid-js` by
 * name, so it behaves exactly as the wrapper did at every call site while
 * retaining nothing of the engine, and nothing of the adapter, on the eager
 * side.
 */
import { type Store } from "@solidjs/signals";
/**
 * Materialize a container TRACE — snapshot then patch batches, the
 * continuation protocol a server projection serializes as when it crosses a
 * boundary (hydration resume in solid-js; the slot border via the serializer's
 * container plugin) — into a live local projection. The result reads like
 * the server value did: not-ready until the snapshot lands, then a
 * read-only store the batches keep updating, done when the trace ends.
 *
 * Created under a detached root (see `detachedRoot`): revival can run inside
 * a render effect's owner, and the store is memoized per trace (see the
 * plugin's WeakMap) — a store owned by its first reader would be disposed by
 * that reader's re-render while other readers still hold it, and one rooted
 * under it would take a hydration id from it. Consumption is pull-driven and
 * the trace is response-bounded, so the projection settles on its own; GC
 * collects the pair with the trace.
 *
 * Materialized for a CLAIM (`claiming` — the reader is about to hydrate
 * server markup rendered from this value: a frame's adopt-time mount, at
 * t=0 or deferred under its hold, frames-rulings 3.1 / 3.2), a replayed
 * backlog beyond the snapshot is PARKED until hydration ends
 * (`onHydrationEnd`; the next microtask when no pass is in progress — what
 * a claim made after hydration-done gets). The snapshot is the state the
 * server's markup shows; the claim renders the fill against that markup and
 * trusts it — a text hole is never rewritten during a claim — so a store
 * already past the markup left the DOM diverged from it for good (the trace
 * had nothing further to emit). Applied after the claim, the backlog re-runs
 * the fill's reads outside hydration and the DOM catches up: the same
 * parking solid's store-shaped async-iterable hydration applies to a
 * buffered backlog (frames-rulings 3.6 (iii), "the consumer parks"). The
 * release order is the one 3.2 pins: claim, the frame's hold release, done,
 * then the backlog. Materialized for a FRESH mount (no server markup to
 * agree with — a stream re-call, a codec-face decode), nothing is parked:
 * the first read is the fold of the whole backlog. Live emissions land
 * after the claim by construction either way. A failure applies in order,
 * after everything queued before it, so it, too, waits on a parked backlog.
 *
 * Consumed by the serialization layer (`@solidjs/web/frames`). Declared, not
 * stripped: this entry's whole surface is the seam, and the subpath's
 * namespace is what makes it non-public.
 */
export declare function materializeContainerTrace(marker: {
    $tr: AsyncIterable<any> | {
        __SEROVAL_STREAM__: true;
    };
    $ta?: number;
}, claiming?: boolean): Store<any>;
