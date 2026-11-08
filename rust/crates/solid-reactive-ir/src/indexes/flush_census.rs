//! ADR 0613: a synchronous `flush()` under an owner withholds the ownerless
//! proofs of the callbacks the queue's drain runs.
//!
//! The dialect says which callbacks run from the drain
//! ([`solid_dialect::Dialect::callback_runs_drained_by_flush`]: a
//! `createEffect` apply, a `createRenderEffect` apply after its first run, a
//! `createReaction` callback). A drain the runtime schedules starts on an
//! empty stack, with no owner; a `flush()` the program calls drains on its
//! own stack, under its caller's owner (rc.17 sets none around the drain).
//! So the "no owner" answer of such a callback holds only while no flush call
//! of the program may run under an owner. This census lists every reference
//! to the flush primitive and classifies the caller as the synchronous
//! dispatch census does (ADR 0356, 0435, 0437, 0438); one call not proven to
//! run with no owner, or one reference that is not a call (the function
//! handed on, aliased or re-exported), vetoes the proofs program-wide. A
//! flush a dependency calls is outside the facts (the documented premise).
//! `render` and `hydrate` drain the same way (ADR 0644), so they are in the
//! census: [`solid_dialect::Dialect::drains_queue_on_caller_stack`].

use super::event_delivery::DispatchCaller;
use super::*;

/// One reference to the flush primitive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FlushSite {
    pub(crate) path: String,
    pub(crate) span: Span,
    /// Whether the reference is the callee of a call of the primitive. Any
    /// other reference hands the function to code this census does not
    /// follow.
    pub(crate) called: bool,
    /// The owner-free caller of the call, when its position proves one.
    pub(crate) caller: Option<DispatchCaller>,
    /// The call runs only on a `createEffect` apply's stack, on the queue of
    /// the same `solid-js` copy (the walk SC2008 proves with,
    /// [`crate::effect_phase::effect_phase_origin`]): the flush that runs
    /// effects is in progress there, so the call drains nothing.
    pub(crate) drains_nothing: bool,
}

impl FlushSite {
    /// Whether the site may drain the queue under an owner.
    pub(crate) const fn vetoes(&self) -> bool {
        !self.called || (self.caller.is_none() && !self.drains_nothing)
    }
}

/// Debug-build tripwire: the census the flush veto is computed from must not
/// read the veto (a reentrant `get_or_init`).
#[cfg(debug_assertions)]
struct FlushVetoComputation {
    outer: bool,
}

#[cfg(debug_assertions)]
thread_local! {
    static FLUSH_VETO_COMPUTING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(debug_assertions)]
impl FlushVetoComputation {
    /// Marks this thread, the census's own or one of its workers'.
    fn enter() -> Self {
        Self {
            outer: FLUSH_VETO_COMPUTING.replace(true),
        }
    }

    fn assert_not_computing() {
        assert!(
            !FLUSH_VETO_COMPUTING.get(),
            "the flush veto was read while its census computes (ADR 0613)"
        );
    }
}

#[cfg(debug_assertions)]
impl Drop for FlushVetoComputation {
    fn drop(&mut self) {
        FLUSH_VETO_COMPUTING.set(self.outer);
    }
}

impl<'a> SemanticLookup<'a> {
    /// ADR 0613: whether some flush of the program may drain the queue under
    /// an owner, so that a callback the drain runs
    /// ([`solid_dialect::Dialect::callback_runs_drained_by_flush`]) is not
    /// proven to run with no owner. The section
    /// [`crate::base::reads::Section::FlushVeto`] binds the answer and the
    /// memo [`super::project_memos::FlushVetoAnswer`] keeps it between
    /// builds.
    pub(crate) fn flush_drains_under_owner(&self) -> bool {
        crate::base::reads::note(crate::base::reads::Section::FlushVeto);
        self.flush_veto()
    }

    /// [`Self::flush_drains_under_owner`] without the section read: the
    /// section's own fingerprint and the memo compute it.
    pub(crate) fn flush_veto(&self) -> bool {
        #[cfg(debug_assertions)]
        FlushVetoComputation::assert_not_computing();
        self.flush_sites().iter().any(FlushSite::vetoes)
    }

    /// Every reference of the program to the flush primitive, classified.
    pub(crate) fn flush_sites(&self) -> &[FlushSite] {
        self.synchronous_flush.get_or_init(|| {
            // The veto's own computation: its value is the section's.
            crate::base::reads::suppress(|| {
                #[cfg(debug_assertions)]
                let _guard = FlushVetoComputation::enter();
                self.classified_flush_sites()
            })
        })
    }

    fn classified_flush_sites(&self) -> Vec<FlushSite> {
        let mut sites = Vec::new();
        if !self
            .project_primitives()
            .iter()
            .any(|primitive| self.dialect.drains_queue_on_caller_stack(*primitive))
        {
            return sites;
        }
        let is_flush = |name: Option<crate::PrimitiveName>| {
            name.as_ref()
                .and_then(crate::PrimitiveName::primitive)
                .is_some_and(|primitive| self.dialect.drains_queue_on_caller_stack(primitive))
        };
        // Each file's sites read only that file and this build's lookup, so
        // the files are classified in parallel; the sites are sorted below.
        let files: &'a [solid_facts::FileFacts] = self.file_slots();
        let per_file = |file: &'a solid_facts::FileFacts| -> Vec<FlushSite> {
            #[cfg(debug_assertions)]
            let _guard = FlushVetoComputation::enter();
            let mut sites = Vec::new();
            // A package analyzed as source (ADRs 0284, 0300) is dependency
            // code: its flush calls are the documented premise, not the
            // census's.
            if self
                .facts
                .dependency_source_root(file.path.as_str())
                .is_some()
            {
                return sites;
            }
            let path = file.path.as_str();
            let callees = file
                .ast
                .calls
                .iter()
                .filter(|call| {
                    self.primitive_at_call(file, call.span)
                        .is_some_and(|primitive| {
                            self.dialect.drains_queue_on_caller_stack(primitive)
                        })
                })
                .map(|call| (file.ast.peel_ts_sugar_span(call.callee), call))
                .collect::<Vec<_>>();
            for (_, call) in &callees {
                sites.push(FlushSite {
                    path: path.to_owned(),
                    span: call.span,
                    called: true,
                    caller: self.flush_caller(file, call),
                    drains_nothing: self.primitive_at_call(file, call.span)
                        == Some(solid_dialect::Primitive::Flush)
                        && self.flush_in_effect_phase(file, call),
                });
            }
            // A reference that is not the callee of one of those calls (an
            // identifier, or a namespace import's member): the function is
            // handed to code the census does not follow.
            let references = file
                .ast
                .reference_declarations
                .iter()
                .map(|(reference, _)| *reference)
                .chain(file.ast.members.iter().map(|member| member.span));
            for reference in references {
                if callees.iter().any(|(callee, _)| *callee == reference)
                    || !is_flush(crate::referenced_primitive(
                        file,
                        reference,
                        self.entities,
                        self.symbol_names,
                        self.dialect,
                    ))
                {
                    continue;
                }
                sites.push(FlushSite {
                    path: path.to_owned(),
                    span: reference,
                    called: false,
                    caller: None,
                    drains_nothing: false,
                });
            }
            sites
        };
        let slots = (0..files.len()).collect::<Vec<_>>();
        sites.extend(
            crate::pipeline::parallel_balanced_results(&slots, |slot| per_file(&files[*slot]))
                .into_iter()
                .flatten(),
        );
        sites.sort_by(|left, right| (&left.path, left.span).cmp(&(&right.path, right.span)));
        sites.dedup();
        sites
    }

    /// Whether every run of the flush call is on a `createEffect` apply's
    /// stack (no other run, `mixed`, is admitted), on the queue the call's own
    /// `solid-js` copy keeps: the queue is running there, and the call returns
    /// without draining (rc.17 `flush`, `globalQueue._running`).
    fn flush_in_effect_phase(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
    ) -> bool {
        self.callee_package_root(file, call.callee)
            .and_then(|root| {
                crate::effect_phase::effect_phase_origin(
                    file,
                    call.span,
                    &root,
                    &self.package_instances,
                    self,
                )
            })
            .is_some_and(|origin| !origin.mixed)
    }

    /// ADR 0639: the `flush()` at `call` runs where no delivery reaches it: a
    /// timer, past an await, module scope, a test runner's callback or helper.
    pub(crate) fn flush_runs_off_any_delivery(
        &self,
        file: &'a solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
    ) -> bool {
        let placed = |at: &solid_facts::ast::CallFact| {
            matches!(
                self.dispatch_caller(file, at),
                Some(
                    DispatchCaller::FreshStack
                        | DispatchCaller::AwaitContinuation
                        | DispatchCaller::ModuleInitialization
                        | DispatchCaller::TestRunner
                )
            ) || crate::execution_role::dispatch_in_test_only_function(file, at.span, self)
        };
        // ADR 0640: or on the stack of a call that does: `createRoot(() => flush())`
        // in a test's callback, a callback a local helper runs in place.
        placed(call)
            || file.ast.functions_owning(call.span).any(|outer| {
                crate::execution_role::entry_on_stack_of(file, call.span, Some(outer), self)
                    .is_some_and(|entry| {
                        entry != call.span && file.ast.call_at(entry).is_some_and(placed)
                    })
            })
    }

    /// The owner-free caller of a flush call: the dispatch census's positional
    /// classes, then a function every invocation of which is an event
    /// delivery (only while no synchronous dispatch can deliver it under an
    /// owner, ADR 0356), then a test-only helper (ADR 0438).
    fn flush_caller(
        &self,
        file: &'a solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
    ) -> Option<DispatchCaller> {
        if let Some(caller) = self.dispatch_caller(file, call) {
            return Some(caller);
        }
        // A function literal the compiler wires as an element's event handler
        // runs from the delivery (the owner graph's own unowned edge for it,
        // `owner_discovery`), in an open program as in a closed one.
        if inline_event_handler_call(file, call.span) && !self.synchronous_event_dispatch_present()
        {
            return Some(DispatchCaller::EventHandler);
        }
        if crate::execution_role::dispatch_in_event_only_function(file, call.span, self)
            && !self.synchronous_event_dispatch_present()
        {
            return Some(DispatchCaller::EventHandler);
        }
        crate::execution_role::dispatch_in_test_only_function(file, call.span, self)
            .then_some(DispatchCaller::TestHelper)
    }
}

/// Whether `span` is written directly in the body of a function literal the
/// compiler reports as an element's event handler (not in a nested function,
/// not in JSX inside it), which is not a generator: the delivery runs it.
fn inline_event_handler_call(file: &solid_facts::FileFacts, span: Span) -> bool {
    if file.compiler.withheld() {
        return false;
    }
    let Some(function) = crate::owners::containing_ast_function(&file.ast, span) else {
        return false;
    };
    !function.generator
        && crate::owners::written_directly_in(&file.ast, function, span)
        && !file
            .ast
            .jsx_containing(span)
            .any(|element| function.span.contains(element.span))
        && file.compiler.callback_roles.iter().any(|callback| {
            callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
                && file.ast.peel_ts_sugar_span(callback.span) == function.span
                && !crate::execution_role::handler_event_denied(file, callback.span)
        })
}
