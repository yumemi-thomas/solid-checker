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
pub(crate) const ROOT_MOUNT_PASS_UNPROVEN: &str = "root-mount-pass-unproven";
pub(crate) const MOUNT_FORM_WINDOW: &str = "mount-form-window";
pub(crate) const NO_HYDRATION_SUBTREE: &str = "no-hydration-subtree";
pub(crate) const HANDED_READ_AFTER_AWAIT: &str = "handed-read-after-await";
pub(crate) const PREFERENCE_READ_UNPROVEN: &str = "preference-read-unproven";

// ADR 0680: every uncertifiable row names its blocker. These name the holes the
// rows carried before the flip, as flags on the row and in its wording only.
pub(crate) const HOST_CALLBACK_TIMING: &str = "host-callback-timing";
pub(crate) const CALLEE_CALLBACK_TIMING: &str = "callee-callback-timing";
pub(crate) const CALLBACK_INVOCATION_UNPROVEN: &str = "callback-invocation-unproven";
pub(crate) const JSX_CENSUS_MISSING: &str = "jsx-census-missing";
pub(crate) const SUMMARY_READ_UNPLACED: &str = "summary-read-unplaced";
pub(crate) const SOURCE_PENDING_UNPROVEN: &str = "source-pending-unproven";
pub(crate) const READ_BACKING_UNPROVEN: &str = "read-backing-unproven";
pub(crate) const OWNED_WRITE_OPTION_UNREAD: &str = "owned-write-option-unread";
pub(crate) const LEAF_OWNER_UNPROVEN: &str = "leaf-owner-unproven";
pub(crate) const DIRECTIVE_OWNER_UNPROVEN: &str = "directive-owner-unproven";
pub(crate) const OWNER_NULLABLE: &str = "owner-nullable";
pub(crate) const OWNER_CALLERS_UNENUMERATED: &str = "owner-callers-unenumerated";
pub(crate) const OWNER_LATER_RUN_UNOWNED: &str = "owner-later-run-unowned";
pub(crate) const COMPONENT_IDENTITY_UNPROVEN: &str = "component-identity-unproven";
pub(crate) const OWNER_ALLOCATION_UNPROVEN: &str = "owner-allocation-unproven";
pub(crate) const OWNER_UNPROVEN: &str = "owner-unproven";
pub(crate) const ASYNC_OPTIONS_OPAQUE: &str = "async-options-opaque";
pub(crate) const ASYNC_READ_CAUGHT: &str = "async-read-caught";
pub(crate) const ASYNC_COMPONENT_PROP: &str = "async-component-prop";
pub(crate) const DECLARED_LOADING_VALUE: &str = "declared-loading-value";
pub(crate) const INVOCATION_CONTEXT_UNPROVEN: &str = "invocation-context-unproven";
pub(crate) const SERVER_RENDERING_UNRESOLVED: &str = "server-rendering-unresolved";
pub(crate) const CALL_ATTRIBUTED_READ: &str = "call-attributed-read";
pub(crate) const CARRIER_RUN_UNPROVEN: &str = "carrier-run-unproven";
pub(crate) const MOUNT_UNRESOLVED: &str = "mount-unresolved";
pub(crate) const CREATION_PASS_UNPROVEN: &str = "creation-pass-unproven";
pub(crate) const HANDLER_DISPATCH_UNPROVEN: &str = "handler-dispatch-unproven";
pub(crate) const LEAF_CALLBACK_OPAQUE: &str = "leaf-callback-opaque";
pub(crate) const CONDITIONAL_RETURN_UNPROVEN: &str = "conditional-return-unproven";
pub(crate) const PROPS_CALLERS_UNENUMERATED: &str = "props-callers-unenumerated";
pub(crate) const STORE_WRITE_UNPROVEN: &str = "store-write-unproven";
pub(crate) const LOADING_HOLD_UNPROVEN: &str = "loading-hold-unproven";
pub(crate) const EFFECT_OWNER_UNPROVEN: &str = "effect-owner-unproven";
pub(crate) const PACKAGE_SOURCE_UNAVAILABLE: &str = "package-source-unavailable";
pub(crate) const FUNCTION_CALL_TARGET_UNRESOLVED: &str = "function-call-target-unresolved";
pub(crate) const RUNTIME_CONFIGURATION_PREMISE: &str = "runtime-configuration-premise";
/// The last resort. `check.mjs` and `blockers.mjs` fail on it: a row that lands
/// here has a hole nobody named, which is a defect in the row, not a blocker.
pub(crate) const PROOF_STEP_UNNAMED: &str = "proof-step-unnamed";

/// Whether `context` names a blocker: kebab-case words, an optional `:detail`.
/// A function's name is no blocker: it says where, not what is missing.
pub(crate) fn named(context: &str) -> bool {
    let head = context.split_once(':').map_or(context, |(head, _)| head);
    head.contains('-')
        && !head.starts_with('-')
        && !head.ends_with('-')
        && head
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// `blocker`, keeping what the row said before as its detail: the function a
/// read is in says where, and the wording's classes still read it.
#[must_use]
pub fn with_detail(blocker: &str, context: &str) -> String {
    if context.is_empty() {
        blocker.to_owned()
    } else {
        format!("{blocker}:{context}")
    }
}

/// The first hole of a read, in the order the rule's proof meets them.
pub(crate) fn for_read(read: &crate::ReactiveRead) -> &'static str {
    if read.execution == crate::ExecutionRole::Unknown {
        FUNCTION_RUN_UNPLACED
    } else if read.host_callback_timing {
        HOST_CALLBACK_TIMING
    } else if read.callee_callback_timing {
        CALLEE_CALLBACK_TIMING
    } else if read.callback_invocation_unproven {
        CALLBACK_INVOCATION_UNPROVEN
    } else if read.missing_jsx_census {
        JSX_CENSUS_MISSING
    } else if read.summary_attributed {
        SUMMARY_READ_UNPLACED
    } else if read.pending_open {
        SOURCE_PENDING_UNPROVEN
    } else {
        READ_BACKING_UNPROVEN
    }
}

pub(crate) fn for_write(write: &crate::ReactiveWrite) -> &'static str {
    if write.execution == crate::ExecutionRole::Unknown {
        FUNCTION_RUN_UNPLACED
    } else {
        OWNED_WRITE_OPTION_UNREAD
    }
}

pub(crate) const fn for_owner(requirement: &crate::OwnerRequirement) -> &'static str {
    if requirement.unplaced_callback {
        FUNCTION_RUN_UNPLACED
    } else if requirement.owner_varies_by_caller {
        OWNER_VARIES_BY_CALLER
    } else if requirement.conditional_owner {
        OWNER_NULLABLE
    } else if requirement.caller_uncertain {
        OWNER_CALLERS_UNENUMERATED
    } else if requirement.later_run_unowned {
        OWNER_LATER_RUN_UNOWNED
    } else if requirement.component_uncertain {
        COMPONENT_IDENTITY_UNPROVEN
    } else if requirement.runtime_uncertain {
        OWNER_ALLOCATION_UNPROVEN
    } else {
        OWNER_UNPROVEN
    }
}

pub(crate) const fn for_async_read(read: &crate::AsyncRead) -> &'static str {
    if read.options_opaque {
        ASYNC_OPTIONS_OPAQUE
    } else if read.caught {
        ASYNC_READ_CAUGHT
    } else if read.component_prop {
        ASYNC_COMPONENT_PROP
    } else if read.declared_loading {
        DECLARED_LOADING_VALUE
    } else if read.host_callback_timing {
        HOST_CALLBACK_TIMING
    } else if read.callee_callback_timing {
        CALLEE_CALLBACK_TIMING
    } else if read.invocation_context_unproven {
        INVOCATION_CONTEXT_UNPROVEN
    } else if read.server_rendering_unresolved {
        SERVER_RENDERING_UNRESOLVED
    } else if read.call_attributed {
        CALL_ATTRIBUTED_READ
    } else if read.carrier_run_unproven {
        CARRIER_RUN_UNPROVEN
    } else if read.mount_unresolved {
        MOUNT_UNRESOLVED
    } else if read.creation_pass_unproven {
        CREATION_PASS_UNPROVEN
    } else if read.provenance_open {
        ASYNC_PROVENANCE_UNPROVEN
    } else {
        PROOF_STEP_UNNAMED
    }
}

/// A rule whose rows have no hole of their own: what its one proof step lacks.
pub(crate) fn for_rule(code: &str) -> &'static str {
    match code {
        "SC1003" | "SC1007" => PROPS_CALLERS_UNENUMERATED,
        "SC2003" => STORE_WRITE_UNPROVEN,
        "SC1001" => READ_BACKING_UNPROVEN,
        "SC1002" => HANDED_READ_AFTER_AWAIT,
        "SC1004" => CONDITIONAL_RETURN_UNPROVEN,
        "SC2001" | "SC2002" | "SC2004" | "SC2005" | "SC3002" => FUNCTION_RUN_UNPLACED,
        "SC2008" => HANDLER_DISPATCH_UNPROVEN,
        "SC2009" => OPTIMISTIC_REACH_UNRESOLVED,
        "SC4001" => OWNER_UNPROVEN,
        "SC5006" => LOADING_HOLD_UNPROVEN,
        "SC6001" => EFFECT_OWNER_UNPROVEN,
        "SC9005" => PACKAGE_SOURCE_UNAVAILABLE,
        _ => PROOF_STEP_UNNAMED,
    }
}

#[cfg(test)]
mod tests {
    use super::named;

    #[test]
    fn a_function_name_is_no_blocker() {
        assert!(named("function-run-unplaced"));
        assert!(named("dependency-source-getter:@solidjs/router defines it"));
        assert!(!named(""));
        assert!(!named("App"));
        assert!(!named("owned scope"));
        assert!(!named("createWorkspace"));
    }
}
