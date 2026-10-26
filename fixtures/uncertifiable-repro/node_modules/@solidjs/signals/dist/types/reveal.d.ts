export type RevealOrder = "sequential" | "together" | "natural";
type OrderAccessor = () => RevealOrder;
type BoolAccessor = () => boolean;
/**
 * Lower-level primitive behind `<Reveal>`: coordinates the order in which the
 * Loading boundaries created directly inside `fn` reveal their content.
 *
 * @param fn the subtree; the Loading boundaries it creates directly are the
 *   group's slots, a nested `createRevealOrder` a composite slot
 * @param options `order` — `"sequential"` (default: front to back, the
 *   first unready slot showing its fallback), `"together"` (all at once),
 *   `"natural"` (each on its own); `collapsed` — in a sequential group, the
 *   slots behind the frontier render nothing instead of their fallbacks
 */
export declare function createRevealOrder<T>(fn: () => T, options?: {
    order?: OrderAccessor;
    collapsed?: BoolAccessor;
}): T;
export {};
