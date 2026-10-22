export interface ScopeCost {
    name: string;
    kind: "effect" | "memo";
    runs: number;
    selfMs: number;
    /**
     * Self-time of PLAIN, non-held runs that produced an unchanged value —
     * the recoverable number. Overlay runs (optimistic/transition) are never
     * counted here: an optimistic recompute landing back on the committed
     * value is the mechanism working, not waste.
     */
    wastedMs: number;
    /** Self-time spent in optimistic/transition (overlay) runs. */
    overlayMs: number;
}
export interface WriteCost {
    /** Root cause name (a signal write, async landing, or refresh target). */
    name: string;
    /** Number of downstream re-runs this root triggered. */
    runs: number;
    /** Summed self-time of every downstream re-run it caused. */
    downstreamMs: number;
}
export interface AttributionCostTables {
    /** Ranked by self-time. */
    scopes: ScopeCost[];
    /** Ranked by the total downstream re-run time each root write caused. */
    writes: WriteCost[];
}
/**
 * Aggregated cost tables since `enable()`: `scopes` ranked by self-time
 * (with `wastedMs` = time spent on unchanged-value runs), `writes` ranked by
 * total downstream re-run time each root write caused.
 */
export declare function costs(): AttributionCostTables;
