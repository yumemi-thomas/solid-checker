//! The deep seam between reactive analysis and dialect rule catalogs.
//!
//! Analysis produces [`Program`] tables. This module alone knows which rows
//! become diagnostics and how a worded diagnostic is assembled; a catalog is
//! a small wording adapter over the closed [`FindingSeed`] vocabulary.

use std::time::Instant;

use typefacts::Location;

use crate::{
    ActionInvocation, AsyncRead, EvidenceStep, Finding, LeafOwnerOperation, OwnerRequirement,
    PrimitiveCreation, Program, ReactiveRead, ReactiveWrite, RuleMetadata, SolveTimings,
    StaticDefect, StaticDefectKind, StaticViolation, finish_findings,
};

/// The few phrases where shared static-defect concepts use dialect APIs.
pub struct StaticDefectTerms {
    pub props_destructure_hint: &'static str,
    pub reactive_object_destructure_hint: &'static str,
    pub missing_effect_message: &'static str,
    pub missing_effect_hint: &'static str,
    pub store_mutation_hint: fn(&str) -> String,
    /// A dialect-owned override for the unavailable-package-behavior hint:
    /// when the dialect knows the export was removed or renamed upstream (the
    /// Solid 2.0 catalog's removed-1.x-API map), the hint should point at
    /// the migration, not at making the source of an export that no longer
    /// exists available. Returning `None` keeps the generic hint.
    pub removed_export_hint: fn(module: &str, export: &str) -> Option<String>,
    /// What the runtime throws when a value that is not JSON-serializable
    /// reaches the default server-function transport, quoted so the message
    /// matches what the developer will see in their console.
    pub rich_argument_transport_throw: &'static str,
    /// The four rich-argument remedies. The message states the fact the
    /// analysis proved, which is the same in any dialect with this transport;
    /// the hint names the serializer to install and the module it comes from,
    /// which is not.
    pub rich_argument_resolved_hint: &'static str,
    pub rich_argument_nested_hint: &'static str,
    pub rich_argument_primitive_hint: &'static str,
    pub rich_argument_unresolved_hint: &'static str,
}

pub struct StaticDefectText {
    pub message: String,
    pub hint: String,
    pub evidence: &'static str,
}

/// Shared prose for version-independent defect concepts. The catalog still
/// owns the dialect terms and external rule identity; identical sentences
/// have one implementation, so they cannot drift between adapters.
#[must_use]
pub fn static_defect_text(defect: &StaticDefect, terms: &StaticDefectTerms) -> StaticDefectText {
    let (message, hint) = match &defect.kind {
        StaticDefectKind::ReactiveObjectDestructure {
            source,
            component_props,
        } => {
            if *component_props {
                (
                    "destructuring props unwraps each property once outside tracking; the bindings are frozen values, and the component never updates when the parent passes new props".into(),
                    terms.props_destructure_hint.into(),
                )
            } else {
                (
                    format!(
                        "destructuring reactive object {source:?} reads its properties once outside tracking; the bindings are frozen values and do not update when the reactive object changes"
                    ),
                    terms.reactive_object_destructure_hint.into(),
                )
            }
        }
        StaticDefectKind::ReactiveReadAfterAwait { accessor, .. } => (
            format!(
                "reactive accessor {accessor:?} is read after an await; dependency tracking ends at the first await, so this read registers no dependency and the computation never re-runs when {accessor:?} changes"
            ),
            "Read reactive values before the first await and carry the results through the async work. If the value must stay live after the await, split the read into its own synchronous computation.".into(),
        ),
        StaticDefectKind::ReactiveReadAfterAwaitThroughCall { accessor, via } => (
            format!(
                "{via} reads reactive accessor {accessor:?} and is called after an await, where dependency tracking has ended; whether it reads {accessor:?} on this call, rather than only on some paths or later, is not proven, so this untracked read is uncertifiable"
            ),
            "Read reactive values before the first await and pass the results to the helper. If the value must stay live after the await, split the read into its own synchronous computation.".into(),
        ),
        StaticDefectKind::ComponentReturnsConditionally => (
            "this component's return value depends on a reactive condition, but a component body runs once; whichever branch is taken at setup renders forever, and the condition is never re-evaluated".into(),
            "Return a single JSX tree and move the branch into it: wrap the alternatives in <Show when={...} fallback={...}> (or <Switch>/<Match> for multiple cases), or use a ternary inside JSX where it stays tracked.".into(),
        ),
        StaticDefectKind::PackageBehaviorUnavailable {
            module,
            export,
            reexported,
            ..
        } => (
            format!(
                "{module} is not analyzed from source here, so the behaviour of {} export {export} is unknown; solid-checker cannot tell whether it reads reactive values, takes tracked callbacks, or returns accessors, so code flowing through it cannot be certified",
                if *reexported { "re-exported" } else { "imported" }
            ),
            (terms.removed_export_hint)(module, export)
                .unwrap_or_else(|| PACKAGE_BEHAVIOUR_UNKNOWN_HINT.to_owned()),
        ),
        StaticDefectKind::MissingEffectFunction => {
            let mut message = terms.missing_effect_message.to_string();
            let mut hint = terms.missing_effect_hint.to_string();
            if defect.uncertain {
                match defect.analysis_context.as_str() {
                    "effect-runtime-entry-uncertain" => {
                        message.push_str(
                            "; a use-server directive makes the selected Solid runtime entry unknown, so this is a client-runtime proof obligation rather than a proven defect",
                        );
                        hint.push_str(
                            " Prove the effective client/server entry through project compiler facts before treating this as a violation.",
                        );
                    }
                    "effect-runtime-entry-and-argument-shape-uncertain" => {
                        message.push_str(
                            "; both the selected Solid runtime entry and the runtime effect argument shape remain unresolved, so neither failure nor safety is proven",
                        );
                        hint.push_str(
                            " Prove the effective client/server entry and the callable runtime argument before treating this as a violation or as safe.",
                        );
                    }
                    _ => {
                        message.push_str(
                            "; the runtime effect argument may be callable or non-callable, so neither failure nor safety is proven",
                        );
                        hint.push_str(
                            " Pass a statically resolved callable function or an exact effect bundle to certify safety.",
                        );
                    }
                }
            }
            (message, hint)
        }
        StaticDefectKind::ReactiveSourceUncaptured { source, callee } => (
            format!(
                "the reactive source {source:?} is passed to {callee}, whose reactive behaviour is not described anywhere: it has no body this analysis reads and is not a Solid primitive; whether reads through it stay tracked cannot be certified"
            ),
            format!(
                "If {callee} comes from a package, it is not analyzed from source here: {PACKAGE_SOURCE_PREMISE} Otherwise, route the call through a project-local adapter whose body makes invocation or retention explicit."
            ),
        ),
        StaticDefectKind::ReactiveDispatchUnresolved {
            callee,
            member: None,
        } if defect
            .analysis_context
            .starts_with(crate::DEPENDENCY_SOURCE_GETTER) =>
        {
            (
                format!(
                    "{callee} may run a getter that a package analyzed as source defines and that reads reactive state, but no proof carries the object, its member and this read's execution from the package to this Get; whether the read is untracked can be neither proven nor ruled out"
                ),
                "Read the member inside JSX or a memo (ADR 0272).".into(),
            )
        }
        StaticDefectKind::ReactiveDispatchUnresolved {
            callee,
            member: None,
        } if defect
            .analysis_context
            .starts_with(crate::STORE_KEY_UNDECIDED) =>
        {
            let reason = &defect.analysis_context[crate::STORE_KEY_UNDECIDED.len()..];
            (
                format!(
                    "{callee} reads a store outside tracking, but whether this Get warns STRICT_READ_UNTRACKED depends on its key: {reason}; whether the read is a strict-read violation can be neither proven nor ruled out"
                ),
                "Read the member inside JSX, a createMemo, or an effect's compute function, or use a known string key solid-checker can decide (ADR 0293).".into(),
            )
        }
        StaticDefectKind::ReactiveDispatchUnresolved {
            callee,
            member: None,
        } if defect.analysis_context == crate::TYPE_ONLY_SOURCE => (
            format!(
                "{callee} runs where a reactive read would not be tracked, but its only evidence of reading reactive state is its declared type, a Solid source alias that a plain function or object also satisfies; no implementation or source flow proves what it reads, so whether this is an untracked reactive read can be neither proven nor ruled out"
            ),
            "Read it inside JSX, a createMemo, or an effect's compute function, or make its source visible to solid-checker: create it with a Solid primitive in the project, or pass one through a path the analysis follows (ADR 0294).".into(),
        ),
        StaticDefectKind::ReactiveDispatchUnresolved {
            callee,
            member: None,
        } if defect.analysis_context == crate::DEPENDENCY_SOURCE_CALL_ERROR => (
            format!(
                "the program that analyzes packages from source types this call of {callee} as an error that TypeScript does not report on the project's own program, so its target and behaviour there are not the project's; whether it reads reactive state cannot be certified"
            ),
            "Packages analyzed from source change what TypeScript sees (for example a package's global augmentation merged into a project interface); make the project's declaration unambiguous, or rename it apart from the package's.".into(),
        ),
        StaticDefectKind::ReactiveDispatchUnresolved {
            callee,
            member: None,
        } if defect
            .analysis_context
            .starts_with(crate::FORWARDED_TO_UNDESCRIBED_CALLEE) =>
        {
            let inner = &defect.analysis_context[crate::FORWARDED_TO_UNDESCRIBED_CALLEE.len()..];
            (
                format!(
                    "this reactive source is passed to {callee}, whose body may hand it on to {inner}, a callee whose reactive behaviour nothing analyzed describes; whether reads through it stay tracked can be neither proven nor ruled out"
                ),
                format!(
                    "Call {inner} with the source where its behaviour is visible, or make {inner}'s source available to solid-checker (ADR 0370)."
                ),
            )
        }
        StaticDefectKind::ReactiveDispatchUnresolved {
            callee,
            member: None,
        } if defect.analysis_context == crate::LISTENER_REACH => (
            format!(
                "{callee} registers a listener that calls project code, and the program holds a synchronous dispatch not proven to run with no owner and no strict-read window, so that code may run on the dispatcher's stack; no proof follows the listener to the reads and writes it reaches, so whether they are legal can be neither proven nor ruled out"
            ),
            "Dispatch from an event handler, a timer or after an await, or register the listener where it is the only caller (ADR 0645).".into(),
        ),
        StaticDefectKind::ReactiveDispatchUnresolved {
            callee,
            member: None,
        } if defect.analysis_context == crate::INSTALLED_MEMBER => (
            format!(
                "{callee} installs a prototype or an accessor on an object, and no Get or write of that object is followed to what it installed: a store as a prototype makes each of them the store's, and an installed setter makes a plain write a call; whether one runs where Solid warns can be neither proven nor ruled out"
            ),
            "Read the store or call the setter by name, where the rules can see it (ADR 0643).".into(),
        ),
        StaticDefectKind::ReactiveDispatchUnresolved {
            callee,
            member: None,
        } if defect.analysis_context == crate::PRIMITIVE_VALUE => (
            format!(
                "Solid primitive {callee} is used as a value here, so the calls that reach it are not followed; the owner, tracking and write rules that apply where it runs are not checked"
            ),
            "Call the primitive directly by its imported name, so solid-checker can see each call.".into(),
        ),
        StaticDefectKind::ReactiveDispatchUnresolved {
            callee,
            member: None,
        } if defect.analysis_context == crate::blockers::PRIMITIVE_ALIAS_CALL => (
            format!(
                "This call resolves to Solid's {callee} through a name solid-checker cannot follow, so no rule sees what {callee} does here"
            ),
            "Call the primitive directly by its imported name.".into(),
        ),
        StaticDefectKind::ReactiveDispatchUnresolved {
            callee,
            member: None,
        } if defect.analysis_context == crate::blockers::IMPORT_UNRESOLVED => (
            format!(
                "{callee} is imported, but the compiler did not resolve the import, so what the value is, and what a call, read or write through it does here, cannot be proven"
            ),
            "Make the imported module and its export resolvable (generate it, install it, or declare its types), so solid-checker can see the value.".into(),
        ),
        StaticDefectKind::ReactiveDispatchUnresolved {
            callee,
            member: None,
        } if defect.analysis_context == crate::blockers::PRIMITIVE_IDENTITY_UNPROVEN => (
            format!(
                "{callee} is used as a value, and which function it is cannot be proven: the type facts attribute a Solid primitive to it, but no import of a Solid module names it, so the calls that reach it are not followed"
            ),
            "Import the value by name from its module, so its identity is exact, or make that module's source available to solid-checker.".into(),
        ),
        StaticDefectKind::ReactiveDispatchUnresolved {
            callee,
            member: Some(member),
        } if defect.analysis_context == crate::PRIMITIVE_RESULT_METHOD => (
            format!(
                "`.{member}(...)` is called on the result of {callee}, so the value it returns is not the getter, tuple or setter the primitive made, and which one it is cannot be proven; a read of it through a helper cannot be certified"
            ),
            "Destructure the primitive's result directly (`const [value, setValue] = createSignal(...)`) so solid-checker can see which slot is the getter.".into(),
        ),
        StaticDefectKind::ReactiveDispatchUnresolved { callee, member } => (
            if let Some(member) = member {
                format!(
                    "{callee} invokes .{member} on a caller-supplied value, but the exact runtime implementation cannot be selected and the possible implementations do not have one proven reactive-read behavior"
                )
            } else {
                format!(
                    "the runtime target of {callee} cannot be selected exactly, and its possible implementations do not have one proven reactive-read behavior"
                )
            },
            "Narrow the value to one exact implementation, or wrap the alternatives in an adapter whose body is available to solid-checker and has one explicit reactive behavior.".into(),
        ),
        StaticDefectKind::ReactiveCallbackUnresolved { callee } => (
            format!(
                "{callee} invokes its callback synchronously after tracking has ended, but the exact callback body cannot be resolved; whether it reads reactive state in that untracked extent cannot be certified"
            ),
            "Pass an exact synchronous function literal directly, or keep the callback body in the project in a form solid-checker can inspect.".into(),
        ),
        StaticDefectKind::ResultAccessCallbackUnplaced { callee } => {
            let reason = match defect.analysis_context.as_str() {
                crate::RESULT_ACCESS_REACTIVE_OPERATION => {
                    "reads or writes reactive state (a signal, store, prop, setter or action)"
                }
                crate::RESULT_ACCESS_OPAQUE_CALL => {
                    "calls code outside the standard library, whose reactive behaviour would run in that scope"
                }
                _ => "has a body solid-checker cannot inspect at this call",
            };
            (
                format!(
                    "{callee} calls this predicate whenever the object it returns is read (a property get, an `in` test, a key enumeration, a spread or merge), in the reading computation's tracking scope and under its owner, or once per property during the call where Proxy is unavailable; the predicate {reason}, and the reader's scope is not known here, so whether it subscribes, goes stale, or writes in an owned scope cannot be certified"
                ),
                "Keep the predicate a pure function of its key, with no signal, store, props or setter access and no calls outside the standard library, or pass the hidden keys as a list instead of a predicate.".into(),
            )
        }
        StaticDefectKind::StructuredReturnUnresolved {
            function,
            property,
            reason,
        } => (
            format!(
                "exported function {function} returns shorthand property {property:?}, but its runtime value cannot be resolved exactly ({reason}); whether that property is a reactive accessor or store path cannot be certified"
            ),
            "Use an exact project-relative named/default import, return an explicitly resolved local binding, or make the external value's source available to the analysis.".into(),
        ),
        StaticDefectKind::ReactiveHandlerRead {
            attribute,
            expression,
        } => (
            format!(
                "{attribute} reads {expression} once during DOM setup; later reactive updates cannot replace the installed listener"
            ),
            format!(
                "Wrap the read so it happens when the event fires: {attribute}={{event => {expression}(event)}}."
            ),
        ),
        StaticDefectKind::HandlerTargetUnresolved {
            attribute,
            expression,
        } => (
            format!(
                "{attribute} hands {expression} to an element as an event handler, and the function it holds is not resolved exactly, so the owner it runs under is not proven"
            ),
            "Bind the handler to a function literal the analysis can read, or write the object literal in the spread or the attribute directly.".into(),
        ),
        StaticDefectKind::HandlerValueUnresolved {
            attribute,
            expression,
        } => {
            if defect.uncertain {
                (
                    format!(
                        "{attribute} is lowered as a native listener, but the runtime shape of {expression} cannot be certified as a callable handler or a valid bound-handler pair"
                    ),
                    format!(
                        "Pass a function directly to {attribute}, or use an explicit two-slot bound-handler tuple whose first slot is callable."
                    ),
                )
            } else {
                (
                    format!(
                        "{attribute} is lowered as a native listener, but {expression} is proven non-callable and not an array-backed bound-handler pair"
                    ),
                    format!("Pass a function to {attribute} instead."),
                )
            }
        }
        StaticDefectKind::UncalledAccessor { name, position } => (
            format!(
                "accessor {name:?} is used as a value in {position}; the expression receives the accessor function itself, not the value it holds, and never updates"
            ),
            format!(
                "Call it: {name}(). Passing {name} uncalled is only correct where the receiver calls it later."
            ),
        ),
        StaticDefectKind::DirectMutation { name, target } => {
            crate::direct_mutation_wording(name, *target, terms.store_mutation_hint)
        }
        StaticDefectKind::ServerFunctionRichArgument { transport } => match transport {
            crate::RichArgumentTransport::ResolvedType {
                function,
                descriptor,
                member,
            } => (
                format!(
                    "server function {function} receives an argument typed {descriptor} ({member}); server-function arguments travel as plain JSON by default, and a value JSON cannot carry faithfully throws at the transport: {:?}",
                    terms.rich_argument_transport_throw
                ),
                terms.rich_argument_resolved_hint.into(),
            ),
            crate::RichArgumentTransport::NestedValue { function } => (
                format!(
                    "server function {function} receives an object holding a Date, Map, Set, RegExp, or typed array; the default server-function transport is plain JSON, which reaches nested values, so the nested one is silently flattened rather than sent"
                ),
                terms.rich_argument_nested_hint.into(),
            ),
            crate::RichArgumentTransport::NonJsonPrimitive { function } => (
                format!(
                    "server function {function} receives a bigint, symbol, or undefined value; the default server-function transport is plain JSON and cannot encode that primitive faithfully"
                ),
                terms.rich_argument_primitive_hint.into(),
            ),
            crate::RichArgumentTransport::Unresolved { reason } => (
                format!(
                    "a client call to a server function has an unresolved rich-argument transport proof: {reason}"
                ),
                terms.rich_argument_unresolved_hint.into(),
            ),
        },
    };
    let evidence = match &defect.kind {
        // The nested claim is about a value the argument *holds*, not the
        // argument's own resolved type, so it cannot borrow the sentence
        // below: that one would assert a fact the analysis never proved. The
        // open-proof case keeps the resolved-type sentence it has always
        // carried; its own uncertainty is what its message says.
        StaticDefectKind::ServerFunctionRichArgument {
            transport: crate::RichArgumentTransport::NestedValue { .. },
        } => {
            "the callee carries a \"use server\" directive, a closed object literal reaching it holds a value in the JSON-unsafe set, and nothing in the project installs an argument serializer"
        }
        StaticDefectKind::ServerFunctionRichArgument { .. } => {
            "the callee carries a \"use server\" directive, the argument's resolved type is in the JSON-unsafe set, and nothing in the project installs an argument serializer"
        }
        StaticDefectKind::ReactiveObjectDestructure {
            component_props: true,
            ..
        } => {
            "the destructuring pattern is bound to proven component props and executes outside tracking"
        }
        StaticDefectKind::ReactiveObjectDestructure {
            component_props: false,
            ..
        } => {
            "the destructuring initializer is proven to return a reactive object and executes outside tracking"
        }
        StaticDefectKind::ComponentReturnsConditionally => {
            "a proven reactive read controls the component's return shape"
        }
        StaticDefectKind::PackageBehaviorUnavailable { .. } => {
            "the imported package's behavior is not available from analyzed source"
        }
        StaticDefectKind::ReactiveDispatchUnresolved { .. }
            if defect.analysis_context == crate::DEPENDENCY_SOURCE_CALL_ERROR =>
        {
            "TypeScript reports a call error here only in the program that adds package sources, not on the project's own program"
        }
        StaticDefectKind::ReactiveDispatchUnresolved { .. }
            if defect
                .analysis_context
                .starts_with(crate::FORWARDED_TO_UNDESCRIBED_CALLEE) =>
        {
            "a reference bound to a reactive accessor occupies this argument position of an analyzed function whose parameter may reach a callee no analyzed source or primitive describes"
        }
        StaticDefectKind::ReactiveDispatchUnresolved { .. } => {
            "the call is type-correct, but exact runtime dispatch or equivalent reactive summaries are not proven"
        }
        StaticDefectKind::ReactiveCallbackUnresolved { .. } => {
            "the built-in callback position is type-correct and synchronous, but the callback body's reactive reads are not available for proof"
        }
        StaticDefectKind::ResultAccessCallbackUnplaced { .. } => {
            "the predicate runs on reads of the returned object, in the reader's tracking scope and ownership, which the call site does not determine"
        }
        StaticDefectKind::StructuredReturnUnresolved { .. } => {
            "the exported shorthand value is type-correct, but its exact runtime binding and reactive return behavior are not proven"
        }
        StaticDefectKind::HandlerTargetUnresolved { .. } if defect.uncertain => {
            "the handler's function is not resolved to an exact literal, so whether it runs under an owner is not proven"
        }
        StaticDefectKind::HandlerValueUnresolved { .. } if defect.uncertain => {
            "TypeScript deliberately skips this hyphenated JSX attribute name, and the runtime handler shape is not closed by the available compiler facts"
        }
        StaticDefectKind::MissingEffectFunction if defect.uncertain => {
            match defect.analysis_context.as_str() {
                "effect-runtime-entry-uncertain" => {
                    "the client entry rejects this effect shape, but the source directive alone does not prove whether the client or server entry executes"
                }
                "effect-runtime-entry-and-argument-shape-uncertain" => {
                    "neither the selected runtime entry nor the callable shape of the runtime effect argument is proven"
                }
                _ => {
                    "the published call is valid, but the runtime effect argument is not proven callable or non-callable"
                }
            }
        }
        // The argument position is what the facts establish. Neither a valid
        // call nor an exact callee proves the call runs or what it does with
        // the accessor, so the claim stays conditional: it asserts no
        // invocation, no invalid API, and no untracked read.
        StaticDefectKind::ReactiveSourceUncaptured { .. } => {
            "a reference bound to a reactive accessor occupies this call expression's argument position; if the call executes, the available analysis does not establish whether reads through that accessor stay tracked"
        }
        StaticDefectKind::ReactiveReadAfterAwaitThroughCall { .. } => {
            "this call runs after the computation's first await, and the callee's body reads the accessor"
        }
        StaticDefectKind::ReactiveReadAfterAwait { .. }
        | StaticDefectKind::MissingEffectFunction
        | StaticDefectKind::ReactiveHandlerRead { .. }
        | StaticDefectKind::HandlerValueUnresolved { .. }
        | StaticDefectKind::HandlerTargetUnresolved { .. }
        | StaticDefectKind::UncalledAccessor { .. }
        | StaticDefectKind::DirectMutation { .. } => {
            "the invalid API shape is statically present at this call"
        }
    };
    StaticDefectText {
        message,
        hint,
        evidence,
    }
}

/// The program tables a dialect catalog intentionally projects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogCapabilities {
    pub actions: bool,
    pub async_reads: bool,
    pub leaf_operations: bool,
    pub directive_creations: bool,
    /// Whether one-shot rendering/component bodies are included in the
    /// owned-write rule. Solid 2.0's runtime guard rejects them; Solid 1.x
    /// only has the feedback-loop hazard inside genuinely tracked scopes.
    pub untracked_rendering_writes: bool,
    /// Whether module-scope reads are reported by the strict-read rule.
    /// The rc.0 runtime installs strict-read contexts only inside component
    /// and effect bodies (probed: a module-scope signal or memo read emits no
    /// `STRICT_READ_UNTRACKED`), so the 2.0 catalog stays silent there; the
    /// 1.x catalog keeps upstream `reactivity` semantics, which report
    /// module-scope-adjacent reads.
    pub module_scope_strict_reads: bool,
    /// Whether a `reactive-handler-frozen` finding owns the
    /// handler expression it claims, suppressing the strict-read finding on
    /// the identical span (the README's one-defect-class-one-rule policy).
    /// The 1.x catalog keeps both, pinned by the upstream parity ledger's
    /// declared rule-split deviation.
    pub handler_expression_owns_strict_read: bool,
}

impl CatalogCapabilities {
    pub const SOLID_1: Self = Self {
        actions: false,
        async_reads: false,
        leaf_operations: false,
        directive_creations: false,
        untracked_rendering_writes: false,
        module_scope_strict_reads: true,
        handler_expression_owns_strict_read: false,
    };

    pub const SOLID_2: Self = Self {
        actions: true,
        async_reads: true,
        leaf_operations: true,
        directive_creations: true,
        untracked_rendering_writes: true,
        module_scope_strict_reads: false,
        handler_expression_owns_strict_read: true,
    };
}

/// Every analysis result that can become a finding.
#[derive(Clone, Copy, Debug)]
pub enum FindingSeed<'a> {
    StrictRead {
        read: &'a ReactiveRead,
        compute_reads_source: bool,
        /// ADR 0467: the read is an effect apply's read of a source proven
        /// async. A run while the source is pending throws
        /// `PENDING_ASYNC_UNTRACKED_READ` instead of the strict-read warning;
        /// which one a run meets is the apply's timing, not proven here.
        apply_reads_async_source: bool,
    },
    OwnedWrite(&'a ReactiveWrite),
    Action(&'a ActionInvocation),
    LeafOperation(&'a LeafOwnerOperation),
    StaticViolation(&'a StaticViolation),
    StaticDefect(&'a StaticDefect),
    DirectiveCreation(&'a PrimitiveCreation),
    OwnerRequirement(&'a OwnerRequirement),
    AsyncRead(&'a AsyncRead),
}

/// The catalog-owned sentences and identity for one typed seed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FindingWording {
    pub metadata: RuleMetadata,
    pub message: String,
    pub hint: String,
    pub evidence: Vec<EvidenceStep>,
}

impl FindingWording {
    #[must_use]
    pub fn new(
        metadata: RuleMetadata,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self {
            metadata,
            message: message.into(),
            hint: hint.into(),
            evidence: vec![],
        }
    }

    #[must_use]
    pub fn with_evidence(mut self, evidence: Vec<EvidenceStep>) -> Self {
        self.evidence = evidence;
        self
    }
}

/// One dialect catalog at the projection seam.
pub trait CatalogWording {
    fn capabilities(&self) -> CatalogCapabilities;
    fn wording(&self, seed: FindingSeed<'_>) -> FindingWording;
    /// ADR 0659: whether this rule's claim is what the client runtime does, so
    /// it holds only where a client build runs the site.
    fn claims_client_runtime(&self, code: &str) -> bool {
        let _ = code;
        false
    }
}

/// Projects all enabled tables and reports construction/ordering time.
#[must_use]
pub fn project_findings(
    program: &Program,
    catalog: &impl CatalogWording,
) -> (Vec<Finding>, SolveTimings) {
    let total_started = Instant::now();
    let construction_started = Instant::now();
    let capabilities = catalog.capabilities();
    let mut findings = Vec::new();
    let async_apply_reads = program
        .async_reads
        .iter()
        .filter(|read| read.async_provenance && read.execution == crate::ExecutionRole::EffectApply)
        .map(|read| &read.location)
        .collect::<std::collections::HashSet<_>>();

    // ADR 0504: where an apply's first run is proven to meet its source
    // pending, rc.17 throws before it would warn: the strict read is not
    // reported there, the pending read is.
    let pending_rows = program
        .async_reads
        .iter()
        .filter(|read| read.apply_first_run_pending)
        .collect::<Vec<_>>();
    let pending_first_runs = pending_rows
        .iter()
        .map(|read| &read.location)
        .collect::<std::collections::HashSet<_>>();
    // A strict read with the row's origin is withdrawn only inside the same
    // apply: another apply that reaches the same read runs at another time.
    let withdrawn_through_origin = |read: &crate::ReactiveRead| {
        read.origin.as_ref().is_some_and(|origin| {
            pending_rows.iter().any(|row| {
                row.pending_origin.as_ref() == Some(origin)
                    && row.pending_apply.as_ref().is_some_and(|apply| {
                        apply.path == read.location.path
                            && apply.start_byte <= read.location.start_byte
                            && read.location.end_byte <= apply.end_byte
                    })
            })
        })
    };
    // ADR 0509: a strict read of a proven async source that may meet it
    // pending (its async row's role reports nothing it proves) is unproven:
    // a pending read throws before rc.17 warns, and an apply whose compute
    // reads nothing never re-runs to warn later. The read is an obligation.
    let async_unproven_reads = program
        .async_reads
        .iter()
        // ADR 0838: an accounting row adds a claim, it never opens a strict
        // read the producers decided.
        .filter(|read| read.async_provenance && !read.apply_first_run_pending && !read.unaccounted)
        .map(|read| &read.location)
        .collect::<std::collections::HashSet<_>>();
    let open_reads = program
        .reads
        .iter()
        .filter(|read| async_unproven_reads.contains(&read.location) && !read.uncertain)
        .map(|read| {
            let mut open = read.clone();
            open.uncertain = true;
            open
        })
        .collect::<Vec<_>>();
    let open_at = open_reads
        .iter()
        .map(|read| (&read.location, read))
        .collect::<std::collections::HashMap<_, _>>();
    // ADR 0933: one account of why a read row is or is not projected, shared by
    // the filter below and the proof log, so the log cannot drift from the
    // rule: the same clauses, in the same order.
    let read_withheld = |read: &crate::ReactiveRead| {
        read_silence(read, capabilities).or_else(|| {
            if pending_first_runs.contains(&read.location) {
                Some(ReadSilence::PendingFirstRun)
            } else if withdrawn_through_origin(read) {
                Some(ReadSilence::WithdrawnThroughOrigin)
            } else {
                None
            }
        })
    };
    if crate::proof_log::enabled() {
        for read in &program.reads {
            let why = read_withheld(read);
            crate::proof_log::row_found_at(
                crate::proof_log::RowTable::Read,
                &read.location,
                Some(read.execution),
                why.is_none(),
                why.map(ReadSilence::name),
                read.origin.as_ref().filter(|_| read.reported_at_origin),
            );
        }
        for write in &program.writes {
            let why = write_silence(write, capabilities);
            crate::proof_log::row(
                crate::proof_log::RowTable::Write,
                &write.location,
                Some(write.execution),
                why.is_none(),
                why.map(WriteSilence::name),
            );
        }
        for action in &program.actions {
            let why = if capabilities.actions {
                action_silence(action)
            } else {
                Some("capability-off")
            };
            crate::proof_log::row(
                crate::proof_log::RowTable::Action,
                &action.location,
                Some(action.execution),
                why.is_none(),
                why,
            );
        }
    }
    findings.extend(
        program
            .reads
            .iter()
            .filter(|read| read_withheld(read).is_none())
            .map(|read| {
                let read = if read.uncertain {
                    read
                } else {
                    open_at.get(&read.location).copied().unwrap_or(read)
                };
                project_finding(
                    FindingSeed::StrictRead {
                        read,
                        compute_reads_source: program.apply_compute_reads.contains(&read.location),
                        apply_reads_async_source: async_apply_reads.contains(&read.location),
                    },
                    catalog,
                )
            }),
    );
    findings.extend(
        distinct_runs(&program.writes, ReactiveWrite::same_row)
            .filter(|write| write_silence(write, capabilities).is_none())
            .map(|write| project_finding(FindingSeed::OwnedWrite(write), catalog)),
    );
    if capabilities.leaf_operations {
        findings.extend(
            distinct_leaf_operations(&program.leaf_operations)
                .into_iter()
                // A call written in the tracked scope has the rule's own finding.
                .filter(|operation| {
                    !operation.observer_guarded()
                        || !program.static_violations.iter().any(|violation| {
                            !violation.uncertain && violation.location == operation.location
                        })
                })
                .map(|operation| project_finding(FindingSeed::LeafOperation(operation), catalog)),
        );
    }
    findings.extend(collapse_unanalyzed_package_defects(
        &program.static_defects,
        catalog,
    ));
    findings.extend(
        program
            .static_violations
            .iter()
            .map(|violation| project_finding(FindingSeed::StaticViolation(violation), catalog)),
    );
    if capabilities.directive_creations {
        findings.extend(
            program
                .directive_creations
                .iter()
                .map(|creation| project_finding(FindingSeed::DirectiveCreation(creation), catalog)),
        );
    }
    findings.extend(
        program
            .missing_owners
            .iter()
            .filter(|requirement| requirement.report)
            .map(|requirement| {
                project_finding(FindingSeed::OwnerRequirement(requirement), catalog)
            }),
    );
    // ADR 0504: a call-attributed pending row stands for its origin read; where
    // that read has its own row, it is reported there only.
    let direct_pending = program
        .async_reads
        .iter()
        .filter(|read| read.apply_first_run_pending && read.pending_origin.is_none())
        .map(|read| &read.location)
        .collect::<std::collections::HashSet<_>>();
    // ADR 0509: where a strict read of a proven async source is open because
    // it may meet the source pending (`open_at`), the pending read is the
    // other outcome: an obligation of its own rule.
    let reported_open = || {
        program
            .reads
            .iter()
            .filter(|read| {
                read_reported(read, capabilities) && open_at.contains_key(&read.location)
            })
            .map(|read| &read.location)
            .collect::<std::collections::HashSet<_>>()
    };
    // ADR 0933: how each async row ends, from the same clauses as the two
    // projections below.
    if crate::proof_log::enabled() {
        let open = reported_open();
        for read in &program.async_reads {
            let clause = async_read_clause(read);
            let superseded = read
                .pending_origin
                .as_ref()
                .is_some_and(|origin| direct_pending.contains(origin));
            let (reported, why) = if !capabilities.async_reads {
                (false, Some("capability-off"))
            } else if !clause.reports()
                && read.async_provenance
                && !read.apply_first_run_pending
                && open.contains(&read.location)
            {
                (true, Some("open-strict-read"))
            } else if clause.reports() && superseded {
                (false, Some("superseded-by-direct-pending"))
            } else if clause.reports() {
                (true, None)
            } else {
                (false, Some(clause.name()))
            };
            crate::proof_log::row(
                crate::proof_log::RowTable::AsyncRead,
                &read.location,
                Some(read.execution),
                reported,
                why,
            );
        }
    }
    if capabilities.async_reads {
        let reported_open = reported_open();
        findings.extend(
            program
                .async_reads
                .iter()
                .filter(|read| {
                    read.async_provenance
                        && !read.apply_first_run_pending
                        && !async_read_reported(read)
                        && reported_open.contains(&read.location)
                })
                .map(|read| {
                    let mut pending = read.clone();
                    pending.apply_first_run_pending = true;
                    let mut finding = project_finding(FindingSeed::AsyncRead(&pending), catalog);
                    finding.kind = "uncertifiable".into();
                    crate::blockers::SOURCE_PENDING_UNPROVEN
                        .clone_into(&mut finding.analysis_context);
                    finding
                }),
        );
    }
    if capabilities.async_reads {
        findings.extend(
            program
                .async_reads
                .iter()
                .filter(|read| async_read_reported(read))
                .filter(|read| {
                    !read
                        .pending_origin
                        .as_ref()
                        .is_some_and(|origin| direct_pending.contains(origin))
                })
                .map(|read| project_finding(FindingSeed::AsyncRead(read), catalog)),
        );
    }
    if capabilities.actions {
        findings.extend(
            distinct_runs(&program.actions, ActionInvocation::same_row)
                .filter(|action| action_reported(action))
                .map(|action| project_finding(FindingSeed::Action(action), catalog)),
        );
    }

    // ADR 0933: each pass below that can drop or demote a finding is a stage
    // the proof log names (a no-op with the log off).
    let stage = crate::proof_log::ProofStage::begin(&findings);
    for finding in &mut findings {
        if finding.kind == "violation" {
            program.runtime_configuration.apply(finding);
        }
    }
    if let Some(stage) = stage {
        stage.end(crate::DropPass::RuntimeConfiguration, &findings);
    }
    let stage = crate::proof_log::ProofStage::begin(&findings);
    findings.retain_mut(|finding| {
        program.execution_paths.apply_deferred_mount(finding);
        !catalog.claims_client_runtime(&finding.id) || program.execution_paths.apply(finding)
    });
    if let Some(stage) = stage {
        stage.end(crate::DropPass::ExecutionPaths, &findings);
    }
    // ADR 0822: a Get a violation proves needs no open row for its value.
    let proved = findings
        .iter()
        .filter(|finding| finding.kind == "violation")
        .map(|finding| finding.primary_location.clone())
        .collect::<crate::HashSet<_>>();
    let stage = crate::proof_log::ProofStage::begin(&findings);
    findings.retain(|finding| {
        !finding
            .analysis_context
            .starts_with(crate::blockers::DEPENDENCY_VALUE_UNFOLLOWED)
            || !proved.contains(&finding.primary_location)
    });
    if let Some(stage) = stage {
        stage.end(crate::DropPass::SupersededByViolation, &findings);
    }
    // ADR 0680: after every step that may make a row uncertifiable.
    for finding in &mut findings {
        if finding.kind == "uncertifiable" && !crate::blockers::named(&finding.analysis_context) {
            finding.analysis_context = crate::blockers::with_detail(
                crate::blockers::for_rule(&finding.id),
                &finding.analysis_context,
            );
        }
    }
    let stage = crate::proof_log::ProofStage::begin(&findings);
    let (findings, timings) = finish_findings(findings, total_started, construction_started);
    if let Some(stage) = stage {
        stage.end(crate::DropPass::EqualFinding, &findings);
    }
    crate::proof_log::findings(&findings);
    (findings, timings)
}

/// `rows` without the rows that are the one before them on other paths
/// (`same`, every field but the multiplicity equal). A write or an action
/// stands for every reachable path to its call
/// ([`ReactiveWrite::multiplicity`]); held once per path, the copies were
/// adjacent and equal, projected to adjacent equal findings, and
/// [`finish_findings`]'s stable sort kept them adjacent and its `dedup`
/// dropped them. Projecting one row of such a run leaves the same findings.
/// ADR 0918: a function several owner calls receive by value is walked once
/// per call, so the same operation at the same place comes once per
/// registration, each under its own call-site gate. One finding stands for
/// them, a proven one when any registration proves it (the operation throws
/// on that registration's run). Order is the first registration's.
fn distinct_leaf_operations(
    operations: &[crate::LeafOwnerOperation],
) -> Vec<&crate::LeafOwnerOperation> {
    let same = |a: &crate::LeafOwnerOperation, b: &crate::LeafOwnerOperation| {
        a.kind == b.kind
            && a.owner == b.owner
            && a.via == b.via
            && a.fix == b.fix
            && a.possible == b.possible
            && a.identity_unproven == b.identity_unproven
            && a.computation_error_unobserved == b.computation_error_unobserved
    };
    let mut kept: Vec<&crate::LeafOwnerOperation> = Vec::with_capacity(operations.len());
    let mut at: crate::HashMap<&Location, Vec<usize>> = crate::HashMap::default();
    for operation in operations {
        let slots = at.entry(&operation.location).or_default();
        match slots.iter().find(|slot| same(kept[**slot], operation)) {
            Some(slot) => {
                if kept[*slot].uncertain && !operation.uncertain {
                    kept[*slot] = operation;
                }
            }
            None => {
                slots.push(kept.len());
                kept.push(operation);
            }
        }
    }
    kept
}

fn distinct_runs<T>(rows: &[T], same: fn(&T, &T) -> bool) -> impl Iterator<Item = &T> {
    rows.iter()
        .enumerate()
        .filter(move |(index, row)| *index == 0 || !same(&rows[index - 1], row))
        .map(|(_, row)| row)
}

/// ADR 0933: why a read row is not projected to a finding. The rule is
/// [`read_silence`]; [`read_reported`] is its absence, so the proof log and
/// the projection read one account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReadSilence {
    /// The row's role is one no read rule reports (tracked JSX, a deferred
    /// callback, an event callback, ...).
    RoleDoesNotReport,
    /// A closed project consumer proves the read non-strict.
    ProjectConsumerNonStrict,
    /// The catalog reports no module-scope strict reads.
    ModuleScopeNotReported,
    /// ADR 0504: the pending read stands for it.
    PendingFirstRun,
    /// ADR 0504: the apply's pending row withdrew it.
    WithdrawnThroughOrigin,
}

impl ReadSilence {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::RoleDoesNotReport => "role-does-not-report",
            Self::ProjectConsumerNonStrict => "project-consumer-non-strict",
            Self::ModuleScopeNotReported => "module-scope-not-reported",
            Self::PendingFirstRun => "pending-first-run",
            Self::WithdrawnThroughOrigin => "withdrawn-through-origin",
        }
    }
}

/// Why `read` is not projected, or `None` where it is. The first two clauses
/// of a read's rule, then the catalog's capability.
pub(crate) fn read_silence(
    read: &crate::ReactiveRead,
    capabilities: CatalogCapabilities,
) -> Option<ReadSilence> {
    if !(read.execution.reports_untracked_read() || read.execution.reports_unplaced()) {
        return Some(ReadSilence::RoleDoesNotReport);
    }
    if read.project_consumer_non_strict {
        return Some(ReadSilence::ProjectConsumerNonStrict);
    }
    if !capabilities.module_scope_strict_reads
        && read.execution == crate::ExecutionRole::ModuleInitialization
    {
        return Some(ReadSilence::ModuleScopeNotReported);
    }
    None
}

fn read_reported(read: &crate::ReactiveRead, capabilities: CatalogCapabilities) -> bool {
    read_silence(read, capabilities).is_none()
}

/// ADR 0933: why a write row is not projected (see [`ReadSilence`]).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WriteSilence {
    /// The source was created with `ownedWrite: true`: its setter is exempt
    /// from the owned-scope write guard.
    OwnedWriteOn,
    /// The row's role is one no write rule reports.
    RoleDoesNotReport,
    /// The catalog reports no render-phase (untracked rendering) writes.
    UntrackedRenderingNotReported,
}

impl WriteSilence {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::OwnedWriteOn => "owned-write-on",
            Self::RoleDoesNotReport => "role-does-not-report",
            Self::UntrackedRenderingNotReported => "untracked-rendering-not-reported",
        }
    }
}

pub(crate) fn write_silence(
    write: &crate::ReactiveWrite,
    capabilities: CatalogCapabilities,
) -> Option<WriteSilence> {
    if write.owned_write == crate::OwnedWriteOption::On {
        return Some(WriteSilence::OwnedWriteOn);
    }
    if !(write.execution.reports_disallowed_write() || write.execution.reports_unplaced()) {
        return Some(WriteSilence::RoleDoesNotReport);
    }
    if !capabilities.untracked_rendering_writes
        && write.execution == crate::ExecutionRole::UntrackedRendering
    {
        return Some(WriteSilence::UntrackedRenderingNotReported);
    }
    None
}

fn write_reported(write: &crate::ReactiveWrite, capabilities: CatalogCapabilities) -> bool {
    write_silence(write, capabilities).is_none()
}

/// ADR 0933: why an action row is not projected: its role reports nothing.
pub(crate) fn action_silence(action: &crate::ActionInvocation) -> Option<&'static str> {
    if action.execution.reports_disallowed_write() || action.execution.reports_unplaced() {
        None
    } else {
        Some("role-does-not-report")
    }
}

fn action_reported(action: &crate::ActionInvocation) -> bool {
    action_silence(action).is_none()
}

/// ADR 0933: which clause of the async-read rule decided a row
/// ([`async_read_clause`]); the first four report, the last two do not.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AsyncClause {
    /// The compiler deleted the read.
    Discarded,
    /// ADR 0504: an apply's first run meets its source pending.
    ApplyFirstRunPending,
    /// A pending read that throws: a leaf owner, module evaluation, an
    /// untracked rendering, or an invocation context not proven.
    PendingUnsuspendable,
    /// A render read with no strict window and no boundary above it.
    UnwindowedNoBoundary,
    /// Tracked JSX outside a Loading boundary.
    TrackedNoBoundary,
    /// No clause speaks.
    NoClause,
}

impl AsyncClause {
    pub(crate) const fn reports(self) -> bool {
        matches!(
            self,
            Self::ApplyFirstRunPending
                | Self::PendingUnsuspendable
                | Self::UnwindowedNoBoundary
                | Self::TrackedNoBoundary
        )
    }

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Discarded => "discarded",
            Self::ApplyFirstRunPending => "apply-first-run-pending",
            Self::PendingUnsuspendable => "pending-unsuspendable",
            Self::UnwindowedNoBoundary => "unwindowed-no-boundary",
            Self::TrackedNoBoundary => "tracked-no-boundary",
            Self::NoClause => "no-clause",
        }
    }
}

pub(crate) fn async_read_reported(read: &crate::AsyncRead) -> bool {
    async_read_clause(read).reports()
}

pub(crate) fn async_read_clause(read: &crate::AsyncRead) -> AsyncClause {
    // Deletion first, ahead of the leaf-owner short-circuit
    // below. Every clause after this one is a claim about a
    // read that happens -- "a pending read here throws at
    // runtime", "this read needs a Loading boundary" -- and a
    // read the compiler deleted happens in neither compiler's
    // output. Ordered rather than merged into the clauses
    // because `read.leaf_owner.is_some()` short-circuits ahead
    // of every role test: a leaf-owned async read inside a
    // deleted value would have been reported on the strength of
    // the leaf owner alone, with reachability unproven.
    //
    // Defensive as much as reachable: with the leaf-owner pass
    // itself now gated on discarded regions (`cleanup.rs`), the
    // `leaf_owner.is_some()` route into this is not known to be
    // constructible from a fixture. The gate stays because the
    // ordering, not the reachability, is what was wrong.
    if read.execution == crate::ExecutionRole::DiscardedRendering {
        return AsyncClause::Discarded;
    }
    // ADR 0504: a read in an apply's first run that meets its source pending.
    if read.apply_first_run_pending {
        return AsyncClause::ApplyFirstRunPending;
    }
    // Pending-read rules (SC5001/SC5002) need proven async
    // provenance; they stay reported for loadingValue-declared
    // sources because the declared window ends at the first
    // real answer (probed, rc.0) — the catalog words them
    // conditionally.
    if read.async_provenance
        && (read.leaf_owner.is_some()
            || read.execution == crate::ExecutionRole::ModuleInitialization
            || read.execution == crate::ExecutionRole::UntrackedRendering
            || (read.execution == crate::ExecutionRole::Unknown
                && read.invocation_context_unproven))
    {
        return AsyncClause::PendingUnsuspendable;
    }
    // Tracked JSX outside a Loading boundary: the SSR client
    // hole (SC5005, error — it subsumes the SC5003 warning on
    // the same read), or the informational boundary warning
    // (SC5003) — suppressed for declared-first-paint sources,
    // whose whole point is to not need a boundary.
    // ADR 0662: a `<Dynamic>` body has no strict window, and still warns with no
    // boundary above it.
    if read.leaf_owner.is_none()
        && read.execution == crate::ExecutionRole::UntrackedUnwindowed
        && !read.under_loading
        && read.async_provenance
        && !read.declared_loading
    {
        return AsyncClause::UnwindowedNoBoundary;
    }
    if read.leaf_owner.is_none()
        && read.execution == crate::ExecutionRole::TrackedJsx
        && !read.under_loading
        && (read.ssr_client_hole
            || read.server_rendering_unresolved
            || (read.async_provenance && !read.declared_loading))
    {
        return AsyncClause::TrackedNoBoundary;
    }
    AsyncClause::NoClause
}

/// The primary location of every row [`project_findings`] may turn into a
/// finding, under any catalog's capabilities (the union of what each one
/// reports): the sites ADR 0458's reachability is asked of.
pub(crate) fn reported_locations(program: &Program) -> Vec<&Location> {
    let every = CatalogCapabilities {
        actions: true,
        async_reads: true,
        leaf_operations: true,
        directive_creations: true,
        untracked_rendering_writes: true,
        module_scope_strict_reads: true,
        handler_expression_owns_strict_read: true,
    };
    let mut locations = Vec::new();
    locations.extend(
        program
            .reads
            .iter()
            .filter(|read| read_reported(read, every))
            .map(|read| &read.location),
    );
    locations.extend(
        program
            .writes
            .iter()
            .filter(|write| write_reported(write, every))
            .map(|write| &write.location),
    );
    locations.extend(program.leaf_operations.iter().map(|row| &row.location));
    locations.extend(program.static_defects.iter().map(|row| &row.location));
    locations.extend(program.static_violations.iter().map(|row| &row.location));
    locations.extend(program.directive_creations.iter().map(|row| &row.location));
    locations.extend(
        program
            .missing_owners
            .iter()
            .filter(|requirement| requirement.report)
            .map(|requirement| &requirement.location),
    );
    locations.extend(
        program
            .async_reads
            .iter()
            .filter(|read| async_read_reported(read))
            .map(|read| &read.location),
    );
    locations.extend(
        program
            .actions
            .iter()
            .filter(|action| action_reported(action))
            .map(|action| &action.location),
    );
    locations
}

/// Removes SC1001 findings already owned by a more specific finding that
/// survived rule enablement. SC5001 names the same pending async read (a
/// violation: the run throws before it would warn; ADR 0511: where both are
/// uncertifiable, ADR 0509's two outcomes stand open and both stay, so a run
/// that meets the source settled has its strict read's obligation), SC1004
/// owns reads inside its return-shape condition, and Solid 2 lets SC1007 own
/// the exact handler expression it reports. Calling this before enablement
/// filtering would let a disabled owner erase an enabled strict-read finding.
pub fn suppress_findings_owned_by_enabled_rules(
    findings: &mut Vec<Finding>,
    capabilities: CatalogCapabilities,
) {
    // ADR 0933: the findings this drops are named, with the owner at the site.
    let stage = crate::proof_log::ProofStage::begin(findings);
    let pending_reads = findings
        .iter()
        // ADR 0838: an accounting row adds a claim; it owns no strict read.
        .filter(|finding| {
            finding.id == "SC5001"
                && finding.analysis_context != crate::blockers::ASYNC_READ_UNACCOUNTED
                && finding.analysis_context != crate::blockers::SOURCE_MODULE_UNRESOLVED
        })
        .map(|finding| {
            (
                (
                    finding.primary_location.path.clone(),
                    finding.primary_location.start_byte,
                    finding.primary_location.end_byte,
                ),
                finding.kind == "uncertifiable",
            )
        })
        .fold(
            std::collections::HashMap::<_, bool>::default(),
            |mut pending, (site, open)| {
                // One proven pending read at the site owns it.
                *pending.entry(site).or_insert(true) &= open;
                pending
            },
        );
    let component_conditions = findings
        .iter()
        .filter(|finding| finding.id == "SC1004")
        .map(|finding| finding.primary_location.clone())
        .collect::<Vec<_>>();
    let handler_reads = if capabilities.handler_expression_owns_strict_read {
        findings
            .iter()
            .filter(|finding| finding.id == "SC1007")
            .map(|finding| {
                (
                    finding.primary_location.path.clone(),
                    finding.primary_location.start_byte,
                    finding.primary_location.end_byte,
                )
            })
            .collect::<std::collections::HashSet<_>>()
    } else {
        std::collections::HashSet::default()
    };
    findings.retain(|finding| {
        if finding.id != "SC1001" {
            return true;
        }
        let location = &finding.primary_location;
        let exact = (
            location.path.clone(),
            location.start_byte,
            location.end_byte,
        );
        let read_open = finding.kind == "uncertifiable";
        pending_reads
            .get(&exact)
            .is_none_or(|pending_open| *pending_open && read_open)
            && !handler_reads.contains(&exact)
            && !component_conditions.iter().any(|condition| {
                condition.path == location.path
                    && condition.start_byte <= location.start_byte
                    && location.end_byte <= condition.end_byte
            })
    });
    if let Some(stage) = stage {
        stage.end(crate::DropPass::OwnedByEnabledRule, findings);
    }
}

/// What lets ordinary analysis read a package's runtime source (ADR 0297),
/// stated where a package's behaviour is reported unknown.
const PACKAGE_SOURCE_PREMISE: &str = "installed Solid packages are analyzed from their runtime source when the run declares a runtime target and build (--runtime-target, --runtime-build) and a Solid Vite plugin resolves from the project, or when --dependency-source names them.";

/// The hint of the package-behaviour-unknown form of `SC9005`.
const PACKAGE_BEHAVIOUR_UNKNOWN_HINT: &str = "Installed Solid packages are analyzed from their runtime source when the run declares a runtime target and build (--runtime-target, --runtime-build) and a Solid Vite plugin resolves from the project, or when --dependency-source names them. A package source analysis cannot read stays uncertifiable.";

/// An unavailable-package-behavior obligation says one thing -- "this
/// package's behavior is not available from analyzed source" -- and it would
/// say it once per import site. The remedy is one action per package, not per
/// import, and nothing distinguishes the sites, so the defects of one package
/// collapse to one finding, anchored at the first site and carrying every
/// other site in `related_locations`. **Nothing is dropped**; the same
/// locations reach the same consumers, grouped by the thing that would fix
/// them. A group of one keeps its exact original wording.
///
/// Every grouped finding names its subject (`subject_kind` `package`), which
/// is what tells a per-file reporter that its related locations are further
/// sites of the same finding rather than supporting context. The ESLint
/// adapter reports such a finding in each file holding one of its sites, so
/// collapsing across a project never hides a file's sites from a per-file
/// consumer.
fn collapse_unanalyzed_package_defects(
    defects: &[StaticDefect],
    catalog: &impl CatalogWording,
) -> Vec<Finding> {
    /// The package an unavailable-behavior defect is about. Every site in one
    /// package has the same answer, so the package is what makes two of these
    /// defects interchangeable.
    fn package_of(defect: &StaticDefect) -> Option<&str> {
        match &defect.kind {
            StaticDefectKind::PackageBehaviorUnavailable { module, .. } => Some(module.as_str()),
            _ => None,
        }
    }

    // First-seen order, so the anchor of each group is the first site the
    // analysis reached and the output order does not depend on a hash.
    let mut order: Vec<&str> = Vec::new();
    let mut grouped: std::collections::HashMap<&str, Vec<&StaticDefect>> =
        std::collections::HashMap::default();
    let mut findings = Vec::new();
    for defect in defects {
        match package_of(defect) {
            Some(module) => {
                let group = grouped.entry(module).or_insert_with(|| {
                    order.push(module);
                    Vec::new()
                });
                group.push(defect);
            }
            // Everything else keeps its own finding, in place.
            None => findings.push(project_finding(FindingSeed::StaticDefect(defect), catalog)),
        }
    }
    for module in order {
        let group = &grouped[module];
        let mut finding = project_finding(FindingSeed::StaticDefect(group[0]), catalog);
        finding.subject_kind = "package".into();
        if group.len() > 1 {
            let mut exports = group
                .iter()
                .filter_map(|defect| match &defect.kind {
                    StaticDefectKind::PackageBehaviorUnavailable { export, .. } => {
                        Some(export.as_str())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            exports.sort_unstable();
            exports.dedup();
            let named = exports.len().min(6);
            let listed = exports[..named].join(", ");
            let remainder = exports.len() - named;
            finding.message = format!(
                "{module} is not analyzed from source here, so its behaviour is unknown; \
                 solid-checker cannot tell whether its exports read reactive values, take tracked \
                 callbacks, or return accessors, so code flowing through them cannot be \
                 certified. {} export{} used across {} import site{}: {listed}{}",
                exports.len(),
                if exports.len() == 1 { "" } else { "s" },
                group.len(),
                if group.len() == 1 { "" } else { "s" },
                if remainder == 0 {
                    String::new()
                } else {
                    format!(", and {remainder} more")
                }
            );
            finding.hint = PACKAGE_BEHAVIOUR_UNKNOWN_HINT.to_owned();
            finding
                .related_locations
                .extend(group[1..].iter().map(|defect| defect.location.clone()));
        }
        findings.push(finding);
    }
    findings
}

/// Projects one seed. Used by the backend for package-behavior issues that
/// are discovered after the reactive [`Program`] has been built.
#[must_use]
pub fn project_finding(seed: FindingSeed<'_>, catalog: &impl CatalogWording) -> Finding {
    let wording = catalog.wording(seed);
    let location = primary_location(seed);
    let mut finding = match seed {
        FindingSeed::OwnerRequirement(requirement) => Finding::for_owner_requirement(
            wording.metadata,
            requirement,
            &wording.message,
            &wording.hint,
        ),
        _ => Finding {
            hint: wording.hint,
            evidence: wording.evidence,
            ..Finding::new(wording.metadata, wording.message, location)
        },
    };

    match seed {
        FindingSeed::StrictRead { read, .. } => {
            finding.analysis_context = read.context.to_string();
            finding.subject_kind = read.kind.to_string();
            finding.related_locations = crate::strict_read_related_locations(read);
            // Fail-honest: a props read whose component's callers cannot be
            // enumerated may or may not be signal-backed, so the finding is a
            // proof obligation, not a proven runtime warning. The census gap is
            // the same escalation for the other half of the proof — the
            // execution context rather than the reactive backing: the compiler
            // never reported on the JSX region this read sits in, and absence
            // of a fact proves neither that the region is untracked nor that
            // it was dropped.
            // A child the pinned fork lowers and the shipped compiler deletes
            // is the third: the fact is present and truthful about its producer,
            // and still proves nothing about the build the user will run.
            // A host callback that may run inside the strict-read window or
            // after it (`ReactiveRead::host_callback_timing`) is a fourth.
            if read.is_uncertifiable() {
                finding.kind = "uncertifiable".into();
            }
            // A read attributed through another function's summary: whether
            // it runs while this call does is not established.
            if read.summary_attributed && finding.kind != "uncertifiable" {
                finding.kind = "uncertifiable".into();
                finding.message = format!(
                    "{}; this read happens inside {}'s body, and nothing proves it runs while this call does, rather than only on some of its paths or later (from a timer, listener, getter, returned accessor or effect), so it is not proven to be read untracked here",
                    finding.message, read.via
                );
            }
        }
        FindingSeed::OwnedWrite(write) => {
            finding.analysis_context = if write.context.is_empty() {
                "owned scope".into()
            } else {
                write.context.to_string()
            };
            finding.related_locations = vec![write.declaration.clone()];
            if write.is_uncertifiable() {
                finding.kind = "uncertifiable".into();
            }
            // The setter's source may carry `ownedWrite: true` in options the
            // analysis cannot read (ADR 0424), which exempts the write.
            if write.owned_write == crate::OwnedWriteOption::Unknown {
                finding.kind = "uncertifiable".into();
            }
        }
        FindingSeed::LeafOperation(operation) => {
            finding.fixes = operation.fix.clone().into_iter().collect();
            // The same escalation an unproven owner forces on owner
            // requirements: when the leaf owner's call site cannot be proven
            // owned (exported helper, conditional owner), the finding is a
            // proof obligation, not a proven runtime violation.
            if operation.uncertain || operation.possible {
                finding.kind = "uncertifiable".into();
            }
            if operation.computation_error_unobserved {
                crate::blockers::COMPUTATION_ERROR_UNOBSERVED
                    .clone_into(&mut finding.analysis_context);
            }
            if operation.identity_unproven {
                finding.fixes.clear();
                finding.analysis_context =
                    crate::blockers::LEAF_OPERATION_IDENTITY_UNPROVEN.to_owned();
            }
        }
        FindingSeed::DirectiveCreation(creation) if creation.uncertain => {
            finding.kind = "uncertifiable".into();
        }
        FindingSeed::StaticViolation(violation) => {
            finding.analysis_context = violation.analysis_context.clone();
            finding.fixes = violation.fixes.clone();
            if violation.uncertain {
                finding.kind = "uncertifiable".into();
            }
        }
        FindingSeed::StaticDefect(defect) => {
            finding.analysis_context = defect.analysis_context.clone();
            finding.fixes = defect.fixes.clone();
            // The same escalation as strict reads: a props-backed defect
            // whose component's callers cannot be enumerated is a proof
            // obligation rather than a proven violation.
            if defect.uncertain {
                finding.kind = "uncertifiable".into();
            }
        }
        FindingSeed::AsyncRead(read) => {
            finding.related_locations = vec![read.declaration.clone()];
            // ADR 0838: a claim no producer accounted for proves nothing.
            if read.unaccounted {
                finding.kind = "uncertifiable".into();
            }
            // Fail-honest: an options argument the analyzer cannot read may
            // declare a loadingValue or seedLoadingValue, and a declared first
            // flight neither throws (SC5001) nor needs a boundary (SC5003,
            // SC5005; ADR 0430) — so the finding is no longer a *proven*
            // defect and becomes a proof obligation instead.
            if (read.options_opaque
                && matches!(finding.id.as_str(), "SC5001" | "SC5003" | "SC5005"))
                || (read.caught && matches!(finding.id.as_str(), "SC5003" | "SC5005"))
            {
                finding.kind = "uncertifiable".into();
            }
            // ADR 0641: rc.21 warns from a render effect that hits the pending
            // value, and a component's prop is a getter: the child decides
            // whether one ever reads it. Nothing here follows it to the child.
            if read.component_prop && finding.id == "SC5003" {
                finding.kind = "uncertifiable".into();
                finding.message = format!(
                    "{}; this read is a component prop, a getter the child may read in a render, in an effect, or never",
                    finding.message
                );
            }
            // ADR 0632: a declared value is served through the first flight,
            // so a read past the creating pass throws, or warns in a leaf, only
            // on a re-ask nothing here proves.
            if read.declared_loading && finding.id == "SC5001" {
                finding.kind = "uncertifiable".into();
            }
            // ADR 0855: while a module evaluates no strict-read window is open,
            // so the read emits nothing and throws a plain NotReadyError, which
            // a `catch` on the evaluating stack swallows. A component body, an
            // apply or a labelled `untrack` emits before it throws: there the
            // finding stands.
            if read.caught
                && finding.id == "SC5001"
                && read.execution == crate::ExecutionRole::ModuleInitialization
                && read.leaf_owner.is_none()
            {
                finding.kind = "uncertifiable".into();
                finding.message = format!(
                    "this read runs while the module evaluates, where a pending read emits no diagnostic and throws a plain NotReadyError, and a `catch` in this module may be on its stack and swallow it; if none is: {}",
                    finding.message
                );
            }
            // The same escalation for the execution window: a host that may
            // run the read's callback inside the component body's strict-read
            // window or after it makes the untracked-read throw one of two
            // outcomes (`PENDING_ASYNC_UNTRACKED_READ` inside, a plain
            // `NotReadyError` after), so it is not a proven throw.
            if read.host_callback_timing && finding.id == "SC5001" {
                finding.kind = "uncertifiable".into();
            }
            // Lexical placement cannot prove an unknown callee runs the
            // callback in this window or leaves pending values unhandled.
            if (read.callee_callback_timing || read.invocation_context_unproven)
                && finding.id == "SC5001"
            {
                finding.kind = "uncertifiable".into();
            }
            if read.server_rendering_unresolved {
                finding.kind = "uncertifiable".into();
            }
            // ADR 0462: a call whose callee's summary reads the source does
            // not prove the read runs while the call renders.
            if read.call_attributed && matches!(finding.id.as_str(), "SC5003" | "SC5005") {
                finding.kind = "uncertifiable".into();
                finding.message = format!(
                    "this call reaches a read of the async source through a project function, and whether that read runs while the call renders is not proven; if it does: {}",
                    finding.message
                );
            }
            // ADR 0563: every async-read finding is a claim about a read that
            // happens; a carrier that may run the read's callback zero times
            // (an empty or unknown-length receiver) proves none.
            if read.carrier_run_unproven {
                finding.kind = "uncertifiable".into();
                finding.message = format!(
                    "the standard-library call that runs this read's callback may run it zero times (its receiver may be empty), so whether this read runs is not proven; if it does: {}",
                    finding.message
                );
            }
            // A missing boundary is proven only for a function whose every
            // render chain was followed to a mount root. Where a chain ends at
            // a function the project never renders, the boundary above it may
            // be written elsewhere (a router root layout, a lazy page).
            if read.mount_unresolved && matches!(finding.id.as_str(), "SC5003" | "SC5005") {
                finding.kind = "uncertifiable".into();
                finding.message = format!(
                    "the component rendering this read is never traced to a render or hydrate root in this project, so whether a Loading boundary is above it (for example in a router layout or around a lazy route) cannot be proven; if none is: {}",
                    finding.message
                );
            }
            // ADR 0570: a read not proven to run in the pass that created
            // its source may run after it settled (a component a `<Show>`
            // mounts on a click); the same exemptions as ADR 0518's.
            if read.creation_pass_unproven
                && !read.apply_first_run_pending
                && ((finding.id == "SC5003"
                    && !read.ssr_client_hole
                    && !read.server_rendering_unresolved)
                    || finding.id == "SC5001")
            {
                finding.kind = "uncertifiable".into();
                finding.message = format!(
                    "the component holding this read is not proven to run in the same pass that created the source (it may mount later, from a <Show> or a click), and a settled read neither throws nor needs a boundary; if it is still pending: {}",
                    finding.message
                );
            }
            // ADR 0662: rendered by `<Dynamic>`, whose memo has no strict window.
            if read.execution == crate::ExecutionRole::UntrackedUnwindowed && finding.id == "SC5003"
            {
                finding.kind = "uncertifiable".into();
                crate::blockers::MOUNT_FORM_WINDOW.clone_into(&mut finding.analysis_context);
            }
            // ADR 0661: SC5003's warning fires only inside `render()`, so a read
            // whose first run is not proven there may be quiet though its source
            // is pending. SC5001 needs no render: the throw has no such gate.
            if read.root_pass_unproven
                && !read.apply_first_run_pending
                && finding.id == "SC5003"
                && !read.ssr_client_hole
                && !read.server_rendering_unresolved
            {
                finding.kind = "uncertifiable".into();
                finding.message = format!(
                    "the first run of this read is not proven inside a render() call, where the runtime checks for a boundary. {}",
                    finding.message
                );
                crate::blockers::ROOT_MOUNT_PASS_UNPROVEN.clone_into(&mut finding.analysis_context);
            }
            // ADR 0518: a top-level `await` may settle the source before the
            // read runs; a settled read neither needs a boundary (SC5003) nor
            // throws (SC5001). SC5003's `ssrSource: "client"` wording throws
            // on the server whatever the client's timing, and an apply's
            // first-run row carries its own proof (ADR 0510).
            if read.settle_unproven
                && ((finding.id == "SC5003"
                    && !read.ssr_client_hole
                    && !read.server_rendering_unresolved)
                    || (finding.id == "SC5001" && !read.apply_first_run_pending))
            {
                finding.kind = "uncertifiable".into();
                finding.message = format!(
                    "a module of this project awaits at its top level, so the source may have settled before this read runs, and a settled read neither throws nor needs a boundary; if it is still pending: {}",
                    finding.message
                );
            }
            // ADR 0665: last, because it decides whether any claim above holds.
            if read.provenance_open && !read.ssr_client_hole {
                finding.kind = "uncertifiable".into();
                finding.message = format!(
                    "no fact proves whether this source's computation returns a Promise or throws NotReadyError; if it does: {}",
                    finding.message
                );
                crate::blockers::ASYNC_PROVENANCE_UNPROVEN
                    .clone_into(&mut finding.analysis_context);
            }
        }
        FindingSeed::Action(action) => {
            if action.is_uncertifiable() {
                finding.kind = "uncertifiable".into();
            }
        }
        FindingSeed::DirectiveCreation(_) | FindingSeed::OwnerRequirement(_) => {}
    }
    if let Some((family, subject)) = coverage_group(seed) {
        finding.coverage_family = family.into();
        finding.coverage_subject = subject;
    }
    if finding.kind == "uncertifiable" && !crate::blockers::named(&finding.analysis_context) {
        let blocker = match seed {
            FindingSeed::StrictRead { read, .. } => Some(crate::blockers::for_read(read)),
            FindingSeed::OwnedWrite(write) => Some(crate::blockers::for_write(write)),
            FindingSeed::Action(_) => Some(crate::blockers::FUNCTION_RUN_UNPLACED),
            FindingSeed::LeafOperation(_) if finding.id == "SC9012" => {
                Some(crate::blockers::LEAF_CALLBACK_OPAQUE)
            }
            FindingSeed::LeafOperation(_) => Some(crate::blockers::LEAF_OWNER_UNPROVEN),
            FindingSeed::DirectiveCreation(_) => Some(crate::blockers::DIRECTIVE_OWNER_UNPROVEN),
            FindingSeed::OwnerRequirement(requirement) => {
                Some(crate::blockers::for_owner(requirement))
            }
            FindingSeed::AsyncRead(read) => Some(crate::blockers::for_async_read(read)),
            FindingSeed::StaticViolation(_) | FindingSeed::StaticDefect(_) => None,
        };
        if let Some(blocker) = blocker {
            finding.analysis_context =
                crate::blockers::with_detail(blocker, &finding.analysis_context);
        }
    }
    finding
}

/// ADR 0205: the coverage family and subject of an analysis-coverage gap, for
/// a reporter to group by. `None` for every finding that is a claim about the
/// user's code.
fn coverage_group(seed: FindingSeed<'_>) -> Option<(&'static str, String)> {
    match seed {
        FindingSeed::StaticDefect(defect) => match &defect.kind {
            StaticDefectKind::PackageBehaviorUnavailable { module, .. } => {
                Some(("package-boundary", package_of_specifier(module).to_owned()))
            }
            StaticDefectKind::ReactiveDispatchUnresolved { callee, .. } => {
                let context = defect.analysis_context.as_str();
                Some((
                    if context.starts_with("parameter-member")
                        || context == crate::EXPORTED_PARAMETER_MEMBER_DISPATCH
                    {
                        "caller-supplied-member"
                    } else {
                        "call-target"
                    },
                    callee.clone(),
                ))
            }
            StaticDefectKind::ReactiveSourceUncaptured { callee, .. } => {
                Some(("undescribed-callee", callee.clone()))
            }
            _ => None,
        },
        FindingSeed::LeafOperation(operation)
            if operation.kind == crate::LeafOwnerOperationKind::UnresolvedCallback =>
        {
            Some(("leaf-callback", operation.owner.clone()))
        }
        _ => None,
    }
}

/// The package a bare import specifier names: `@scope/name` or `name`, without
/// a subpath. Anything else is returned unchanged.
fn package_of_specifier(specifier: &str) -> &str {
    let segments = if specifier.starts_with('@') { 2 } else { 1 };
    specifier
        .match_indices('/')
        .nth(segments - 1)
        .map_or(specifier, |(index, _)| &specifier[..index])
}

fn primary_location(seed: FindingSeed<'_>) -> Location {
    match seed {
        FindingSeed::StrictRead { read, .. } => match &read.origin {
            Some(origin) if read.reported_at_origin => origin.clone(),
            _ => read.location.clone(),
        },
        FindingSeed::OwnedWrite(value) => value.location.clone(),
        FindingSeed::Action(value) => value.location.clone(),
        FindingSeed::LeafOperation(value) => value.location.clone(),
        FindingSeed::StaticViolation(value) => value.location.clone(),
        FindingSeed::StaticDefect(value) => value.location.clone(),
        FindingSeed::DirectiveCreation(value) => value.location.clone(),
        FindingSeed::OwnerRequirement(value) => value.location.clone(),
        FindingSeed::AsyncRead(value) => value.location.clone(),
    }
}

#[cfg(test)]
mod tests;
