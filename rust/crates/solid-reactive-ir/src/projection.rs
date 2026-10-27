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
        StaticDefectKind::ReactiveReadAfterAwait { accessor } => (
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
        } if defect.analysis_context == crate::PRIMITIVE_VALUE => (
            format!(
                "Solid primitive {callee} is used as a value here, so the calls that reach it are not followed; the owner, tracking and write rules that apply where it runs are not checked"
            ),
            "Call the primitive directly by its imported name, so solid-checker can see each call.".into(),
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

    findings.extend(
        program
            .reads
            .iter()
            .filter(|read| read_reported(read, capabilities))
            .map(|read| {
                project_finding(
                    FindingSeed::StrictRead {
                        read,
                        compute_reads_source: program.apply_compute_reads.contains(&read.location),
                    },
                    catalog,
                )
            }),
    );
    findings.extend(
        program
            .writes
            .iter()
            .filter(|write| write_reported(write, capabilities))
            .map(|write| project_finding(FindingSeed::OwnedWrite(write), catalog)),
    );
    if capabilities.leaf_operations {
        findings.extend(
            program
                .leaf_operations
                .iter()
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
    if capabilities.async_reads {
        findings.extend(
            program
                .async_reads
                .iter()
                .filter(|read| async_read_reported(read))
                .map(|read| project_finding(FindingSeed::AsyncRead(read), catalog)),
        );
    }
    if capabilities.actions {
        findings.extend(
            program
                .actions
                .iter()
                .filter(|action| action_reported(action))
                .map(|action| project_finding(FindingSeed::Action(action), catalog)),
        );
    }

    for finding in &mut findings {
        if finding.kind == "violation" {
            program.runtime_configuration.apply(finding);
        }
    }
    finish_findings(findings, total_started, construction_started)
}

fn read_reported(read: &crate::ReactiveRead, capabilities: CatalogCapabilities) -> bool {
    (read.execution.reports_untracked_read() || read.execution.reports_unplaced())
        && !read.project_consumer_non_strict
        && (capabilities.module_scope_strict_reads
            || read.execution != crate::ExecutionRole::ModuleInitialization)
}

fn write_reported(write: &crate::ReactiveWrite, capabilities: CatalogCapabilities) -> bool {
    write.owned_write != crate::OwnedWriteOption::On
        && (write.execution.reports_disallowed_write() || write.execution.reports_unplaced())
        && (capabilities.untracked_rendering_writes
            || write.execution != crate::ExecutionRole::UntrackedRendering)
}

fn action_reported(action: &crate::ActionInvocation) -> bool {
    action.execution.reports_disallowed_write() || action.execution.reports_unplaced()
}

fn async_read_reported(read: &crate::AsyncRead) -> bool {
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
        return false;
    }
    // Pending-read rules (SC5001/SC5002) need proven async
    // provenance; they stay reported for loadingValue-declared
    // sources because the declared window ends at the first
    // real answer (probed, rc.0) — the catalog words them
    // conditionally.
    if read.async_provenance
        && (read.leaf_owner.is_some()
            || read.execution == crate::ExecutionRole::ModuleInitialization
            || read.execution == crate::ExecutionRole::UntrackedRendering)
    {
        return true;
    }
    // Tracked JSX outside a Loading boundary: the SSR client
    // hole (SC5005, error — it subsumes the SC5003 warning on
    // the same read), or the informational boundary warning
    // (SC5003) — suppressed for declared-first-paint sources,
    // whose whole point is to not need a boundary.
    read.leaf_owner.is_none()
        && read.execution == crate::ExecutionRole::TrackedJsx
        && !read.under_loading
        && (read.ssr_client_hole
            || read.server_rendering_unresolved
            || (read.async_provenance && !read.declared_loading))
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
/// survived rule enablement. SC5001 names the same pending async read, SC1004
/// owns reads inside its return-shape condition, and Solid 2 lets SC1007 own
/// the exact handler expression it reports. Calling this before enablement
/// filtering would let a disabled owner erase an enabled strict-read finding.
pub fn suppress_findings_owned_by_enabled_rules(
    findings: &mut Vec<Finding>,
    capabilities: CatalogCapabilities,
) {
    let pending_reads = findings
        .iter()
        .filter(|finding| finding.id == "SC5001")
        .map(|finding| {
            (
                finding.primary_location.path.clone(),
                finding.primary_location.start_byte,
                finding.primary_location.end_byte,
            )
        })
        .collect::<std::collections::HashSet<_>>();
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
        !pending_reads.contains(&exact)
            && !handler_reads.contains(&exact)
            && !component_conditions.iter().any(|condition| {
                condition.path == location.path
                    && condition.start_byte <= location.start_byte
                    && location.end_byte <= condition.end_byte
            })
    });
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
            // Fail-honest: an options argument the analyzer cannot read may
            // declare a loadingValue or seedLoadingValue, and a declared first
            // flight neither throws (SC5001) nor needs a boundary (SC5003,
            // SC5005; ADR 0430) — so the finding is no longer a *proven*
            // defect and becomes a proof obligation instead.
            if read.options_opaque && matches!(finding.id.as_str(), "SC5001" | "SC5003" | "SC5005")
            {
                finding.kind = "uncertifiable".into();
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
        FindingSeed::StrictRead { read, .. } => read.location.clone(),
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
