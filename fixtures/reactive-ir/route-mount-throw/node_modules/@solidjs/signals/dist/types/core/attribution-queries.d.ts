/**
 * Point queries over the engine's live state — the devtools/console view of
 * one scope. Their own module so a records-only consumer never ships them;
 * they read the engine's ring buffer and the graph, and register nothing.
 */
import { type RerunEvent } from "./attribution.js";
/**
 * Re-run history for one scope — pass a memo/effect accessor or raw node,
 * or a scope's name as a string (an out-of-process consumer such as the
 * diagnostics bridge holds no node, only the `nodeName` the records
 * carry). Records name their scope by `nodeId`; a node that has never run
 * under the engine has none, and no history. By name, every scope of that
 * name answers. A view of `history("rerun")`, so it shares that buffer's
 * gate: runs nothing wanted a record of (no `rerun` listener, fold or log
 * at the time) left no record and are not here — a console session that
 * wants them subscribes or imports a fold first.
 */
export declare function why(target: unknown): RerunEvent[];
/**
 * Current dependency names of one scope — the devtools subscription view.
 * Read from the graph, not a record, so it answers with or without an
 * audience for re-run records (and with the engine disabled).
 */
export declare function subscriptions(target: unknown): string[];
