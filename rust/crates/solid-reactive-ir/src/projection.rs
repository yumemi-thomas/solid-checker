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
    /// A dialect-owned override for the missing-contract-export hint: when
    /// the dialect knows the export was removed or renamed upstream (the
    /// Solid 2.0 catalog's removed-1.x-API map), the hint should point at
    /// the migration, not at writing a contract entry for an export that no
    /// longer exists. Returning `None` keeps the generic contract hint.
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
        StaticDefectKind::ComponentReturnsConditionally => (
            "this component's return value depends on a reactive condition, but a component body runs once; whichever branch is taken at setup renders forever, and the condition is never re-evaluated".into(),
            "Return a single JSX tree and move the branch into it: wrap the alternatives in <Show when={...} fallback={...}> (or <Switch>/<Match> for multiple cases), or use a ternary inside JSX where it stays tracked.".into(),
        ),
        StaticDefectKind::PackageContractExportMissing {
            module,
            export,
            reexported,
            ..
        } => {
            if defect.analysis_context.starts_with("obsolete-policy1-receipt:") {
                (
                    format!(
                        "the discovered reactivity contract for {module} was authorized only by obsolete proof policy 1; its claims cannot be used for {} export {export}",
                        if *reexported { "re-exported" } else { "imported" }
                    ),
                    "Policy-1 receipts cannot be grandfathered. Reacquire the exact package artifact and run policy-2 certification; until every demanded witness and the mandatory probe gate verify, this import remains uncertifiable.".into(),
                )
            } else if let Some(claims) = defect
                .analysis_context
                .strip_prefix("unbound-contract-claims:")
            {
                (
                    format!(
                        "the reactivity contract for {module} states {claims} for {} export {export}, but this call site gives the claim nothing to bind to, so what the callback receives cannot be certified",
                        if *reexported { "re-exported" } else { "imported" }
                    ),
                    format!(
                        "Pass the callback to {export} as an inline function literal so the contract's argument claims land on its parameters. A callback passed by name, or a claim shape solid-checker does not model, keeps the call uncertifiable. See docs/package-contracts.md for the format."
                    ),
                )
            } else if let Some(contexts) = defect
                .analysis_context
                .strip_prefix(crate::contracts::CONTEXT_PREMISE_UNMET_CONTEXT)
            {
                (
                    format!(
                        "the reactivity contract for {module} states its claims for {} export {export} only in a program where the context it exports as {contexts} receives no value from outside {module}, and this project provides it, passes it where solid-checker cannot follow, or installs another package that depends on {module}; none of the export's claims apply here",
                        if *reexported { "re-exported" } else { "imported" }
                    ),
                    format!(
                        "Read {contexts} only through useContext({contexts}). A value the project provides itself (<{contexts} value={{…}}>, createComponent({contexts}, …)) replaces the package's, so the certified claims about {export} cannot hold for it."
                    ),
                )
            } else if let Some(claims) = defect
                .analysis_context
                .strip_prefix("unknown-contract-claims:")
            {
                (
                    format!(
                        "the reactivity contract for {module} leaves {claims} unknown for {} export {export}; code whose proof depends on those claims cannot be certified",
                        if *reexported { "re-exported" } else { "imported" }
                    ),
                    format!(
                        "Audit {module} export {export} against the exact selected runtime artifact. Prove the open semantic leaves, issue a receipt for the finalized stable-v1 document, and register that exact document/receipt pair in .solid-checker/accepted-contracts.json. An open leaf is not negative proof. See docs/package-contracts.md for the workflow."
                    ),
                )
            } else {
                (
                    format!(
                        "{module} is not analyzed from source here, so the behaviour of {} export {export} is unknown; solid-checker cannot tell whether it reads reactive values, takes tracked callbacks, or returns accessors, so code flowing through it cannot be certified",
                        if *reexported { "re-exported" } else { "imported" }
                    ),
                    (terms.removed_export_hint)(module, export).unwrap_or_else(|| {
                        PACKAGE_BEHAVIOUR_UNKNOWN_HINT.to_owned()
                    }),
                )
            }
        }
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
                "Read the member inside JSX or a memo, or analyze the package through an accepted contract, whose getter rows state this Get (ADR 0272).".into(),
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
                "{callee} runs where a reactive read would not be tracked, but its only evidence of reading reactive state is its declared type, a Solid source alias that a plain function or object also satisfies; no implementation, contract or source flow proves what it reads, so whether this is an untracked reactive read can be neither proven nor ruled out"
            ),
            "Read it inside JSX, a createMemo, or an effect's compute function, or make its source visible to solid-checker: create it with a Solid primitive in the project, or pass one through a path the analysis follows (ADR 0294).".into(),
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
            "Use an exact project-relative named/default import, return an explicitly resolved local binding, or provide an audited package contract for the external value.".into(),
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
        StaticDefectKind::PackageContractExportMissing { .. }
            if defect
                .analysis_context
                .starts_with("unbound-contract-claims:") =>
        {
            "the imported package contract carries a claim about this callback that the call site does not let solid-checker bind"
        }
        StaticDefectKind::PackageContractExportMissing { .. }
            if defect
                .analysis_context
                .starts_with("unknown-contract-claims:") =>
        {
            "the imported package contract explicitly marks a required effect claim as unknown"
        }
        StaticDefectKind::PackageContractExportMissing { .. }
            if defect
                .analysis_context
                .starts_with(crate::contracts::CONTEXT_PREMISE_UNMET_CONTEXT) =>
        {
            "the imported package contract states its claims under a context premise this project does not meet"
        }
        // The acceptance gate and the missing-summary case shared this arm, and
        // its wording is only true of the second: at the acceptance gate there
        // is no contract to have a summary in. Split, because a reader acts on
        // the difference — one is `contract certify`, the other is an audit of
        // a contract that already exists.
        StaticDefectKind::PackageContractExportMissing { .. }
            if defect.analysis_context == crate::contracts::UNACCEPTED_IMPORT_CONTEXT =>
        {
            "this project accepted no contract for the imported package"
        }
        StaticDefectKind::PackageContractExportMissing { .. } => {
            "the imported package has a contract, but this export has no effect summary"
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
        StaticDefectKind::ReactiveReadAfterAwait { .. }
        | StaticDefectKind::MissingEffectFunction
        | StaticDefectKind::ReactiveHandlerRead { .. }
        | StaticDefectKind::HandlerValueUnresolved { .. }
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
            .filter(|read| {
                read.execution.reports_untracked_read()
                    && !read.project_consumer_non_strict
                    && (capabilities.module_scope_strict_reads
                        || read.execution != crate::ExecutionRole::ModuleInitialization)
            })
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
            .filter(|write| {
                !write.allowed_by_option
                    && write.execution.reports_disallowed_write()
                    && (capabilities.untracked_rendering_writes
                        || write.execution != crate::ExecutionRole::UntrackedRendering)
            })
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
    findings.extend(collapse_unaccepted_contract_defects(
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
                .filter(|read| {
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
                })
                .map(|read| project_finding(FindingSeed::AsyncRead(read), catalog)),
        );
    }
    if capabilities.actions {
        findings.extend(
            program
                .actions
                .iter()
                .filter(|action| action.execution.reports_disallowed_write())
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
        std::collections::HashSet::new()
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

/// The hint of the package-behaviour-unknown form of `SC9005` (P7 of the
/// contract-removal plan): no package contract is consumed.
const PACKAGE_BEHAVIOUR_UNKNOWN_HINT: &str = "Package contracts are no longer consumed: installed Solid packages are analyzed from their runtime source when the run declares a runtime target and build (--runtime-target, --runtime-build) and a Solid Vite plugin resolves from the project, or when --dependency-source names them. A package source analysis cannot read stays uncertifiable.";

/// Projects one seed. Used by the backend for package-contract issues that
/// are discovered after the reactive [`Program`] has been built.
#[must_use]
/// The acceptance gate says one thing — "this project has no accepted contract
/// for that package" — and it used to say it once per import site.
///
/// Measured on `solid-primitives-next/site`: 88 of the project's 191 findings
/// were this, and they named **17 packages**. Sixty-four of them were the same
/// sentence about `@solid-primitives/utils`. The per-site repetition carried no
/// information a reader could act on separately: the fix is one `contract
/// certify` per package, not per import, and nothing distinguishes the sites.
///
/// So they collapse to one finding per package, anchored at the first site and
/// carrying every other site in `related_locations` — visible in JSON output
/// and counted in the default renderer's help line. **Nothing is dropped**; the
/// same locations reach the same consumers, grouped by the thing that would fix
/// them.
///
/// The claim gates collapse too, one level finer, and for the same reason —
/// but only where they were raised at an *import*.
/// `unknown-contract-claims:` does say something specific about *that export* —
/// which is why it is not folded into the package — but at an import binding it
/// says nothing specific about the *site*: the message names the package, the
/// export and the open domains, and nothing else. Repeating it at every import
/// of that export is the same non-information the acceptance gate used to emit
/// per site.
///
/// At an *argument* it is a different finding, and `analysis_context` cannot
/// tell the two apart: `unknown-contract-claims:callbacks` is emitted both by
/// `push_unknown_contract_claims` at a binding and by `interproc` at one exact
/// call argument. So the producer records it — [`ContractDefectSite`] — and
/// this reads it. Getting that wrong is not theoretical: grouping argument
/// sites by export collapsed `package-callback-arguments-consumer` from four
/// findings to two, merging a rest parameter that absorbs a descriptor with an
/// `arguments` object that observes one, which is the distinction that fixture
/// exists to pin.
///
/// Measured while bundling: a five-file project importing four `@kobalte/utils`
/// exports produced 35 findings that were 7 distinct sentences, each repeated
/// five times with an empty `related_locations`. The same project with no
/// accepted contract produced 1. Delivering contracts must not cost a reader
/// that trade.
///
/// So the rule is: defects whose projected finding would be identical except
/// for its location become one finding. For the acceptance gate that is the
/// whole package, because no export-specific claim has been read yet; for a
/// claim gate it is the exact `(package, export, claims)`. A group of one keeps
/// its exact original wording either way.
///
/// **Open claims at call arguments collapse too** — the one argument-site
/// obligation that does. `unknown-contract-claims:callbacks` at an argument
/// says "the accepted contract for M leaves callbacks unknown for export E",
/// and nothing about that argument: the fix is closing E's domain in the
/// contract, once, not anything at the call. Measured on kobalte core
/// (`phase22/2026-09-26-project-side-certification-on-kobalte-core.md`, defect
/// 7): admitting a partly closed contract for `@solid-primitives/form` turned
/// its one acceptance-gate finding into 31 per-call warnings, so a project
/// that got strictly better reported more. They now group per
/// `(package, open domains)` for the project, anchored at the first site in
/// `(path, start, end)` order, with the rest in `related_locations` and the
/// count in the message.
///
/// **Open claims at imports group the same way** (ADR 0224). Accepting the
/// Solid Primitives contracts turned one acceptance-gate notice per package
/// into one per used export: the rc.13 corpus went from 675 to 741 `SC9005`
/// rows while its answers only improved. The fix is still the package's
/// contract, once, so every export with the same open domains shares one
/// finding, and a group of several exports names them in its message.
///
/// What still keeps its own finding at an argument is everything whose fix is
/// *at that call*: an unbound claim (`unbound-contract-claims:` — pass the
/// callback inline there), the argument-object/rest-parameter distinction
/// `package-callback-arguments-consumer` pins, and a runtime-identity
/// conflict. Import-site and argument-site open-claims groups stay apart in
/// the key because they count different things ("import sites", "call
/// sites"); their domain sets are disjoint today anyway — an import raises
/// `reactiveReads`/`returns`/`ownerRequirements`/`asyncBehavior`, an argument
/// raises exactly `callbacks`.
///
/// Every grouped finding names its subject (`subject_kind` `package` or
/// `package-export`), which is what tells a per-file reporter that its related
/// locations are further sites of the same finding rather than supporting
/// context. The ESLint adapter reports such a finding in each file holding one
/// of its sites, so collapsing across a project never hides a file's sites
/// from a per-file consumer.
fn collapse_unaccepted_contract_defects(
    defects: &[StaticDefect],
    catalog: &impl CatalogWording,
) -> Vec<Finding> {
    /// What makes two of these defects interchangeable.
    #[derive(Clone, Copy, Eq, Hash, PartialEq)]
    enum Interchangeable<'a> {
        /// The acceptance gate: the import matched no accepted contract at all,
        /// so no export-specific claim has been read yet and every site in the
        /// package has the same answer.
        Package(&'a str),
        /// A claim gate: an accepted contract leaves these exact domains open
        /// for this exact export. Different exports, and different open
        /// domains, are different findings.
        Claim(&'a str, &'a str, &'a str),
        /// Open claims at imports: an accepted contract leaves these exact
        /// domains open, for one or more of the package's exports. The fix is
        /// the package's contract, once, so the exports share one finding.
        OpenClaims(&'a str, &'a str),
        /// Open claims at call arguments: an accepted contract leaves these
        /// exact domains open, for one or more exports, observed at calls.
        CallClaim(&'a str, &'a str),
    }

    /// The exports a group names, sorted and distinct.
    fn group_exports<'d>(group: &[&'d StaticDefect]) -> Vec<&'d str> {
        let mut exports = group
            .iter()
            .filter_map(|defect| match &defect.kind {
                StaticDefectKind::PackageContractExportMissing { export, .. } => {
                    Some(export.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        exports.sort_unstable();
        exports.dedup();
        exports
    }

    /// "A, B, C, and 2 more": at most six names.
    fn listed(exports: &[&str]) -> String {
        let named = exports.len().min(6);
        let remainder = exports.len() - named;
        let mut listed = exports[..named].join(", ");
        if remainder > 0 {
            listed.push_str(&format!(", and {remainder} more"));
        }
        listed
    }

    /// The sentence for open claims shared by several exports, in the single
    /// export's wording with the exports listed instead of one name.
    fn open_claims_message(
        module: &str,
        context: &str,
        exports: &[&str],
        sites: usize,
        site_noun: &str,
        domain_noun: &str,
    ) -> String {
        let domains = context
            .strip_prefix("unknown-contract-claims:")
            .unwrap_or(context);
        format!(
            "the reactivity contract for {module} leaves {domains} unknown for {} {domain_noun} \
             exports: {}; code whose proof depends on those claims cannot be certified \
             ({sites} {site_noun}{})",
            exports.len(),
            listed(exports),
            if sites == 1 { "" } else { "s" },
        )
    }

    fn interchangeable(defect: &StaticDefect) -> Option<Interchangeable<'_>> {
        let StaticDefectKind::PackageContractExportMissing {
            module,
            export,
            site,
            ..
        } = &defect.kind
        else {
            return None;
        };
        // An obligation raised at an exact argument of an exact call keeps its
        // own finding when the site *is* the content: an unbound claim, whose
        // fix is at that call. Open claims at an argument name the package,
        // the export and the open domains and nothing about the call, so they
        // group like the import-site sentence does.
        if *site == crate::ContractDefectSite::Argument {
            return defect
                .analysis_context
                .starts_with("unknown-contract-claims:")
                .then_some(Interchangeable::CallClaim(
                    module.as_str(),
                    defect.analysis_context.as_str(),
                ));
        }
        // The exact context, not "not one of the specific prefixes": a defect
        // raised because an *accepted* contract has no summary for this export
        // is about that export, and collapsing it under the package would say
        // the package has no contract when it does.
        Some(
            if defect.analysis_context == crate::contracts::UNACCEPTED_IMPORT_CONTEXT {
                Interchangeable::Package(module.as_str())
            } else if defect
                .analysis_context
                .starts_with("unknown-contract-claims:")
            {
                // The fix is the same for every export the contract leaves
                // these domains open on: complete the package's contract.
                Interchangeable::OpenClaims(module.as_str(), defect.analysis_context.as_str())
            } else {
                Interchangeable::Claim(
                    module.as_str(),
                    export.as_str(),
                    defect.analysis_context.as_str(),
                )
            },
        )
    }

    // First-seen order, so the anchor of each group is the first site the
    // analysis reached and the output order does not depend on a hash.
    let mut order: Vec<Interchangeable<'_>> = Vec::new();
    let mut grouped: std::collections::HashMap<Interchangeable<'_>, Vec<&StaticDefect>> =
        std::collections::HashMap::new();
    let mut findings = Vec::new();
    for defect in defects {
        match interchangeable(defect) {
            Some(key) => {
                let group = grouped.entry(key).or_insert_with(|| {
                    order.push(key);
                    Vec::new()
                });
                group.push(defect);
            }
            // Everything else keeps its own finding, in place.
            None => findings.push(project_finding(FindingSeed::StaticDefect(defect), catalog)),
        }
    }
    for key in order {
        let group = &grouped[&key];
        if let Interchangeable::CallClaim(..) = key {
            // Anchored in a stable order rather than first-seen: call-site
            // obligations are gathered per file by the interprocedural pass,
            // and the anchor must not depend on the order files are visited.
            // An exact duplicate site is one site.
            let mut sites = group.clone();
            sites.sort_by(|left, right| {
                (
                    &left.location.path,
                    left.location.start_byte,
                    left.location.end_byte,
                )
                    .cmp(&(
                        &right.location.path,
                        right.location.start_byte,
                        right.location.end_byte,
                    ))
            });
            sites.dedup_by(|left, right| left.location == right.location);
            let mut finding = project_finding(FindingSeed::StaticDefect(sites[0]), catalog);
            finding.subject_kind = "package-export".into();
            let exports = group_exports(&sites);
            if exports.len() > 1 {
                let Interchangeable::CallClaim(module, context) = key else {
                    unreachable!("matched above")
                };
                finding.subject_kind = "package".into();
                finding.message = open_claims_message(
                    module,
                    context,
                    &exports,
                    sites.len(),
                    "call site",
                    "called",
                );
            } else if sites.len() > 1 {
                finding.message = format!("{} ({} call sites)", finding.message, sites.len());
            }
            finding
                .related_locations
                .extend(sites[1..].iter().map(|defect| defect.location.clone()));
            findings.push(finding);
            continue;
        }
        let mut finding = project_finding(FindingSeed::StaticDefect(group[0]), catalog);
        if let Interchangeable::OpenClaims(module, context) = key {
            let exports = group_exports(group);
            finding.subject_kind = "package-export".into();
            if exports.len() > 1 {
                finding.subject_kind = "package".into();
                finding.message = open_claims_message(
                    module,
                    context,
                    &exports,
                    group.len(),
                    "import site",
                    "imported",
                );
            } else if group.len() > 1 {
                finding.message = format!("{} ({} import sites)", finding.message, group.len());
            }
            finding
                .related_locations
                .extend(group[1..].iter().map(|defect| defect.location.clone()));
            findings.push(finding);
            continue;
        }
        let Interchangeable::Package(module) = key else {
            // A claim group keeps the message it already has -- it names the
            // package, the export and the open domains, which is the whole
            // content -- and gains the other sites and their count.
            finding.subject_kind = "package-export".into();
            if group.len() > 1 {
                finding.message = format!("{} ({} import sites)", finding.message, group.len());
                finding
                    .related_locations
                    .extend(group[1..].iter().map(|defect| defect.location.clone()));
            }
            findings.push(finding);
            continue;
        };
        finding.subject_kind = "package".into();
        if group.len() > 1 {
            let mut exports = group
                .iter()
                .filter_map(|defect| match &defect.kind {
                    StaticDefectKind::PackageContractExportMissing { export, .. } => {
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
            // A package-owned read is an implementation obligation. Its
            // compact row does not establish a strict-read warning: untracked
            // tracking alone cannot distinguish a labelled strict window
            // from a clearing. Explicitly cleared reads never reach this row.
            // A read attributed through another function's summary: whether
            // it runs while this call does is not established.
            if read.summary_attributed && !read.package_internal && finding.kind != "uncertifiable"
            {
                finding.kind = "uncertifiable".into();
                finding.message = format!(
                    "{}; this read happens inside {}'s body, and nothing proves it runs while this call does, rather than only on some of its paths or later (from a timer, listener, getter, returned accessor or effect), so it is not proven to be read untracked here",
                    finding.message, read.via
                );
            }
            if read.package_internal {
                finding.kind = "uncertifiable".into();
                finding.message = format!(
                    "{}'s package contract states a read of its own reactive state while it runs, but its strict-read execution context here is not established; whether Solid emits STRICT_READ_UNTRACKED remains uncertifiable, so this is a package-implementation obligation rather than proven misuse at this call",
                    read.via
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
            // ADR 0119: severity by gate. An accepted contract that leaves
            // claims open is analysis over a stated partial premise, so its
            // obligation is a warning; every other SC9005 gate -- no accepted
            // contract, an obsolete policy, an export the contract omits, a
            // claim the site cannot bind -- stays the rule's error. The kind
            // stays `uncertifiable`, so the run still certifies nothing. This
            // inverts the owner-requirement precedent
            // (`Finding::for_owner_requirement`), which *raises* an uncertain
            // requirement to error; the ADR says why.
            if matches!(
                defect.kind,
                StaticDefectKind::PackageContractExportMissing { .. }
            ) && defect
                .analysis_context
                .starts_with("unknown-contract-claims:")
            {
                finding.severity = "warning".into();
            }
            // The acceptance gate at an import whose package *has* an
            // acceptance -- a project catalog entry -- that was refused: say
            // so, and why, as one more evidence step. The message stays the
            // gate's own, so a project with no catalog reads exactly as before.
            if let StaticDefectKind::PackageContractExportMissing {
                admission_refusal: Some(refusal),
                ..
            } = &defect.kind
            {
                finding.evidence.push(EvidenceStep {
                    message: refusal.clone(),
                    location: None,
                });
            }
        }
        FindingSeed::AsyncRead(read) => {
            finding.related_locations = vec![read.declaration.clone()];
            // Fail-honest: an options argument the analyzer cannot read may
            // declare a loadingValue, and a declared first flight cannot
            // throw — so the untracked-read error is no longer a *proven*
            // runtime throw and becomes a proof obligation instead. The
            // boundary rules keep their reporting: ordinary SC5003 is
            // informational either way, and the leaf-owner SC5001 variant's
            // throw is timing-dependent by nature.
            if read.options_opaque && finding.id == "SC5001" {
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
        FindingSeed::Action(_)
        | FindingSeed::DirectiveCreation(_)
        | FindingSeed::OwnerRequirement(_) => {}
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
            StaticDefectKind::PackageContractExportMissing { module, .. } => {
                Some(("package-contract", package_of_specifier(module).to_owned()))
            }
            StaticDefectKind::ReactiveDispatchUnresolved { callee, .. } => {
                let context = defect.analysis_context.as_str();
                Some((
                    if context.starts_with("parameter-member")
                        || context.starts_with("contract-parameter-member")
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
