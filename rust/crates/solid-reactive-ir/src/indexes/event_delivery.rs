//! Visible synchronous DOM dispatch withholds an ownerless event proof.

use super::*;

/// What a synchronous dispatch call is dispatched on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DispatchReceiver {
    /// `HTMLElement.click`: the receiver is an element by its declaration.
    Element,
    /// `EventTarget.dispatchEvent` on a receiver nothing proves.
    Unknown,
    /// The global `window` (ADR 0435).
    Window,
    /// The global `document`, where the dialect says it reaches no JSX
    /// handler (ADR 0435).
    Document,
    /// An element the function creates and attaches only under `document.body`
    /// or the root element, on a path nothing in the program listens on
    /// (ADR 0640).
    NoListener,
    /// ADR 1099: a click (or `remove()`) of an anchor the function creates and appends to `document.body` or the root element (ADR 0640) in a
    /// program that listens for `click`: its path holds no JSX element, so it reaches no element handler (Solid's delegated
    /// handler walks the target's path), while a listener on `document` or `window` still hears it.
    NoElementHandler,
}

impl DispatchReceiver {
    /// A receiver that cannot reach an element handler.
    const fn excluded(self) -> bool {
        matches!(
            self,
            Self::Window | Self::Document | Self::NoListener | Self::NoElementHandler
        )
    }
}

/// A caller whose stack carries no owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DispatchCaller {
    /// A callback a reviewed host scheduler runs from an empty stack.
    FreshStack,
    /// Code an `await` of its own async function dominates (ADR 0320's
    /// continuation): it resumes from the microtask queue on an empty stack,
    /// with no owner and no strict-read window (ADR 0477).
    AwaitContinuation,
    /// Module top level, outside every function, class and JSX element.
    ModuleInitialization,
    /// The callback of `runWithOwner(null, …)`: no owner, but a strict-read
    /// window of the enclosing code stays open, so only owner proofs use it.
    DetachedOwner,
    /// A callback a reviewed test runner invokes from its own scheduler
    /// (ADR 0438): no owner and no strict-read window.
    TestRunner,
    /// A helper every one of whose invocations is a call written in a test
    /// runner's callback, or in another such helper (ADR 0438): it runs on the
    /// runner's stack or a continuation of it, with no owner and no window.
    TestHelper,
    /// A function whose every invocation is an event delivery (ADR 0327's
    /// census): a delivery it makes runs on the stack of a delivery, which
    /// has no owner and no window (ADR 0437).
    EventHandler,
    /// A `createEffect`'s apply on a dialect whose effect window takes only
    /// its queue's event (ADR 0639): a delivery reads in no window, and still
    /// runs under the owner and inside the flush.
    EffectApply,
}

impl DispatchCaller {
    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::FreshStack => "scheduled callback",
            Self::AwaitContinuation => "await continuation",
            Self::ModuleInitialization => "module initialization",
            Self::DetachedOwner => "runWithOwner(null)",
            Self::TestRunner => "test runner callback",
            Self::TestHelper => "test-only helper",
            Self::EventHandler => "event handler",
            Self::EffectApply => "effect apply",
        }
    }
}

/// One call resolved to `EventTarget.dispatchEvent` or `HTMLElement.click`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DispatchSite {
    pub(crate) path: String,
    pub(crate) span: Span,
    pub(crate) receiver: DispatchReceiver,
    pub(crate) caller: Option<DispatchCaller>,
}

impl DispatchSite {
    /// Whether the site withholds ownerless event proofs (owner and write
    /// roles): a delivery can inherit its caller's owner. ADR 1099: an effect's apply runs from the queue's flush with no owner of
    /// its own (rc.21 `@solidjs/signals` runs `_effectFn` under no `runWithOwner`), so its delivery carries an owner only where a
    /// flush drains under one: [`Self::delivers_in_effect_apply`] with `SemanticLookup::flush_referenced`
    /// (the owner bit of `SemanticLookup::dispatch_veto`).
    pub(crate) const fn withholds_owner_proofs(&self) -> bool {
        !self.receiver.excluded() && self.caller.is_none()
    }

    /// Whether the site withholds an event handler's untracked-read proof: a
    /// delivery can run inside its caller's strict-read window. Only a caller
    /// with no window excludes it.
    pub(crate) const fn withholds_read_proofs(&self) -> bool {
        !self.receiver.excluded()
            && !matches!(
                self.caller,
                Some(
                    DispatchCaller::FreshStack
                        | DispatchCaller::AwaitContinuation
                        | DispatchCaller::ModuleInitialization
                        | DispatchCaller::TestRunner
                        | DispatchCaller::TestHelper
                        | DispatchCaller::EventHandler
                        | DispatchCaller::EffectApply
                )
            )
    }

    /// ADR 1115: a site whose read proof rests on the delivery setting `window.event`: a dispatch from an effect's apply (ADR 0639:
    /// the apply's strict-read window takes only its queue's event) or from an event handler (ADR 0437: that delivery's own window is
    /// closed by its event). A listener in a shadow tree runs with `window.event` unset (rc.21 `dist/dev-shared.js:5796, 5803`), so the
    /// window holds for it. A listener on the window or the document is in no shadow tree.
    pub(crate) const fn read_proof_rests_on_event(&self) -> bool {
        !self.receiver.excluded()
            && matches!(
                self.caller,
                Some(DispatchCaller::EffectApply | DispatchCaller::EventHandler)
            )
    }

    /// A delivery in an effect's apply that reaches a handler. Its read proof
    /// holds only while no flush drains under an owner.
    pub(crate) const fn delivers_in_effect_apply(&self) -> bool {
        !self.receiver.excluded() && matches!(self.caller, Some(DispatchCaller::EffectApply))
    }

    /// ADR 0645: whether a listener may run on this site's stack under an owner
    /// or in a strict-read window. `window` and `document` reach no JSX handler,
    /// so the proofs above leave them out, but a listener is on them.
    pub(crate) const fn withholds_listener_proofs(&self) -> bool {
        !matches!(self.receiver, DispatchReceiver::NoListener)
            && !matches!(
                self.caller,
                Some(
                    DispatchCaller::FreshStack
                        | DispatchCaller::AwaitContinuation
                        | DispatchCaller::ModuleInitialization
                        | DispatchCaller::TestRunner
                        | DispatchCaller::TestHelper
                        | DispatchCaller::EventHandler
                )
            )
    }

    /// Whether a delivery may run inside an effect's apply, whose `flush()` guard
    /// takes no event (ADR 0639). The same callers exclude it as a strict-read
    /// window, but the receiver does not: a listener on `window` flushes too.
    pub(crate) const fn withholds_flush_proofs(&self) -> bool {
        !matches!(self.receiver, DispatchReceiver::NoListener)
            && !matches!(
                self.caller,
                Some(
                    DispatchCaller::FreshStack
                        | DispatchCaller::AwaitContinuation
                        | DispatchCaller::ModuleInitialization
                        | DispatchCaller::TestRunner
                        | DispatchCaller::TestHelper
                        | DispatchCaller::EventHandler
                )
            )
    }
}

/// Debug-build tripwire: the event-only census the final dispatch classes read
/// must not read the veto they are computing (a reentrant `get_or_init`).
#[cfg(debug_assertions)]
struct VetoComputation;

#[cfg(debug_assertions)]
thread_local! {
    static VETO_COMPUTING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(debug_assertions)]
impl VetoComputation {
    fn enter() -> Self {
        VETO_COMPUTING.set(true);
        Self
    }

    fn assert_not_computing() {
        assert!(
            !VETO_COMPUTING.get(),
            "the dispatch veto was read while its census computes (ADR 0437)"
        );
    }
}

#[cfg(debug_assertions)]
impl Drop for VetoComputation {
    fn drop(&mut self) {
        VETO_COMPUTING.set(false);
    }
}

/// A member call spelled as a dispatch: `el.click()`, `el.dispatchEvent(e)`.
pub(crate) fn spelled_dispatch(file: &FileFacts, call: &solid_facts::ast::CallFact) -> bool {
    !call.construct
        && file
            .ast
            .member_at(file.ast.peel_ts_sugar_span(call.callee))
            .is_some_and(|member| match file.source_text(member.property) {
                Some("click") => call.arguments.is_empty(),
                Some("dispatchEvent") => call.arguments.len() == 1,
                _ => false,
            })
}

impl SemanticLookup<'_> {
    /// Whether some module of the program calls direct `eval`, which can read
    /// or write any binding it sees: read once per build.
    pub(crate) fn program_has_eval(&self) -> bool {
        self.hazard_present(crate::base::Hazard::Eval)
    }

    /// ADR 0292: [`crate::interproc::accessor_provenance::reflective_prototype_write`],
    /// scanned once per build.
    pub(crate) fn reflective_prototype_write_present(&self) -> bool {
        crate::base::reads::note(crate::base::reads::Section::ReflectivePrototypeWrite);
        *self.reflective_prototype_write.get_or_init(|| {
            crate::interproc::accessor_provenance::scan_reflective_prototype_write(self)
        })
    }

    /// ADR 1115: whether some listener of the program may sit in a shadow tree ([`scan_shadow_roots`]), scanned once per build.
    pub(crate) fn shadow_roots_possible(&self) -> bool {
        crate::base::reads::note(crate::base::reads::Section::ShadowRoots);
        *self.shadow_roots.get_or_init(|| scan_shadow_roots(self))
    }

    /// ADR 0634: some analyzed file names `markRaw` or writes a `shallow`
    /// option. A record that entered a shallow store is raw for every later
    /// store (`createStore(rec)` returns it as it is), so a store over a value
    /// no literal states may then be no proxy.
    pub(crate) fn raw_store_values_present(&self) -> bool {
        crate::base::reads::note(crate::base::reads::Section::RawStoreValues);
        *self.raw_store_values.get_or_init(|| {
            self.file_slots().iter().any(|file| {
                file.ast
                    .identifiers
                    .iter()
                    .any(|identifier| file.source_text(identifier.span) == Some("markRaw"))
                    || file.ast.object_properties.iter().any(|property| {
                        file.source_text(property.key)
                            .is_some_and(|key| key.trim_matches(['"', '\'']) == "shallow")
                    })
            })
        })
    }

    /// ADR 0634: the store the binding `binding` holds, created in `file`,
    /// may be its raw value: its value is `markRaw(...)`, or names a value no
    /// literal or derive function states and some file may mark one raw.
    pub(crate) fn store_value_may_be_raw(
        &self,
        file: &FileFacts,
        binding: &solid_facts::ast::BindingFact,
    ) -> bool {
        use solid_facts::ast::ArgumentValueKind;
        let Some(call) = binding
            .call_initializer
            .and_then(|initializer| file.ast.call_at(initializer))
        else {
            return false;
        };
        if !matches!(
            self.primitive_at_call(file, call.span),
            Some(
                solid_dialect::Primitive::CreateStore
                    | solid_dialect::Primitive::CreateOptimisticStore
            )
        ) {
            return false;
        }
        let Some(first) = call.arguments.first() else {
            return false;
        };
        let value = file.ast.peel_ts_sugar_span(first.span);
        if first.runtime_value_kind.is_data_literal()
            || file.ast.array_literals.contains(&value)
            || file
                .ast
                .object_get_shapes
                .iter()
                .any(|shape| shape.span == value)
            || matches!(
                first.value,
                ArgumentValueKind::Function | ArgumentValueKind::AsyncFunction
            )
        {
            return false;
        }
        self.raw_store_values_present()
    }

    /// ADR 0634: [`Self::store_value_may_be_raw`] of the store declared at
    /// `declaration`, for a read any layer reports.
    pub(crate) fn store_declared_may_be_raw(&self, declaration: &crate::Location) -> bool {
        self.file_by_path(declaration.path.as_ref())
            .is_some_and(|file| {
                file.ast
                    .bindings
                    .iter()
                    .find(|binding| {
                        binding
                            .names
                            .iter()
                            .any(|name| u64::from(name.span.start) == declaration.start_byte)
                    })
                    .is_some_and(|binding| self.store_value_may_be_raw(file, binding))
            })
    }

    /// ADR 0356, narrowed by ADR 0435: DOM dispatch runs handlers on its
    /// caller's stack. A visible synchronous dispatch withholds the ownerless
    /// proof for event-only invocations unless its target cannot reach a JSX
    /// handler or its caller has no owner. This is counterevidence, not a
    /// guessed handler edge. A project method with the same spelling does not
    /// select either library declaration.
    pub(crate) fn synchronous_event_dispatch_present(&self) -> bool {
        crate::base::reads::note(crate::base::reads::Section::DispatchVeto);
        self.dispatch_veto().0
    }

    /// [`Self::synchronous_event_dispatch_present`] for reads in an event
    /// handler, whose proof also needs the delivery outside a strict-read
    /// window.
    pub(crate) fn synchronous_event_read_dispatch_present(&self) -> bool {
        crate::base::reads::note(crate::base::reads::Section::DispatchVeto);
        let veto = self.dispatch_veto();
        // An apply's delivery has no window and no owner only while the queue
        // drains on an empty stack: under an owned flush it has that owner,
        // and the handler's write throws (`g10`, ADR 0639 with ADR 0613).
        veto.1 || (veto.4 && self.flush_drains_under_owner())
    }

    /// ADR 1115 part 2: whether an effect's apply may deliver an event to a listener in a shadow tree, which runs with `window.event`
    /// unset and so inside the apply's effect-callback window: a primitive created in the handler then throws
    /// PRIMITIVE_IN_EFFECT_CALLBACK before any later check at that creation.
    pub(crate) fn apply_delivery_in_shadow_possible(&self) -> bool {
        crate::base::reads::note(crate::base::reads::Section::DispatchVeto);
        self.dispatch_veto().4 && self.shadow_roots_possible()
    }

    /// [`DispatchSite::withholds_flush_proofs`] of any dispatch of the program.
    pub(crate) fn synchronous_event_flush_dispatch_present(&self) -> bool {
        crate::base::reads::note(crate::base::reads::Section::DispatchVeto);
        self.dispatch_veto().2
    }

    /// [`DispatchSite::withholds_listener_proofs`] of any dispatch of the program.
    pub(crate) fn synchronous_listener_dispatch_present(&self) -> bool {
        crate::base::reads::note(crate::base::reads::Section::DispatchVeto);
        self.dispatch_veto().3
    }

    /// Per-build existential. Every consumer, including a false answer,
    /// records the section. Its next-build fingerprint recomputes the whole
    /// predicate with that build's file membership, symbols and call indexes.
    pub(crate) fn listener_project_reach(&self) -> bool {
        crate::base::reads::note(crate::base::reads::Section::ListenerProjectReach);
        *self.listener_project_reach.get_or_init(|| {
            crate::base::reads::suppress(|| crate::listener_reach::project_reaches(self))
        })
    }

    /// The veto answers, owner, read, flush and listener proofs
    /// ([`Self::synchronous_event_dispatch_present`],
    /// [`Self::synchronous_event_read_dispatch_present`],
    /// [`Self::synchronous_event_flush_dispatch_present`]): the section
    /// [`crate::base::reads::Section::DispatchVeto`] binds them, and the memo
    /// [`super::project_memos::DispatchVetoAnswer`] keeps them between builds
    /// (ADR 0450). With no
    /// call anywhere that resolves to a dispatch (the hazard
    /// [`crate::base::Hazard::SynchronousDispatch`]) neither holds, whatever
    /// the censuses say, and they are not computed.
    pub(crate) fn dispatch_veto(&self) -> (bool, bool, bool, bool, bool) {
        #[cfg(debug_assertions)]
        VetoComputation::assert_not_computing();
        if !self.hazard_present(crate::base::Hazard::SynchronousDispatch) {
            return (false, false, false, false, false);
        }
        let sites = self.event_dispatch_sites();
        let delivers_in_effect_apply = sites.iter().any(DispatchSite::delivers_in_effect_apply);
        (
            // ADR 1099: an apply's delivery has an owner only when a flush drains the queue under one. The flush census
            // asks this answer (its sites' callers), so the owner bit takes the answer before it
            // (`SemanticLookup::flush_referenced`), inside this memo.
            sites.iter().any(DispatchSite::withholds_owner_proofs)
                || (delivers_in_effect_apply && self.flush_referenced()),
            // ADR 1115: a delivery's read proof that rests on its event holds only where no listener sits in a shadow tree.
            sites.iter().any(DispatchSite::withholds_read_proofs)
                || (sites.iter().any(DispatchSite::read_proof_rests_on_event)
                    && self.shadow_roots_possible()),
            sites.iter().any(DispatchSite::withholds_flush_proofs),
            sites.iter().any(DispatchSite::withholds_listener_proofs),
            delivers_in_effect_apply,
        )
    }

    /// [`Self::event_dispatch_sites`] as the report states them, in source
    /// order of the files.
    pub(crate) fn event_dispatch_reports(&self) -> Vec<crate::EventDispatchSite> {
        self.event_dispatch_sites()
            .iter()
            .filter_map(|site| {
                let file = self.file_by_path(&site.path)?;
                let before = file.source.get(..site.span.start as usize)?;
                let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
                Some(crate::EventDispatchSite {
                    path: site.path.clone(),
                    line: u32::try_from(line).unwrap_or(u32::MAX),
                    receiver: match site.receiver {
                        DispatchReceiver::Element => "element",
                        DispatchReceiver::Unknown => "unknown",
                        DispatchReceiver::Window => "window",
                        DispatchReceiver::Document => "document",
                        DispatchReceiver::NoListener => "no listener",
                        DispatchReceiver::NoElementHandler => "no element handler",
                    }
                    .to_owned(),
                    caller: site.caller.map(|caller| caller.word().to_owned()),
                    withholds_owner_proofs: site.withholds_owner_proofs()
                        || (site.delivers_in_effect_apply() && self.flush_referenced()),
                    withholds_read_proofs: site.withholds_read_proofs(),
                })
            })
            .collect()
    }

    /// Every synchronous dispatch call of the program, classified: the
    /// statically placed sites ([`Self::placed_dispatch_sites`]), then the ones
    /// in an event-only function (ADR 0437) or a test-only helper (ADR 0438).
    ///
    /// The second step reads the two censuses, which read no veto (an
    /// answer asked of the veto while this computes is a bug; debug builds
    /// fail on it). Because the census is independent of the veto, no round
    /// of removal feeds another: a site stays out of the veto by an induction
    /// over time, not by the sites already out.
    pub(crate) fn event_dispatch_sites(&self) -> &[DispatchSite] {
        self.synchronous_event_dispatch.get_or_init(|| {
            // The veto's own computation: its value is the section's.
            crate::base::reads::suppress(|| self.classified_dispatch_sites())
        })
    }

    fn classified_dispatch_sites(&self) -> Vec<DispatchSite> {
        {
            #[cfg(debug_assertions)]
            let _guard = VetoComputation::enter();
            let mut sites = self.placed_dispatch_sites().to_vec();
            for site in &mut sites {
                // Every receiver: the flush veto reads the caller of a `window`
                // dispatch too.
                if site.caller.is_none()
                    && self.file_by_path(&site.path).is_some_and(|file| {
                        crate::execution_role::dispatch_in_event_only_function(
                            file, site.span, self,
                        )
                    })
                {
                    site.caller = Some(DispatchCaller::EventHandler);
                } else if site.caller.is_none()
                    && self.file_by_path(&site.path).is_some_and(|file| {
                        crate::execution_role::dispatch_in_test_only_function(file, site.span, self)
                    })
                {
                    site.caller = Some(DispatchCaller::TestHelper);
                }
            }
            sites
        }
    }

    /// Every synchronous dispatch call of the program with the receiver and
    /// caller that its own written position decides. Nothing here may consult
    /// an event proof: the answer feeds them.
    fn placed_dispatch_sites(&self) -> &[DispatchSite] {
        self.placed_event_dispatch.get_or_init(|| {
            let mut sites = Vec::new();
            let mut click_listener = None;
            // The calls the base index lists as dispatches, per file in file
            // order ([`crate::base::Hazard::SynchronousDispatch`], the same
            // resolution of each call's callee), not every call of the
            // program.
            for (file, entries) in self.slot_entries(self.base_hazards()) {
                for call in entries
                    .iter()
                    .filter(|(hazard, _)| *hazard == crate::base::Hazard::SynchronousDispatch)
                    .filter_map(|(_, ordinal)| file.ast.calls.get(*ordinal as usize))
                {
                    let name = self
                        .resolved_callee_call(file, call.callee)
                        .and_then(crate::runtime_semantics::standard_call_name);
                    // ADR 0642: a `click` or `dispatchEvent` no declaration
                    // names: `any`, an interface or a type literal's signature
                    // says nothing of the runtime receiver, which may be an
                    // element. A project method with a body is no hazard.
                    let Some(name) = name else {
                        if self
                            .function_called_at(
                                file.path.as_str(),
                                file.ast.peel_ts_sugar_span(call.callee),
                            )
                            .is_none()
                            && !self.declared_with_a_body(file, call)
                        {
                            sites.push(DispatchSite {
                                path: file.path.to_string(),
                                span: call.span,
                                receiver: DispatchReceiver::Unknown,
                                caller: self
                                    .dispatch_caller(file, call)
                                    .or_else(|| self.dispatch_in_effect_apply(file, call.span))
                                    .or_else(|| {
                                        self.dispatch_in_effect_apply_helper(file, call.span)
                                    }),
                            });
                        }
                        continue;
                    };
                    let receiver = match name {
                        // ADR 1099: removing the created anchor delivers only to its own path
                        // (the anchor, then the root): no element handler either.
                        "ChildNode.remove"
                            if self.created_element_unattached_to_a_listener(file, call) =>
                        {
                            DispatchReceiver::NoElementHandler
                        }
                        "HTMLElement.click"
                            if self.created_element_unattached_to_a_listener(file, call) =>
                        {
                            if *click_listener
                                .get_or_insert_with(|| self.program_listens_for_click())
                            {
                                DispatchReceiver::NoElementHandler
                            } else {
                                DispatchReceiver::NoListener
                            }
                        }
                        // ADR 1080: a navigation delivers only the event types the host fact names (`History.go` and
                        // `Location.reload`: `beforeunload`; `Location.assign` and `Location.replace`: `beforeunload`, and
                        // `popstate` for a fragment). With no listener for any of them, it delivers to none.
                        _ if crate::runtime_semantics::synchronously_delivered_event_types(
                            name,
                        )
                        .is_some_and(|events| !self.program_listens_for_any(events)) =>
                        {
                            DispatchReceiver::NoListener
                        }
                        "EventTarget.dispatchEvent" => self.dispatch_receiver(file, call),
                        // ADR 0937: lib.dom's global `dispatchEvent`, the
                        // window's own method called bare.
                        "dispatchEvent" => DispatchReceiver::Window,
                        _ => DispatchReceiver::Element,
                    };
                    sites.push(DispatchSite {
                        path: file.path.to_string(),
                        span: call.span,
                        receiver,
                        caller: self
                            .dispatch_caller(file, call)
                            .or_else(|| self.dispatch_in_effect_apply(file, call.span))
                            .or_else(|| self.dispatch_in_effect_apply_helper(file, call.span)),
                    });
                }
            }
            sites
        })
    }

    /// The call's selected declaration is a method or a function-valued
    /// property the program writes: it has a body, unlike an interface's or a
    /// type literal's signature.
    fn declared_with_a_body(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
    ) -> bool {
        self.resolved_callee_call(file, call.callee)
            .and_then(|resolved| resolved.declaration.as_ref())
            .is_some_and(|declaration| {
                let Some(file) = self.file_by_path(declaration.location.path.as_ref()) else {
                    return false;
                };
                let at = |span: Span| u64::from(span.start) == declaration.location.start_byte;
                file.ast.functions.iter().any(|function| {
                    at(function.span)
                        || function
                            .method_name
                            .as_ref()
                            .is_some_and(|name| at(name.span))
                        || function.name.as_ref().is_some_and(|name| at(name.span))
                }) || file.ast.object_properties.iter().any(|property| {
                    (at(property.key) || at(property.span))
                        && file
                            .ast
                            .function_at(file.ast.peel_ts_sugar_span(property.value))
                            .is_some()
                })
            })
    }

    /// ADR 0640: the receiver of this `click()` (ADR 1099: or `remove()`) is a `const` the function fills
    /// from `document.createElement("a")`, and every use of it is a property write
    /// (no `on*`), `click()`, `remove()`, or the argument of an append to
    /// `document.body` or the root element. Its path holds no JSX element.
    fn created_element_unattached_to_a_listener(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
    ) -> bool {
        let ast = &file.ast;
        let Some(member) = ast.member_at(ast.peel_ts_sugar_span(call.callee)) else {
            return false;
        };
        let Some(declared) = ast.reference_declaration(ast.peel_ts_sugar_span(member.object))
        else {
            return false;
        };
        let created = ast
            .bindings
            .iter()
            .find(|binding| {
                binding.immutable
                    && binding.shape == solid_facts::ast::BindingShape::Identifier
                    && binding.names.iter().any(|name| name.span == declared)
            })
            .and_then(|binding| binding.call_initializer)
            .and_then(|initializer| ast.call_at(initializer));
        let creator =
            created.and_then(|created| ast.member_at(ast.peel_ts_sugar_span(created.callee)));
        if !creator.is_some_and(|creator| {
            file.source_text(creator.property) == Some("createElement")
                && self.global_dom_variable(file, ast.peel_ts_sugar_span(creator.object))
                    == Some("document")
        }) {
            return false;
        }
        // ADR 0645: an anchor's activation names no other element. A `label`
        // clicks its control, and a checkbox fires `change` (Chrome).
        if !created.is_some_and(|created| {
            created.arguments.len() == 1
                && matches!(
                    file.source_text(ast.peel_ts_sugar_span(created.arguments[0].span)),
                    Some("\"a\"" | "'a'" | "`a`")
                )
        }) {
            return false;
        }
        ast.identifiers
            .iter()
            .filter(|identifier| {
                identifier.span != declared
                    && ast.reference_declaration(identifier.span) == Some(declared)
            })
            .all(|identifier| {
                let Some(used) = ast
                    .members
                    .iter()
                    .find(|used| ast.peel_ts_sugar_span(used.object) == identifier.span)
                else {
                    return self.appended_under_the_root(file, identifier.span);
                };
                let property = file.source_text(used.property).unwrap_or("on");
                if ast.computed_members.contains(&used.span) {
                    return false;
                }
                let called = ast.calls_with_peeled_callee(used.span).next().is_some();
                if called {
                    matches!(property, "click" | "remove")
                } else {
                    !property.starts_with("on")
                        && ast
                            .assignments
                            .iter()
                            .any(|assignment| assignment.target.contains(used.span))
                }
            })
    }

    /// `value` is the one argument of `document.body.append(value)`, or of
    /// `appendChild` on it or on `document.documentElement`.
    fn appended_under_the_root(&self, file: &solid_facts::FileFacts, value: Span) -> bool {
        let ast = &file.ast;
        ast.arguments_containing(value).any(|(call, _)| {
            call.arguments.len() == 1
                && ast.peel_ts_sugar_span(call.arguments[0].span) == value
                && ast
                    .member_at(ast.peel_ts_sugar_span(call.callee))
                    .is_some_and(|append| {
                        matches!(
                            file.source_text(append.property),
                            Some("append" | "appendChild")
                        ) && ast
                            .member_at(ast.peel_ts_sugar_span(append.object))
                            .is_some_and(|root| {
                                matches!(
                                    file.source_text(root.property),
                                    Some("body" | "documentElement")
                                ) && self
                                    .global_dom_variable(file, ast.peel_ts_sugar_span(root.object))
                                    == Some("document")
                            })
                    })
        })
    }

    /// ADR 0640: something in the program may hear a `click` that bubbles to the
    /// root: a handler on a JSX `body` or `html`, an `onclick` property, or an
    /// `addEventListener` for `click` or an event nothing names, on a target
    /// other than one whose declaration has no place on a DOM path. ADR 0645:
    /// the census is of every way to reach the method, not of its direct calls:
    /// a bare `addEventListener`, a literal key, a read of the method (`.call`,
    /// an alias), a computed key on a DOM global.
    fn program_listens_for_click(&self) -> bool {
        self.program_listens(&["onclick"], &|file, call| {
            self.registers_no_click(file, call)
        })
    }

    /// ADR 1080: [`Self::program_listens_for_click`] for any of `events` (the types a dispatch delivers): a handler on a JSX
    /// `body` or `html`, an `on<event>` property, or an `addEventListener` that may register one of them (an event nothing
    /// names, or a helper's type parameter some call fills with anything but a literal of another type), on a target other
    /// than one whose declaration has no place on a DOM path.
    fn program_listens_for_any(&self, events: &[&str]) -> bool {
        let properties = events
            .iter()
            .map(|event| format!("on{event}"))
            .collect::<Vec<_>>();
        let properties = properties.iter().map(String::as_str).collect::<Vec<_>>();
        self.program_listens(&properties, &|file, call| {
            self.registers_none_of(file, call, events)
        })
    }

    /// The census walk [`Self::program_listens_for_click`] and [`Self::program_listens_for_any`] share: `properties` are the
    /// `on*` names that register a listener by assignment or by a literal's key, `quiet` says a registration call names none
    /// of the events.
    fn program_listens(
        &self,
        properties: &[&str],
        registers_none: &dyn Fn(&solid_facts::FileFacts, &solid_facts::ast::CallFact) -> bool,
    ) -> bool {
        self.file_slots().iter().any(|file| {
            let ast = &file.ast;
            let quiet = |callee: Span| {
                ast.calls_with_peeled_callee(callee)
                    .next()
                    .is_some_and(|call| registers_none(file, call))
            };
            let named = |member: &solid_facts::ast::MemberFact, name: &str| {
                if ast.computed_members.binary_search(&member.span).is_ok() {
                    ast.literal_computed_members
                        .iter()
                        .any(|literal| literal.span == member.span && literal.key == name)
                } else {
                    file.source_text(member.property) == Some(name)
                }
            };
            ast.jsx_elements.iter().any(|element| {
                matches!(file.source_text(element.name.span), Some("body" | "html"))
                    && element.attributes.iter().any(|attribute| {
                        file.source_text(attribute.local_name)
                            .is_none_or(|name| name.starts_with("on"))
                    })
            }) || ast.assignments.iter().any(|assignment| {
                let target = ast.peel_ts_sugar_span(assignment.target);
                file.source_text(target).is_none()
                    || ast.member_at(target).is_some_and(|member| {
                        properties.iter().any(|property| named(member, property))
                    })
            }) || ast.object_properties.iter().any(|property| {
                file.source_text(property.key).is_some_and(|key| {
                    let key = key.trim_matches(['\'', '"']);
                    properties.contains(&key)
                })
            }) || ast.members.iter().any(|member| {
                (named(member, "addEventListener") && !quiet(member.span))
                    || (ast.computed_members.binary_search(&member.span).is_ok()
                        && !ast
                            .literal_computed_members
                            .iter()
                            .any(|literal| literal.span == member.span)
                        && self
                            .global_dom_variable(file, ast.peel_ts_sugar_span(member.object))
                            .is_some())
            }) || ast.identifiers.iter().any(|identifier| {
                file.source_text(identifier.span) == Some("addEventListener")
                    && ast.reference_declaration(identifier.span).is_none()
                    // ADR 1080: `typeof addEventListener` reads no member and hands the method nowhere (an inert operand:
                    // the operand of `typeof` or `void`, either side of a strict equality).
                    && !ast.inert_operands.contains(&identifier.span)
                    && !ast
                        .members
                        .iter()
                        .any(|member| member.property == identifier.span)
                    && !quiet(identifier.span)
            })
        })
    }

    /// The call registers a listener for a literal event other than `click`, or
    /// on a target whose declaration has no place on a DOM path.
    fn registers_no_click(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
    ) -> bool {
        let names_another_event = call.arguments.first().is_some_and(|event| {
            !event.spread
                && file.source_text(event.span).is_some_and(|text| {
                    let quoted = text.len() >= 2
                        && matches!(text.as_bytes()[0], b'"' | b'\'')
                        && text.as_bytes()[text.len() - 1] == text.as_bytes()[0];
                    quoted && !text[1..text.len() - 1].eq_ignore_ascii_case("click")
                })
        });
        names_another_event
            || self
                .resolved_callee_call(file, call.callee)
                .and_then(crate::runtime_semantics::standard_call_name)
                .is_some_and(|name| {
                    name.split('.').next().is_some_and(|target| {
                        matches!(
                            target,
                            "AbortSignal"
                                | "BroadcastChannel"
                                | "EventSource"
                                | "MediaQueryList"
                                | "MessagePort"
                                | "WebSocket"
                                | "Worker"
                                | "XMLHttpRequest"
                        )
                    })
                })
    }

    /// ADR 1080: the call registers a listener for none of `events`: its type is a string literal of another event (the
    /// argument's literal fact, compared without case), or a plain parameter of the function the call is written in whose
    /// every call fills it so, or the target is one whose declaration has no place on a DOM path (as
    /// [`Self::registers_no_click`] reads it).
    fn registers_none_of(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        events: &[&str],
    ) -> bool {
        let other_literal = |argument: &solid_facts::ast::ArgumentFact| {
            !argument.spread
                && matches!(&argument.literal_value, solid_facts::ast::ArgumentLiteralFact::String(name)
                    if !events.iter().any(|event| name.eq_ignore_ascii_case(event)))
        };
        let Some(event) = call.arguments.first() else {
            return false;
        };
        other_literal(event)
            || self.parameter_filled_only_by(file, event.span, &other_literal)
            || self
                .resolved_callee_call(file, call.callee)
                .and_then(crate::runtime_semantics::standard_call_name)
                .is_some_and(|name| {
                    name.split('.').next().is_some_and(|target| {
                        matches!(
                            target,
                            "AbortSignal"
                                | "BroadcastChannel"
                                | "EventSource"
                                | "MediaQueryList"
                                | "MessagePort"
                                | "WebSocket"
                                | "Worker"
                                | "XMLHttpRequest"
                        )
                    })
                })
    }

    /// ADR 1080: `value` is a plain parameter of the function it is written in (an identifier, no default, never written),
    /// that function is bound by a name every reference of which, in any file, is the callee of a direct call in this file
    /// (no copy, export, hand-off or `new`), and every such call passes, at the parameter's position and before any spread,
    /// an argument `accepted` takes. `@solidjs/router`'s `bindEvent(target, type, handler)` is one, its calls passing
    /// `"popstate"`, `"hashchange"`, `"scroll"`, `"pagehide"`, `"pageshow"` and `"load"`.
    fn parameter_filled_only_by(
        &self,
        file: &solid_facts::FileFacts,
        value: Span,
        accepted: &dyn Fn(&solid_facts::ast::ArgumentFact) -> bool,
    ) -> bool {
        let ast = &file.ast;
        let value = ast.peel_ts_sugar_span(value);
        let Some(declaration) = ast.reference_declaration(value) else {
            return false;
        };
        let Some(function) = crate::owners::containing_ast_function(ast, value) else {
            return false;
        };
        let Some(index) = function.parameters.iter().position(|parameter| {
            parameter.shape == solid_facts::ast::BindingShape::Identifier
                && parameter.initializer.is_none()
                && parameter.names.first().map(|name| name.span) == Some(declaration)
        }) else {
            return false;
        };
        if super::binding_written(file, declaration) {
            return false;
        }
        let Some(name) = crate::owners::function_binding_name(file, function) else {
            return false;
        };
        if super::binding_written(file, name.span) {
            return false;
        }
        let Some(symbol) = self.entities.at(file.path.as_str(), name.span) else {
            return false;
        };
        let references = self.symbol_references(symbol.as_ref());
        let mut calls = 0usize;
        for reference in references {
            if reference.path.as_ref() != file.path.as_str() {
                return false;
            }
            let (Ok(start), Ok(end)) = (
                u32::try_from(reference.start_byte),
                u32::try_from(reference.end_byte),
            ) else {
                return false;
            };
            let at = Span::new(start, end);
            if at == name.span {
                continue;
            }
            let Some(caller) = ast.calls_with_peeled_callee(at).next() else {
                return false;
            };
            if caller.construct
                || caller
                    .arguments
                    .iter()
                    .take(index + 1)
                    .any(|argument| argument.spread)
                || !caller.arguments.get(index).is_some_and(accepted)
            {
                return false;
            }
            calls += 1;
        }
        calls > 0
    }

    /// ADR 0639: `site` is written directly in a `createEffect`'s apply. A render
    /// effect's window is the body's, and takes any event.
    fn dispatch_in_effect_apply(
        &self,
        file: &solid_facts::FileFacts,
        site: Span,
    ) -> Option<DispatchCaller> {
        (self.dialect.effect_window_takes_only_its_event()
            && file.ast.arguments_containing(site).any(|(call, index)| {
                crate::execution_role::direct_callback_contains(
                    file,
                    call.arguments[index].span,
                    site,
                ) && self.primitive_at_call(file, call.span)
                    == Some(solid_dialect::Primitive::CreateEffect)
                    && self
                        .dialect
                        .apply_callback_argument(solid_dialect::Primitive::CreateEffect)
                        == Some(index)
            }))
        .then_some(DispatchCaller::EffectApply)
    }

    /// ADR 1099: `site` belongs directly to a `const` arrow or function expression (not a generator) declared inside a
    /// function body, so no other module names it, and every reference to that binding is the callee of a call an
    /// effect's apply makes directly ([`Self::dispatch_in_effect_apply`]): the helper runs only on the apply's stack. A
    /// helper never called, or read any other way (an argument, `.call`, `new`), is no answer.
    fn dispatch_in_effect_apply_helper(
        &self,
        file: &solid_facts::FileFacts,
        site: Span,
    ) -> Option<DispatchCaller> {
        let ast = &file.ast;
        let helper = ast.innermost_function_owning(site)?;
        if helper.generator {
            return None;
        }
        let binding = ast.bindings.iter().find(|binding| {
            binding.immutable
                && binding.shape == solid_facts::ast::BindingShape::Identifier
                && binding
                    .initializer
                    .is_some_and(|initializer| ast.peel_ts_sugar_span(initializer) == helper.span)
        })?;
        if binding.names.len() != 1 || !ast.any_function_body_containing(binding.declaration) {
            return None;
        }
        let declared = binding.names[0].span;
        let mut references = ast
            .identifiers
            .iter()
            .filter(|identifier| {
                identifier.span != declared
                    && ast.reference_declaration(identifier.span) == Some(declared)
            })
            .peekable();
        references.peek()?;
        references
            .all(|identifier| {
                let mut calls = ast.calls_with_peeled_callee(identifier.span);
                calls
                    .next()
                    .is_some_and(|call| self.dispatch_in_effect_apply(file, call.span).is_some())
                    && calls.next().is_none()
            })
            .then_some(DispatchCaller::EffectApply)
    }

    /// `window.dispatchEvent(…)` and `document.dispatchEvent(…)` where the
    /// receiver is the host's global variable, resolved through its
    /// declarations (never its spelling): no project binding, every
    /// declaration ambient in a declaration file no analyzed source is, and
    /// the `lib.dom.d.ts` `var` named `window` or `document`. An event
    /// dispatched on `window` reaches only `window`'s listeners; the delegated
    /// handler of a JSX element needs an element on the event's path
    /// ([`solid_dialect::Dialect::document_dispatch_reaches_no_element_handler`]
    /// for `document`).
    fn dispatch_receiver(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
    ) -> DispatchReceiver {
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        let Some(member) = file.ast.member_at(callee) else {
            return DispatchReceiver::Unknown;
        };
        match self.global_dom_variable(file, file.ast.peel_ts_sugar_span(member.object)) {
            Some("window") => DispatchReceiver::Window,
            Some("document") if self.dialect.document_dispatch_reaches_no_element_handler() => {
                DispatchReceiver::Document
            }
            _ => DispatchReceiver::Unknown,
        }
    }

    /// The name of the `lib.dom.d.ts` global `var` the identifier at `value`
    /// is, when it is one (see [`Self::dispatch_receiver`]).
    pub(crate) fn global_dom_variable(
        &self,
        file: &solid_facts::FileFacts,
        value: Span,
    ) -> Option<&'static str> {
        if file
            .ast
            .identifiers
            .binary_search_by_key(&value, |identifier| identifier.span)
            .is_err()
            || file.ast.reference_declaration(value).is_some()
        {
            return None;
        }
        let symbol = self.symbol_at(file.path.as_str(), value)?;
        if !symbol.alias_target().is_empty() || symbol.declarations().is_empty() {
            return None;
        }
        let mut name = None;
        for declaration in symbol.declarations() {
            let file_name = self.installed_declaration_file(declaration)?;
            if declaration.kind.as_ref() == "variable" && file_name == "lib.dom.d.ts" {
                name = Some(declaration.name.as_ref());
            }
        }
        // The name is the global variable's own, from its declaration.
        match name? {
            "window" => Some("window"),
            "document" => Some("document"),
            _ => None,
        }
    }

    /// ADR 1033: whether the name at `property` (a member's property) resolves to a member of the pinned `lib.dom.d.ts` called `name`, and nothing else:
    /// every declaration of its symbol is there (none in a program file or another declaration file), so the read is the DOM's own property.
    pub(crate) fn lib_dom_member(
        &self,
        file: &solid_facts::FileFacts,
        property: Span,
        name: &str,
    ) -> bool {
        let Some(symbol) = self.symbol_at(file.path.as_str(), property) else {
            return false;
        };
        symbol.alias_target().is_empty()
            && !symbol.declarations().is_empty()
            && symbol.declarations().iter().all(|declaration| {
                self.installed_declaration_file(declaration) == Some("lib.dom.d.ts")
                    && declaration.name.as_ref() == name
            })
    }

    /// The file name of a declaration in a declaration file that no program file is (a library's, not the project's), or `None`.
    fn installed_declaration_file<'d>(
        &self,
        declaration: &'d typefacts::Declaration,
    ) -> Option<&'d str> {
        let path = declaration.location.path.as_ref();
        let file_name = std::path::Path::new(path).file_name()?.to_str()?;
        (self.file_by_path(path).is_none() && file_name.ends_with(".d.ts")).then_some(file_name)
    }

    /// The owner-free caller of a dispatch call, from where the call is
    /// written alone: no event proof, no owner graph (they depend on this).
    /// A call in a function nothing places keeps the veto: unresolved is not
    /// evidence.
    pub(super) fn dispatch_caller(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
    ) -> Option<DispatchCaller> {
        if file.ast.any_function_body_containing(call.span) {
            // ADR 0477: after an await its own function reaches on every path
            // to it, the call runs on a continuation, never on its caller's
            // stack (a JSX expression, tracked region or field initializer
            // there runs later under an owner, and is not a continuation).
            // The callback classes name the caller first.
            return self.dispatch_callback_caller(file, call.span).or_else(|| {
                crate::execution_role::follows_dominating_await(file, call.span)
                    .then_some(DispatchCaller::AwaitContinuation)
            });
        }
        // A parameter list is in no function's body and runs at its function's
        // call, wherever that is (ADR 0613): no caller class.
        if crate::owners::in_parameter_list(&file.ast, call.span) {
            return None;
        }
        // Module evaluation: no reactive owner or window is current before a
        // function is invoked. A class element or JSX region is evaluated
        // elsewhere (a construction, a render effect).
        let in_class = file
            .ast
            .classes
            .iter()
            .any(|class| class.span.contains(call.span));
        let in_jsx = file.ast.jsx_containing(call.span).next().is_some();
        (!in_class && !in_jsx).then_some(DispatchCaller::ModuleInitialization)
    }

    /// The registration call whose suite body (a synchronous function literal
    /// written directly as the argument of a reviewed runner's registration,
    /// which the runner runs inside the call) directly contains `site`.
    pub(crate) fn inline_suite_body_registration<'file>(
        &self,
        file: &'file solid_facts::FileFacts,
        site: Span,
    ) -> Option<&'file solid_facts::ast::CallFact> {
        let (call, _) = file.ast.arguments_containing(site).find(|(call, index)| {
            matches!(
                call.arguments[*index].value,
                solid_facts::ast::ArgumentValueKind::Function
            ) && crate::execution_role::direct_callback_contains(
                file,
                call.arguments[*index].span,
                site,
            )
        })?;
        (self.test_runner_callback(file, call) == Some(TestRunnerCallback::InlineSuiteBody))
            .then_some(call)
    }

    /// The caller of a dispatch inside the function literal handed directly to
    /// a reviewed fresh-stack scheduler or to `runWithOwner(null, …)`.
    fn dispatch_callback_caller(
        &self,
        file: &solid_facts::FileFacts,
        site: Span,
    ) -> Option<DispatchCaller> {
        if self.dialect.fresh_stack_callback_owner() == Some(solid_dialect::CallbackOwner::None)
            && let Some((call, _)) =
                crate::execution_role::fresh_stack_callback_call(file, site, self)
        {
            return Some(
                if self.test_runner_callback(file, call) == Some(TestRunnerCallback::Scheduled) {
                    DispatchCaller::TestRunner
                } else {
                    DispatchCaller::FreshStack
                },
            );
        }
        // ADR 0438: a suite body its runner runs inside the registration call
        // is a synchronous callback of that call, so its caller is the caller
        // of the registration written at that position.
        if let Some(registration) = self.inline_suite_body_registration(file, site) {
            return self.dispatch_caller(file, registration);
        }
        let (call, index) = file.ast.arguments_containing(site).find(|(call, index)| {
            matches!(
                call.arguments[*index].value,
                solid_facts::ast::ArgumentValueKind::Function
                    | solid_facts::ast::ArgumentValueKind::AsyncFunction
            ) && crate::execution_role::direct_callback_contains(
                file,
                call.arguments[*index].span,
                site,
            )
        })?;
        let owner = call.arguments.first()?;
        (index == 1
            && !owner.spread
            && matches!(
                owner.value,
                solid_facts::ast::ArgumentValueKind::Null
                    | solid_facts::ast::ArgumentValueKind::Undefined
            )
            && self.primitive_at_call(file, call.span)
                == Some(solid_dialect::Primitive::RunWithOwner))
        .then_some(DispatchCaller::DetachedOwner)
    }
}

/// The names through which code makes or reaches a shadow tree (ADR 1115): attaching one, reading one, declaring one in parsed
/// markup, a custom element (whose class commonly attaches one), and the calls that hand out a node inside one.
const SHADOW_SPELLINGS: &[&str] = &[
    "attachShadow",
    "shadowRoot",
    "ShadowRoot",
    "shadowrootmode",
    "setHTMLUnsafe",
    "parseHTMLUnsafe",
    "getRootNode",
    "composedPath",
    "customElements",
];

/// ADR 1115: whether some listener of the program may sit in a shadow tree, where rc.21 runs it with `window.event` unset. True
/// (fail closed) when any analysed file (the project's, and the dependency source the build reads) spells a name of
/// [`SHADOW_SPELLINGS`] (a text test, which only refuses more), evaluates code from a string, or loads a module the program does not
/// analyse that the dialect does not own, or one by a computed specifier: code nothing here reads may attach a shadow. rc.21's own
/// `@solidjs/web` attaches none.
pub(crate) fn scan_shadow_roots(lookup: &super::SemanticLookup<'_>) -> bool {
    if lookup.hazard_present(crate::base::Hazard::Eval)
        || lookup.hazard_present(crate::base::Hazard::CodeEvaluatingCall)
    {
        return true;
    }
    let Some(imports) = lookup.resolved_imports() else {
        return true;
    };
    let program = lookup
        .files()
        .iter()
        .map(|file| file.path.as_str())
        .collect::<std::collections::HashSet<_>>();
    lookup.files().iter().any(|file| {
        if SHADOW_SPELLINGS
            .iter()
            .any(|name| file.source.contains(name))
        {
            return true;
        }
        let ast = &file.ast;
        let unread = |at: solid_facts::core::Span, specifier: &str| {
            if lookup.dialect.owns_module(specifier) {
                return false;
            }
            match imports.specifier(file.path.as_str(), at, specifier) {
                solid_facts::SpecifierAttestation::Attested(row) => {
                    row.resolution == solid_facts::ImportResolution::Unresolved
                        || !program.contains(row.resolved_path.as_ref())
                }
                solid_facts::SpecifierAttestation::Unattested => true,
            }
        };
        ast.imports
            .iter()
            .filter(|import| !import.type_only)
            .any(|import| unread(import.span, import.module.as_str()))
            || ast
                .exports
                .iter()
                .filter(|export| !export.type_only)
                .any(|export| {
                    export
                        .module
                        .as_deref()
                        .is_some_and(|module| unread(export.span, module))
                })
            || ast
                .module_loads
                .iter()
                .any(|load| match load.specifier.as_deref() {
                    Some(specifier) => unread(load.span, specifier),
                    // A load by a computed specifier may load anything.
                    None => true,
                })
    })
}
