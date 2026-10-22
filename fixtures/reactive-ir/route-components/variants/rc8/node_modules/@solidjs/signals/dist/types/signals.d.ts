import type { Disposable, Refreshable } from "./core/index.js";
/**
 * Low-level reactive-cleanup primitive. Registers a callback that runs when
 * the surrounding owner is disposed.
 *
 * **In 2.0 user code this is rare.** The two cases where you might reach for
 * it have better-shaped tools:
 *
 * - **Component lifecycle (mount/unmount, listeners, intervals):** use
 *   {@link onSettled} and **return** a cleanup function. Setup and teardown
 *   stay paired in one block. This replaces the 1.x `onMount` + `onCleanup`
 *   pairing.
 * - **Cleanup tied to an effect run:** `onCleanup` does not belong in
 *   `createEffect`'s apply phase. If a compute phase genuinely needs per-run
 *   teardown, that's usually a sign the work should be a memo/projection
 *   instead, or moved to `onSettled` if it's lifecycle-shaped.
 *
 * Where `onCleanup` is the right tool is **library / custom-primitive
 * internals** — coordinating disposal inside a `createRoot` body, or wiring
 * cleanup to a captured owner via `runWithOwner` from a custom factory.
 * Application code rarely needs to write any of those shapes directly.
 *
 * Must be called inside an owner. Calling outside an owner is a no-op (with a
 * dev-mode warning).
 *
 * Cannot be used inside `createTrackedEffect` or `onSettled` — return a
 * cleanup function from the callback body instead.
 *
 * Cleanups run in unwind order: an owner's children are disposed before its
 * own cleanups, and within one owner later registrations run before earlier
 * ones. In production a component body shares its enclosing owner, so
 * register cleanup before creating children when the order between them
 * matters.
 *
 * @example
 * ```ts
 * // Library shape: thread a resource's disposal into a *captured* owner
 * // from a factory that has no settle-phase setup of its own. `onSettled`
 * // would queue a callback we don't need; `onCleanup` is the leaner
 * // primitive when the only job is "register disposal on this owner".
 * function bindToOwner<T extends { dispose(): void }>(owner: Owner, resource: T): T {
 *   runWithOwner(owner, () => onCleanup(() => resource.dispose()));
 *   return resource;
 * }
 * ```
 */
export declare function onCleanup(fn: Disposable): Disposable;
/**
 * A zero-arg getter for a reactive value. Calling it inside a tracking scope
 * (memo, effect compute, JSX expression) subscribes the scope to changes.
 *
 * Reading outside any tracking scope simply returns the current value without
 * creating a subscription.
 */
export type Accessor<T> = () => T;
export type SourceAccessor<T> = Refreshable<Accessor<T>>;
export declare function accessor<T>(node: any): SourceAccessor<T>;
/**
 * A signal setter. Accepts either a new value or an updater `(prev) => next`.
 *
 * If the type permits `undefined`, `setState()` (no args) clears to `undefined`.
 *
 * To store a function as the value itself (rather than as an updater), wrap it
 * with an updater: `setHandler(() => myHandler)`.
 */
export type Setter<in out T> = {
    <U extends T>(...args: undefined extends T ? [] : [value: Exclude<U, Function> | ((prev: T) => U)]): undefined extends T ? undefined : U;
    <U extends T>(value: (prev: T) => U): U;
    <U extends T>(value: Exclude<U, Function>): U;
    <U extends T>(value: Exclude<U, Function> | ((prev: T) => U)): U;
};
/** A `[get, set]` pair returned from `createSignal` / `createOptimistic`. */
export type Signal<T> = [get: SourceAccessor<T>, set: Setter<T>];
export type ComputeFunction<Prev, Next extends Prev = Prev> = (v: Prev) => PromiseLike<Next> | AsyncIterable<Next> | Next;
export type EffectFunction<Prev, Next extends Prev = Prev> = (v: Next, p?: Prev) => (() => void) | void;
export type EffectBundle<Prev, Next extends Prev = Prev> = {
    effect: EffectFunction<Prev, Next>;
    /**
     * Intercepts compute-phase errors (thrown by the compute function or arriving
     * from upstream sources). Effect-phase throws are NOT routed here — they are
     * your own imperative code and escalate to the nearest error boundary.
     *
     * This is the error arm of the effect phase: it runs on the same queue and
     * in the same imperative, writable scope as `effect` (signal writes are
     * legal), receives the error the user code threw, and observes settled
     * outcomes — an error that recovers before the effect phase runs the
     * `effect` arm instead, and a held transition defers it like `effect`.
     */
    error: (err: unknown, cleanup: () => void) => void;
};
/** Options shared by every effect primitive. */
interface BaseEffectOptions {
    /** Debug name (dev mode only) */
    name?: string;
}
/** Options for effect primitives that support deferring/scheduling their initial run (`createEffect`, `createRenderEffect`, `createReaction`). */
export interface EffectOptions extends BaseEffectOptions {
    /** When true, defers the initial effect execution until the next change */
    defer?: boolean;
    /**
     * When true, enqueues the initial effect callback through the effect queue instead of running
     * it synchronously at creation. Lets the initial run participate in transitions -- if any
     * source throws `NotReadyError` during the compute phase, the callback is held until the
     * transition settles.
     *
     * Primarily for render effects that need transition-aware initial mounts (e.g. the root
     * `insert()` in `render()`).
     */
    schedule?: boolean;
    /**
     * Advanced. When true, asserts the compute function returns synchronous
     * values only (never `PromiseLike` / `AsyncIterable`). Skips the
     * async-shape probe in `recompute` for a small fixed-cost win per run.
     * Intended for compiler emissions (`_$effect`) and library code that
     * provably returns sync values. Returning a Promise or async iterable
     * from a `sync: true` effect is undefined behavior — the value will be
     * stored as-is and never awaited.
     */
    sync?: boolean;
    /**
     * Advanced (integration tier). When true, the effect is invisible to the
     * hydration id scheme: it inherits its parent's id instead of consuming a
     * child slot, and during hydration its compute runs live instead of
     * adopting the serialized server value (its first run is not frozen to
     * the server's decision).
     *
     * For **client-only effects created while hydrating** — effects with no
     * server-rendered counterpart (a router wiring link state, scroll
     * restoration, etc.). An id-consuming node the server never created would
     * shift every later sibling's hydration id, making serialized lookups and
     * template claims after it miss. `transparent` is also the supported
     * alternative to branching on hydration state
     * (`if (hydrating) createEffect(...)`), which freezes whatever the first
     * run decided: create the effect unconditionally and let it observe live
     * state instead.
     *
     * SSR ignores this option (a server-side effect always allocates its id
     * slot), so only mark effects the server does not create. Outside
     * hydration it is a no-op.
     */
    transparent?: boolean;
}
/** Options for plain signals created with `createSignal(value)` or `createOptimistic(value)`. */
export interface SignalOptions<T> {
    /** Debug name (dev mode only) */
    name?: string;
    /**
     * Custom equality function, or `false` to always notify subscribers.
     * Defaults to reference equality (`isEqual`). Pass a comparator (e.g.
     * `(a, b) => a.id === b.id`) for value-based equality, or `false` to
     * notify on every write regardless of equality.
     */
    equals?: false | ((prev: T, next: T) => boolean);
    /** Suppress dev-mode warnings when writing inside an owned scope */
    ownedWrite?: boolean;
    /** Callback invoked when the signal loses all subscribers */
    unobserved?: () => void;
}
/**
 * Options for read-only memos created with `createMemo`.
 * Also used in combination with `SignalOptions` for writable memos
 * (`createSignal(fn)` / `createOptimistic(fn)`).
 */
export interface MemoOptions<T> {
    /** Stable identifier for the owner hierarchy */
    id?: string;
    /** Debug name (dev mode only) */
    name?: string;
    /**
     * Advanced (integration tier). When true, the memo is invisible to the
     * hydration id scheme: it inherits its parent's id instead of consuming a
     * child slot, and during hydration it computes live instead of adopting
     * the serialized server value. For client-only memos with no
     * server-rendered counterpart — see {@link EffectOptions.transparent} for
     * the full semantics. No-op outside hydration.
     */
    transparent?: boolean;
    /**
     * @internal Framework plumbing (the `solid-js/refresh` HMR memo): in the
     * observe tiers the memo has no name, is no owner-path segment, and the
     * attribution engine records nothing about it, while what it owns stays
     * observed. Not part of the public API.
     */
    _plumbing?: boolean;
    /**
     * Custom equality function, or `false` to always notify subscribers.
     * Defaults to reference equality (`isEqual`). Pass a comparator (e.g.
     * `(a, b) => a.id === b.id`) for value-based equality, or `false` to
     * notify on every recompute regardless of equality.
     */
    equals?: false | ((prev: T, next: T) => boolean);
    /** Callback invoked when the computed loses all subscribers */
    unobserved?: () => void;
    /**
     * When true, defers the initial computation until the value is first read,
     * **and** opts the memo into autodisposal — once it has no remaining
     * subscribers it is torn down and recomputed from scratch on the next read.
     * Use it for compute-on-demand values that should not retain state across
     * idle periods. Non-lazy owned memos live for their owner's lifetime and
     * never autodispose.
     */
    lazy?: boolean;
    /**
     * Advanced. When true, asserts the compute function returns synchronous
     * values only (never `PromiseLike` / `AsyncIterable`). Skips the
     * async-shape probe in `recompute` for a small fixed-cost win per run.
     * Intended for compiler emissions (`_$memo`) and library code that
     * provably returns sync values. Returning a Promise or async iterable
     * from a `sync: true` memo is undefined behavior — the value will be
     * stored as-is and never awaited.
     */
    sync?: boolean;
    /**
     * Commit #0: a committed value the memo is born with, shown until the
     * compute's first real answer lands. While that first answer is in flight
     * the memo reads as a settled value everywhere — nothing suspends to a
     * `<Loading>` boundary, no transition is held (first-flight work is
     * loading-class, like a boundary fallback), and `isPending(memo)` stays
     * **false**: commit #0 answers the question by declaration, so first-load
     * affordances are driven from the value itself (a `null` placeholder, a
     * `skeleton: true` field, etc.). Once the first answer lands, the loading
     * value leaves the lineage forever: refetches use normal pending semantics
     * (stale value shown, `isPending` true, boundaries/transitions coordinate)
     * — the canonical guard is `data.skeleton || isPending(data)`, whose two
     * terms cover the two disjoint states.
     *
     * Typed strictly as `T`: to use `null`/`undefined` as the placeholder,
     * declare it in the memo's type (e.g. `createMemo<User | null>(...)`), so
     * every consumer sees the nullable window honestly. If the placeholder is
     * shaped data standing in for real data, encode its provenance in the data
     * (e.g. a `skeleton: true` field) rather than letting it impersonate truth.
     *
     * The loading value is also the compute's first `prev`, so `prev`-based
     * memos fold from it.
     */
    loadingValue?: T;
}
export type NoInfer<T extends any> = [T][T extends any ? 0 : never];
/**
 * Creates a simple reactive state with a getter and setter.
 *
 * When called with a plain value, creates a signal with `SignalOptions` (name, equals, ownedWrite, unobserved).
 * When called with a function, creates a writable memo with `SignalOptions & MemoOptions` (adds id, lazy).
 *
 * ```typescript
 * // Plain signal
 * const [state, setState] = createSignal<T>(value, options?: SignalOptions<T>);
 * // Writable memo (function overload)
 * const [state, setState] = createSignal<T>(fn, initialValue?, options?: SignalOptions<T> & MemoOptions<T>);
 * ```
 * @param value initial value of the state; if empty, the state's type will automatically extended with undefined
 * @param options optional object with a name for debugging purposes and equals, a comparator function for the previous and next value to allow fine-grained control over the reactivity
 *
 * @returns `[state: Accessor<T>, setState: Setter<T>]`
 *
 * @example
 * ```ts
 * const [count, setCount] = createSignal(0);
 *
 * count();              // 0
 * setCount(1);          // explicit value
 * setCount(c => c + 1); // updater
 * ```
 *
 * @example
 * ```ts
 * // Writable memo: derives from `fn()` and can be written like a signal.
 * const [user, setUser] = createSignal(() => fetchUser(userId()));
 *
 * // Within the frame the write wins over a same-tick recompute; the next
 * // change to `userId` re-derives (the compute receives the written value
 * // as `prev`). While a transaction holds a re-derived value, a write from
 * // outside it does not replace that derivation — it becomes its `prev`.
 * setUser({ ...user(), name: "Alice" });
 * ```
 *
 * @description https://docs.solidjs.com/reference/basic-reactivity/create-signal
 */
export declare function createSignal<T>(): Signal<T | undefined>;
export declare function createSignal<T>(value: Exclude<T, Function>, options?: SignalOptions<T>): Signal<T>;
export declare function createSignal<T>(fn: ComputeFunction<T>, options?: SignalOptions<T> & MemoOptions<T>): Signal<T>;
/**
 * Creates a readonly derived reactive memoized signal.
 *
 * ```typescript
 * const value = createMemo<T>(compute, options?: MemoOptions<T>);
 * ```
 * @param compute a function that receives its previous value and returns a new value used to react on a computation
 * @param options `MemoOptions` -- id, name, equals, unobserved, lazy, transparent
 *
 * @example
 * ```ts
 * const [first, setFirst] = createSignal("Ada");
 * const [last, setLast] = createSignal("Lovelace");
 *
 * const fullName = createMemo(() => `${first()} ${last()}`);
 *
 * fullName(); // "Ada Lovelace"
 * ```
 *
 * @example
 * ```ts
 * // Async memo — reads surface as pending inside <Loading>
 * const user = createMemo(async () => {
 *   const res = await fetch(`/users/${id()}`);
 *   return res.json();
 * });
 * ```
 *
 * @description https://docs.solidjs.com/reference/basic-reactivity/create-memo
 */
export declare function createMemo<T>(compute: ComputeFunction<NoInfer<T>, T>, options: MemoOptions<T> & {
    loadingValue: T;
}): SourceAccessor<T>;
export declare function createMemo<T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, options?: MemoOptions<T>): SourceAccessor<T>;
/**
 * Creates a reactive effect with **separate compute and effect phases**.
 *
 * - `compute(prev)` runs reactively — *put all reactive reads here*. The
 *   returned value is passed to `effect` and is also the new "previous" value
 *   for the next run.
 * - `effect(next, prev?)` runs imperatively (untracked) after the queue
 *   flushes. *Put DOM writes / fetch / logging / subscriptions here.* It may
 *   return a cleanup function which runs before the next effect or on
 *   disposal.
 *
 * Reactive reads inside `effect` will *not* re-trigger this effect — that's
 * intentional. If you need a single-phase tracked effect, use
 * `createTrackedEffect` (with the tradeoffs noted there).
 *
 * Pass an `EffectBundle` (`{ effect, error }`) instead of a plain function to
 * intercept **compute-phase** errors — errors thrown by `compute` or arriving
 * from upstream reactive sources (including async rejections), which your own
 * code has no frame to `try/catch`. The `error` handler is the error arm of
 * the effect phase: it runs on the same schedule and in the same imperative,
 * writable scope as `effect` (setting error state via signals is fine), and
 * only for *settled* errors — a transient error that recovers before the
 * effect phase runs `effect` with the recovered value instead, and a held
 * transition defers it exactly as it defers `effect`. Without an `error`
 * handler a compute-phase error is logged and the effect simply skips that
 * run — a non-render effect's reactivity failing does not crash the app.
 * Rethrowing from `error` escalates it to the nearest error boundary
 * (halting the system if none exists).
 *
 * The **effect phase is different**: it is your own imperative code, so handle
 * failures with `try/catch` where they occur. An uncaught effect-phase throw
 * is treated as an unhandled application error — caught by the nearest
 * `createErrorBoundary`/`<Errored>`, and permanently halting the reactive
 * system if there is none. It is *not* routed to the bundle's `error` handler.
 *
 * ```typescript
 * createEffect<T>(compute, effectFn | { effect, error }, options?: EffectOptions);
 * ```
 * @param compute a function that receives its previous value and returns a new value used to react on a computation
 * @param effectFn a function that receives the new value and is used to perform side effects (return a cleanup function), or an `EffectBundle` with `effect` and `error` handlers
 * @param options `EffectOptions` -- name, defer, schedule, transparent
 *
 * @example
 * ```ts
 * const [count, setCount] = createSignal(0);
 *
 * createEffect(
 *   () => count(),                  // compute: tracks `count`
 *   value => console.log(value)     // effect: side effect
 * );
 *
 * setCount(1); // logs 1 after the next flush
 * ```
 *
 * @example
 * ```ts
 * createEffect(
 *   () => userId(),
 *   id => {
 *     const ctrl = new AbortController();
 *     fetch(`/users/${id}`, { signal: ctrl.signal });
 *     return () => ctrl.abort(); // cleanup before next run / disposal
 *   }
 * );
 * ```
 *
 * @description https://docs.solidjs.com/reference/basic-reactivity/create-effect
 */
export declare function createEffect<T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, effectFn: EffectFunction<NoInfer<T>, T> | EffectBundle<NoInfer<T>, T>, options?: EffectOptions): void;
/**
 * Creates a reactive computation that runs during the render phase as DOM elements
 * are created and updated but not necessarily connected.
 *
 * Same compute / effect split as `createEffect`, but scheduled inside the render
 * queue rather than after it. Reach for this only when authoring renderer
 * plumbing (custom DOM bindings, JSX-generated `insert()` / `spread()` calls).
 * App code should use `createEffect`.
 *
 * ```typescript
 * createRenderEffect<T>(compute, effectFn, options?: EffectOptions);
 * ```
 * @param compute a function that receives its previous value and returns a new value used to react on a computation
 * @param effectFn a function that receives the new value and is used to perform side effects
 * @param options `EffectOptions` -- name, defer, schedule, transparent
 *
 * @example
 * ```ts
 * // Custom directive: bind an element's textContent to a reactive source.
 * function bindText(el: HTMLElement, source: () => string) {
 *   createRenderEffect(
 *     () => source(),
 *     value => { el.textContent = value; }
 *   );
 * }
 * ```
 *
 * @description https://docs.solidjs.com/reference/secondary-primitives/create-render-effect
 */
export declare function createRenderEffect<T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, effectFn: EffectFunction<NoInfer<T>, T>, options?: EffectOptions): void;
/**
 * Creates a tracked reactive effect where dependency tracking and side effects happen
 * in the same scope.
 *
 * @deprecated Do not use in new code. For a side effect that follows reactive
 * state, use `createEffect(compute, effect)` — it separates tracking from the
 * side effect, knows its dependencies before it runs, and participates in
 * async and transitions. For one-time DOM work after render (measuring,
 * attaching third-party widgets to a ref), use `onSettled`. Tracking from
 * inside the effect phase — the only thing this primitive adds — is retained
 * solely to ease 1.x migration: it runs beside user-effect callbacks after
 * values commit, never holds a transition, and cannot observe a write staged
 * earlier in the same flush by a signal it has not read yet.
 *
 * WARNING: Because tracking and effects happen in the same scope, this primitive
 * may run multiple times for a single change or show tearing (reading inconsistent
 * state). Use only when dynamic subscription patterns require same-scope tracking.
 *
 * The callback runs during the flush itself: writes made inside it are queued
 * into the same flush's continuation and are never visible to the callback's
 * own reads (reads return settled values, as in every effect-phase scope), and
 * `flush()` cannot be called from inside it (dev throws; production is a
 * no-op) — defer with `queueMicrotask(() => flush())` if needed.
 *
 * ```typescript
 * createTrackedEffect(compute, options?: { name?: string });
 * ```
 * @param compute a function that contains reactive reads to track and returns an optional cleanup function to run on disposal or before next execution
 * @param options -- name
 *
 * @example
 * ```ts
 * createTrackedEffect(() => {
 *   const target = focusedNode();
 *   if (!target) return;
 *
 *   const handler = () => log(target.value());
 *   target.on("change", handler);
 *
 *   return () => target.off("change", handler);
 * });
 * ```
 *
 * @description https://docs.solidjs.com/reference/secondary-primitives/create-tracked-effect
 */
export declare function createTrackedEffect(compute: () => void | (() => void), options?: BaseEffectOptions): void;
/**
 * Creates a reactive computation that runs after the render phase with flexible tracking.
 *
 * ```typescript
 * const track = createReaction(effectFn, options?: EffectOptions);
 * track(() => { // reactive reads });
 * ```
 * @param effectFn a function (or `EffectBundle`) that is called when tracked function is invalidated
 * @param options `EffectOptions` -- name, defer
 *
 * @example
 * ```ts
 * const [count, setCount] = createSignal(0);
 *
 * const track = createReaction(() => {
 *   console.log("count changed once, re-arm to listen again");
 *   track(() => count()); // re-arm
 * });
 *
 * track(() => count()); // initial arm
 *
 * setCount(1); // logs once, reaction re-armed for next change
 * ```
 *
 * @description https://docs.solidjs.com/reference/secondary-primitives/create-reaction
 */
export declare function createReaction(effectFn: EffectFunction<undefined> | EffectBundle<undefined>, options?: EffectOptions): (tracking: () => void) => void;
/**
 * Awaits a reactive expression and returns its first fully-settled value as a
 * `Promise`. Pending async reads (`createMemo` returning a promise, etc.) are
 * waited on; once the expression returns synchronously without `NotReadyError`
 * the promise resolves with that value. If the expression settles with an
 * error instead — including an async source that rejects — the promise
 * rejects with it.
 *
 * Must be called *outside* a tracking scope — it doesn't subscribe, it just
 * resolves the current value once.
 *
 * @example
 * ```ts
 * const user = createMemo(() => fetch(`/users/${id()}`).then(r => r.json()));
 *
 * // outside any reactive scope
 * const initial = await resolve(() => user());
 * ```
 *
 * @param fn a reactive expression to resolve
 */
export declare function resolve<T>(fn: () => T): Promise<T>;
/**
 * Invalidates one reactive source, forcing it to re-execute even if its inputs
 * haven't changed, and returns a promise for the target's NEXT QUIESCENT
 * STATE — the re-ask (and anything that supersedes it) has settled.
 *
 * Pass either a Solid-created accessor or a projected store created from
 * `createStore(fn, ...)` / `createProjection(...)`. `refresh()` is a
 * write-like invalidation operation: it does not read the target's value, and
 * refreshing a plain signal accessor is a no-op that resolves immediately.
 *
 * The returned promise is safe to ignore (fire-and-forget refresh is
 * unchanged, and a failed refetch will not surface an unhandled rejection).
 * Awaiting it gives imperative flows the settle point without a reactive
 * read:
 * - Accessor targets resolve with the settled value; store targets resolve
 *   with the store node passed (reads through it are fresh after the await).
 * - A failed re-ask rejects with the error (inside an action's generator,
 *   `yield refresh(x)` throws back at the yield point and the action reverts
 *   like any other failure).
 * - Semantics are quiescence, not flight identity: if another refresh (or
 *   any invalidation) supersedes this one mid-flight, the promise waits for
 *   — and delivers — whatever finally lands.
 * - Inside an action, truth landing into the held transaction is STAGED;
 *   the promise still settles then (matching `resolve()`/`until()`, #2930)
 *   and delivers the staged value — the caller's own optimistic override is
 *   never the delivered value.
 * - The re-ask itself stays verdict-quiet exactly as before: `isPending`
 *   does not flip for a bare refresh (pair with `affects()` for a visible
 *   pending window).
 *
 * @example
 * ```ts
 * const user = createMemo(async () => fetch(`/users/${id()}`).then(r => r.json()));
 *
 * // Fire-and-forget re-fetch
 * <button onClick={() => refresh(user)}>Reload</button>;
 *
 * // Imperative settle point
 * const fresh = await refresh(user);
 * ```
 */
export declare function refresh<T>(target: Refreshable<T>): Promise<T extends (...args: any) => infer V ? V : T>;
/** Falsy values a truthy predicate result is narrowed against. */
export type Truthy<T> = Exclude<T, false | 0 | 0n | "" | null | undefined>;
/**
 * The global `AbortSignal` when the consumer's libs declare one (DOM,
 * WebWorker, `@types/node`), else the surface `until` uses — so these
 * declarations check under neither lib without widening the global type.
 */
type GlobalAbortSignal = typeof globalThis extends {
    AbortSignal: {
        prototype: infer S;
    };
} ? S : {
    readonly aborted: boolean;
    readonly reason: any;
    addEventListener(type: "abort", listener: () => void, options?: {
        once?: boolean;
    }): void;
    removeEventListener(type: "abort", listener: () => void): void;
};
export interface UntilOptions {
    /** Reject with `TimeoutError` if the predicate has not turned truthy within
     * this many milliseconds. Strongly recommended when the confirming truth
     * arrives over a transport that can drop (sockets, subscriptions). */
    timeout?: number;
    /** Reject with `signal.reason` on abort. */
    signal?: GlobalAbortSignal;
}
/**
 * Awaits a reactive predicate and resolves the first time it settles *truthy*,
 * with that (narrowed) value. Falsy results and pending async reads both mean
 * "not yet": the subscription stays live and re-evaluates as sources change.
 * If the predicate settles with an error — a throw, or an async source that
 * rejects — the promise rejects with it, as do timeout and abort.
 *
 * Where {@link resolve} answers "what is this value" (first settled value,
 * whatever it is), `until` answers "when does the world confirm this
 * condition". The difference matters inside an `action()`: `yield until(...)`
 * holds the action's transaction — and any optimistic state riding it — open
 * until the condition is independently true.
 *
 * To make that sound, `until`'s predicate reads the AUTHORITATIVE view — and
 * this is the one read-semantics difference from `resolve`, which reads the
 * normal (transaction's own) view where overrides are visible:
 *
 * - **Optimistic overrides are invisible** to the predicate. Your own
 *   tentative write can never satisfy your own ack, even on the
 *   single-primitive shape where the optimistic store IS the live-fed store.
 *   (Derived computeds serve their normal cached values — express the
 *   condition over sources of truth, not derived views of the overlay.)
 * - **Everything else reads normally, including uncommitted transition-staged
 *   data.** Real data is real wherever it currently lives. This is
 *   load-bearing, not a loophole: truth that arrives *into* the open
 *   transaction (a `refresh()` this action issued, an entangled landing)
 *   stages and cannot commit until the hold releases — a predicate that
 *   refused staged reads would deadlock on the very data it is waiting for.
 *
 * This is the acknowledgment mechanism for mutations confirmed on a live data
 * channel (sockets, subscriptions, live queries) rather than by the mutation's
 * own response: correlate by a client-generated id or version in the predicate,
 * and let truth arrive however it arrives — push, refetch, or another tab.
 *
 * Failure composes with action semantics: a rejection is thrown back into the
 * generator at the `yield` point — catchable there, or the action fails and
 * its optimistic state reverts.
 *
 * Must be called *outside* a tracking scope.
 *
 * Inside an action, call it from a step: after an `await`, put a bare `yield`
 * before `yield until(...)`. The runtime cannot hook an async generator's
 * `await` continuation, so the `until(...)` expression — which CREATES the
 * predicate's reader — would otherwise run outside the transaction; created
 * there it is born held (A29) and replays only at the commit its own promise
 * holds open (#3482). See {@link action}.
 *
 * @example
 * ```ts
 * const send = action(async function* (text: string) {
 *   const clientId = crypto.randomUUID();
 *   setMessages(m => { m.push({ clientId, text, pending: true }); }); // optimistic
 *   await socket.send({ clientId, text }); // fire-and-forget transport
 *   yield; // re-enter the transaction after the await
 *   // Hold until the live source echoes the write (authoritative view —
 *   // the optimistic row above cannot satisfy this):
 *   yield until(() => messages.some(m => m.clientId === clientId), { timeout: 10_000 });
 * });
 * ```
 *
 * @param fn a reactive predicate over authoritative state
 * @param options optional `timeout` (ms) and abort `signal`
 */
export declare function until<T>(fn: () => T, options?: UntilOptions): Promise<Truthy<T>>;
/**
 * Creates an optimistic signal that can be used to optimistically update a value
 * and then revert it back to the previous value at end of transition.
 *
 * When called with a plain value, creates an optimistic signal with `SignalOptions` (name, equals, ownedWrite, unobserved).
 * When called with a function, creates a writable optimistic memo with `SignalOptions & MemoOptions` (adds id, lazy).
 *
 * ```typescript
 * // Plain optimistic signal
 * const [state, setState] = createOptimistic<T>(value, options?: SignalOptions<T>);
 * // Writable optimistic memo (function overload)
 * const [state, setState] = createOptimistic<T>(fn, options?: SignalOptions<T> & MemoOptions<T>);
 * ```
 * @param value initial value of the signal; if empty, the signal's type will automatically extended with undefined
 * @param options optional object with a name for debugging purposes and equals, a comparator function for the previous and next value to allow fine-grained control over the reactivity
 *
 * @returns `[state: Accessor<T>, setState: Setter<T>]`
 *
 * @example
 * ```ts
 * const [todos, setTodos] = createOptimistic(initialTodos);
 *
 * const addTodo = action(function* (text: string) {
 *   const tempId = crypto.randomUUID();
 *   setTodos(t => [...t, { id: tempId, text, pending: true }]); // optimistic
 *   const saved = yield api.createTodo(text);
 *   setTodos(t => t.map(x => (x.id === tempId ? saved : x)));   // reconcile
 * });
 * ```
 *
 * @description https://docs.solidjs.com/reference/basic-reactivity/create-optimistic-signal
 */
export declare function createOptimistic<T>(): Signal<T | undefined>;
export declare function createOptimistic<T>(value: Exclude<T, Function>, options?: SignalOptions<T>): Signal<T>;
export declare function createOptimistic<T>(fn: ComputeFunction<T>, options?: SignalOptions<T> & MemoOptions<T>): Signal<T>;
/**
 * Schedules `callback` to run **once** after the reactive graph has fully
 * settled — i.e. once every pending async read inside the current owner has
 * resolved and the queue has flushed. Each call registers a single fire; it
 * does not create an ongoing subscription.
 *
 * The canonical lifecycle primitive in 2.0. Three main usages:
 *
 * - **Component-level setup-and-teardown** *(the most common shape)*: run
 *   setup after the component's first stable render and **return a cleanup
 *   function** to dispose it on owner disposal. This is the replacement for
 *   the 1.x `onMount` + `onCleanup` pairing — setup and teardown live in one
 *   block, and `onCleanup` is no longer the right tool for component
 *   bodies. (`onMount` no longer exists in 2.0.)
 * - **Post-settle "ready" hook:** run once after a component's first stable
 *   render — analytics ping, focus, scroll-into-view, etc. No cleanup needed.
 * - **Inside an event handler:** schedule work to run after the action /
 *   transition triggered by the event has completed.
 *
 * Reactive reads inside the callback are *not* tracked — to react to
 * subsequent settles, register a new `onSettled` each time.
 *
 * The callback runs during the settle flush itself, which gives it the same
 * write semantics as every other effect-phase scope (the effect half of
 * `createEffect`, event handlers):
 *
 * - **Writes** are queued into the same flush's continuation — dependent memos
 *   and effects update before the flush returns — but reads inside the
 *   callback keep returning the settled (pre-write) values. A callback never
 *   observes its own unsettled write. Functional setters still compose:
 *   `set(v => v + 1)` twice increments twice.
 * - **`flush()` cannot be called** from inside the callback — the flush is
 *   already running (dev throws; production is a no-op). To force a drain
 *   after this settle, defer it: `queueMicrotask(() => flush())`.
 *
 * `onCleanup` is **not** allowed inside the callback — return a cleanup
 * function instead. The returned cleanup runs on owner disposal.
 *
 * A cleanup return is only honored when `onSettled` is called from an **owned**
 * scope (e.g. a component body). When it fires out of band from an *unowned*
 * scope — an event handler, a tracked effect, or another `onSettled` — there is
 * no owner lifecycle to bind a cleanup to; returning one is a dev-mode error
 * (and is dropped in production). Use the post-settle/event-handler forms below
 * for one-shot work, and keep setup-with-teardown in an owned scope.
 *
 * @example
 * ```tsx
 * // Component-level setup + teardown — replaces onMount + onCleanup.
 * // Subscribe to an external source on mount, unsubscribe on dispose.
 * function useViewportWidth() {
 *   const [width, setWidth] = createSignal(window.innerWidth);
 *   onSettled(() => {
 *     const onResize = () => setWidth(window.innerWidth);
 *     window.addEventListener("resize", onResize);
 *     return () => window.removeEventListener("resize", onResize);
 *   });
 *   return width;
 * }
 * ```
 *
 * @example
 * ```tsx
 * // Post-settle "ready" hook — no cleanup needed.
 * function Dashboard() {
 *   const data = createMemo(async () => fetchData());
 *
 *   onSettled(() => {
 *     analytics.track("dashboard.ready");
 *   });
 *
 *   return <Loading fallback={<Spinner />}><pre>{data()}</pre></Loading>;
 * }
 * ```
 *
 * @example
 * ```tsx
 * // Event-handler — runs after the action settles.
 * function SaveButton() {
 *   const save = action(function* () {
 *     yield api.save();
 *   });
 *
 *   const handleClick = () => {
 *     save();
 *     onSettled(() => toast("Saved!"));
 *   };
 *
 *   return <button onClick={handleClick}>Save</button>;
 * }
 * ```
 *
 * @param callback Function to run; may return a cleanup function that fires
 *   on owner disposal
 */
export declare function onSettled(callback: () => void | (() => void)): void;
export {};
