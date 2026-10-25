//! Visible synchronous DOM dispatch withholds an ownerless event proof.

use super::*;

/// What a synchronous dispatch call is dispatched on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DispatchReceiver {
    /// `HTMLElement.click`: the receiver is an element by its declaration.
    Element,
    /// `EventTarget.dispatchEvent` on a receiver nothing proves.
    Unknown,
    /// The global `window` (ADR 0393).
    Window,
    /// The global `document`, where the dialect says it reaches no JSX
    /// handler (ADR 0393).
    Document,
}

impl DispatchReceiver {
    /// A receiver that cannot reach an element handler.
    const fn excluded(self) -> bool {
        matches!(self, Self::Window | Self::Document)
    }
}

/// A caller whose stack carries no owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DispatchCaller {
    /// A callback a reviewed host scheduler runs from an empty stack.
    FreshStack,
    /// Module top level, outside every function, class and JSX element.
    ModuleInitialization,
    /// The callback of `runWithOwner(null, …)`: no owner, but a strict-read
    /// window of the enclosing code stays open, so only owner proofs use it.
    DetachedOwner,
    /// A callback a reviewed test runner invokes from its own scheduler
    /// (ADR 0396): no owner and no strict-read window.
    TestRunner,
    /// A helper every one of whose invocations is a call written in a test
    /// runner's callback, or in another such helper (ADR 0396): it runs on the
    /// runner's stack or a continuation of it, with no owner and no window.
    TestHelper,
    /// A function whose every invocation is an event delivery (ADR 0327's
    /// census): a delivery it makes runs on the stack of a delivery, which
    /// has no owner and no window (ADR 0395).
    EventHandler,
}

impl DispatchCaller {
    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::FreshStack => "scheduled callback",
            Self::ModuleInitialization => "module initialization",
            Self::DetachedOwner => "runWithOwner(null)",
            Self::TestRunner => "test runner callback",
            Self::TestHelper => "test-only helper",
            Self::EventHandler => "event handler",
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
    /// roles): a delivery can inherit its caller's owner.
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
            "the dispatch veto was read while its census computes (ADR 0395)"
        );
    }
}

#[cfg(debug_assertions)]
impl Drop for VetoComputation {
    fn drop(&mut self) {
        VETO_COMPUTING.set(false);
    }
}

impl SemanticLookup<'_> {
    /// ADR 0356, narrowed by ADR 0393: DOM dispatch runs handlers on its
    /// caller's stack. A visible synchronous dispatch withholds the ownerless
    /// proof for event-only invocations unless its target cannot reach a JSX
    /// handler or its caller has no owner. This is counterevidence, not a
    /// guessed handler edge. A project method with the same spelling does not
    /// select either library declaration.
    pub(crate) fn synchronous_event_dispatch_present(&self) -> bool {
        #[cfg(debug_assertions)]
        VetoComputation::assert_not_computing();
        self.event_dispatch_sites()
            .iter()
            .any(DispatchSite::withholds_owner_proofs)
    }

    /// [`Self::synchronous_event_dispatch_present`] for reads in an event
    /// handler, whose proof also needs the delivery outside a strict-read
    /// window.
    pub(crate) fn synchronous_event_read_dispatch_present(&self) -> bool {
        #[cfg(debug_assertions)]
        VetoComputation::assert_not_computing();
        self.event_dispatch_sites()
            .iter()
            .any(DispatchSite::withholds_read_proofs)
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
                    }
                    .to_owned(),
                    caller: site.caller.map(|caller| caller.word().to_owned()),
                    withholds_owner_proofs: site.withholds_owner_proofs(),
                    withholds_read_proofs: site.withholds_read_proofs(),
                })
            })
            .collect()
    }

    /// Every synchronous dispatch call of the program, classified: the
    /// statically placed sites ([`Self::placed_dispatch_sites`]), then the ones
    /// in an event-only function (ADR 0395) or a test-only helper (ADR 0396).
    ///
    /// The second step reads the two censuses, which read no veto (an
    /// answer asked of the veto while this computes is a bug; debug builds
    /// fail on it). Because the census is independent of the veto, no round
    /// of removal feeds another: a site stays out of the veto by an induction
    /// over time, not by the sites already out.
    pub(crate) fn event_dispatch_sites(&self) -> &[DispatchSite] {
        self.synchronous_event_dispatch.get_or_init(|| {
            #[cfg(debug_assertions)]
            let _guard = VetoComputation::enter();
            let mut sites = self.placed_dispatch_sites().to_vec();
            for site in &mut sites {
                if site.caller.is_none()
                    && !site.receiver.excluded()
                    && self.file_by_path(&site.path).is_some_and(|file| {
                        crate::execution_role::dispatch_in_event_only_function(
                            file, site.span, self,
                        )
                    })
                {
                    site.caller = Some(DispatchCaller::EventHandler);
                } else if site.caller.is_none()
                    && !site.receiver.excluded()
                    && self.file_by_path(&site.path).is_some_and(|file| {
                        crate::execution_role::dispatch_in_test_only_function(file, site.span, self)
                    })
                {
                    site.caller = Some(DispatchCaller::TestHelper);
                }
            }
            sites
        })
    }

    /// Every synchronous dispatch call of the program with the receiver and
    /// caller that its own written position decides. Nothing here may consult
    /// an event proof: the answer feeds them.
    fn placed_dispatch_sites(&self) -> &[DispatchSite] {
        self.placed_event_dispatch.get_or_init(|| {
            let mut sites = Vec::new();
            for file in self.files() {
                for call in &file.ast.calls {
                    let Some(name) = self
                        .resolved_callee_call(file, call.callee)
                        .and_then(crate::runtime_semantics::standard_call_name)
                    else {
                        continue;
                    };
                    let receiver = match name {
                        "HTMLElement.click" => DispatchReceiver::Element,
                        "EventTarget.dispatchEvent" => self.dispatch_receiver(file, call),
                        _ => continue,
                    };
                    sites.push(DispatchSite {
                        path: file.path.to_string(),
                        span: call.span,
                        receiver,
                        caller: self.dispatch_caller(file, call),
                    });
                }
            }
            sites
        })
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
    fn global_dom_variable(
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
            let path = declaration.location.path.as_ref();
            let file_name = std::path::Path::new(path).file_name()?.to_str()?;
            if self.file_by_path(path).is_some() || !file_name.ends_with(".d.ts") {
                return None;
            }
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

    /// The owner-free caller of a dispatch call, from where the call is
    /// written alone: no event proof, no owner graph (they depend on this).
    /// A call in a function nothing places keeps the veto: unresolved is not
    /// evidence.
    fn dispatch_caller(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
    ) -> Option<DispatchCaller> {
        if file.ast.any_function_body_containing(call.span) {
            return self.dispatch_callback_caller(file, call.span);
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
            return Some(if self.is_test_runner_registration(file, call) {
                DispatchCaller::TestRunner
            } else {
                DispatchCaller::FreshStack
            });
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
