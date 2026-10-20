export interface FeedbackSource {
    /** The async nodes the holds waited on; empty when an action alone kept them open. */
    sources: string[];
    holds: number;
    /** Summed wait across the holds (ms). */
    heldMs: number;
    worstMs: number;
    /** Holds with no acknowledgment at all — the SILENT_HOLD signature, at any duration. */
    silent: number;
    silentMs: number;
    /** Holds whose only acknowledgment was a `latest()` shadow: the input showed, nothing said "loading". */
    latestOnly: number;
    /**
     * Holds whose quiescent tail (last write to join → commit) reached
     * `longHolds.infoMs`, acknowledged or not — the LONG_HOLD signature at the
     * table level. The affordance is not the whole answer there: a fallback
     * (`Loading` keyed with `on`), a preload, a cache, or a faster source is.
     * `longMs` sums the tails.
     */
    long: number;
    longMs: number;
    /** Which affordances answered, and in how many holds — ranked. */
    acknowledgedBy: {
        by: string;
        holds: number;
    }[];
    /** Interactions whose writes were held here, ranked by holds. */
    interactions: {
        interaction: string;
        holds: number;
    }[];
    /** Distinct root writes that were held. */
    writes: string[];
    /** Holds an action opened or joined. */
    actions: number;
}
export interface FeedbackInteraction {
    /** `click on button#next "Next →"` — type and target; repeated dispatches fold together. */
    interaction: string;
    /** Distinct dispatches seen (by dispatch time). */
    dispatches: number;
    /** Re-runs traced back to this interaction, and their summed self-time. */
    runs: number;
    selfMs: number;
    /** The most re-run self-time a single dispatch caused — the long-flush hazard. */
    worstDispatchMs: number;
    /** Holds this interaction's writes waited in — the silent-hold hazard. */
    holds: number;
    heldMs: number;
    silentMs: number;
    worstHoldMs: number;
}
/** Per async source: how many flights it started, and how many it threw away. */
export interface FlightStats {
    source: string;
    /** Flights registered (a recompute that produced a new promise/iterable). */
    flights: number;
    /** Flights that landed (whether or not the value changed). */
    landed: number;
    /**
     * Flights superseded by a newer one before landing — the search-as-you-type
     * signature when large: every keystroke asked, most answers were discarded.
     * A debounced/equality-gated derivation between input and fetch is the repair.
     */
    abandoned: number;
    /** Summed and worst wall time of landed flights (ms). */
    landedMs: number;
    worstMs: number;
}
/** Per loading boundary: how long, and how briefly, it showed its fallback. */
export interface FallbackStats {
    /** The boundary's owner path (`<App> › <Feed>`), or `boundary` when unnamed. */
    boundary: string;
    /** Times the fallback was shown. */
    shows: number;
    /** Summed and worst fallback duration (ms) across completed shows. */
    shownMs: number;
    worstMs: number;
    /**
     * Shows shorter than the flash window (default 150ms): a spinner that
     * appeared and vanished — the other end of the SILENT_HOLD spectrum, too
     * much feedback for too little wait. A preload, a cache, or lifting the
     * fetch above the boundary removes the flash.
     */
    flashes: number;
}
/**
 * Per route: what navigating to it cost, folded from settled
 * `NavigationEvent`s — the route-level view a router integration used to
 * have to build itself, from the runtime's own facts.
 */
export interface FeedbackNavigation {
    /** The route pattern (`/users/:id`), or the concrete `to` when the router gave no pattern. */
    name: string;
    navigations: number;
    /** Summed and worst request-to-settle time (ms) across settled navigations. */
    settledMs: number;
    worstMs: number;
    /** Navigations whose writes waited in a hold, and the time they waited. */
    held: number;
    heldMs: number;
    /** Held navigations the screen acknowledged nothing for — the SILENT_HOLD signature. */
    silent: number;
    /** Navigations overwritten by a later one before they landed. */
    superseded: number;
    /** Navigations a redirect sent elsewhere on the way (keyed by where they ended up). */
    redirected: number;
}
export interface AttributionFeedbackTables {
    sources: FeedbackSource[];
    interactions: FeedbackInteraction[];
    /** Routes ranked by the time spent held navigating to them, then by total settle time. */
    navigations: FeedbackNavigation[];
    /** Async sources ranked by abandoned flights, then by flights. */
    flights: FlightStats[];
    /** Loading boundaries ranked by flashes, then by time shown. */
    fallbacks: FallbackStats[];
}
/**
 * What the user waited on, folded from holds and the interaction on each
 * re-run: `sources` ranks async sources by the silent time writes spent held
 * behind them (with which affordances answered, how often, and which
 * interactions were held); `interactions` ranks user events by the total time
 * they cost — re-run work caused (long-flush hazard) beside time held
 * (silent-hold hazard). Facts at every duration; SILENT_HOLD is the
 * thresholded verdict. Three more tables round out the picture: `navigations`
 * ranks routes by the time spent held navigating to them (folded from settled
 * navigations), `flights` counts each async source's flights and how many
 * were abandoned before landing (the re-ask storm), and `fallbacks` measures
 * how long each loading boundary showed its fallback and how often that was a
 * flash.
 */
export declare function feedback(): AttributionFeedbackTables;
