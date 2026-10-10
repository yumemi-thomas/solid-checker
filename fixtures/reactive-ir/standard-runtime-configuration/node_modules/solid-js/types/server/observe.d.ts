import type { ServerObserve } from "@solidjs/signals";
import type { RecoveryEvent, RecoveryLive } from "../recovery.js";
/** The process-wide slots, created on first call from any copy of `solid-js`. */
export declare function serverSlots(): ServerObserve;
/**
 * One `<Loading>` boundary that WAITED during a server render — discovered
 * with pending async, then settled — delivered on
 * `OBSERVE.records.subscribe("boundary", …)` once it settled and, when a
 * `<Reveal>` group held its swap, once it was revealed. A boundary whose
 * content rendered on its first pass emits nothing: there was no wait to
 * attribute, the same rule as the client's `hold` records.
 */
export interface BoundaryEvent {
    /**
     * The boundary's hydration id — the id `SSR_RENDER_ERROR_CONTAINED` names
     * in `data.boundary`, and the `<template id="pl-…">` placeholder's.
     */
    id: string;
    /** `performance.now()` at discovery: the first render pass began. */
    at: number;
    /**
     * Discovery → settle, in milliseconds: from the first render pass to the
     * content being complete (`"settled"`), or to the decision that the
     * server will not produce it (the other outcomes).
     */
    durationMs: number;
    /**
     * Settle → reveal: how long a `<Reveal>` group held the finished content
     * back for its siblings (`order="together"`, a sequential tail). `0` when
     * the swap was issued as the boundary settled, including every boundary
     * outside a group.
     */
    heldMs: number;
    /**
     * Render passes over the boundary's content: the discovery pass plus one
     * per wait. `2` is one round of async; a higher count is a sequential
     * chain — a read that depended on the answer to the previous one.
     */
    passes: number;
    /**
     * `"settled"` — the content rendered on the server and swapped in.
     * `"fallback"` — the renderer had no stream to settle into
     * (`renderToString`, or a collapsed slot under it): the fallback shipped
     * final and the client renders the content. `"client"` — the content is
     * client-only (`ssrSource: "client"`): the client renders it after
     * hydration. `"error"` — the content threw; `live.error` is the value as
     * thrown, and the paired `SSR_RENDER_ERROR_CONTAINED` finding says where
     * it went.
     */
    outcome: "settled" | "fallback" | "client" | "error";
    /**
     * `true` when the outcome reached the client after the shell had flushed
     * — the user saw the fallback, then the swap. `false` when the boundary
     * settled in time to inline into the shell (or never streamed at all).
     */
    streamed: boolean;
    /** The `<Reveal>` group coordinating this boundary's swap, if any. */
    revealGroup?: string;
    /** Root-first component labels enclosing the boundary, as on diagnostics. */
    ownerPath?: string[];
}
/** The live half of a boundary record. */
export interface BoundaryLive {
    /** The thrown value, when `outcome` is `"error"`. */
    error?: unknown;
}
export type BoundaryListener = (event: BoundaryEvent, live: BoundaryLive) => void;
/**
 * The trace-provider slot — `OBSERVE.server.trace`. The CONTAINER is this
 * runtime's (a single replaceable provider, see `serverSlots`); what a
 * provider is — its argument, its answer — is the web runtime's, which
 * augments this interface with `provide` (`trace.ts` in `@solidjs/web`).
 * Declared empty here so that runtime has one place to type it.
 */
export interface ServerTrace {
}
declare module "@solidjs/signals" {
    interface RecordTypes {
        /** `<Loading>` boundaries that waited during a server render — see `BoundaryEvent`. */
        boundary: {
            event: BoundaryEvent;
            live: BoundaryLive;
        };
        /** The client rendering a boundary the server handed over — see `RecoveryEvent`. */
        recovery: {
            event: RecoveryEvent;
            live: RecoveryLive;
        };
    }
    interface ServerObserve {
        /** The trace-context provider slot — see `ServerTrace`. */
        trace: ServerTrace;
    }
}
