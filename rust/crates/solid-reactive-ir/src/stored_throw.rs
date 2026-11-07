//! ADR 0893: where a plain throw ends.
//!
//! rc.21 has two kinds of runtime failure. A coded diagnostic is emitted at
//! the call, whatever happens to the throw after it. A plain `throw new Error`
//! (`resolve`, `until`, a static `dynamic` source, `useContext`) says nothing
//! until something reports the error object: an effect compute logs it, a
//! render effect halts on it, a component body fails the render. A memo and
//! the other computes that store their result keep it on the node instead
//! (`recompute`'s catch, `notifyStatus`), and a read rethrows it. So a
//! violation of those rules needs a run whose throw reaches a report: the
//! compute's own, or a read that is itself reported.
//!
//! The walk is per entry: one entry that reports keeps the violation,
//! whatever the function's other entries are. What it cannot follow is
//! [`ThrowEnd::Open`], which changes nothing.

use solid_dialect::{Execution, Primitive};
use solid_facts::FileFacts;
use solid_facts::ast::{CallFact, FunctionFact, JsxElementFact};
use solid_facts::compiler::CallbackRoleKind;
use solid_facts::core::Span;

use crate::ExecutionRole;
use crate::execution_role::{Reach, SiteReach};
use crate::indexes::ComponentStatus;
use crate::local_access::HandOff;
use crate::static_api::StaticApiContext;

/// Compute reads followed, and calls walked out of a function, per question.
const THROW_DEPTH: usize = 6;
/// Component mounts followed to a boundary above.
const MOUNT_DEPTH: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ThrowEnd {
    /// One entry's throw reaches a report.
    Reported,
    /// The entries read end in a stored error no proven read reports, a
    /// `try`, a handling boundary, or a bound.
    Unobserved,
    /// Nothing read decides.
    Open,
}

/// When the site throws.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Thrower {
    /// On every run (`useContext`, a static `dynamic` source).
    Always,
    /// Only with an observer active (`resolve`, `until`): a run in a position
    /// with none does not throw.
    UnderObserver,
}

pub(crate) fn throw_end(
    file: &FileFacts,
    site: Span,
    allowed: &[Span],
    context: &StaticApiContext<'_>,
    thrower: Thrower,
) -> ThrowEnd {
    let walk = Walk {
        allowed,
        context,
        thrower,
    };
    match walk.site(file, site, THROW_DEPTH) {
        // Nothing followed reaches the site, and it is written in a compute
        // that stores its error: that compute is the one run in view.
        ThrowEnd::Open if walk.inside_unread_compute(file, site, THROW_DEPTH) => {
            ThrowEnd::Unobserved
        }
        end => end,
    }
}

/// ADR 0893: the requirements whose claim is a plain throw: an owner proof
/// proves the throw, not that anything reports it.
pub(crate) fn mark_owner_requirements(
    ctx: &crate::pipeline::AnalysisContext<'_>,
    draft: &mut crate::pipeline::ProgramDraft,
) {
    use crate::OwnerRequirementOperation as Operation;
    let context = crate::static_api::context_of(ctx);
    let lookup = context.lookup;
    let mut allowed = std::collections::HashMap::new();
    let mut unobserved = |location: &crate::Location, thrower: Thrower| {
        let Some(file) = lookup.file_by_path(location.path.as_ref()) else {
            return false;
        };
        let (Ok(start), Ok(end)) = (
            u32::try_from(location.start_byte),
            u32::try_from(location.end_byte),
        ) else {
            return false;
        };
        let allowed = allowed
            .entry(file.path.to_string())
            .or_insert_with(|| crate::execution_role::allowed_callback_spans(file, lookup));
        throw_end(file, Span::new(start, end), allowed, &context, thrower) == ThrowEnd::Unobserved
    };
    for requirement in &mut draft.missing_owners {
        if !requirement.uncertain
            && matches!(
                requirement.operation,
                Operation::ContextRead | Operation::ContextProvider
            )
            && unobserved(&requirement.location, Thrower::Always)
        {
            requirement.uncertain = true;
            requirement.computation_error_unobserved = true;
        }
    }
    // ADR 0641's tracked-effect row of `resolve` and `until`: the same throw.
    for operation in &mut draft.leaf_operations {
        if !operation.uncertain
            && operation.observer_guarded()
            && unobserved(&operation.location, Thrower::UnderObserver)
        {
            operation.uncertain = true;
            operation.computation_error_unobserved = true;
        }
    }
}

struct Walk<'w, 'c> {
    allowed: &'w [Span],
    context: &'w StaticApiContext<'c>,
    thrower: Thrower,
}

impl Walk<'_, '_> {
    fn site(&self, file: &FileFacts, site: Span, depth: usize) -> ThrowEnd {
        let Some(depth) = depth.checked_sub(1) else {
            return ThrowEnd::Unobserved;
        };
        let lookup = self.context.lookup;
        let observer = self.thrower == Thrower::UnderObserver;
        if observer && crate::execution_role::past_suspension(file, site, lookup).is_some() {
            return ThrowEnd::Open;
        }
        let climb = crate::execution_role::climb_in_place_layers(file, site, lookup);
        // A climb that stopped proves nothing about what it did not read.
        if climb.as_ref().is_some_and(|climb| climb.exhausted) {
            return ThrowEnd::Unobserved;
        }
        let climb = climb.filter(|climb| !climb.in_list);
        if observer
            && climb
                .as_ref()
                .is_some_and(|climb| climb.suspended.is_some())
        {
            return ThrowEnd::Open;
        }
        let at = climb.map_or(site, |climb| climb.at);
        if caught(file, site) || caught(file, at) {
            return ThrowEnd::Unobserved;
        }
        if let Some((container, index)) = file.ast.arguments_containing(at).find(|(call, index)| {
            crate::execution_role::direct_callback_runs(file, call.arguments[*index].span, at)
        }) {
            return self.argument(file, container, index, at, depth);
        }
        let function = crate::owners::containing_body_function(&file.ast, at);
        if self.tracked_jsx(file, at, function) {
            return if handled_above(file, at, lookup, MOUNT_DEPTH) {
                ThrowEnd::Unobserved
            } else {
                ThrowEnd::Reported
            };
        }
        let Some(function) = function else {
            return if observer {
                ThrowEnd::Open
            } else {
                ThrowEnd::Reported
            };
        };
        if file.compiler.callback_roles.iter().any(|callback| {
            callback.role == CallbackRoleKind::EventHandler && callback.span == function.span
        }) {
            return if observer {
                ThrowEnd::Open
            } else {
                ThrowEnd::Reported
            };
        }
        if lookup.function_component_status(file, function) == ComponentStatus::Proven {
            return match (observer, handled_above(file, at, lookup, MOUNT_DEPTH)) {
                (true, _) => ThrowEnd::Open,
                (false, true) => ThrowEnd::Unobserved,
                (false, false) => ThrowEnd::Reported,
            };
        }
        self.entries(file, function, depth)
    }

    fn tracked_jsx(&self, file: &FileFacts, at: Span, function: Option<&FunctionFact>) -> bool {
        let same_function = |region: Span| {
            crate::owners::containing_body_function(&file.ast, region).map(|owner| owner.span)
                == function.map(|owner| owner.span)
        };
        file.compiler
            .tracked_regions
            .iter()
            .any(|region| region.span.contains(at) && same_function(region.span))
            && crate::execution_role::semantic_execution_role(
                file,
                at,
                self.allowed,
                self.context.entities,
                self.context.symbol_names,
                self.context.lookup,
            ) == ExecutionRole::TrackedJsx
    }

    /// The function is an argument of `container`, at `index`.
    fn argument(
        &self,
        file: &FileFacts,
        container: &CallFact,
        index: usize,
        at: Span,
        depth: usize,
    ) -> ThrowEnd {
        let lookup = self.context.lookup;
        let observer = self.thrower == Thrower::UnderObserver;
        let Some(primitive) = lookup.primitive_at_call(file, container.span) else {
            return match crate::owners::containing_body_function(&file.ast, at) {
                Some(function) => self.entries(file, function, depth),
                None => ThrowEnd::Open,
            };
        };
        let dialect = lookup.dialect;
        if dialect.callback_swallows_throw(primitive, index) {
            return ThrowEnd::Unobserved;
        }
        if dialect.inline_callback_keeps_observer(primitive, index) {
            return self.site(file, container.span, depth);
        }
        let execution =
            crate::owners::callback_execution_at_call(file, container, primitive, index, lookup);
        let deferred = dialect.runs_callback_deferred(primitive);
        match execution {
            Some(Execution::Tracked) if !deferred => {
                if !dialect.compute_throw_reports(primitive, index) {
                    self.compute(file, container, depth)
                } else if dialect.compute_throw_logged(primitive) {
                    if effect_error_arm_open(file, container) {
                        ThrowEnd::Unobserved
                    } else {
                        ThrowEnd::Reported
                    }
                } else if handled_above(file, container.span, lookup, MOUNT_DEPTH) {
                    // R34: this compute escalates to the boundary above.
                    ThrowEnd::Unobserved
                } else {
                    ThrowEnd::Reported
                }
            }
            Some(Execution::Inline) if !(observer && deferred) => {
                self.site(file, container.span, depth)
            }
            _ => ThrowEnd::Open,
        }
    }

    fn inside_unread_compute(&self, file: &FileFacts, site: Span, depth: usize) -> bool {
        let lookup = self.context.lookup;
        file.ast.functions_owning(site).any(|function| {
            file.ast
                .arguments_containing(function.span)
                .filter(|(call, index)| {
                    file.ast.peel_ts_sugar_span(call.arguments[*index].span) == function.span
                })
                .any(|(call, index)| {
                    lookup
                        .primitive_at_call(file, call.span)
                        .is_some_and(|primitive| {
                            crate::owners::callback_execution_at_call(
                                file, call, primitive, index, lookup,
                            ) == Some(Execution::Tracked)
                                && !lookup.dialect.runs_callback_deferred(primitive)
                                && !lookup.dialect.compute_throw_reports(primitive, index)
                                && self.compute(file, call, depth) == ThrowEnd::Unobserved
                        })
                })
        })
    }

    /// A function that is only referenced here. If it runs, nothing read says
    /// it runs at this position, or that what runs it lets the throw through:
    /// a compute here still decides, and a report does not follow (R36).
    fn reference(&self, file: &FileFacts, reference: Span, depth: usize) -> ThrowEnd {
        match self.site(file, reference, depth) {
            ThrowEnd::Reported => ThrowEnd::Unobserved,
            end => end,
        }
    }

    fn scheduler_reaches_host(&self, file: &FileFacts, reference: Span) -> bool {
        let lookup = self.context.lookup;
        file.ast
            .arguments_containing(reference)
            .find(|(call, index)| {
                file.ast.peel_ts_sugar_span(call.arguments[*index].span) == reference
            })
            .and_then(|(call, _)| lookup.resolved_callee_call(file, call.callee))
            .is_some_and(crate::runtime_semantics::scheduler_throw_reaches_host)
    }

    /// A `const` alias of the function: `const compute = inner`.
    fn alias_end(&self, file: &FileFacts, reference: Span, depth: usize) -> Option<ThrowEnd> {
        let binding = file
            .ast
            .bindings_with_peeled_initializer(reference)
            .find(|binding| {
                binding.immutable && binding.shape == solid_facts::ast::BindingShape::Identifier
            })?;
        let name = binding.names.first()?;
        let mut end = ThrowEnd::Open;
        for identifier in file.ast.reference_identifiers_to(name.span) {
            let at = identifier.span;
            let entry = file
                .ast
                .arguments_containing(at)
                .find(|(call, index)| {
                    file.ast.peel_ts_sugar_span(call.arguments[*index].span) == at
                })
                .filter(|(call, _)| {
                    self.context
                        .lookup
                        .primitive_at_call(file, call.span)
                        .is_some()
                })
                .map_or_else(
                    || self.reference(file, at, depth),
                    |(call, index)| self.argument(file, call, index, at, depth),
                );
            match entry {
                ThrowEnd::Reported => return Some(ThrowEnd::Reported),
                ThrowEnd::Unobserved => end = ThrowEnd::Unobserved,
                ThrowEnd::Open => {}
            }
        }
        Some(end)
    }

    /// The compute `creation` stores its error: reported where a proven read
    /// is.
    fn compute(&self, file: &FileFacts, creation: &CallFact, depth: usize) -> ThrowEnd {
        let Some(accessor) = crate::execution_role::computation_accessor(file, creation) else {
            return ThrowEnd::Unobserved;
        };
        let lookup = self.context.lookup;
        let reads = || {
            file.ast
                .reference_identifiers_to(accessor.span)
                .filter(|identifier| {
                    file.ast.calls.iter().any(|call| {
                        !call.construct
                            && file.ast.peel_ts_sugar_span(call.callee) == identifier.span
                    })
                })
        };
        if reads().any(|read| self.read_reported(file, read.span, depth)) {
            return ThrowEnd::Reported;
        }
        // ADR 0576 keeps a lazy compute no read runs: nothing throws there.
        let lazy = creation.arguments.first().is_some_and(|compute| {
            let compute = file.ast.peel_ts_sugar_span(compute.span);
            crate::execution_role::lazy_creation(file, compute, lookup).is_some()
        });
        let read_runs =
            reads().any(|read| SiteReach::new(lookup).site(file, read.span) == Reach::MayRun);
        if lazy && !read_runs {
            return ThrowEnd::Open;
        }
        ThrowEnd::Unobserved
    }

    /// A direct call of a stored compute's accessor rethrows. A read rethrows
    /// on every run, observer or not.
    fn read_reported(&self, file: &FileFacts, read: Span, depth: usize) -> bool {
        let lookup = self.context.lookup;
        SiteReach::new(lookup).site(file, read) == Reach::MayRun
            && crate::execution_role::past_suspension(file, read, lookup).is_none()
            && !in_component_getter(file, read, lookup)
            && Walk {
                allowed: self.allowed,
                context: self.context,
                thrower: Thrower::Always,
            }
            .site(file, read, depth)
                == ThrowEnd::Reported
    }

    /// Per entry: one that reports decides.
    fn entries(&self, file: &FileFacts, function: &FunctionFact, depth: usize) -> ThrowEnd {
        let lookup = self.context.lookup;
        // A literal with no binding has no value to be current (ADR 0857).
        let rebound = crate::owners::function_binding_name(file, function).is_some()
            && !lookup.function_value_is_current(file, function);
        if function.generator || rebound {
            return self.reference(file, function.span, depth);
        }
        // A literal called where it is written.
        if let Some(call) = file.ast.calls.iter().find(|call| {
            !call.construct && file.ast.peel_ts_sugar_span(call.callee) == function.span
        }) {
            return self.site(file, call.span, depth);
        }
        let hand_offs = crate::local_access::function_reference_hand_offs(file, function, lookup)
            .unwrap_or_else(|| {
                vec![(
                    function.span,
                    crate::local_access::hand_off_at(file, function.span, lookup),
                )]
            });
        let mut end = ThrowEnd::Open;
        for (reference, hand_off) in hand_offs {
            let entry = match hand_off {
                HandOff::Call => file
                    .ast
                    .calls
                    .iter()
                    .find(|call| {
                        call.direct_callee
                            && !call.construct
                            && file.ast.peel_ts_sugar_span(call.callee) == reference
                    })
                    .map_or(ThrowEnd::Unobserved, |call| {
                        self.site(file, call.span, depth)
                    }),
                HandOff::Inline(carrier) => self.site(file, carrier, depth),
                // Only a timer's and a microtask's throw is past the program;
                // a promise handler's may be a `catch` or a `try` away (R37).
                HandOff::FreshStack if !self.scheduler_reaches_host(file, reference) => {
                    ThrowEnd::Unobserved
                }
                HandOff::FreshStack if self.thrower == Thrower::Always => ThrowEnd::Reported,
                HandOff::FreshStack => ThrowEnd::Open,
                _ => file
                    .ast
                    .arguments_containing(reference)
                    .find(|(call, index)| {
                        file.ast.peel_ts_sugar_span(call.arguments[*index].span) == reference
                    })
                    .filter(|(call, _)| lookup.primitive_at_call(file, call.span).is_some())
                    .map_or_else(
                        || {
                            self.alias_end(file, reference, depth)
                                .unwrap_or_else(|| self.reference(file, reference, depth))
                        },
                        |(call, index)| self.argument(file, call, index, reference, depth),
                    ),
            };
            match entry {
                ThrowEnd::Reported => return ThrowEnd::Reported,
                ThrowEnd::Unobserved => end = ThrowEnd::Unobserved,
                ThrowEnd::Open => {}
            }
        }
        end
    }
}

fn caught(file: &FileFacts, span: Span) -> bool {
    file.ast
        .try_catches
        .iter()
        .any(|caught| caught.block.contains(span))
}

/// Whether an effect's error may go to an `error` arm rather than the log:
/// any second argument but a function, which is the apply.
fn effect_error_arm_open(file: &FileFacts, call: &CallFact) -> bool {
    use solid_dialect::OptionLiteral;
    let Some(second) = call.arguments.get(1) else {
        return false;
    };
    if file
        .ast
        .function_at(file.ast.peel_ts_sugar_span(second.span))
        .is_some()
    {
        return false;
    }
    crate::call_option_literal(file, call, 1, "error") != OptionLiteral::Absent
}

/// `Errored` logs only a function fallback with no parameters or a value
/// (`solid-js` `dist/solid.dev.js:1692-1698`): one that takes the error is
/// the program's own handling, and one no syntax decides is not proven to log.
fn fallback_handles(file: &FileFacts, element: &JsxElementFact) -> bool {
    use solid_facts::ast::JsxAttributeValueKind as Kind;
    if !element.spreads.is_empty() {
        return true;
    }
    let Some(fallback) = element
        .attributes
        .iter()
        .find(|attribute| file.source_text(attribute.name) == Some("fallback"))
    else {
        return false;
    };
    let Some(expression) = fallback.expression else {
        return false;
    };
    let value = file.ast.peel_ts_sugar_span(expression);
    if let Some(function) = file.ast.function_at(value) {
        return !function.parameters.is_empty() || function.rest_parameter;
    }
    !matches!(
        fallback.value_kind,
        Kind::Element | Kind::Fragment | Kind::String
    ) && !file
        .ast
        .jsx_elements
        .iter()
        .any(|inner| inner.span == value)
}

/// ADR 0893: whether a boundary above takes the error at `at`: in this file,
/// or above every mount of the component holding it.
fn handled_above(
    file: &FileFacts,
    at: Span,
    lookup: &crate::indexes::SemanticLookup<'_>,
    depth: usize,
) -> bool {
    let lexical = file.ast.jsx_elements.iter().any(|element| {
        element.span.contains(at)
            && crate::owners::element_primitive(lookup, file, element) == Some(Primitive::Errored)
            && !element
                .attributes
                .iter()
                .any(|attribute| attribute.span.contains(at))
            && fallback_handles(file, element)
    });
    let called = file.ast.arguments_containing(at).any(|(call, index)| {
        index == 0
            && lookup.primitive_at_call(file, call.span) == Some(Primitive::CreateErrorBoundary)
    });
    if lexical || called {
        return true;
    }
    let Some(depth) = depth.checked_sub(1) else {
        return false;
    };
    let Some(component) = crate::owners::containing_body_function(&file.ast, at) else {
        return false;
    };
    if !lookup.program_closed
        || lookup.function_component_status(file, component) != ComponentStatus::Proven
        || !crate::owners::direct_render_sites(lookup, file, component).is_empty()
    {
        return false;
    }
    let mounts = lookup.function_call_sites(file.path.as_str(), component.span);
    !mounts.is_empty()
        && mounts
            .iter()
            .all(|(caller, tag)| handled_above(caller, *tag, lookup, depth))
}

/// A read in a component's prop or child runs only if the component reads it.
/// A lowercase tag is an intrinsic by JSX's own rule, and a boundary always
/// runs its children.
fn in_component_getter(
    file: &FileFacts,
    read: Span,
    lookup: &crate::indexes::SemanticLookup<'_>,
) -> bool {
    file.ast.jsx_elements.iter().any(|element| {
        let intrinsic = file
            .source_text(element.name.span)
            .is_some_and(|name| name.starts_with(|first: char| first.is_ascii_lowercase()));
        let boundary = matches!(
            crate::owners::element_primitive(lookup, file, element),
            Some(Primitive::Errored | Primitive::Loading)
        ) && !element
            .attributes
            .iter()
            .any(|attribute| attribute.span.contains(read));
        element.span.contains(read) && !intrinsic && !boundary
    })
}
