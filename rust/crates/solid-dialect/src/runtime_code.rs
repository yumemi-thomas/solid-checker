//! The Solid runtime's development diagnostic codes the checker answers for, and what a dialect says of each.
//!
//! A code Solid's dev build reports (`FLUSH_IN_ACTION`, `STRICT_READ_UNTRACKED`, ...) is a runtime fact. Which checker rules claim it,
//! which only decline it, and whether Solid reports it from the scheduler instead of at the offending call (a *placeless* code) are
//! the dialect's answers ([`Dialect::code_policy`](crate::Dialect::code_policy)), so no analysis spells a table from codes to
//! rules itself (AGENTS.md invariant 4). The codes listed are those `scripts/fuzz/codes.mjs` (`codePolicy`) gives a claiming or a
//! declining rule; the others (engine consequences, performance notes, server-only and typed-away codes) have no row to answer.

/// A Solid dev diagnostic code that has a claiming or a declining rule.
///
/// Exhaustive on purpose: a code added here is a compile error in [`RuntimeCode::name`] and in every dialect's answer until it is
/// decided. The name is the code as the pinned dev dist spells it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RuntimeCode {
    StrictReadUntracked,
    NoOwnerCleanup,
    NoOwnerEffect,
    NoOwnerBoundary,
    NoOwnerContext,
    ContextNotFound,
    SettledCleanupUnowned,
    ReactiveWriteInOwnedScope,
    StoreWriteOutsideSetter,
    CleanupInForbiddenScope,
    PrimitiveInForbiddenScope,
    AsyncOutsideLoadingBoundary,
    PendingAsyncUntrackedRead,
    PendingAsyncForbiddenScope,
    SyncNodeReceivedAsync,
    UntrackedReadAfterAwait,
    MissingEffectFn,
    ActionCalledInOwnedScope,
    ResolveInReactiveScope,
    UntilInReactiveScope,
    FlushInAction,
    FlushInEffectCallback,
    LoadingOnOutsideHold,
    LoadingOnAccessor,
    OnFlushInOwnedScope,
    OptimisticWriteBeforeLoad,
    CaughtReactiveRead,
    FlushInTrackedQueueCallback,
    DynamicStaticPromise,
    PrimitiveInEffectCallback,
}

impl RuntimeCode {
    /// Every code, in the order of the policy table of `scripts/fuzz/codes.mjs`.
    pub const ALL: [Self; 30] = [
        Self::StrictReadUntracked,
        Self::NoOwnerCleanup,
        Self::NoOwnerEffect,
        Self::NoOwnerBoundary,
        Self::NoOwnerContext,
        Self::ContextNotFound,
        Self::SettledCleanupUnowned,
        Self::ReactiveWriteInOwnedScope,
        Self::StoreWriteOutsideSetter,
        Self::CleanupInForbiddenScope,
        Self::PrimitiveInForbiddenScope,
        Self::AsyncOutsideLoadingBoundary,
        Self::PendingAsyncUntrackedRead,
        Self::PendingAsyncForbiddenScope,
        Self::SyncNodeReceivedAsync,
        Self::UntrackedReadAfterAwait,
        Self::MissingEffectFn,
        Self::ActionCalledInOwnedScope,
        Self::ResolveInReactiveScope,
        Self::UntilInReactiveScope,
        Self::FlushInAction,
        Self::FlushInEffectCallback,
        Self::LoadingOnOutsideHold,
        Self::LoadingOnAccessor,
        Self::OnFlushInOwnedScope,
        Self::OptimisticWriteBeforeLoad,
        Self::CaughtReactiveRead,
        Self::FlushInTrackedQueueCallback,
        Self::DynamicStaticPromise,
        Self::PrimitiveInEffectCallback,
    ];

    /// The code as Solid's dev dist spells it (`FLUSH_IN_ACTION`).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::StrictReadUntracked => "STRICT_READ_UNTRACKED",
            Self::NoOwnerCleanup => "NO_OWNER_CLEANUP",
            Self::NoOwnerEffect => "NO_OWNER_EFFECT",
            Self::NoOwnerBoundary => "NO_OWNER_BOUNDARY",
            Self::NoOwnerContext => "NO_OWNER_CONTEXT",
            Self::ContextNotFound => "CONTEXT_NOT_FOUND",
            Self::SettledCleanupUnowned => "SETTLED_CLEANUP_UNOWNED",
            Self::ReactiveWriteInOwnedScope => "REACTIVE_WRITE_IN_OWNED_SCOPE",
            Self::StoreWriteOutsideSetter => "STORE_WRITE_OUTSIDE_SETTER",
            Self::CleanupInForbiddenScope => "CLEANUP_IN_FORBIDDEN_SCOPE",
            Self::PrimitiveInForbiddenScope => "PRIMITIVE_IN_FORBIDDEN_SCOPE",
            Self::AsyncOutsideLoadingBoundary => "ASYNC_OUTSIDE_LOADING_BOUNDARY",
            Self::PendingAsyncUntrackedRead => "PENDING_ASYNC_UNTRACKED_READ",
            Self::PendingAsyncForbiddenScope => "PENDING_ASYNC_FORBIDDEN_SCOPE",
            Self::SyncNodeReceivedAsync => "SYNC_NODE_RECEIVED_ASYNC",
            Self::UntrackedReadAfterAwait => "UNTRACKED_READ_AFTER_AWAIT",
            Self::MissingEffectFn => "MISSING_EFFECT_FN",
            Self::ActionCalledInOwnedScope => "ACTION_CALLED_IN_OWNED_SCOPE",
            Self::ResolveInReactiveScope => "RESOLVE_IN_REACTIVE_SCOPE",
            Self::UntilInReactiveScope => "UNTIL_IN_REACTIVE_SCOPE",
            Self::FlushInAction => "FLUSH_IN_ACTION",
            Self::FlushInEffectCallback => "FLUSH_IN_EFFECT_CALLBACK",
            Self::LoadingOnOutsideHold => "LOADING_ON_OUTSIDE_HOLD",
            Self::LoadingOnAccessor => "LOADING_ON_ACCESSOR",
            Self::OnFlushInOwnedScope => "ON_FLUSH_IN_OWNED_SCOPE",
            Self::OptimisticWriteBeforeLoad => "OPTIMISTIC_WRITE_BEFORE_LOAD",
            Self::CaughtReactiveRead => "CAUGHT_REACTIVE_READ",
            Self::FlushInTrackedQueueCallback => "FLUSH_IN_TRACKED_QUEUE_CALLBACK",
            Self::DynamicStaticPromise => "DYNAMIC_STATIC_PROMISE",
            Self::PrimitiveInEffectCallback => "PRIMITIVE_IN_EFFECT_CALLBACK",
        }
    }
}

/// What a dialect says of one runtime code (`None` from [`Dialect::code_policy`](crate::Dialect::code_policy) is *no policy*: the
/// code has no ending credits, never "every rule credits it").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodePolicy {
    /// The rules, by catalog name (`Rule::metadata().name`), whose violation or own obligation answers a diagnostic of the code.
    pub claiming: &'static [&'static str],
    /// The rules whose *uncertifiable* finding at the site is the checker declining the code without claiming it; a violation of one
    /// of them is still a wrong rule.
    pub declining: &'static [&'static str],
    /// Solid reports the code from the scheduler, not from the offending call: the diagnostic names no authored line, so a program is
    /// held to it by file, never by line.
    pub placeless: bool,
}

#[cfg(test)]
mod tests {
    use super::RuntimeCode;

    #[test]
    fn runtime_code_names_are_distinct_screaming_snake_case() {
        let mut seen = std::collections::HashSet::new();
        for code in RuntimeCode::ALL {
            let name = code.name();
            assert!(
                !name.is_empty()
                    && name
                        .bytes()
                        .all(|byte| byte.is_ascii_uppercase() || byte == b'_'),
                "{code:?} is spelled {name}"
            );
            assert!(seen.insert(name), "{name} names two codes");
        }
    }

    #[test]
    fn runtime_code_all_lists_every_variant_once_in_declaration_order() {
        // `name` is an exhaustive match, so a variant missing from `ALL` is caught by the count against the discriminants.
        for (index, code) in RuntimeCode::ALL.iter().enumerate() {
            assert_eq!(*code as usize, index, "{code:?} is out of order in ALL");
        }
        assert_eq!(RuntimeCode::ALL.len(), 30);
    }
}
