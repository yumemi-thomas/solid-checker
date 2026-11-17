import type { Accessor } from "./signals.js";
import { type Store } from "./store/store.js";
/**
 * Declares that in-flight work will change the targeted data: the named
 * slot(s) — and everything DERIVED from them — read as pending
 * (`isPending` → `true`) from the declaration until the surrounding
 * transaction settles or reverts. A mark lives on its own channel — a
 * refcount on the marked node plus dep-graph reachability in the verdict
 * layer — so the marked values themselves stay readable (a mark is a promise
 * of change, not an absence of value), no reader ever suspends on one, and
 * completion/settlement accounting never sees one. This is the declaration
 * verb of the pending model — additive only. A mark can turn pending ON for
 * data the graph can't see changing yet; nothing can turn pending OFF while
 * a real change is in flight — a quiet `refresh()` re-ask under a mark still
 * reads pending (declaring the reload is what makes it a real question).
 *
 * Targets:
 * - `affects(store)` — a store proxy (any record, root or nested): every
 *   record reachable from it at declaration time reads pending, including
 *   through captured child proxies (e.g. `<For>` rows). Siblings are
 *   untouched; records added after the declaration are not covered.
 * - `affects(record, key)` — exactly the named slot of the record. One key
 *   per call (keys do NOT form a path — target the owning record directly).
 * - `affects(accessor)` — a source accessor (signal or memo): the source
 *   reads pending.
 *
 * Typically called at the top of an `action` alongside optimistic writes —
 * both are up-front declarations about the same mutation. Outside any
 * transaction the mark is released at the end of the current flush.
 *
 * @example
 * ```ts
 * const send = action(function* (text: string) {
 *   setState(s => { s.messages.push({ text, status: "sending" }); });
 *   affects(state.messages.at(-1)!, "status"); // this slot pends until settle
 *   yield api.send(text);
 * });
 *
 * const reload = action(function* () {
 *   affects(thing);      // the whole store pends…
 *   refresh(thing);      // …over this otherwise-quiet re-ask
 *   yield api.done();
 * });
 * ```
 */
export declare function affects(target: Accessor<unknown> | Store<object>): void;
export declare function affects<T extends object>(target: Store<T>, key: keyof T): void;
