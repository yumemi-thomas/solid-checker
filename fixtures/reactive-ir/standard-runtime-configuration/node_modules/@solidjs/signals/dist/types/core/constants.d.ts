export declare const REACTIVE_NONE = 0;
export declare const REACTIVE_CHECK: number;
export declare const REACTIVE_DIRTY: number;
export declare const REACTIVE_RECOMPUTING_DEPS: number;
export declare const REACTIVE_IN_HEAP: number;
export declare const REACTIVE_IN_HEAP_HEIGHT: number;
export declare const REACTIVE_ZOMBIE: number;
export declare const REACTIVE_DISPOSED: number;
export declare const REACTIVE_OPTIMISTIC_DIRTY: number;
export declare const REACTIVE_SNAPSHOT_STALE: number;
export declare const REACTIVE_LAZY: number;
export declare const REACTIVE_MANUAL_WRITE: number;
/**
 * The pending recompute is a re-ask of the same question: `refresh()` dirtied
 * the node while no tracked input changed value. Cleared whenever a real
 * value-change notification arrives (`insertSubs`), and consumed by
 * `recompute` into the node's `_reask` classification — a quiet (re-ask)
 * pending window does not read as pending (question-scoped pending model).
 */
export declare const REACTIVE_REASK: number;
/**
 * A dependency write landed while this subscriber was mid-recompute — a
 * nested pull committed beneath one of its reads (#3037). The heap refuses
 * RECOMPUTING nodes, so recompute's tail consumes this latch and reschedules:
 * values the pass read before the nested commit are stale. Only set for
 * links validated this pass (gen-current): a write to an untouched link is
 * either re-read later in the pass (fresh) or trimmed with it (not a dep).
 */
export declare const REACTIVE_MISSED_WAKE: number;
export declare const CONFIG_OWNED_WRITE: number;
export declare const CONFIG_NO_SNAPSHOT: number;
export declare const CONFIG_TRANSPARENT: number;
export declare const CONFIG_IN_SNAPSHOT_SCOPE: number;
export declare const CONFIG_CHILDREN_FORBIDDEN: number;
export declare const CONFIG_AUTO_DISPOSE: number;
export declare const CONFIG_SYNC: number;
export declare const CONFIG_OPTIMISTIC: number;
export declare const CONFIG_HAS_COMPANIONS: number;
export declare const CONFIG_HAS_SNAPSHOT: number;
export declare const CONFIG_HAS_LANE: number;
/** Set on a FIREWALL computed when any of its child signals creates an
 * isPending()/latest() companion. Gates the post-recompute child-companion
 * walk (#3038): a store computed's `_child` chain holds one node per
 * materialized leaf, so walking it unconditionally makes every update cost
 * O(all leaves ever read). Sticky — set at companion creation, never
 * cleared; sync-only apps never set it and never pay the walk. */
export declare const CONFIG_CHILD_COMPANIONS: number;
/** Set on a computed when its first firewall child signal is installed
 * (projection machinery). Gates markNode's firewall-children walk with one
 * masked read of the always-present _config — the walk's old `_child` read
 * moved into the cold extension (§12), and an unconditional `_x` deref per
 * marked node measurably taxed the propagation hot path (diamond -22%). */
export declare const CONFIG_FW_CHILDREN: number;
/** Authoritative-view reader (`until()`): while this node computes, reads
 * dodge active optimistic OVERRIDES only — the predicate must observe
 * arriving truth, never the caller's own tentative writes (which would
 * trivially satisfy it). Everything else reads normally, INCLUDING
 * transition-staged `_pendingValue`: staged data is authoritative (optimism
 * lives only in override slots), and a hold that refused staged reads would
 * deadlock on data the open transaction itself is holding (a refresh the
 * action issued lands staged and cannot commit until the hold releases).
 * read() checks the bit on the reading computation (`context`) directly — no
 * ambient flag — so a shared computed the predicate pulls recomputes as
 * itself (no bit) under the normal view, and its cache never forks. */
export declare const CONFIG_AUTHORITATIVE_READ: number;
/** Sticky mark: an authoritative-view reader read this node PAST an active
 * override. The ack shape — an authoritative arrival EQUAL to the override —
 * rides paths that are deliberately silent under A17 (every ordinary reader
 * sees the override, so an equal landing changes nothing for them). A marked
 * node notifies those readers on such paths anyway, so the landed truth is
 * seen without re-firing ordinary subscribers. Never cleared — only nodes an
 * until() predicate observed mid-override pay. */
export declare const CONFIG_AUTHORITATIVE_OBSERVED: number;
/** Promise-delivery effect (resolve()/until()): commits its computed value
 * directly even when recomputing under its own held transition. These
 * effects deliver applies on a microtask (#2930) instead of the stashed
 * effect queues, so the value must ride the same immediate schedule — a
 * staged value with an immediate apply delivers stale state (resolve) or
 * deadlocks the hold (until). Safe because the node is a private leaf: no
 * subscriber reads an effect's value, only its own apply does. */
export declare const CONFIG_DIRECT_COMMIT: number;
/** Fresh-pull reader (awaitable `refresh()`'s waiter effect): a read of a
 * dirty source recomputes it inline even when the height gate defers to the
 * flush. Closes the same-flush ordering race where a waiter created
 * alongside a refresh() mark read the PRE-re-ask value as settled and
 * delivered stale; with the pull, the waiter either parks on the re-ask's
 * pending window (async — woken by the settle walk, which runs on every
 * landing including equal-value ones) or serves its sync answer. resolve()
 * deliberately keeps that race — its contract is "first settled value"
 * (#2930), not "next quiescent state". */
export declare const CONFIG_FRESH_READ: number;
/** HELD truth (#3164): this node's staged `_pendingValue` is confirming
 * truth riding a transaction that retains optimism, revealed only at that
 * transaction's settle. Two arming sites, one meaning: the store fold
 * (a landing staged into the retaining transaction) and until()'s
 * flip-entanglement (a foreign carrier's staged write, stolen when it
 * flipped the awaited predicate truthy). Override-covered nodes never
 * arm: the override is their display and its revert their notification
 * (A17).
 *
 * To a DERIVING reader the truth is a staged value like any other: a memo
 * or user effect served it enters the transaction and is held with it
 * (A29), so a pass that composes it with a superseded override's truth
 * composes ONE staged world — never staged truth beside committed
 * neighbours (#3568: the mask served the retaining transaction's own pass
 * the landed `length` through the override while the rows past it stayed
 * committed, and `<For>` walked into a hole). Stale readers of a foreign
 * transaction keep committed through the stale-of-foreign clause,
 * untracked reads keep committed (Rule 1), and latest() and until()'s
 * predicate tunnel through — the exemption that keeps holds deadlock-free.
 *
 * The one reader the mark gates is a LANE pass, owning transaction or not
 * (ruled 2026-09-22, superseding #3589's owner exemption): a lane applies
 * its frame display-ahead at the park, so a lane pass served the truth
 * would paint the confirmation beside the optimism it confirms —
 * `saving=true` beside the saved row, a frame no timeline contains
 * (GabbeV's union tear). It keeps committed and is re-run by the reveal's
 * post-revert wake. A lane under the retaining transaction owns its
 * overrides and its lane cargo (`ownsLane`), not the transaction's
 * confirming truth. Cleared at commit (the commit IS the reveal);
 * subscribers masked during the hold are woken by finalizePureQueue's
 * post-revert pass. */
export declare const CONFIG_HELD_TRUTH: number;
/** SLOT node (store leaf): created through `slotSignal` with `_host`/`_key`
 * backrefs baked into the literal. The unobserved sweep dispatches these to
 * the ONE shared hook (`setSlotUnobserved`) instead of a per-node closure
 * held in a per-node extension — store mounts materialize one signal per
 * touched leaf, so per-node allocations (options object, equals closure,
 * unobserved closure, NodeExtension) were the measured create-floor bytes
 * (warm dbmon profile: store node machinery ~36% + GC ~29%). */
export declare const CONFIG_SLOT_NODE: number;
/** Optimistic node whose own source arrived with a value DIFFERENT from its
 * active override (A18 supersession, #3331). The override survives only as
 * the displayed value — untracked reads and the applied frame keep it until
 * the owning transaction commits — while the graph has already moved to the
 * staged truth in `_pendingValue`: tracked readers see it and the corrected
 * cascade is that transaction's held work. Set by the two own-source write
 * paths (asyncWrite, transition-held recompute); cleared by a fresh optimistic
 * write (a new override re-masks) and by the revert. */
export declare const CONFIG_OVERRIDE_SUPERSEDED: number;
/** HELD children (#3404): this node's `_firstChild` chain (and `_disposal`
 * list) was built by a recompute whose result has not committed — a staged
 * value, a pending window, or a run under a held transaction. A later
 * recompute may tear those children down immediately: nothing observable
 * was ever built on them. Unset, the children belong to the committed frame
 * and a recompute defers them as zombies (`_pendingFirstChild`) until this
 * node commits — regardless of whether the recompute runs under a
 * transaction. A parked node (status propagation stamps `_transition`
 * without recomputing) recomputed when its source lands otherwise disposed
 * its committed children mid-hold, running their cleanups before the
 * transaction's atomic reveal. Cleared by `commitPendingNode`. Transaction
 * work only (A15 lane work and transaction work, #3698): zombies are parked
 * by a pass under a held transaction; a pass over a lane parks a LANE frame
 * instead (`CONFIG_LANE_FRAME`), whatever the node's kind. */
export declare const CONFIG_HELD_CHILDREN: number;
/** The frame parked in `_pendingFirstChild` / `_pendingDisposal` is a LANE
 * frame (#3662, #3698; A15 lane work and transaction work): a lane pass —
 * on an effect or a memo alike — publishes into the lane's frame (an
 * effect's run; a memo's derived override, A17), so the frame it replaces
 * leaves the screen when the lane applies (A30) — not at the action's
 * commit like #3404's transaction zombies, and not at the pass (a held lane
 * defers the apply with the frame still displayed). Drained by the lane's
 * render entry the parking site pushed ahead of the new frame's effects
 * (cleanups before side effects), by `commitPendingNode` if a hold commits
 * the node first, or with the owner's death. While set the parked frame is
 * not a hold (the node is not queued or stamped for it — lane work never
 * makes its node transaction work), a superseding pass disposes the
 * never-shown live children on the spot, and a lane-channel dirty on a
 * member is cancelled (`laneZombie`). Ruled 2026-09-28 (#3698): a pass is
 * lane work or transaction work by its owner, never by node kind. #3662
 * flagged effects only, and a memo's lane pass parked a transaction zombie
 * that queued and stamped the memo as the action's pending node, so its
 * next mainline recompute re-entered the hold. */
export declare const CONFIG_LANE_FRAME: number;
/** In-flight async node whose inputs were PUBLISHED while it was pending: a
 * batch or transaction committed with the node still `STATUS_PENDING` (an
 * unobserved flight, #3305), so the inputs are on screen and the node's
 * committed `_value` is stale against them. Governs read()'s reveal
 * carve-out: a stale (render) reader in some OTHER transaction may show a
 * foreign-held pending node's committed value — parallel transactions, no
 * entanglement — only while that value is coherent with the visible frame,
 * i.e. while the flight's inputs are themselves held (unpublished) and not
 * lane-revealed. Set by `commitPendingNodes`; cleared when the node next
 * enters pending fresh (a new flight from a settled state). */
export declare const CONFIG_INPUTS_PUBLISHED: number;
/** A28 (4): the node was written inside a recompute that ran OUTSIDE a flush
 * (a creation-time compute — boundary machinery, a mapArray's first run). Such
 * a write is promoted at that recompute's end: readers in the same block see
 * it. Cleared when the next flush begins; set only on that rare path. */
export declare const CONFIG_PROMOTED: number;
/** A28 for same-tick adoption: the node was staged outside a flush and then
 * adopted by a transaction (initTransition) before any flush carried the
 * staging — the stamp says "held", but nothing flushed is staged for it, so
 * on no channel is the write visible yet: `latest()` answers the committed
 * value, the verdict sees nothing pending (as the store's leaves already did
 * through their own selection). Cleared when the carrying flush re-stamps the
 * transaction's pending nodes (reassignPendingTransition). */
export declare const CONFIG_ADOPTED_UNFLUSHED: number;
/** The node's active override is a DERIVED one: a lane pass published its
 * speculative result into the override slot instead of `_value` (lanes
 * stage — an optimistic derivation is an override, #3479). Its truth is not
 * `_value` but a recompute from its inputs' truth, so the body-end
 * supersession (`endOptimism`) and the authoritative-flight blockage
 * (`transitionBlocked`) skip it; the revert drops the override and re-derives
 * it (`resolveOptimisticNodes`). Cleared with the override. */
export declare const CONFIG_DERIVED_OVERRIDE: number;
/** Observe tiers only: the node is framework plumbing (the `solid-js/refresh`
 * HMR memo between a component's root and its body) — it has no name, is no
 * segment of any owner path, and the attribution engine records nothing about
 * it (creation, re-runs, checks), while the nodes it owns stay fully observed.
 * Set from the internal `_plumbing` option at creation; never set in prod. */
export declare const CONFIG_PLUMBING: number;
export declare const STATUS_NONE = 0;
export declare const STATUS_PENDING: number;
export declare const STATUS_ERROR: number;
export declare const STATUS_UNINITIALIZED: number;
export declare const EFFECT_PURE = 0;
export declare const EFFECT_RENDER = 1;
export declare const EFFECT_USER = 2;
export declare const EFFECT_TRACKED = 3;
/** OR-ed into the `type` a lane passes to its effect runners: lane runs
 * apply ahead of their transaction and are exempt from ownership parking. */
export declare const LANE_RUN = 4;
export declare const NOT_PENDING: {};
export declare const NO_SNAPSHOT: {};
/**
 * Stand-in stored in `_overrideValue` for an optimistic write of literal
 * `undefined` (#2898). The slot doubles as the optimistic-node brand
 * (`undefined` = not optimistic, `NOT_PENDING` = at rest), so the raw value
 * would erase the node's optimistic identity: the write turns invisible and
 * follow-up writes route off the optimistic path and commit permanently.
 * Same shape as NO_SNAPSHOT. Sites that surface the override VALUE unwrap
 * via `visibleOverrideValue`; slot identity tests stay raw.
 */
export declare const OVERRIDE_UNDEFINED: {};
/** Unwrap an active override's stored value for surfacing to readers (#2898). */
export declare function unwrapOverride<T = any>(v: unknown): T;
export declare const STORE_SNAPSHOT_PROPS = "sp";
export declare const SUPPORTS_PROXY: boolean;
export declare const defaultContext: {};
/**
 * Brand symbol used by `Refreshable<T>` values (projection stores, async
 * memos) to expose their underlying computation to `refresh()`. Not part of
 * the user-facing API.
 *
 * @internal
 */
export declare const $REFRESH: unique symbol;
/**
 * Brand applied to values that participate in the `refresh()` re-run protocol.
 * Accessors receive this handle internally; projected stores expose it through
 * their public return type so user-defined hooks that wrap `createOptimisticStore`
 * / `createProjection` / projection-form `createStore` can have their return
 * types inferred without leaking the internal `$REFRESH` symbol into public type
 * signatures (TS4058).
 */
export type Refreshable<T> = T & {
    readonly [$REFRESH]: any;
};
