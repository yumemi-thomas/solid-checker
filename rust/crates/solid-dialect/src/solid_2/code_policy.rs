//! Solid 2.0's runtime-code policy: which checker rules claim each Solid dev diagnostic code, which only decline it, and which codes are
//! placeless.
//!
//! One literal line per code. `scripts/fuzz/codes.mjs` (`codePolicy`, `placelessCodes`) is the harness's copy of this table;
//! the rules crate's test holds every rule name here to an exact `Rule::metadata().name` (AGENTS.md invariant 3), so a renamed rule
//! cannot leave a name that matches nothing.

use crate::{CodePolicy, RuntimeCode};

/// `(variant, the code's name, claiming rules, declining rules, placeless)`.
type Row = (
    RuntimeCode,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    bool,
);

// One line per code, on purpose: `scripts/fuzz/codes.test.mjs` reads these lines, and a diff of the table is a diff of the policy.
#[rustfmt::skip]
pub(super) const TABLE: [Row; 30] = [
    (RuntimeCode::StrictReadUntracked, "STRICT_READ_UNTRACKED", &["strict-read-untracked", "no-destructure", "reactive-handler-frozen", "components-return-once"], &[], false),
    (RuntimeCode::NoOwnerCleanup, "NO_OWNER_CLEANUP", &["missing-owner"], &[], false),
    (RuntimeCode::NoOwnerEffect, "NO_OWNER_EFFECT", &["missing-owner", "primitive-in-directive-application"], &[], false),
    (RuntimeCode::NoOwnerBoundary, "NO_OWNER_BOUNDARY", &["missing-owner"], &[], false),
    (RuntimeCode::NoOwnerContext, "NO_OWNER_CONTEXT", &["missing-owner"], &[], false),
    (RuntimeCode::ContextNotFound, "CONTEXT_NOT_FOUND", &["missing-context-provider"], &["missing-owner"], false),
    (RuntimeCode::SettledCleanupUnowned, "SETTLED_CLEANUP_UNOWNED", &["missing-owner"], &[], true),
    (RuntimeCode::ReactiveWriteInOwnedScope, "REACTIVE_WRITE_IN_OWNED_SCOPE", &["no-write-in-pure-computation"], &[], false),
    (RuntimeCode::StoreWriteOutsideSetter, "STORE_WRITE_OUTSIDE_SETTER", &["no-store-mutation-outside-setter"], &[], false),
    (RuntimeCode::CleanupInForbiddenScope, "CLEANUP_IN_FORBIDDEN_SCOPE", &["leaf-owner-forbidden-call"], &[], false),
    (RuntimeCode::PrimitiveInForbiddenScope, "PRIMITIVE_IN_FORBIDDEN_SCOPE", &["leaf-owner-forbidden-call"], &[], false),
    (RuntimeCode::AsyncOutsideLoadingBoundary, "ASYNC_OUTSIDE_LOADING_BOUNDARY", &["async-outside-loading-boundary"], &[], true),
    (RuntimeCode::PendingAsyncUntrackedRead, "PENDING_ASYNC_UNTRACKED_READ", &["pending-async-unsuspendable-read"], &[], false),
    (RuntimeCode::PendingAsyncForbiddenScope, "PENDING_ASYNC_FORBIDDEN_SCOPE", &["pending-async-unsuspendable-read"], &[], false),
    (RuntimeCode::SyncNodeReceivedAsync, "SYNC_NODE_RECEIVED_ASYNC", &["sync-computation-received-async"], &[], false),
    (RuntimeCode::UntrackedReadAfterAwait, "UNTRACKED_READ_AFTER_AWAIT", &["reactive-read-after-await"], &[], false),
    (RuntimeCode::MissingEffectFn, "MISSING_EFFECT_FN", &["missing-effect-function"], &[], false),
    (RuntimeCode::ActionCalledInOwnedScope, "ACTION_CALLED_IN_OWNED_SCOPE", &["action-called-in-owned-scope"], &[], false),
    (RuntimeCode::ResolveInReactiveScope, "RESOLVE_IN_REACTIVE_SCOPE", &["no-await-primitive-in-tracked-scope"], &[], false),
    (RuntimeCode::UntilInReactiveScope, "UNTIL_IN_REACTIVE_SCOPE", &["no-await-primitive-in-tracked-scope"], &[], false),
    (RuntimeCode::FlushInAction, "FLUSH_IN_ACTION", &["flush-in-action"], &[], false),
    (RuntimeCode::FlushInEffectCallback, "FLUSH_IN_EFFECT_CALLBACK", &["flush-in-effect-callback"], &[], false),
    (RuntimeCode::LoadingOnOutsideHold, "LOADING_ON_OUTSIDE_HOLD", &["loading-on-outside-hold"], &[], true),
    (RuntimeCode::LoadingOnAccessor, "LOADING_ON_ACCESSOR", &["loading-on-accessor"], &[], true),
    (RuntimeCode::OnFlushInOwnedScope, "ON_FLUSH_IN_OWNED_SCOPE", &["on-flush-in-owned-scope"], &[], false),
    (RuntimeCode::OptimisticWriteBeforeLoad, "OPTIMISTIC_WRITE_BEFORE_LOAD", &["optimistic-write-before-load"], &[], false),
    (RuntimeCode::CaughtReactiveRead, "CAUGHT_REACTIVE_READ", &["caught-reactive-read"], &[], true),
    (RuntimeCode::FlushInTrackedQueueCallback, "FLUSH_IN_TRACKED_QUEUE_CALLBACK", &["leaf-owner-forbidden-call"], &[], false),
    (RuntimeCode::DynamicStaticPromise, "DYNAMIC_STATIC_PROMISE", &["static-dynamic-async-source"], &[], false),
    (RuntimeCode::PrimitiveInEffectCallback, "PRIMITIVE_IN_EFFECT_CALLBACK", &["primitive-in-effect-callback"], &[], false),
];

/// The policy of `code`. The match is exhaustive: a new [`RuntimeCode`] is a compile error here until its row is placed.
pub(super) fn policy(code: RuntimeCode) -> Option<CodePolicy> {
    let (variant, _, claiming, declining, placeless) = match code {
        RuntimeCode::StrictReadUntracked => TABLE[0],
        RuntimeCode::NoOwnerCleanup => TABLE[1],
        RuntimeCode::NoOwnerEffect => TABLE[2],
        RuntimeCode::NoOwnerBoundary => TABLE[3],
        RuntimeCode::NoOwnerContext => TABLE[4],
        RuntimeCode::ContextNotFound => TABLE[5],
        RuntimeCode::SettledCleanupUnowned => TABLE[6],
        RuntimeCode::ReactiveWriteInOwnedScope => TABLE[7],
        RuntimeCode::StoreWriteOutsideSetter => TABLE[8],
        RuntimeCode::CleanupInForbiddenScope => TABLE[9],
        RuntimeCode::PrimitiveInForbiddenScope => TABLE[10],
        RuntimeCode::AsyncOutsideLoadingBoundary => TABLE[11],
        RuntimeCode::PendingAsyncUntrackedRead => TABLE[12],
        RuntimeCode::PendingAsyncForbiddenScope => TABLE[13],
        RuntimeCode::SyncNodeReceivedAsync => TABLE[14],
        RuntimeCode::UntrackedReadAfterAwait => TABLE[15],
        RuntimeCode::MissingEffectFn => TABLE[16],
        RuntimeCode::ActionCalledInOwnedScope => TABLE[17],
        RuntimeCode::ResolveInReactiveScope => TABLE[18],
        RuntimeCode::UntilInReactiveScope => TABLE[19],
        RuntimeCode::FlushInAction => TABLE[20],
        RuntimeCode::FlushInEffectCallback => TABLE[21],
        RuntimeCode::LoadingOnOutsideHold => TABLE[22],
        RuntimeCode::LoadingOnAccessor => TABLE[23],
        RuntimeCode::OnFlushInOwnedScope => TABLE[24],
        RuntimeCode::OptimisticWriteBeforeLoad => TABLE[25],
        RuntimeCode::CaughtReactiveRead => TABLE[26],
        RuntimeCode::FlushInTrackedQueueCallback => TABLE[27],
        RuntimeCode::DynamicStaticPromise => TABLE[28],
        RuntimeCode::PrimitiveInEffectCallback => TABLE[29],
    };
    debug_assert!(
        variant == code,
        "{code:?}'s arm names the row of {variant:?}"
    );
    Some(CodePolicy {
        claiming,
        declining,
        placeless,
    })
}

#[cfg(test)]
mod tests {
    use super::{TABLE, policy};
    use crate::RuntimeCode;

    #[test]
    fn code_policy_has_a_row_for_every_runtime_code_in_its_arm() {
        assert_eq!(TABLE.len(), RuntimeCode::ALL.len());
        for code in RuntimeCode::ALL {
            let row = TABLE
                .iter()
                .find(|row| row.0 == code)
                .unwrap_or_else(|| panic!("{code:?} has no row"));
            let answered = policy(code).unwrap_or_else(|| panic!("{code:?} has no policy"));
            assert_eq!(answered.claiming, row.2, "{code:?} claiming");
            assert_eq!(answered.declining, row.3, "{code:?} declining");
            assert_eq!(answered.placeless, row.4, "{code:?} placeless");
            assert_eq!(
                row.1,
                code.name(),
                "{code:?} is spelled differently in its row"
            );
        }
    }

    #[test]
    fn code_policy_rows_follow_the_order_of_the_codes() {
        for (row, code) in TABLE.iter().zip(RuntimeCode::ALL) {
            assert_eq!(row.0, code, "the table is in the order of RuntimeCode::ALL");
        }
    }

    #[test]
    fn code_policy_declining_and_placeless_are_the_decided_few() {
        let declining: Vec<_> = TABLE
            .iter()
            .filter(|row| !row.3.is_empty())
            .map(|row| (row.1, row.3))
            .collect();
        assert_eq!(declining, [("CONTEXT_NOT_FOUND", &["missing-owner"][..])]);
        let placeless: Vec<_> = TABLE.iter().filter(|row| row.4).map(|row| row.1).collect();
        assert_eq!(
            placeless,
            [
                "SETTLED_CLEANUP_UNOWNED",
                "ASYNC_OUTSIDE_LOADING_BOUNDARY",
                "LOADING_ON_OUTSIDE_HOLD",
                "LOADING_ON_ACCESSOR",
                "CAUGHT_REACTIVE_READ"
            ]
        );
    }

    #[test]
    fn code_policy_every_code_has_a_claiming_rule() {
        for row in &TABLE {
            assert!(!row.2.is_empty(), "{} has no claiming rule", row.1);
        }
    }
}
