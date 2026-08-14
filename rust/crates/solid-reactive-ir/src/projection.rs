//! The deep seam between reactive analysis and dialect rule catalogs.
//!
//! Analysis produces [`Program`] tables. This module alone knows which rows
//! become diagnostics and how a worded diagnostic is assembled; a catalog is
//! a small wording adapter over the closed [`FindingSeed`] vocabulary.

use std::time::Instant;

use typefacts::Location;

use crate::{
    ActionInvocation, AsyncRead, EvidenceStep, Finding, InvalidCleanupReturn, LeafOwnerOperation,
    OwnerRequirement, PrimitiveCreation, Program, ReactiveRead, ReactiveWrite, RuleMetadata,
    SolveTimings, StaticDefect, StaticDefectKind, StaticViolation, UnresolvedCleanupReturn,
    finish_findings,
};

/// The few phrases where shared static-defect concepts use dialect APIs.
pub struct StaticDefectTerms {
    pub props_destructure_hint: &'static str,
    pub reactive_object_destructure_hint: &'static str,
    pub missing_effect_message: &'static str,
    pub missing_effect_hint: &'static str,
    pub tracked_derived_scope: &'static str,
    pub store_mutation_hint: fn(&str) -> String,
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
        StaticDefectKind::ExecutionMapIncomplete => (
            "the Solid compiler did not classify this JSX expression as tracked, untracked, or a callback; without an execution role, solid-checker cannot certify any reactive read inside it".into(),
            "Simplify the expression: hoist complex logic into a createMemo and interpolate the accessor. If this persists on plain JSX, re-run with fresh compiler facts and report the pattern as a solid-checker issue.".into(),
        ),
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
        StaticDefectKind::PreferComponentSyntax { name } => (
            format!(
                "JSX-returning function {name:?} is called imperatively inside JSX; this hides component identity and can evaluate setup logic in the caller's reactive expression"
            ),
            format!(
                "Rename it with an uppercase component name and render it as <{} />. If this is intentionally a value helper, return data rather than JSX.",
                uppercase_first(name)
            ),
        ),
        StaticDefectKind::ImplicitDraggableBoolean => (
            "the draggable attribute uses JSX boolean shorthand, which emits an empty attribute value; HTML treats that as the invalid/default state rather than draggable=true".into(),
            "Write draggable=\"true\" for a static attribute, or draggable={condition} for a dynamic boolean value.".into(),
        ),
        StaticDefectKind::InvalidJsxNesting {
            parent,
            child,
            ancestor,
        } => (
            format!(
                "HTML parsing changes <{child}> nested {} <{parent}>, so the browser DOM differs from the authored JSX and can fail hydration",
                if *ancestor { "inside" } else { "directly under" }
            ),
            format!(
                "Move <{child}> outside <{parent}> or add the HTML-required wrapper so the server and browser construct the same tree."
            ),
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
            format!(
                "Add an export summary for {export} to the package's solid-reactivity.json (reactive reads, callbacks, return kind); an empty summary certifies explicitly that the export is not reactive. See docs/package-contracts.md for the format."
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
        StaticDefectKind::MissingEffectFunction => (
            terms.missing_effect_message.into(),
            terms.missing_effect_hint.into(),
        ),
        StaticDefectKind::UntrackedDerivedFunction { name } => (
            format!(
                "{name} derives from reactive state but every call to it is untracked, so its reads subscribe to nothing and the derivation never updates"
            ),
            format!(
                "Call {name} from a tracking scope — {} — or inline the value if a one-off read at setup is what was meant.",
                terms.tracked_derived_scope
            ),
        ),
        StaticDefectKind::ReactiveSourceUncaptured { source, callee } => (
            format!(
                "the reactive source {source:?} is passed to {callee}, whose reactive behaviour is not described anywhere: it has no body in this project, no package contract entry, and is not a Solid primitive; whether reads through it stay tracked cannot be certified"
            ),
            format!(
                "Describe {callee} in its package's solid-reactivity.json — which arguments it tracks and what it returns — or keep the function in the project so its body is analysed. See docs/package-contracts.md."
            ),
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
        StaticDefectKind::HandlerCallResult {
            attribute,
            callee,
            call,
        } => (
            format!(
                "{attribute} is given the result of calling {callee}, not a function; the call runs once during setup and its value is bound as the listener"
            ),
            format!("Wrap it: {attribute}={{() => {call}}}, or pass the function itself uncalled."),
        ),
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
        StaticDefectKind::PreferComponentSyntax { .. } => {
            "the resolved local function directly returns JSX and this call is inside JSX"
        }
        StaticDefectKind::ImplicitDraggableBoolean => {
            "the intrinsic draggable attribute has no explicit value"
        }
        StaticDefectKind::InvalidJsxNesting { .. } => {
            "the intrinsic JSX ancestor chain is statically known and HTML parsing changes this nesting"
        }
        StaticDefectKind::PackageContractExportMissing { .. } => {
            "the imported package has a contract, but this export has no effect summary"
        }
        StaticDefectKind::UnknownCallbackExecution { .. } => {
            "TypeScript resolved the callable parameter, but no exact runtime contract proves when the external helper invokes it"
        }
        StaticDefectKind::ExecutionMapIncomplete
        | StaticDefectKind::ReactiveReadAfterAwait { .. }
        | StaticDefectKind::MissingEffectFunction
        | StaticDefectKind::UntrackedDerivedFunction { .. }
        | StaticDefectKind::ReactiveSourceUncaptured { .. }
        | StaticDefectKind::ReactiveHandlerRead { .. }
        | StaticDefectKind::HandlerCallResult { .. }
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

fn uppercase_first(value: &str) -> String {
    let mut characters = value.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(characters).collect()
    })
}

/// The program tables a dialect catalog intentionally projects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogCapabilities {
    pub actions: bool,
    pub async_reads: bool,
    pub cleanup_returns: bool,
}

impl CatalogCapabilities {
    pub const SOLID_1: Self = Self {
        actions: false,
        async_reads: false,
        cleanup_returns: false,
    };

    pub const SOLID_2: Self = Self {
        actions: true,
        async_reads: true,
        cleanup_returns: true,
    };
}

/// Why an imported Solid-aware package cannot provide usable summaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageContractIssueKind {
    Missing,
    Unverified,
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
    InvalidCleanupReturn(&'a InvalidCleanupReturn),
    UnresolvedCleanupReturn(&'a UnresolvedCleanupReturn),
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
            .filter(|read| read.execution.reports_untracked_read())
            .map(|read| project_finding(FindingSeed::StrictRead(read), catalog)),
    );
    findings.extend(
        program
            .writes
            .iter()
            .filter(|write| !write.allowed_by_option && write.execution.reports_disallowed_write())
            .map(|write| project_finding(FindingSeed::OwnedWrite(write), catalog)),
    );
    findings.extend(
        program
            .leaf_operations
            .iter()
            .map(|operation| project_finding(FindingSeed::LeafOperation(operation), catalog)),
    );
    if capabilities.cleanup_returns {
        findings.extend(
            program.invalid_cleanup_returns.iter().map(|invalid| {
                project_finding(FindingSeed::InvalidCleanupReturn(invalid), catalog)
            }),
        );
        findings.extend(program.unresolved_cleanup_returns.iter().map(|unresolved| {
            project_finding(FindingSeed::UnresolvedCleanupReturn(unresolved), catalog)
        }));
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
    findings.extend(
        program
            .directive_creations
            .iter()
            .map(|creation| project_finding(FindingSeed::DirectiveCreation(creation), catalog)),
    );
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
                    read.leaf_owner.is_some()
                        || read.execution == crate::ExecutionRole::ModuleInitialization
                        || read.execution == crate::ExecutionRole::UntrackedRendering
                        || read.execution == crate::ExecutionRole::TrackedJsx && !read.under_loading
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
        }
        FindingSeed::StaticViolation(violation) => {
            finding.analysis_context = violation.analysis_context.clone();
            finding.fixes = violation.fixes.clone();
        }
        FindingSeed::StaticDefect(defect) => {
            finding.analysis_context = defect.analysis_context.clone();
            finding.fixes = defect.fixes.clone();
        }
        FindingSeed::AsyncRead(read) => {
            finding.related_locations = vec![read.declaration.clone()];
        }
        FindingSeed::PackageContractIssue(_) => {
            finding.analysis_context = "package contract completeness".into();
            finding.subject_kind = "package".into();
        }
        FindingSeed::Action(_)
        | FindingSeed::InvalidCleanupReturn(_)
        | FindingSeed::UnresolvedCleanupReturn(_)
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
        FindingSeed::InvalidCleanupReturn(value) => value.location.clone(),
        FindingSeed::UnresolvedCleanupReturn(value) => value.location.clone(),
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
                FindingSeed::InvalidCleanupReturn(_) => "invalid-cleanup",
                FindingSeed::UnresolvedCleanupReturn(_) => "unresolved-cleanup",
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
            }],
            invalid_cleanup_returns: vec![InvalidCleanupReturn {
                primitive: "onSettled".into(),
                location: location(5),
            }],
            unresolved_cleanup_returns: vec![UnresolvedCleanupReturn {
                primitive: "createEffect".into(),
                location: location(6),
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
            ["action", "async", "invalid-cleanup", "unresolved-cleanup"]
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
            }],
            ..Program::default()
        };
        let (findings, _) =
            project_findings(&program, &RecordingCatalog(CatalogCapabilities::SOLID_2));
        assert!(findings.is_empty());
    }
}
