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
        } => (
            format!(
                "the reactivity contract for {module} has no entrypoint/export summary for {} export {export}; solid-checker cannot tell whether it reads reactive values, takes tracked callbacks, or returns accessors, so code flowing through it cannot be certified",
                if *reexported { "re-exported" } else { "imported" }
            ),
            (terms.removed_export_hint)(module, export).unwrap_or_else(|| {
                format!(
                    "Add an export summary for {export} to the package's solid-reactivity.json (reactive reads, callbacks, return kind); an empty summary certifies explicitly that the export is not reactive. See docs/package-contracts.md for the format."
                )
            }),
        ),
        StaticDefectKind::PackageContractEnvironmentDependent {
            module,
            export,
            reexported,
        } => (
            format!(
                "the reactivity contract for {module} has different certified behavior for conditional runtime targets at {} export {export}; solid-checker has no selected environment and cannot apply one variant without guessing",
                if *reexported { "re-exported" } else { "imported" }
            ),
            format!(
                "Select and pin the package's runtime conditions before certifying {export}, or publish one environment-independent contract summary. See docs/package-contracts.md for the format."
            ),
        ),
        StaticDefectKind::UnknownCallbackExecution {
            package,
            entrypoint,
            function,
            parameter,
            parameter_type,
            required_execution,
            contract_stub,
        } => (
            format!(
                "callback parameter {parameter} ({parameter_type}) of {package}{entrypoint}:{function} reaches a call whose execution timing is unknown; this callback cannot be certified"
            ),
            format!(
                "Audit the implementation and add an explicit contract for {package}{entrypoint} export {function} with callback parameter {parameter}. Required behavior: {required_execution}. Generate from package source with `solid-checker contract generate --package-root <package-root> --entrypoint {entrypoint}`, or edit this JSON stub (then replace its placeholders and review the evidence): {contract_stub}"
            ),
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
                "the reactive source {source:?} is passed to {callee}, whose reactive behaviour is not described anywhere: it has no body in this project, no package contract entry, and is not a Solid primitive; whether reads through it stay tracked cannot be certified"
            ),
            format!(
                "If {callee} comes from a package, describe its callback behavior in solid-reactivity.json. Otherwise, route the call through a project-local adapter whose body makes invocation or retention explicit. See docs/package-contracts.md."
            ),
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
    };
    let evidence = match &defect.kind {
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
        StaticDefectKind::PackageContractExportMissing { .. } => {
            "the imported package has a contract, but this export has no effect summary"
        }
        StaticDefectKind::PackageContractEnvironmentDependent { .. } => {
            "the imported package has conditional effect summaries, but no runtime environment was selected"
        }
        StaticDefectKind::UnknownCallbackExecution { .. } => {
            "TypeScript resolved the callable parameter, but no exact runtime contract proves when the external helper invokes it"
        }
        StaticDefectKind::ReactiveDispatchUnresolved { .. } => {
            "the call is type-correct, but exact runtime dispatch or equivalent reactive summaries are not proven"
        }
        StaticDefectKind::ReactiveCallbackUnresolved { .. } => {
            "the built-in callback position is type-correct and synchronous, but the callback body's reactive reads are not available for proof"
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
        StaticDefectKind::ReactiveReadAfterAwait { .. }
        | StaticDefectKind::MissingEffectFunction
        | StaticDefectKind::ReactiveSourceUncaptured { .. }
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
    /// Whether an `expected-function-got-expression` finding owns the
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

/// Why an imported Solid-aware package cannot provide usable summaries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PackageContractIssueKind {
    Missing,
    Unverified,
    /// A contract exists and is well-formed, but describes a different release
    /// of the package than the one installed. It is evidence about an artifact
    /// this project no longer has, so it is dropped rather than applied.
    ///
    /// `Bundled` is the same fact about this checker's own audited contract,
    /// which the consumer cannot regenerate; the two carry different remedies
    /// and so cannot share one kind.
    Stale {
        contract_version: String,
        installed_version: String,
    },
    StaleBundled {
        audited_version: String,
        installed_version: String,
    },
}

/// The backend-owned package discovery facts a catalog words as SC9005.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageContractIssue {
    pub package: String,
    pub contract_path: String,
    pub status: PackageContractIssueKind,
    pub location: Location,
}

/// Every analysis result that can become a finding.
#[derive(Clone, Copy, Debug)]
pub enum FindingSeed<'a> {
    StrictRead(&'a ReactiveRead),
    OwnedWrite(&'a ReactiveWrite),
    Action(&'a ActionInvocation),
    LeafOperation(&'a LeafOwnerOperation),
    StaticViolation(&'a StaticViolation),
    StaticDefect(&'a StaticDefect),
    DirectiveCreation(&'a PrimitiveCreation),
    OwnerRequirement(&'a OwnerRequirement),
    AsyncRead(&'a AsyncRead),
    PackageContractIssue(&'a PackageContractIssue),
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
                    && (capabilities.module_scope_strict_reads
                        || read.execution != crate::ExecutionRole::ModuleInitialization)
            })
            .map(|read| project_finding(FindingSeed::StrictRead(read), catalog)),
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
    findings.extend(
        program
            .static_defects
            .iter()
            .map(|defect| project_finding(FindingSeed::StaticDefect(defect), catalog)),
    );
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

    // One defect class, one rule: when expected-function-got-expression
    // claims a handler expression, the strict-read finding on the identical
    // span is the same defect worded twice — the handler rule carries the
    // more specific consequence, so it wins and the strict read is dropped.
    if capabilities.handler_expression_owns_strict_read {
        let handler_spans: std::collections::HashSet<_> = program
            .static_defects
            .iter()
            .filter(|defect| matches!(defect.kind, StaticDefectKind::ReactiveHandlerRead { .. }))
            .map(|defect| {
                (
                    defect.location.path.clone(),
                    defect.location.start_byte,
                    defect.location.end_byte,
                )
            })
            .collect();
        if !handler_spans.is_empty() {
            // SC1001 is the strict-read rule's stable diagnostic code in
            // every catalog; the external rule name differs per dialect.
            findings.retain(|finding| {
                finding.id != "SC1001"
                    || !handler_spans.contains(&(
                        finding.primary_location.path.clone(),
                        finding.primary_location.start_byte,
                        finding.primary_location.end_byte,
                    ))
            });
        }
    }

    finish_findings(findings, total_started, construction_started)
}

/// Projects one seed. Used by the backend for package-contract issues that
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
        FindingSeed::StrictRead(read) => {
            finding.analysis_context = read.context.to_string();
            finding.subject_kind = read.kind.to_string();
            finding.related_locations = crate::strict_read_related_locations(read);
            // Fail-honest: a props read whose component's callers cannot be
            // enumerated may or may not be signal-backed, so the finding is a
            // proof obligation, not a proven runtime warning.
            if read.uncertain {
                finding.kind = "uncertifiable".into();
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
            if operation.uncertain {
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
        }
        FindingSeed::AsyncRead(read) => {
            finding.related_locations = vec![read.declaration.clone()];
            // Fail-honest: an options argument the analyzer cannot read may
            // declare a loadingValue, and a declared first flight cannot
            // throw — so the untracked-read error is no longer a *proven*
            // runtime throw and becomes a proof obligation instead. The
            // boundary rules keep their reporting: SC5003 is informational
            // either way, and SC5002's throw is timing-dependent by nature.
            if read.options_opaque && finding.id == "SC5001" {
                finding.kind = "uncertifiable".into();
            }
            if read.server_rendering_unresolved && finding.id == "SC5005" {
                finding.kind = "uncertifiable".into();
            }
        }
        FindingSeed::PackageContractIssue(_) => {
            finding.analysis_context = "package contract completeness".into();
            finding.subject_kind = "package".into();
        }
        FindingSeed::Action(_)
        | FindingSeed::DirectiveCreation(_)
        | FindingSeed::OwnerRequirement(_) => {}
    }
    finding
}

fn primary_location(seed: FindingSeed<'_>) -> Location {
    match seed {
        FindingSeed::StrictRead(value) => value.location.clone(),
        FindingSeed::OwnedWrite(value) => value.location.clone(),
        FindingSeed::Action(value) => value.location.clone(),
        FindingSeed::LeafOperation(value) => value.location.clone(),
        FindingSeed::StaticViolation(value) => value.location.clone(),
        FindingSeed::StaticDefect(value) => value.location.clone(),
        FindingSeed::DirectiveCreation(value) => value.location.clone(),
        FindingSeed::OwnerRequirement(value) => value.location.clone(),
        FindingSeed::AsyncRead(value) => value.location.clone(),
        FindingSeed::PackageContractIssue(value) => value.location.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{ActionInvocation, AsyncRead, ExecutionRole};

    struct RecordingCatalog(CatalogCapabilities);

    impl CatalogWording for RecordingCatalog {
        fn capabilities(&self) -> CatalogCapabilities {
            self.0
        }

        fn wording(&self, seed: FindingSeed<'_>) -> FindingWording {
            let message = match seed {
                FindingSeed::StrictRead(_) => "read",
                FindingSeed::OwnedWrite(_) => "write",
                FindingSeed::Action(_) => "action",
                FindingSeed::LeafOperation(_) => "leaf",
                FindingSeed::StaticViolation(_) => "static-violation",
                FindingSeed::StaticDefect(_) => "static-defect",
                FindingSeed::DirectiveCreation(_) => "directive",
                FindingSeed::OwnerRequirement(_) => "owner",
                FindingSeed::AsyncRead(_) => "async",
                FindingSeed::PackageContractIssue(_) => "package",
            };
            FindingWording::new(
                RuleMetadata {
                    code: "TEST00",
                    name: "test",
                    severity: "warning",
                    uncertifiable: false,
                },
                message,
                "",
            )
        }
    }

    fn location(index: u64) -> Location {
        Location {
            path: "projection.tsx".into(),
            start_byte: index,
            end_byte: index + 1,
        }
    }

    #[test]
    fn catalog_capabilities_gate_whole_program_tables() {
        let program = Program {
            actions: vec![ActionInvocation {
                action: "save".into(),
                location: location(1),
                declaration: location(2),
                execution: ExecutionRole::TrackedJsx,
                context: "App".into(),
            }],
            async_reads: vec![AsyncRead {
                accessor: Arc::from("user()"),
                location: location(3),
                declaration: location(4),
                execution: ExecutionRole::TrackedJsx,
                leaf_owner: None,
                under_loading: false,
                async_provenance: true,
                declared_loading: false,
                options_opaque: false,
                ssr_client_hole: false,
                server_rendering_unresolved: false,
            }],
            ..Program::default()
        };

        let (solid_one, _) =
            project_findings(&program, &RecordingCatalog(CatalogCapabilities::SOLID_1));
        assert!(solid_one.is_empty());

        let (solid_two, _) =
            project_findings(&program, &RecordingCatalog(CatalogCapabilities::SOLID_2));
        assert_eq!(
            solid_two
                .iter()
                .map(|finding| finding.message.as_str())
                .collect::<Vec<_>>(),
            ["action", "async"]
        );
    }

    #[test]
    fn selection_filters_safe_rows_before_the_catalog_seam() {
        let program = Program {
            actions: vec![ActionInvocation {
                action: "save".into(),
                location: location(1),
                declaration: location(2),
                execution: ExecutionRole::EventCallback,
                context: "handler".into(),
            }],
            async_reads: vec![AsyncRead {
                accessor: Arc::from("user()"),
                location: location(3),
                declaration: location(4),
                execution: ExecutionRole::TrackedJsx,
                leaf_owner: None,
                under_loading: true,
                async_provenance: true,
                declared_loading: false,
                options_opaque: false,
                ssr_client_hole: false,
                server_rendering_unresolved: false,
            }],
            ..Program::default()
        };
        let (findings, _) =
            project_findings(&program, &RecordingCatalog(CatalogCapabilities::SOLID_2));
        assert!(findings.is_empty());
    }
}
