//! The flip's named blockers (`scripts/flip/PLAN.md`): the `analysis_context`
//! of an uncertifiable that replaces a silent exit or an assumed proof step.
//! One file, so the read and write tracks add no lines to `lib.rs`. A constant
//! is used from the commit that flips its rule.
#![allow(dead_code)]

/// A function no call edge places may run anyway: a member, a slot, a tag, a
/// class element, a value handed where nothing follows it.
pub(crate) const FUNCTION_RUN_UNPLACED: &str = "function-run-unplaced";

/// ADR 0659: only server entries are visible, nothing was selected, and a
/// client entry may sit outside the project.
pub(crate) const CLIENT_ROOT_UNPROVEN: &str = "client-root-unproven";
/// ADR 0659: a site under a build constant no selection decides here.
pub(crate) const BUILD_CONSTANT_ARM: &str = "build-constant-arm";

/// ADR 0750: a search stopped at its depth limit (a chain of calls, promises,
/// layers or values longer than it follows) before it decided.
pub(crate) const SEARCH_DEPTH_LIMIT: &str = "search-depth-limit";
/// ADR 0750: a search's work budget, site cap or round cap ran out before it
/// decided.
pub(crate) const SEARCH_BUDGET_EXHAUSTED: &str = "search-budget-exhausted";

// Track W.
pub(crate) const SOLID_VERSION_UNCLASSIFIED: &str = "solid-version-unclassified";
pub(crate) const RESPONSE_HEAD_POSITION_UNPROVEN: &str = "response-head-position-unproven";
pub(crate) const SERVER_FUNCTION_VALUE_HANDED: &str = "server-function-value-handed";
pub(crate) const LEAF_OPERATION_IDENTITY_UNPROVEN: &str = "leaf-operation-identity-unproven";
pub(crate) const OPTIMISTIC_REACH_UNRESOLVED: &str = "optimistic-reach-unresolved";
pub(crate) const PROVIDER_NOT_PROVEN_ABOVE: &str = "provider-not-proven-above";
pub(crate) const OWNER_VARIES_BY_CALLER: &str = "owner-varies-by-caller";
pub(crate) const STORE_OPERATION_MAY_NOT_WRITE: &str = "store-operation-may-not-write";

// Track R.
pub(crate) const LOADING_ON_VALUE_UNPROVEN: &str = "loading-on-value-unproven";
pub(crate) const SPREAD_CONTENT_UNPROVEN: &str = "spread-content-unproven";
pub(crate) const CONTENT_SOURCE_VALUE_UNPROVEN: &str = "content-source-value-unproven";
pub(crate) const ACCESSOR_VALUE_COERCED: &str = "accessor-value-coerced";
pub(crate) const HANDLER_ROOT_UNPROVEN: &str = "handler-root-unproven";
pub(crate) const STATIC_SOURCE_UNPROVEN: &str = "static-source-unproven";
pub(crate) const COMPUTE_RETURN_UNPROVEN: &str = "compute-return-unproven";
pub(crate) const DESTRUCTURE_ROOT_UNPROVEN: &str = "destructure-root-unproven";
pub(crate) const CAUGHT_READ_UNPROVEN: &str = "caught-read-unproven";
pub(crate) const ASYNC_PROVENANCE_UNPROVEN: &str = "async-provenance-unproven";
pub(crate) const SOURCE_PENDING_UNPROVEN: &str = "source-pending-unproven";
pub(crate) const MOUNT_FORM_WINDOW: &str = "mount-form-window";
pub(crate) const HANDED_READ_AFTER_AWAIT: &str = "handed-read-after-await";
pub(crate) const PREFERENCE_READ_UNPROVEN: &str = "preference-read-unproven";
