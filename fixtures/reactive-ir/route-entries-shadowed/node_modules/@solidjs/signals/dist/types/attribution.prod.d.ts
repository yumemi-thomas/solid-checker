/**
 * `@solidjs/signals/attribution` as the prod tier resolves it.
 *
 * A prod build has no hook sites — `__OBSERVE__` folded every one out — so an
 * engine installed there would never hear a fact. Rather than make apps guard
 * the import per tier, prod resolves this inert twin: the same `Attribution`
 * surface, every query empty, `enable()` a no-op. Type-checked against the
 * real engine's interface so the two cannot drift.
 */
import type { Attribution } from "./core/attribution.js";
import type * as Engine from "./attribution.js";
export declare const attribution: Attribution;
export declare const costs: typeof Engine.costs;
export declare const feedback: typeof Engine.feedback;
export declare const why: typeof Engine.why;
export declare const subscriptions: typeof Engine.subscriptions;
export declare const formatRerun: typeof Engine.formatRerun;
export declare const formatOrigin: typeof Engine.formatOrigin;
export declare const graphSize: typeof Engine.graphSize;
export type { Acknowledgement, Attribution, AttributionOptions, AttributionValues, ChangeKind, ChangeOrigin, ChangeRecord, CreateEvent, EffectRunEvent, FallbackEvent, FlightEvent, FlightLink, FlushEvent, GraphEvent, GraphSize, HeldWrite, HistoryRecords, HistoryType, HoldEvent, InteractionEvent, NavigationEvent, NavigationHop, RerunEvent, WaterfallRecord } from "./core/attribution.js";
export type { AttributionCostTables, ScopeCost, WriteCost } from "./core/attribution-costs.js";
export type { AttributionFeedbackTables, FallbackStats, FeedbackInteraction, FeedbackNavigation, FeedbackSource, FlightStats } from "./core/attribution-feedback.js";
