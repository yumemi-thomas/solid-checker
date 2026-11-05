//! Dialect-specific `createEffect` call semantics.
//!
//! This is the single seam for questions that depend on both the published
//! TypeScript overloads and the runtime's argument handling. Keeping the
//! classification here prevents the static API and ownership passes from
//! inventing subtly different models of the same call.

use solid_dialect::Version;
use solid_facts::FileFacts;
use solid_facts::ast::{ArgumentFact, CallFact, RuntimeValueKind};
use typefacts::{ResolvedCallValidity, RuntimeValueDomain};

use crate::Primitive;
use crate::indexes::SemanticLookup;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EffectCallSemantics {
    /// The published overload accepts the call, but the client runtime is
    /// still proven to receive no callable effect function.
    pub(crate) missing_effect_function: ProofStatus,
    /// Why the missing-function result is uncertifiable, when it is. Keeping
    /// this separate prevents diagnostics from describing argument ambiguity
    /// as a server-entry problem.
    pub(crate) missing_effect_uncertainty: EffectFunctionUncertainty,
    /// The client runtime reaches computation allocation. Solid 2 throws
    /// before allocation for an absent/nullish apply argument.
    pub(crate) owner_registration: ProofStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProofStatus {
    No,
    Proven,
    Uncertain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EffectFunctionUncertainty {
    None,
    RuntimeEntry,
    ArgumentShape,
    RuntimeEntryAndArgumentShape,
    /// The value is proven non-callable, but it fails only when a function
    /// the call returns runs, and no run is proven (ADR 0701).
    ReturnedFunctionRun,
}

#[derive(Clone, Copy)]
enum ExpandedArgument<'a> {
    Visible(&'a ArgumentFact),
    HiddenBySpread,
    Absent,
    Uncertain,
}

impl EffectFunctionUncertainty {
    pub(crate) const fn analysis_context(self) -> &'static str {
        match self {
            Self::None => "",
            Self::RuntimeEntry => "effect-runtime-entry-uncertain",
            Self::ArgumentShape => "effect-argument-shape-uncertain",
            Self::RuntimeEntryAndArgumentShape => {
                "effect-runtime-entry-and-argument-shape-uncertain"
            }
            Self::ReturnedFunctionRun => "effect-returned-function-run-unproven",
        }
    }
}

impl ProofStatus {
    fn with_server_entry_possible(self, server_entry_possible: bool) -> Self {
        if server_entry_possible && self == Self::Proven {
            Self::Uncertain
        } else {
            self
        }
    }
}

fn missing_effect_uncertainty(
    status: ProofStatus,
    server_entry_possible: bool,
) -> EffectFunctionUncertainty {
    match (status, server_entry_possible) {
        (ProofStatus::No, _) | (ProofStatus::Proven, false) => EffectFunctionUncertainty::None,
        (ProofStatus::Proven, true) => EffectFunctionUncertainty::RuntimeEntry,
        (ProofStatus::Uncertain, false) => EffectFunctionUncertainty::ArgumentShape,
        (ProofStatus::Uncertain, true) => EffectFunctionUncertainty::RuntimeEntryAndArgumentShape,
    }
}

pub(crate) fn classify_effect_call(
    file: &FileFacts,
    call: &CallFact,
    primitive: Primitive,
    lookup: &SemanticLookup<'_>,
) -> EffectCallSemantics {
    let server_entry_possible = under_server_directive(file, call);
    // ADR 0701: the effect function's slot is the dialect's answer. A
    // primitive with none shares the server-entry answer below and nothing
    // else. `createTrackedEffect(compute, options?)` has no effect slot.
    let Some(slot) = lookup.dialect.effect_function_slot(primitive) else {
        return EffectCallSemantics {
            missing_effect_function: ProofStatus::No,
            missing_effect_uncertainty: EffectFunctionUncertainty::None,
            owner_registration: ProofStatus::Proven
                .with_server_entry_possible(server_entry_possible),
        };
    };
    match lookup.dialect.version() {
        Version::V2 => {}
        // `Version::V1` survives for classification only (ADR 0110 § 1): a
        // resolved major this build carries no dialect for is refused with
        // `SC9013` before any file is analyzed, so this arm is unreachable
        // rather than dead. It is kept exhaustive on purpose -- a variant
        // added for a future major must be a compile error *here*, because
        // where that dialect puts the apply slot is a question this seam has
        // to be told rather than one it may inherit from 2.0 -- and it fails
        // closed meanwhile: an unmodelled call shape proves nothing about a
        // missing effect function or about reaching computation allocation.
        Version::V1 => {
            return EffectCallSemantics {
                missing_effect_function: ProofStatus::Uncertain,
                missing_effect_uncertainty: EffectFunctionUncertainty::ArgumentShape,
                owner_registration: ProofStatus::Uncertain,
            };
        }
    }
    let call_is_valid = lookup
        .resolved_callee_call(file, call.callee)
        .is_some_and(|resolved| resolved.validity == ResolvedCallValidity::Valid);
    // A runtime with no `undefined` guard allocates the node whatever the
    // argument is, so the owner pass keeps the call.
    let unguarded_registration =
        ProofStatus::Proven.with_server_entry_possible(server_entry_possible);
    if call.arguments.iter().any(|argument| argument.spread) {
        // A spread's static tuple type can prove where the apply slot is only
        // when Type Facts reports exactLength. Optional/rest/array/unequal
        // union shapes stay explicit uncertainty. Even an exact spread does
        // not describe a hidden slot's value, so only proven absence (or a
        // visible argument preceding/following exact spreads) is classified.
        let apply = expanded_argument(file, call, slot.index, lookup);
        let missing_effect_function = if !call_is_valid {
            ProofStatus::No
        } else {
            match apply {
                ExpandedArgument::Visible(argument) => {
                    v2_effect_function_status(file, argument, slot, lookup)
                }
                ExpandedArgument::Absent => ProofStatus::Proven,
                ExpandedArgument::HiddenBySpread | ExpandedArgument::Uncertain => {
                    ProofStatus::Uncertain
                }
            }
        };
        let owner_registration = if !slot.guards_undefined {
            unguarded_registration
        } else {
            match apply {
                ExpandedArgument::Visible(argument) => {
                    v2_owner_registration(file, argument, lookup)
                        .with_server_entry_possible(server_entry_possible)
                }
                ExpandedArgument::Absent => ProofStatus::No,
                ExpandedArgument::HiddenBySpread | ExpandedArgument::Uncertain => {
                    ProofStatus::Uncertain
                }
            }
        };
        return finish(
            missing_effect_function,
            slot,
            server_entry_possible,
            owner_registration,
        );
    }

    let apply = call.arguments.get(slot.index);
    // 2.0 retains a deprecated, type-correct one-argument `createEffect`
    // overload returning `never`, while the client runtime throws
    // MISSING_EFFECT_FN. Invalid raw arguments are owned by TypeScript and
    // therefore cannot reach this diagnostic.
    let missing_effect_function = if call_is_valid {
        apply.map_or(ProofStatus::Proven, |argument| {
            v2_effect_function_status(file, argument, slot, lookup)
        })
    } else {
        ProofStatus::No
    };
    // The shipped build reads `createEffect`'s apply argument before creating
    // the effect node: it throws on `=== undefined`, then dereferences
    // `.effect`. So absence and nullishness are proven pre-allocation throws;
    // every other bad value is allocated first and fails later.
    let owner_registration = if slot.guards_undefined {
        apply.map_or(ProofStatus::No, |argument| {
            v2_owner_registration(file, argument, lookup)
                .with_server_entry_possible(server_entry_possible)
        })
    } else {
        unguarded_registration
    };
    finish(
        missing_effect_function,
        slot,
        server_entry_possible,
        owner_registration,
    )
}

/// A proof that fails only when a function the call returns runs
/// (`createReaction`'s tracker) proves nothing at the call: uncertain, with
/// its own blocker (ADR 0701).
fn finish(
    missing_effect_function: ProofStatus,
    slot: solid_dialect::EffectFunctionSlot,
    server_entry_possible: bool,
    owner_registration: ProofStatus,
) -> EffectCallSemantics {
    if slot.fails == solid_dialect::EffectFunctionFailure::ReturnedFunctionRun
        && missing_effect_function == ProofStatus::Proven
    {
        return EffectCallSemantics {
            missing_effect_function: ProofStatus::Uncertain,
            missing_effect_uncertainty: EffectFunctionUncertainty::ReturnedFunctionRun,
            owner_registration,
        };
    }
    EffectCallSemantics {
        missing_effect_uncertainty: missing_effect_uncertainty(
            missing_effect_function,
            server_entry_possible,
        ),
        missing_effect_function: missing_effect_function
            .with_server_entry_possible(server_entry_possible),
        owner_registration,
    }
}

fn expanded_argument<'a>(
    file: &FileFacts,
    call: &'a CallFact,
    wanted: usize,
    lookup: &SemanticLookup<'_>,
) -> ExpandedArgument<'a> {
    let mut position = 0usize;
    for argument in &call.arguments {
        if !argument.spread {
            if position == wanted {
                return ExpandedArgument::Visible(argument);
            }
            position = position.saturating_add(1);
            continue;
        }
        let span = file
            .ast
            .spreads
            .iter()
            .find(|spread| spread.span == argument.span)
            .map_or(argument.span, |spread| spread.argument);
        let Some(length) = lookup
            .entity_at(file.path.as_str(), span)
            .and_then(|entity| entity.tuple_shape)
            .and_then(typefacts::TupleShape::exact_length)
            .and_then(|length| usize::try_from(length).ok())
        else {
            return if position > wanted {
                ExpandedArgument::Absent
            } else {
                ExpandedArgument::Uncertain
            };
        };
        let end = position.saturating_add(length);
        if wanted >= position && wanted < end {
            return ExpandedArgument::HiddenBySpread;
        }
        position = end;
    }
    ExpandedArgument::Absent
}

fn v2_effect_function_status(
    file: &FileFacts,
    argument: &ArgumentFact,
    slot: solid_dialect::EffectFunctionSlot,
    lookup: &SemanticLookup<'_>,
) -> ProofStatus {
    match argument.runtime_value_kind {
        // No bundle is read there: an object literal is the value called,
        // and an object literal is never callable.
        RuntimeValueKind::Object if argument.exact_object_literal && !slot.accepts_bundle => {
            if argument.runtime_type_escape {
                ProofStatus::Proven
            } else {
                ProofStatus::Uncertain
            }
        }
        RuntimeValueKind::Function => ProofStatus::No,
        RuntimeValueKind::Nullish | RuntimeValueKind::Primitive | RuntimeValueKind::Array => {
            if argument.runtime_type_escape {
                ProofStatus::Proven
            } else {
                // Without an escape this shape is ordinarily rejected by the
                // published overload. A Valid answer and the runtime shape
                // disagree, so retain the obligation without duplicating a
                // TypeScript diagnostic.
                ProofStatus::Uncertain
            }
        }
        RuntimeValueKind::Object if argument.exact_object_literal => {
            let value_span = argument.value_span.unwrap_or(argument.span);
            let effect = file.ast.object_properties.iter().rfind(|property| {
                value_span.contains(property.span)
                    && argument.property_names.contains(&property.key)
                    && file.source_text(property.key) == Some("effect")
            });
            match effect {
                None if argument.runtime_type_escape => ProofStatus::Proven,
                None => ProofStatus::Uncertain,
                Some(property) if property.value_kind == RuntimeValueKind::Function => {
                    ProofStatus::No
                }
                Some(property)
                    if property.value_kind.is_proven_noncallable()
                        && (argument.runtime_type_escape || property.runtime_type_escape) =>
                {
                    ProofStatus::Proven
                }
                Some(property) => match lookup
                    .entity_at(file.path.as_str(), property.value)
                    .and_then(|entity| entity.runtime_value_domain)
                {
                    Some(domain)
                        if only_callable(domain)
                            && !argument.runtime_type_escape
                            && !property.runtime_type_escape =>
                    {
                        ProofStatus::No
                    }
                    Some(domain)
                        if !domain.unknown()
                            && !domain.may_be_callable()
                            && (argument.runtime_type_escape || property.runtime_type_escape) =>
                    {
                        ProofStatus::Proven
                    }
                    _ => ProofStatus::Uncertain,
                },
            }
        }
        RuntimeValueKind::Object | RuntimeValueKind::Unknown => {
            match runtime_value_domain(file, argument, lookup) {
                Some(domain) if only_callable(domain) && !argument.runtime_type_escape => {
                    ProofStatus::No
                }
                _ => ProofStatus::Uncertain,
            }
        }
    }
}

fn runtime_value_domain(
    file: &FileFacts,
    argument: &ArgumentFact,
    lookup: &SemanticLookup<'_>,
) -> Option<RuntimeValueDomain> {
    let runtime_span = argument.value_span.unwrap_or(argument.span);
    lookup
        .entity_at(file.path.as_str(), runtime_span)
        .and_then(|entity| entity.runtime_value_domain)
}

fn only_callable(domain: RuntimeValueDomain) -> bool {
    domain.may_be_callable()
        && !domain.may_be_undefined()
        && !domain.may_be_other()
        && !domain.unknown()
}

fn v2_owner_registration(
    file: &FileFacts,
    argument: &ArgumentFact,
    lookup: &SemanticLookup<'_>,
) -> ProofStatus {
    match argument.runtime_value_kind {
        RuntimeValueKind::Nullish => ProofStatus::No,
        RuntimeValueKind::Primitive
        | RuntimeValueKind::Function
        | RuntimeValueKind::Object
        | RuntimeValueKind::Array => ProofStatus::Proven,
        RuntimeValueKind::Unknown => {
            // Allocation precedes every failure except the exact nullish
            // guard. For a non-literal value, only compiler-proven
            // callability rules null/undefined out. Query the peeled runtime
            // expression when an assertion is present: the wrapper's
            // apparent callable type is not evidence about the value that is
            // actually passed.
            let runtime_span = argument.value_span.unwrap_or(argument.span);
            if lookup
                .entity_at(file.path.as_str(), runtime_span)
                .and_then(|entity| entity.runtime_value_domain)
                .is_some_and(|domain| only_callable(domain) && !argument.runtime_type_escape)
            {
                ProofStatus::Proven
            } else {
                ProofStatus::Uncertain
            }
        }
    }
}

/// Whether a `use server` directive covers this call.
///
/// Neither dialect's core package reads this directive -- `grep` finds it
/// nowhere in `solid-js@1.9.14` or `solid-js@2.0.0-rc.0`. It is a framework
/// and bundler convention: SolidStart extracts the annotated body into a
/// server module, and the server export condition then resolves `solid-js` to
/// its server entry. So the directive never *proves* server mode, in either
/// dialect.
///
/// What it does establish is doubt because the client and server entries
/// resolve differently, which is why this is not dialect-specific. 1.9.14's
/// server `createEffect` is a bare no-op (`dist/server.js`: `function
/// createEffect(fn, value) {}`), and 2.0.0-rc.0 routes to `serverEffect`, which
/// ignores the apply argument and emits no lifecycle diagnostic at all
/// (`dist/server.js`; the file contains no `emitDiagnostic` call). A
/// client-runtime claim -- MISSING_EFFECT_FN, or an ownerless effect -- is
/// therefore uncertifiable until a project/compiler fact proves which entry
/// executes. A claim absent on both paths remains absent.
fn under_server_directive(file: &FileFacts, call: &CallFact) -> bool {
    file.ast
        .module_directives
        .iter()
        .any(|directive| directive.value == "use server")
        || file.ast.functions.iter().any(|function| {
            function.body.contains(call.span) && function.has_directive("use server")
        })
}
