import type { Computed, Signal } from "./types.js";
/**
 * "Why did this run" attribution — the engine behind
 * `@solidjs/signals/attribution`.
 *
 * The runtime already knows the full dependency set of every scope; this
 * module surfaces it. Every value commit stamps its node with a ChangeRecord
 * (a write, an async landing, a refresh() invalidation, or a derived change
 * whose `causes` chain back to root writes). When a computation re-executes,
 * the deps whose stamp is newer than the node's last run are its causes, so
 * each re-run can be explained as a chain down to the originating write:
 *
 *   [why-run] effect "docTitle" ran (run 4)
 *     ← memo "userLabel" changed (#6)
 *       ← signal "notifications" write (#5) 2 → 3
 *
 * This module is the attribution ENGINE: all semantics live here, and it is
 * decoupled from the core. `enable()` installs it into the core's narrow
 * observe-tier hook points (attribution-hooks.ts); core's only obligation is
 * to call those hooks with true facts. Disabled cost is one null check per
 * hook site; prod builds fold the sites out entirely. Nothing in the core
 * imports this module — it is reachable only through the package's
 * `./attribution` entry, so an observe build that never imports it never
 * ships it. The same hook surface is the intended substrate for external
 * consumers (devtools) — one mechanism, two front-ends.
 */
export type ChangeKind = "write" | "derived" | "async" | "refresh";
/**
 * Provenance of a root change: the imperative frame that performed it.
 *
 * - `interaction` — a user event handler (the web runtime marks dispatch via
 *   `withInteraction`). `name` is the event type, `target` the element hit
 *   (`button#next "Next →"` — the quoted text as `AttributionOptions.values`
 *   allows), `at` the dispatch time on the `performance.now()` clock — the
 *   base every feedback-latency number is measured from.
 * - `effect` — an effect callback (`name` = the effect's name; `run` = the
 *   compute run whose effect phase performed the write, when that run was
 *   recorded — so a write can be joined to the re-run that produced it).
 * - `action` — a step of an `action()` generator (`name` = the generator's
 *   name, when it has one). Writes after an `await` (not a `yield`) run in a
 *   bare microtask and stamp `external` — the documented escape.
 * - `async` — an async landing (`name` = the node whose flight landed).
 * - `navigation` — a router's navigation, declared via `withOrigin` around
 *   the location write (`name` = the matched route pattern `/users/:id`;
 *   `to`/`from` the concrete paths; `params` what the pattern bound; `at`
 *   when it was requested). The router-agnostic seam: any router that wraps
 *   its write gets navigations named by route in every hold, re-run and
 *   verdict, with no per-router knowledge anywhere in the engine.
 * - `external` — none of the above: timers, sockets, promise callbacks, setup.
 *
 * `interaction` on a non-interaction frame is the user event the frame runs
 * under — an action started by a click, an effect whose run was caused by a
 * click's write, a landing whose flight a click started, a navigation a link
 * click performed. It is what lets every downstream cost be keyed by the
 * interaction that paid for it.
 */
export interface ChangeOrigin {
    kind: "interaction" | "effect" | "action" | "async" | "navigation" | "external";
    name?: string;
    target?: string;
    /**
     * When the frame opened (`performance.now()` clock). Always set for
     * `interaction` and `navigation`; set on an `effect` frame only while an
     * `effect` record listener exists (the callback's timed start).
     */
    at?: number;
    interaction?: ChangeOrigin;
    /** `effect` only: the `RerunEvent.run` of the compute run this callback belongs to. */
    run?: number;
    /** `navigation` only: concrete destination and departure paths, and the bound params. */
    to?: string;
    from?: string;
    params?: Readonly<Record<string, string | undefined>>;
}
export interface ChangeRecord {
    /** Global monotonic change sequence — orders causes across the app. */
    seq: number;
    kind: ChangeKind;
    name: string;
    /**
     * Identity of the node that changed — the signal written, the memo whose
     * value changed — in the same id space as `RerunEvent.nodeId`, so a
     * derived cause joins the run that produced it and repeated writes to
     * one signal join each other where `name` alone would merge every
     * unnamed `signal`. Stamped on every record the engine makes; optional
     * for a record built elsewhere (a `HeldWrite`, a deserialized artifact).
     */
    nodeId?: number;
    /**
     * Short previews of the value transition (writes only) — present under
     * `AttributionOptions.values: "full"`, never carried otherwise.
     */
    prev?: string;
    value?: string;
    /** First user frames of the triggering write's stack (opt-in). */
    stack?: string[];
    /** For derived changes: the upstream changes that produced this one. */
    causes?: ChangeRecord[];
    /** Root changes only: who performed the write. */
    origin?: ChangeOrigin;
    /** Root changes only: when the write was stamped (`performance.now()` clock). */
    at?: number;
}
export interface RerunEvent {
    /** Global monotonic run sequence. */
    run: number;
    /** When the run started (`performance.now()` clock). */
    at: number;
    /** How many times this node has re-run since attribution was enabled. */
    nodeRuns: number;
    nodeKind: "effect" | "memo";
    nodeName: string;
    /**
     * Identity of the scope that ran, stable for the node's lifetime within
     * the process: every run of one memo/effect carries the same `nodeId`, so
     * runs join to a scope after the record has left the process (where
     * `nodeName` alone would merge every unnamed `effect`). The engine's own
     * per-node id, also what `ChangeOrigin.run` and the cycle/relay checks
     * key on; not meaningful across processes or sessions. In-process
     * consumers get the live node as the `live` argument beside the record
     * (`OBSERVE.records.subscribe("rerun", (event, node) => …)`).
     */
    nodeId: number;
    /**
     * The deps that changed since this node's previous run. Empty means the
     * re-run was not triggered by a tracked value change (creation-adjacent
     * pull, error retry, or a cause this prototype does not stamp yet).
     */
    causes: ChangeRecord[];
    /** Dependency count after this run. */
    depCount: number;
    /** Names of deps this run subscribed to that the previous run did not. */
    depsAdded: string[];
    /** Names of deps the previous run had that this run dropped. */
    depsRemoved: string[];
    /** Wall time of this run excluding nested recomputes (ms). */
    selfMs: number;
    /** Wall time of this run including nested recomputes (ms). */
    totalMs: number;
    /**
     * Whether the run produced a changed value. A PLAIN memo run with
     * `changed: false` was pure waste — the equality cutoff stopped it from
     * notifying anyone. Effects run with `_equals: false` in core (their
     * effect phase re-fires on every recompute), so the engine derives this
     * fact itself: an effect run whose compute output is identical to the
     * previous run's reports `changed: false` — the phase re-fired with the
     * same input, pure waste. Side-effect-only computes (`undefined` output)
     * are exempt: identity of `undefined` proves nothing about their work.
     * Summed as `wastedMs` in costs() (plain, non-held runs only — see
     * `phase`).
     */
    changed: boolean;
    /**
     * Which posture this run executed under. "optimistic" = under an
     * optimistic lane (overlay recompute); "held" = a hold was open or owns
     * the node (the run may be replayed/settled later); "plain" = an ordinary
     * committed run. Overlay runs are real work (they count toward time
     * budgets) but are never blamed as waste, and costs() reports their time
     * separately as `overlayMs`.
     */
    phase: "plain" | "held" | "optimistic";
    /**
     * The changed value was parked in `_pendingValue` (held) rather than
     * committed directly; its reveal happens on the hold's own schedule. Held
     * runs are excluded from waste accounting.
     */
    held: boolean;
    /** The user interaction this run traces back to through its causes, if any. */
    interaction?: ChangeOrigin;
}
/**
 * A computation's creation run — the first run, the one with no causes to
 * explain (a `RerunEvent` is every run after it). The same measurements as a
 * re-run, less the causal fields a first run cannot have, and `interaction`
 * inherited from whatever built the node: the enclosing recompute (a
 * parent's fn creating children) or the handler/effect frame at the top of
 * the stack. Creation time is already charged to the interaction record's
 * `created`; this is the per-node face of that sum.
 */
export interface CreateEvent {
    /** When the run started (`performance.now()` clock). */
    at: number;
    nodeKind: "effect" | "memo";
    nodeName: string;
    /** Same id space as `RerunEvent.nodeId` — the node's later re-runs join here. */
    nodeId: number;
    /** Dependency count after this run. */
    depCount: number;
    /** Wall time of this run excluding nested recomputes (ms) — children created inside report their own. */
    selfMs: number;
    /** Wall time of this run including nested recomputes (ms). */
    totalMs: number;
    /** Posture the run executed under — see `RerunEvent.phase`. */
    phase: "plain" | "held" | "optimistic";
    /** The value was parked in `_pendingValue` rather than committed — see `RerunEvent.held`. */
    held: boolean;
    /** The interaction whose handler or flush built this node, if any. */
    interaction?: ChangeOrigin;
}
/**
 * One run of an effect's imperative half — the callback that touches the DOM
 * or the outside world — timed from entry to exit, cleanup included, whether
 * or not it threw. The compute half is the `RerunEvent`/`CreateEvent` with
 * the same `nodeId` that preceded it in the flush; `run` joins the two for a
 * re-run and is absent for a creation's first callback.
 */
export interface EffectRunEvent {
    /** When the callback started (`performance.now()` clock). */
    at: number;
    /** Wall time of the callback, nested runs included (ms). */
    durationMs: number;
    nodeId: number;
    nodeName: string;
    /** `RerunEvent.run` of the compute run whose effect phase this is; absent for a creation. */
    run?: number;
    /** The interaction the preceding compute run traced to, if any. */
    interaction?: ChangeOrigin;
}
/**
 * One `flush()` drain: from the scheduler picking up scheduled work until
 * every batch it processed has committed (effects ran) or been parked in a
 * held transition. The unit React's "Scheduler" track paints; the engine's
 * `flushEnd` settles interactions and navigations on the same instant.
 * Counts cover the runs the engine recorded inside the drain (excluded
 * scopes not counted).
 */
export interface FlushEvent {
    /** When the drain started (`performance.now()` clock). */
    at: number;
    /** Wall time of the drain (ms). */
    durationMs: number;
    /** Re-runs recorded during the drain. */
    runs: number;
    /** Creation runs during the drain. */
    created: number;
    /** A transition was judged incomplete during the drain — some of its writes stayed staged. */
    held: boolean;
    /**
     * The interaction every recorded run of the drain traced to, when there
     * was exactly one; absent when none did, or when runs for several
     * interactions shared the drain.
     */
    interaction?: ChangeOrigin;
}
/**
 * One async flight — a promise or async iterable an async computation
 * registered — from its origin (the earliest the engine knows: a
 * `markFlight` preload mark, or first sight at registration) to the moment
 * it landed or was superseded by the node's next flight (`abandoned` — the
 * answer will be discarded). A flight whose node is disposed mid-air
 * produces no record.
 */
export interface FlightEvent {
    nodeId: number;
    nodeName: string;
    /** Owner-chain labels of the async node, root first, when the node is owned. */
    ownerPath?: string[];
    /** When the flight started (`performance.now()` clock). */
    at: number;
    /** Wall time in the air (ms). */
    durationMs: number;
    outcome: "landed" | "abandoned";
    /** The interaction whose write started the flight, if any. */
    interaction?: ChangeOrigin;
}
/**
 * One showing of a loading boundary's fallback, delivered when it stops
 * showing. `ownerPath` names the boundary by its subtree's owner chain
 * (`["<App>", "<Feed>"]`); absent when the subtree never reported a path.
 * The fold in `feedback().fallbacks` is this record summed per boundary,
 * with the flash verdict.
 */
export interface FallbackEvent {
    ownerPath?: string[];
    /** When the fallback appeared (`performance.now()` clock). */
    at: number;
    /** How long it stayed (ms). */
    shownMs: number;
    /** The interaction whose work the boundary was waiting on, if the engine could tell. */
    interaction?: ChangeOrigin;
}
/**
 * The live graph's size at a navigation's settle — the moment an app has
 * finished moving between two screens, so a count that climbs visit after
 * visit is a root or a subscription the previous screen left behind.
 * Delivered on `OBSERVE.records.subscribe("graph", …)` per settled
 * navigation; a walk of the owner tree from the registered top-level roots,
 * made only when something listens or `graphGrowth` is on.
 */
export interface GraphEvent extends GraphSize {
    /** When the navigation settled (`performance.now()` clock). */
    at: number;
    /** The route pattern the navigation matched (`name`), else its `to`. */
    route?: string;
    /** The navigation this count belongs to. */
    navigation: NavigationEvent;
}
/**
 * The live reactive graph's size, as `graphSize()` counts it: the owner tree
 * from the registered top-level roots, then every computation and signal
 * reachable from it through dependencies and subscriptions — which is how
 * an ownerless effect (created with no owner, kept alive by its sources)
 * is found — and the edges between them.
 */
export interface GraphSize {
    /** Top-level roots alive (`render()`'s, module-scope `createRoot()`s, panels). */
    roots: number;
    /** Owners in the tree: roots, component owners, owned computations. */
    owners: number;
    /** Computations (memos, effects, boundaries), owned or reached through a subscription. */
    computations: number;
    /** Signals reached through a computation's dependencies. */
    signals: number;
    /** Dependency links — each computation's sources, counted once. */
    edges: number;
}
/**
 * What user data the engine's records carry — see `AttributionOptions.values`.
 * Ordered: `"full"` carries the most, `"none"` the least.
 */
export type AttributionValues = "full" | "labels" | "none";
export interface AttributionOptions {
    /** Pretty-print each re-run to the console (default true). */
    log?: boolean;
    /**
     * What user data records carry. Default per build tier: `"full"` in dev
     * builds, `"none"` in observe builds — a production observability
     * artifact carries no user data unless a holder asks for it. Records name things —
     * owner paths, `name` options, store paths, route patterns — and are
     * otherwise numbers, kinds and outcomes; this option governs the fields
     * that quote application data: the value previews on a root write
     * (`ChangeRecord.prev`/`value`, and the `HeldWrite.prev`/`value` copied
     * from them), the text of the element an interaction hit
     * (`ChangeOrigin.target` / `InteractionEvent.target` — the `"Next →"` in
     * `button#next "Next →"`), and every sentence built from those (the
     * console log, `formatRerun`, `formatOrigin`, the SILENT_HOLD / LONG_HOLD
     * / STACKED_HOLDS verdicts naming the interaction, OPTIMISTIC_REVERTED's
     * shown and settled values). Applied where the record is BUILT, so
     * nothing downstream — a ring buffer, a fold, a listener, an exporter —
     * ever holds what the level excludes.
     *
     * - `"full"` — previews of the written values (strings quoted and cut at
     *   40 characters, numbers and booleans verbatim, everything else a type
     *   tag such as `Array(12)`) and the element's text: what makes dev
     *   output readable.
     * - `"labels"` — no value previews; element text kept only on a `button`
     *   or an `a` (the control's caption — what the person pressed is the
     *   point of an interaction record) and dropped for anything else (the
     *   text of a `div` or a `td` is content).
     * - `"none"` — no value previews, no element text: names, numbers, kinds
     *   and outcomes only. What an observe-tier holder that carries records
     *   out of the process (an APM adapter) should pass.
     *
     * Across holds the LEAST permissive level wins — the one key that
     * combines the other way from the rest, where a hold can only ask for
     * more: a holder that must not see user data is not overruled by one
     * that wants it. A holder that names no level asks for the tier's
     * default, so in an observe build it tightens to `"none"` beside anyone;
     * a single holder passing `"full"` there gets `"full"`, and an explicit
     * `"full"` never loosens what another holder demanded. Governs what is
     * built from the moment the level is in effect; records already in a
     * ring buffer keep what they carried. A
     * navigation's concrete paths and params (`ChangeOrigin.to`/`from`/
     * `params`) are the router's description and are not governed here.
     */
    values?: AttributionValues;
    /**
     * Run the cost checks — the thresholded findings over the engine's own
     * accounting: `hotRuns`, `hotTime`, `wideDeps`, `unstableMemos`,
     * `fanOut`, `wastedRecompute` (default true). `false` turns all six off at once, whatever
     * their individual settings, so a consumer that wants records only (an
     * exporter, a profiler track) pays for none of their per-node bookkeeping.
     * Hold, long-hold and waterfall tracking are records with verdicts on top,
     * not checks, and are unaffected; disable those through their own options.
     */
    checks?: boolean;
    /** Capture the user stack frame of each write — slow (default false). */
    stacks?: boolean;
    /** Ring-buffer size for each `history(type)` buffer (default 200). */
    historyLimit?: number;
    /**
     * Hot-scope warning: emit a diagnostic when one scope re-runs `count`
     * times within `windowMs` (default 120 runs / 1000ms — deliberately above
     * animation-frame cadence, so a legitimate rAF-driven scope at 60/s does
     * not cry wolf). `false` disables.
     */
    hotRuns?: {
        count: number;
        windowMs: number;
    } | false;
    /**
     * Wide-scope warning: emit a diagnostic when a scope's dependency count
     * reaches this (default 30) — the coarse-read / helper-leak signature.
     * Re-warns only if the count then grows by another 50%. `false` disables.
     */
    wideDeps?: number | false;
    /**
     * Time-budget warning: emit a diagnostic when one scope's summed self-time
     * inside `windowMs` exceeds `budgetMs` (default 8ms / 1000ms — half a frame
     * spent in one scope). Unlike `hotRuns` this catches the few-but-expensive
     * scope that run counts miss. `false` disables.
     */
    hotTime?: {
        budgetMs: number;
        windowMs: number;
    } | false;
    /**
     * Unstable-output warning: emit a diagnostic when a memo commits a
     * referentially-new but shallowly-equivalent plain object/array on this
     * many consecutive runs (default 4). Such a memo's equality gate never
     * closes — every subscriber re-runs on every upstream change — which makes
     * it a fan-out amplifier that is otherwise only findable by profiling.
     * `false` disables.
     */
    unstableMemos?: number | false;
    /**
     * Wasted-recompute warning: emit WASTED_RECOMPUTE when a scope re-ran at
     * least `minRuns` times within `windowMs` and `ratio` or more of those
     * runs produced an unchanged value while costing `budgetMs` or more of
     * compute in all (default 5 runs / 80% / 2ms / 1000ms). The equality gate
     * closed every time: the scope's inputs changed without changing its
     * result, so the runs were pure cost — the profiler-shaped fact
     * `costs().wastedMs` sums, as a finding. Held and overlay runs are not
     * counted (they may be replayed). Once per window per scope. `false`
     * disables.
     */
    wastedRecompute?: {
        minRuns: number;
        ratio: number;
        budgetMs: number;
        windowMs: number;
    } | false;
    /**
     * HUGE_FAN_OUT threshold while the engine is enabled: emit the finding
     * when a committed root invalidation (write, refresh, async landing)
     * reaches a node with at least this many subscribers (default 250). The
     * same code the always-on core check emits from GRAPH_SIZE_WARN_AT (2000)
     * up, with `data.write` naming the invalidation; the engine hands over to
     * the core there, so one change never carries two findings. Once per
     * node, re-warning once the count has grown by another 500. `false`
     * leaves only the always-on threshold.
     */
    fanOut?: number | false;
    /**
     * Async-waterfall warning: emit a diagnostic when an async flight that
     * could only start after an upstream flight resolved (its recompute's
     * cause chain reaches the upstream's async landing, and its origin
     * post-dates that landing) forms a sequential chain of 2+ flights, each
     * of which took at least `minFlightMs` (default 50ms). The duration gate
     * is one safety valve for what the graph cannot see: a settled
     * preload/cache hit resolves fast and never warns. In-flight preloads are
     * absolved by origin: `markFlight()` stamps (and first-seen identity)
     * prove work predated the upstream landing — parallel, not sequential.
     * Chains of 2 emit at `info` severity, structured channel only (a
     * dependent fetch is sometimes intrinsic, and unmarked external preloads
     * are invisible); 3+ escalate to `warn` with console output. `false`
     * disables.
     */
    waterfalls?: {
        minFlightMs: number;
    } | false;
    /**
     * Silent-hold warning: emit a diagnostic when a user's writes were held
     * behind async work for at least `infoMs` (default 100ms — RAIL's "feels
     * instant" ceiling) and the screen never acknowledged the wait — no
     * `isPending()`/`latest()` reader downstream of the held writes or their
     * blockers, no optimistic value, no `affects()` mark, and no lane effect
     * painted while held. Below `warnMs`
     * (default 200ms — the INP "good" ceiling) the event is advisory
     * (structured channel only); at or above it the console gets the finding.
     * The engine measures to the commit, not the paint, so every number is a
     * floor on what the user saw; the thresholds sit at the strict end of the
     * band on purpose. Holds that staged no root write (initial loads, bare
     * `refresh()`) are never judged: nothing the user did went unanswered.
     * `false` disables hold tracking altogether (`longHolds` included).
     */
    holds?: {
        infoMs: number;
        warnMs: number;
    } | false;
    /**
     * Long-hold warning: emit a diagnostic when a hold's quiescent tail — the
     * time from the LAST write to join it until it committed — reached
     * `infoMs` (default 500ms), `warn` from `warnMs` (default 1000ms, where
     * RAIL says the user loses the thread). Measured from the last join so a
     * hold that keeps taking input (typing) is judged by each wait, not by its
     * lifetime. A hold this long is past what a stale screen should carry,
     * acknowledged or not: the honest UI is a fallback, which a `Loading`
     * boundary gives only when it has not revealed yet or its `on` prop
     * changed. Reported as LONG_HOLD when the hold was acknowledged; a silent
     * long hold stays one SILENT_HOLD with the boundary repair appended.
     * `false` disables.
     */
    longHolds?: {
        infoMs: number;
        warnMs: number;
    } | false;
    /**
     * Graph-growth warning: emit GRAPH_GROWTH when the live owner count at the
     * settle of the same route has climbed on `visits` consecutive visits
     * (default 3) to `ratio` or more of the first (default 1.25) — a root or a
     * subscription each visit leaves behind, the leak class a heap snapshot
     * finds. The count is a walk at settle, never per node. `false` disables.
     */
    graphGrowth?: {
        visits: number;
        ratio: number;
    } | false;
    /**
     * Abandoned-flights warning: emit ABANDONED_FLIGHTS when one async source
     * abandons `count` flights within `windowMs` — each superseded by the
     * next before it landed (default 3 / 1000ms: the request-per-keystroke
     * signature, every input asking again and the answers discarded). Once
     * per window per source. `false` disables.
     */
    abandonedFlights?: {
        count: number;
        windowMs: number;
    } | false;
    /**
     * Fallback-flash finding: emit FALLBACK_FLASH (`info`) when a `Loading`
     * boundary's fallback shows for less than `FALLBACK_FLASH_MS` (150ms) — a
     * spinner that appeared and vanished, too much feedback for too little
     * wait (default true). `false` disables.
     */
    fallbackFlashes?: boolean;
    /**
     * Stacked-holds warning: emit STACKED_HOLDS when `count` or more
     * interactions are waiting in one hold when it commits (default 3) — the
     * person kept clicking or typing while the first answer was in the air,
     * and every one of them waited on the same source. `false` disables.
     */
    stackedHolds?: {
        count: number;
    } | false;
    /**
     * Optimistic-revert finding: emit OPTIMISTIC_REVERTED (`info`) when an
     * optimistic value the screen showed is replaced by a different one —
     * reverted at settle, or superseded by the truth (default true). The
     * runtime's own optimistic nodes — `isPending`/`latest` companions and
     * derived overrides — are never judged: they are the acknowledgement
     * machinery, not a guess the person saw. `false` disables.
     */
    optimisticReverts?: boolean;
}
/** A fallback shown for less than this is a flash: feedback for a wait too short to need it. */
export declare const FALLBACK_FLASH_MS = 150;
/**
 * The ring buffers `attribution.history(type)` reads, by type. Four of the
 * five are the channel's records kept since the window opened; `waterfall`
 * is a fact the engine keeps but never emits — a graph-provable sequential
 * flight chain (`WaterfallRecord`), of which the ASYNC_WATERFALL finding is
 * the thresholded view.
 */
export interface HistoryRecords {
    rerun: RerunEvent;
    waterfall: WaterfallRecord;
    hold: HoldEvent;
    navigation: NavigationEvent;
    interaction: InteractionEvent;
}
export type HistoryType = keyof HistoryRecords;
/** @internal The engine's clock: `performance.now()` where it exists. */
export declare const now: () => number;
/**
 * @internal The fold tables' seam. The engine emits records; the tables that
 * fold them — `costs()` (scopes and writes), `feedback()` (sources,
 * interactions, navigations, flights, fallbacks) — live in their own modules
 * and register here when imported, so a consumer that only subscribes to
 * records never ships them. Each hook fires at the moment the engine has the
 * fact; `reset` on `enable()`/`disable()`. With nothing registered every
 * site is one empty loop.
 */
export interface FoldHooks {
    rerun?(el: Computed<any>, event: RerunEvent): void;
    hold?(event: HoldEvent): void;
    navigation?(event: NavigationEvent): void;
    /** A flight started; `abandoned` when it superseded one still in the air. */
    flightStart?(el: Computed<any>, abandoned: boolean): void;
    flightLanded?(el: Computed<any>, ms: number): void;
    fallback?(boundary: object, tree: Computed<any> | undefined, shown: boolean): void;
    reset?(): void;
}
/** @internal */
export declare function registerFold(hooks: FoldHooks): void;
/** @internal Root cause names of a cause chain — the writes/landings/refreshes the chain bottoms out in. */
export declare function rootsOf(causes: ChangeRecord[], out: Set<string>): void;
export declare function nodeName(node: Signal<any> | Computed<any>): string;
/** `click on button#next "Next →"`, `effect "syncTitle"`, `action "save"`, `navigation to /users/:id`, … */
export declare function formatOrigin(origin: ChangeOrigin): string;
export declare function formatRerun(event: RerunEvent): string;
/**
 * The engine: turn it on and read its ring buffers. Its records are
 * delivered on the core's channel — `OBSERVE.records.subscribe("rerun" |
 * "hold" | "interaction" | "navigation" | "create" | "effect" | "flush" |
 * "flight" | "fallback" | "graph", (event, live) => …)` — the same place
 * the runtimes' records arrive, so a consumer has one subscribe. The folds
 * over those records — `costs()`, `feedback()` — and the point queries —
 * `why()`, `subscriptions()` — and the formatters — `formatRerun()`,
 * `formatOrigin()` — are named exports of `@solidjs/signals/attribution`
 * rather than methods here, so a consumer that only wants records (a
 * production adapter) does not ship the tables a console or an agent reads;
 * importing a fold is what turns its accounting on.
 */
export interface Attribution {
    /**
     * Install the engine, or take one more hold on it, and return the release
     * for that hold. The engine is shared by every consumer in the page — a
     * profiler track, an APM adapter, a diagnostics capture — so each
     * `enable()` is a hold: the first installs the hooks and resets
     * everything; one taken while already enabled opens a fresh window over
     * the ring buffers and folds (`history(type)`, `costs()`, `feedback()` …
     * read from here on) without disturbing the live tracking state or
     * anyone's subscriptions, so a capture begun beside a running consumer
     * still measures only its own scenario.
     *
     * Options combine across holds by the most demanding value per key: a
     * hold's `opts` say what it wants (the defaults fill what it leaves
     * unsaid), and the engine does whatever any holder asked for — the log
     * prints while any holder wants it, a check runs while any holder wants
     * it and at the most sensitive threshold requested, `historyLimit` is the
     * largest. A hold can add to what another asked for, never take it away,
     * so the result does not depend on the order holds were taken; releasing
     * a hold withdraws its requests — a track enabled with `log: false` beside
     * a console session never silences it, and a capture with tight
     * thresholds beside a records-only adapter runs the checks for its own
     * duration. One key runs the other way: `values` combines to the LEAST
     * permissive level any holder asked for, so an adapter that must not see
     * user data (`values: "none"`) is honoured beside a console session that
     * wants everything. The release is idempotent; the last release uninstalls the
     * hooks and clears every ring buffer. A consumer that re-`enable()`s to
     * reopen its window must release both holds (or `disable()`).
     *
     * Subscriptions are not the engine's: its records arrive on
     * `OBSERVE.records`, whose listeners outlive any hold — subscribe before
     * or after `enable()`, and unsubscribe with the function `subscribe`
     * returned.
     */
    enable(opts?: AttributionOptions): () => void;
    /**
     * Tear the engine down whatever holds are outstanding: drops every hold,
     * uninstalls the hooks and clears every ring buffer. The console's and a
     * test harness's reset — a consumer sharing the page with others releases
     * its own hold with the function `enable()` returned instead. Idempotent;
     * a `disable()` with nothing enabled is a no-op reset. Listeners on
     * `OBSERVE.records` are untouched: they are the channel's.
     */
    disable(): void;
    /**
     * The ring buffer of `type` since `enable()` — the last `historyLimit`
     * records (default 200), oldest first; the same objects the channel
     * delivered. Facts, not verdicts: each is recorded regardless of the
     * thresholds the findings apply to it.
     *
     * - `"rerun"` — every re-run recorded (see `RerunEvent`). Kept only while
     *   a record had an audience — a `rerun` listener, an imported fold
     *   (`costs`/`feedback`), or the console log; a records-only consumer
     *   that wants none of those pays for none, and reads an empty buffer.
     * - `"waterfall"` — every graph-provable sequential flight chain, warned
     *   or not: the ASYNC_WATERFALL finding is the duration-gated view.
     * - `"hold"` — every settled transition hold that staged at least one root
     *   write, acknowledged or not: SILENT_HOLD / LONG_HOLD are the
     *   thresholded verdicts (`HoldEvent.silent` / `.long` carry them).
     * - `"navigation"` — every navigation a router declared via `withOrigin`,
     *   settled or not: what route, under which interaction, how many writes,
     *   and — once its writes are through — how long that took and how
     *   (`committed`, `held` with the `HoldEvent` attached, `superseded`).
     * - `"interaction"` — every user interaction a runtime declared via
     *   `withInteraction`, settled or not: what was dispatched, when, what it
     *   wrote, the re-runs and creations it caused, the holds its writes
     *   waited in and the navigations it performed — and, once through, how
     *   long the person waited (`settledMs`) and how it ended. The
     *   per-dispatch record `feedback().interactions` folds by name.
     */
    history<K extends HistoryType>(type: K): readonly HistoryRecords[K][];
    /**
     * Cooperative preload declaration: stamp a flight object (promise or async
     * iterable) with its true kickoff time BEFORE the reactive graph sees it.
     * A route preloader or query cache calls this on the promise it hands out
     * (on the WRAPPER it mints, with the original kickoff time — wrapping
     * defeats identity tracking otherwise); any dependent that later awaits it
     * is then judged against the real start — work already in the air when its
     * upstream landed is parallel, never a waterfall link. Callable while
     * attribution is disabled (marks made at navigation time must survive a
     * later enable()).
     */
    markFlight(flight: object, startedAt?: number): void;
}
/** @internal The id the engine's records name `node` by, if it has one yet — read without assigning. */
export declare function nodeIdOf(node: object): number | undefined;
/** One landed flight: its node name, wall duration, and upstream chain. */
export interface FlightLink {
    name: string;
    ms: number;
}
export interface WaterfallRecord {
    /** Sequential flights, oldest first, ending at the flight that landed. */
    chain: FlightLink[];
    /** Summed wall time of the chain — the serialized cost. */
    sequentialMs: number;
}
export interface HeldWrite {
    name: string;
    prev?: string;
    value?: string;
    origin?: ChangeOrigin;
}
export interface HoldEvent {
    /**
     * When the wait began (`performance.now()` clock): the interaction that
     * performed the held writes when one is known (`interaction.at`) or the
     * first flush that parked them, whichever is earlier. `at + holdMs` is the
     * commit.
     */
    at: number;
    /** Wall time the user waited: `at` to the commit. */
    holdMs: number;
    /**
     * The quiescent tail: from the LAST held write to join (the user's final
     * input) to the commit. Equal to `holdMs` for a single write; shorter when
     * the hold kept taking input. The LONG_HOLD measure.
     */
    tailMs: number;
    /** The user interaction whose writes were held, when the stamp is known. */
    interaction?: ChangeOrigin;
    /**
     * The declared unit of work the held writes belong to — the `navigation`
     * a router described via `withOrigin` — when one is known. What names the
     * hold by route (`navigation to /users/:id`) rather than by signal; the
     * same object as `NavigationEvent.origin`, so the two join by identity.
     */
    origin?: ChangeOrigin;
    /** Flushes that ended with the hold still open. */
    flushes: number;
    /** Root signal writes staged behind the hold (the user's unanswered input). */
    heldWrites: HeldWrite[];
    /** Async nodes the hold waited on (union across its parked flushes). */
    blockers: string[];
    /**
     * Feedback the graph provably rendered for this hold — an `isPending()`
     * reader, a `latest()` shadow, an optimistic value, an `affects()` mark —
     * one entry per affordance, in the order found. Empty and
     * `paintedDuringHold === 0` is the SILENT_HOLD signature.
     */
    acknowledgements: Acknowledgement[];
    /**
     * Effect callbacks that ran inside the hold's parked flushes. Mainline
     * effects are stashed while a hold is open, so these are lane effects —
     * readers of optimistic values and of `isPending()`/`latest()` companions,
     * i.e. the screen changing in response to the hold. An unrelated effect
     * cannot land here: it waits with everything else.
     */
    paintedDuringHold: number;
    /** The hold was opened (or joined) by an `action()`. */
    action: boolean;
    /**
     * The engine's silent-hold verdict, stamped at settle: no affordance
     * acknowledged the wait (`acknowledgements` empty) and nothing painted
     * while it was open (`paintedDuringHold === 0`). Duration-free — the
     * SILENT_HOLD finding is this above `holds.infoMs` — so a consumer can
     * flag every silent wait or apply its own floor, and a record that left
     * the process still carries the verdict.
     */
    silent: boolean;
    /**
     * The engine's long-hold verdict, stamped at settle: the quiescent tail
     * (`tailMs`) reached `longHolds.infoMs` under the options in effect;
     * `false` when long-hold tracking is off. The LONG_HOLD finding is this
     * verdict, tiered by `warnMs`.
     */
    long: boolean;
}
/**
 * One way the screen acknowledged a hold. `reader` is where it was painted —
 * the owner path of the first effect the census found reading the
 * affordance (`["<App>", "<Feed>", "effect"]`), so a consumer can say WHICH
 * screen answered, not only that one did. Absent when the affordance was
 * registered but the census found no reader through the graph (an optimistic
 * store: its readers are proxy traps, not nodes). `feedback()` ranks
 * acknowledgements by `kind:source` (`isPending:posts`).
 */
export interface Acknowledgement {
    kind: "isPending" | "latest" | "optimistic" | "affects";
    /** The node the affordance hangs on — the async source for `isPending`/`latest`, the optimistic/affected node otherwise. */
    source: string;
    reader?: string[];
}
/**
 * The live graph's size — a walk, not a counter: nothing is charged at node
 * creation, disposal, write or re-run; the engine asks at a navigation's
 * settle. Two passes. The owner tree from the registered top-level roots
 * gives `owners` and seeds the computations. Then each computation's
 * dependency links give `edges` and the `signals` and computations they
 * reach, and each newly met source's subscriber list gives the computations
 * that read it — including one created with no owner, which no chain holds
 * and only its sources keep alive: the subscription leak a heap snapshot
 * finds. Measured at 5–10 ns per link; a 50k-owner graph walks in under a
 * millisecond. Dormant nodes are spliced out of their chain and are not
 * counted unless a subscription still reaches them.
 */
export declare function graphSize(): GraphSize;
/** A destination a navigation abandoned when a redirect sent it elsewhere. */
export interface NavigationHop {
    name?: string;
    to?: string;
    params?: Readonly<Record<string, string | undefined>>;
    /** When the redirect away from it was declared (`performance.now()` clock). */
    at: number;
}
export interface NavigationEvent {
    /** The matched route pattern the router gave — `/users/:id`. After a redirect, the final one. */
    name?: string;
    to?: string;
    from?: string;
    params?: Readonly<Record<string, string | undefined>>;
    /** When the navigation was requested (`performance.now()` clock). */
    at: number;
    /**
     * The route the document arrived on, declared by the router at its initial
     * match (`NavigationRef.initial`): `at` is the document's navigation start
     * (`0` unless the router passed its own), there is no `from`, `writes` is
     * `0`, and it settles `committed` when the router's frame closes — the
     * record names the route the page loaded as; its timing is Navigation
     * Timing's, not the runtime's.
     */
    initial?: true;
    /** The user interaction it ran under, when known — a link click. */
    interaction?: ChangeOrigin;
    /** Root writes the frame performed, redirect hops included. */
    writes: number;
    /** Destinations abandoned along the way, in order — present only when a redirect occurred. */
    redirects?: NavigationHop[];
    /**
     * Wall time from the request to settle: the end of the drain that committed
     * its writes, or the commit of the hold they waited in. `undefined` while
     * unsettled.
     */
    settledMs?: number;
    outcome?: "committed" | "held" | "superseded";
    /** The hold its writes waited in, when hold tracking recorded one. */
    hold?: HoldEvent;
    /**
     * The frame object its writes were stamped with — `ChangeRecord.origin` on
     * each, `HoldEvent.origin` on the hold. Join key, by identity.
     */
    origin: ChangeOrigin;
}
export interface InteractionEvent {
    /** Event type — `click`, `keydown`, `input`… */
    name: string;
    /** The element hit, as the runtime described it — `button#next "Next →"`. */
    target?: string;
    /**
     * When the interaction began (`performance.now()` clock): the browser event's
     * own timestamp when the runtime supplied it, else the moment the handler
     * frame opened. Joins `PerformanceEventTiming.startTime` for the same event.
     */
    at: number;
    /**
     * Browser event creation to handler entry (ms) — the queueing the browser's
     * INP counts as input delay. Present only when `at` predates the frame.
     */
    inputDelayMs?: number;
    /** Wall time of the handler itself, entry to return. */
    handlerMs: number;
    /** Root writes attributed to the frame: the handler's, and those of frames it opened (a navigation). */
    writes: number;
    /** Re-runs traced back to this interaction while the record was open. */
    runs: number;
    /** Computations created in those runs or in the frame's flushes (the "create 1,000 rows" work). */
    created: number;
    /** Summed self-time of `runs` and `created` (ms). Quantized per run; `settledMs` is the wall clock. */
    runMs: number;
    /** Holds its writes waited in, in settle order. */
    holds: HoldEvent[];
    /** Navigations performed under it, in open order. */
    navigations: NavigationEvent[];
    /**
     * Dispatch to settle: the handler's return when it wrote nothing, the end of
     * the drain that committed its writes, or the commit of the last hold they
     * waited in — whichever came last. `undefined` while unsettled.
     */
    settledMs?: number;
    outcome?: "idle" | "committed" | "held";
    /**
     * The handler returned a thenable (`async () => { await save(); … }`) and
     * the record waited for it: handler return → the promise settling, in
     * milliseconds, capped at `ASYNC_HANDLER_CAP_MS`. The continuation runs
     * with no frame on the stack, so writes it makes are not attributed to
     * this interaction — only its duration is. Absent when the handler
     * returned synchronously.
     */
    continuationMs?: number;
    /**
     * The frame object every downstream fact carries — `ChangeOrigin.interaction`
     * on writes and frames, `RerunEvent.interaction`, `HoldEvent.interaction`,
     * `NavigationEvent.interaction`. Join key, by identity.
     */
    origin: ChangeOrigin;
}
export declare const attribution: Attribution;
