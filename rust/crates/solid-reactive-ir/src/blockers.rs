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

/// ADR 0852: a throw rejects an async stepper's flight, whose error propagation
/// to a read or error boundary is not proved.
pub(crate) const ASYNC_FLIGHT_ERROR_UNOBSERVED: &str = "async-flight-error-unobserved";
/// ADR 0893: a plain throw ends stored on a computation, or in a `try` or a
/// handling boundary, and no read that reports it is proven.
pub(crate) const COMPUTATION_ERROR_UNOBSERVED: &str = "computation-error-unobserved";

// Track W.
pub(crate) const SOLID_VERSION_UNCLASSIFIED: &str = "solid-version-unclassified";
pub(crate) const RESPONSE_HEAD_POSITION_UNPROVEN: &str = "response-head-position-unproven";
pub(crate) const SERVER_FUNCTION_VALUE_HANDED: &str = "server-function-value-handed";
pub(crate) const LEAF_OPERATION_IDENTITY_UNPROVEN: &str = "leaf-operation-identity-unproven";
pub(crate) const OPTIMISTIC_REACH_UNRESOLVED: &str = "optimistic-reach-unresolved";
pub(crate) const PROVIDER_NOT_PROVEN_ABOVE: &str = "provider-not-proven-above";
/// ADR 0949: a finding the ADR 0284 drop discards in a package, reported at the project site that enters the package code
/// (`package-finding-at-entry:<code>`), or at the project's import of the package when no entry is proven.
pub const PACKAGE_FINDING_AT_ENTRY: &str = "package-finding-at-entry";
/// ADR 0949: a signal setter a project hands to a package function in a shape no walk reads (inside an object
/// literal or an array, as a JSX attribute): the package may call it, and the owned scope that runs the package
/// code may not write a signal. SC2001's own obligation at the handing site, the rule the runtime code
/// (`REACTIVE_WRITE_IN_OWNED_SCOPE`) is claimed by.
pub(crate) const SETTER_HANDED_TO_PACKAGE: &str = "setter-handed-to-package";
/// ADR 0949: a reactive accessor a project hands to a package function in a shape no walk reads: the package may
/// call it, and where nothing tracks the call Solid logs `STRICT_READ_UNTRACKED`. SC1001's own obligation at the
/// handing site, the rule that code is claimed by.
pub(crate) const ACCESSOR_HANDED_TO_PACKAGE: &str = "accessor-handed-to-package";
/// ADR 0949: a store a project hands to a package function in a shape no walk reads: the package may write it
/// (`STORE_WRITE_OUTSIDE_SETTER`, SC2003) or read it untracked (SC1001).
pub(crate) const STORE_HANDED_TO_PACKAGE: &str = "store-handed-to-package";
/// ADR 0949: an action a project hands to a package function in a shape no walk reads: the package may call it in
/// an owned scope (`ACTION_CALLED_IN_OWNED_SCOPE`, SC2002).
pub(crate) const ACTION_HANDED_TO_PACKAGE: &str = "action-handed-to-package";
/// ADR 0949, addendum 6: a project site in a leaf owner's callback that may run package code which does something a leaf
/// owner refuses (`onCleanup`, a creating call, `flush`) in a shape the call walk does not follow (a `new`, a getter, a
/// reference), or in an effect's apply callback that may run package code that flushes or creates a computation, or
/// after an await of an async tracked computation that may run package code that reads a reactive source: the claiming
/// rule's own obligation at the site.
pub(crate) const PACKAGE_OPERATION_IN_EFFECT_APPLY: &str = "package-operation-in-effect-apply";
pub(crate) const PACKAGE_OPERATION_AFTER_AWAIT: &str = "package-operation-after-await";
/// ADR 0959: the callback of a primitive that catches what its callback throws (`isPending`) is async, so its synchronous
/// throw is a rejection the primitive does not catch: what the observer guard does there is an obligation.
pub(crate) const ASYNC_SLOT_UNMODELLED: &str = "async-slot-unmodelled";
/// ADR 0961: the call is written in a generator that an effect's apply (or an action's step) may step, and the walk follows
/// the stepping only to the call that creates the iterator: whether the apply runs this body, and how far, is not proven.
pub(crate) const GENERATOR_STEP_UNPLACED: &str = "generator-step-unplaced";
/// ADR 0961 (PD): the site is written in a parameter initializer (a default value, a destructuring default, an IIFE, a closure or a
/// helper call in one) of a function that is called or referenced in the program. The initializer runs when a call of the function
/// omits the argument, so it may run wherever that function's calls do, and no walk here reads which calls omit it.
pub(crate) const PARAMETER_INITIALIZER_UNPLACED: &str = "parameter-initializer-unplaced";
/// ADR 0963: the site is written directly in a class field's or static block's initializer, which runs where an instance is
/// constructed (or the class is evaluated), and no call edge shows that construction: the exact witness (a `new` of the class,
/// of a subclass, or of an unresolved class) is a later proof target, so the site may run wherever one is.
pub(crate) const FIELD_INITIALIZER_UNPLACED: &str = "field-initializer-unplaced";
/// ADR 0964: the call invokes a function value the walk cannot resolve (a parameter, a stored callback, a member of a local value that
/// names no function body): whatever function the program stored there runs on the stack of the call, and may read a source there.
pub(crate) const UNRESOLVED_INVOCATION: &str = "unresolved-invocation";
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
/// ADR 0886: no row and no named proof ended the site's execution context.
pub(crate) const EXECUTION_CONTEXT_UNACCOUNTED: &str = "execution-context-unaccounted";
pub(crate) const ASYNC_OPTIONS_OPAQUE: &str = "async-options-opaque";
/// ADR 0936: a site of the universe (`proof_universe.rs`) that no row, census
/// obligation, compiler discard or named silence ends, one blocker per family
/// as `async-read-unaccounted` and `execution-context-unaccounted` are for
/// theirs. A strict read (a call of an accessor, a Get of a store or props root).
pub(crate) const STRICT_READ_UNACCOUNTED: &str = "strict-read-unaccounted";
/// ADR 0936: a call of a setter.
pub(crate) const WRITE_UNACCOUNTED: &str = "write-unaccounted";
/// ADR 0936: a call of an action.
pub(crate) const ACTION_UNACCOUNTED: &str = "action-unaccounted";
/// ADR 0936: an assignment, `delete` or in-place array method through a store
/// or props root.
pub(crate) const STORE_WRITE_UNACCOUNTED: &str = "store-write-unaccounted";
/// ADR 0936: a call of `onFlush`.
pub(crate) const ON_FLUSH_UNACCOUNTED: &str = "on-flush-unaccounted";
/// Batch 11, slice 4: a setter call whose source may be an optimistic one that has not answered (or whose primitive is not resolved) with no SC2009
/// row, SC9012, discard or named silence: the code's own uncertifiable row.
pub(crate) const OPTIMISTIC_WRITE_UNACCOUNTED: &str = "optimistic-write-unaccounted";
/// ADR 0838: a pending rule's claim at a read no producer accounted for: no
/// row of that rule, no SC9012 at the read, no recorded reason.
pub(crate) const ASYNC_READ_UNACCOUNTED: &str = "async-read-unaccounted";
/// ADR 0926: SC5003's or SC5008's claim at a read a pending source's census
/// walk met where an SC9012 row holds it: no row follows the value to the
/// read, so nothing places it for a code that has no position of its own.
pub(crate) const PENDING_READ_UNRESOLVED: &str = "pending-read-unresolved";
/// ADR 0891: the same claim, where the source's initial value comes from an
/// import nothing resolves: `createSignal(fromAbsentModule())` may be a
/// function, and its compute anything.
pub(crate) const SOURCE_MODULE_UNRESOLVED: &str = "source-module-unresolved";
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
/// ADR 0803: the program writes the array iterator visibly, so a primitive's
/// array pattern may not hold the primitive's own slots.
pub(crate) const TUPLE_ITERATION_WRITTEN: &str = "tuple-iteration-written";
pub(crate) const CREATED_ACCESSOR_READ_UNPROVEN: &str = "created-accessor-read-unproven";

// `value_use.rs`: where a proven source leaves what the analysis follows.
/// ADR 0884: a call whose compiler-resolved declaration is a dialect primitive,
/// through a name no primitive rule resolves.
pub(crate) const PRIMITIVE_ALIAS_CALL: &str = "primitive-alias-call";
pub(crate) const VALUE_HELD_IN_BINDING: &str = "value-held-in-binding";
pub(crate) const VALUE_HELD_IN_MEMBER: &str = "value-held-in-member";
pub(crate) const VALUE_THROUGH_LOGICAL: &str = "value-through-logical";
pub(crate) const VALUE_IN_CLASS_FIELD: &str = "value-in-class-field";
pub(crate) const VALUE_THROUGH_GENERATOR: &str = "value-through-generator";
pub(crate) const VALUE_THROUGH_AWAIT: &str = "value-through-await";
pub(crate) const VALUE_THROUGH_NEW: &str = "value-through-new";
pub(crate) const VALUE_AS_PARAMETER_DEFAULT: &str = "value-as-parameter-default";
pub(crate) const VALUE_IN_CARRIER: &str = "value-in-carrier";
pub(crate) const VALUE_TO_UNRESOLVED_CALLEE: &str = "value-to-unresolved-callee";
pub(crate) const VALUE_FROM_PRIMITIVE_CALLBACK: &str = "value-from-primitive-callback";
pub(crate) const VALUE_BOUND_HELD: &str = "value-bound-held";
pub(crate) const VALUE_FORWARDED: &str = "value-forwarded";
pub(crate) const VALUE_ROOT_IN_HELPER: &str = "value-root-in-helper";
pub(crate) const VALUE_USE_UNCLASSIFIED: &str = "value-use-unclassified";
/// Review item 10: a source that may be pending, a bare child of an element,
/// which inserts and calls it: no row reads it there.
pub(crate) const VALUE_PENDING_IN_JSX: &str = "value-pending-in-jsx";
/// `export default` of a literal or a call: no binding names it, so no importer
/// is walked (W's census review H1).
pub(crate) const VALUE_EXPORTED_UNNAMED: &str = "value-exported-unnamed";
/// ADR 0923: a thrown value, where the `catch` that binds it is not the
/// throwing function's own (its callers' are not walked), or binds a pattern.
pub(crate) const VALUE_THROWN: &str = "value-thrown";
pub(crate) const RUNTIME_CONFIGURATION_PREMISE: &str = "runtime-configuration-premise";
/// ADR 0694: a store Get that only leads to a write, in a window no read in
/// that body is proven in. `via` carries it: the row reads as through this.
pub(crate) const WRITE_PATH_READ_UNPROVEN: &str = "write-path-read-unproven";
pub(crate) const WRITE_PATH_VIA: &str = "a write's path";
/// ADR 0820: a generator's body past its first suspension, on a stepping the
/// walk does not prove.
pub(crate) const GENERATOR_STEPPED: &str = "generator-stepped";
/// ADR 0822: a Get of a value a selected package produces where the walk over
/// its source ends: a class, a proxy, a call nothing resolves, an accessor
/// installed at run time.
pub(crate) const DEPENDENCY_VALUE_UNFOLLOWED: &str = "dependency-value-unfollowed";
/// ADR 0822: importing a package installs a function on a global, and that
/// function may read a source wherever it is called.
pub(crate) const DEPENDENCY_GLOBAL_READ: &str = "dependency-global-read";
/// ADR 0823: a prop read in a leaf owner (createTrackedEffect, onSettled)
/// whose callers do not prove the value is no pending async getter: an
/// unseen caller, or a visible one passing a value nothing classifies.
pub(crate) const PROP_PENDING_UNPROVEN: &str = "prop-pending-unproven";
/// ADR 0825: the producer attributes a Solid primitive to a value no import
/// confirms (ADR 0711): which function it is, and so what reaches it, is open.
pub(crate) const PRIMITIVE_IDENTITY_UNPROVEN: &str = "primitive-identity-unproven";
/// ADR 0827: a runtime use of an import the compiler did not resolve (a
/// missing module, an export its target lacks): what the value is, and so
/// what any call, read, tag or write through it does, is unknown.
pub(crate) const IMPORT_UNRESOLVED: &str = "import-unresolved";
/// ADR 0829: a store `delete` whose operand is neither proven optional and
/// writable nor proven rejected by TypeScript (a computed or index key, a
/// required `T | undefined`, `unknown`): whether `tsc` accepts it, and so
/// whether it is this rule's, is open.
pub(crate) const DELETE_OPERAND_UNPROVEN: &str = "delete-operand-unproven";
/// ADR 0830: a call of a Solid primitive TypeScript rejects (an absent or
/// non-callable apply, a cleanup that is no function, a target `refresh` or
/// `affects` refuses) on a line a `@ts-ignore`, `@ts-expect-error` or
/// `@ts-nocheck` may silence, or in JavaScript `tsc` may not check. Nothing
/// may report it, and the runtime may throw or warn there.
pub(crate) const SOLID_CALL_REJECTED_UNCHECKED: &str = "solid-call-rejected-unchecked";
/// ADR 0703: an `httpStatus`/`httpHeader` call below a `Loading` boundary in a
/// project that server-renders: whether the boundary settles before the shell
/// flush (the head applies) or after it (the call is dropped) is open.
pub(crate) const BOUNDARY_SETTLE_ORDER_UNPROVEN: &str = "boundary-settle-order-unproven";
/// ADR 0427: an export of a module-level `"use server"` file whose value no
/// fact shows to be a function (a call of a project wrapper): registration
/// throws at server boot unless it is one.
pub(crate) const SERVER_EXPORT_VALUE_UNPROVEN: &str = "server-export-value-unproven";
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
    if read.via.as_ref() == WRITE_PATH_VIA {
        WRITE_PATH_READ_UNPROVEN
    } else if read.execution == crate::ExecutionRole::Unknown {
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
    if requirement.flight_error_unobserved {
        ASYNC_FLIGHT_ERROR_UNOBSERVED
    } else if requirement.computation_error_unobserved {
        COMPUTATION_ERROR_UNOBSERVED
    } else if requirement.context_unaccounted {
        EXECUTION_CONTEXT_UNACCOUNTED
    } else if requirement.provider_open
        && requirement.through_dependency.is_some()
        && matches!(
            requirement.operation,
            crate::OwnerRequirementOperation::ContextProvider
        )
    {
        // ADR 0943: a package's context read by the package's own code, reached at
        // this call: no provider is proven above it (the project reads of the
        // same context are `StaticViolation`s with this blocker).
        PROVIDER_NOT_PROVEN_ABOVE
    } else if requirement.unplaced_callback {
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
    if read.source_module_unresolved {
        SOURCE_MODULE_UNRESOLVED
    } else if read.unaccounted && read.read_unresolved {
        PENDING_READ_UNRESOLVED
    } else if read.unaccounted {
        ASYNC_READ_UNACCOUNTED
    } else if read.options_opaque {
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
    } else if read.settle_unproven {
        // ADR 0518: a top-level `await` may settle the source before the
        // read; the last hole, so a row with an earlier one keeps its name.
        SOURCE_PENDING_UNPROVEN
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
        "SC6001" | "SC6002" => EFFECT_OWNER_UNPROVEN,
        "SC9005" => PACKAGE_SOURCE_UNAVAILABLE,
        _ => PROOF_STEP_UNNAMED,
    }
}

/// ADR 0915: host inference read an input that exists and could not be
/// observed (a dangling symlink, a permission error, a package manifest that
/// does not parse). Emitted by the backend (`check::fail_closed`).
pub(crate) const HOST_INPUT_UNREADABLE: &str = "host-input-unreadable";

/// ADR 0840: a standard method of a census member on a receiver no fact
/// proves real, while the program may put a function at that member of an
/// object of its own.
pub(crate) const STANDARD_MEMBER_IMPOSTOR: &str = "standard-member-impostor";

/// Whether `analysis_context` names one of ADR 0949's handed-value claims (`*-handed-to-package`): an uncertifiable row
/// the backend drops where a violation of the same rule stands at the same site.
#[must_use]
pub fn is_package_handed_claim(analysis_context: &str) -> bool {
    matches!(
        analysis_context,
        SETTER_HANDED_TO_PACKAGE
            | ACCESSOR_HANDED_TO_PACKAGE
            | STORE_HANDED_TO_PACKAGE
            | ACTION_HANDED_TO_PACKAGE
    )
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
