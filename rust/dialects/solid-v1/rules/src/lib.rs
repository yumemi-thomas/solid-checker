//! The Solid 1.x rule catalog: projects the reactive IR's `Program` onto the
//! dialect's findings.
//!
//! The engine's analysis is shared with the 2.0 dialect; what is dialect-
//! specific here is which tables can fire at all under 1.x vocabulary, the
//! external rule names (`v1/<rule>`), and every sentence of message and hint —
//! a 1.x diagnostic never tells its reader to call an API their Solid version
//! does not have.

mod rules;

use solid_reactive_ir::{
    CatalogCapabilities, CatalogWording, FindingSeed, FindingWording, OwnerRequirementOperation,
    PackageContractIssue, PackageContractIssueKind, Program, ReactiveSourceKind,
    ReactiveWriteOperation, StaticDefect, StaticDefectKind, project_finding, project_findings,
    strict_read_evidence, strict_read_message,
};

pub use rules::{Rule, docs_url, manifest_json};
pub use solid_reactive_ir::{EvidenceStep, Finding, RuleMetadata, SolveTimings};

struct Catalog;

#[must_use]
pub fn solve_measured(program: &Program) -> (Vec<Finding>, SolveTimings) {
    project_findings(program, &Catalog)
}

#[must_use]
pub fn package_contract_finding(issue: &PackageContractIssue) -> Finding {
    project_finding(FindingSeed::PackageContractIssue(issue), &Catalog)
}

fn strict_read_hint(kind: &str) -> &'static str {
    if kind == "component-props" {
        "Move the prop read into a tracking scope: read props.<name> directly in JSX, derive it with createMemo(() => props.<name>), or read it in the callback of createEffect(fn). Do not use untrack() to make a prop reactive; untrack() only documents an intentional one-time snapshot."
    } else {
        "Move the read into a tracking scope: JSX, a createMemo, or the callback of createEffect(fn). If a one-time snapshot is intended, wrap the read in untrack() to make that explicit."
    }
}

impl CatalogWording for Catalog {
    fn capabilities(&self) -> CatalogCapabilities {
        CatalogCapabilities::SOLID_1
    }

    fn wording(&self, seed: FindingSeed<'_>) -> FindingWording {
        match seed {
            FindingSeed::StrictRead(read) => FindingWording::new(
                Rule::StrictReadUntracked.metadata(),
                strict_read_message(read),
                strict_read_hint(&read.kind),
            )
            .with_evidence(strict_read_evidence(read)),
            FindingSeed::OwnedWrite(write) => owned_write_wording(write),
            FindingSeed::LeafOperation(_) => {
                panic!("the Solid 1.x catalog does not project leaf-owner operations")
            }
            FindingSeed::StaticDefect(defect) => static_defect_wording(defect),
            FindingSeed::StaticViolation(violation) => static_violation_wording(violation),
            FindingSeed::DirectiveCreation(_) => {
                panic!("the Solid 1.x catalog does not project directive creations")
            }
            FindingSeed::OwnerRequirement(requirement) => {
                let (rule, message, hint) = match requirement.operation {
                    OwnerRequirementOperation::Cleanup => (
                        Rule::NoOwnerCleanup,
                        "onCleanup is called without a reactive owner; no scope's disposal can trigger it, so this cleanup function will never run",
                        "Call onCleanup inside a component or computation, or create the surrounding scope with createRoot so disposal exists.",
                    ),
                    OwnerRequirementOperation::Boundary => (
                        Rule::NoOwnerBoundary,
                        "boundary is created without a reactive owner; it can never be disposed, and the subtree it manages will leak",
                        "Render boundaries inside a component tree rooted by render() or hydrate(), or under an explicit createRoot; a boundary created in a bare helper function has no owner to attach to.",
                    ),
                    OwnerRequirementOperation::Effect => (
                        Rule::NoOwnerEffect,
                        "effect is created without a reactive owner; nothing will ever dispose it, so it keeps running and holding its subscriptions for the lifetime of the app",
                        "Create effects inside a component or computation so their owner disposes them. For deliberate module-scope reactivity, wrap the setup in createRoot(dispose => ...) and keep the dispose handle.",
                    ),
                    OwnerRequirementOperation::SettledCleanup => {
                        panic!("Solid 1.x analysis emitted a 2.0-only settled-cleanup requirement")
                    }
                };
                FindingWording::new(rule.metadata(), message, hint)
            }
            FindingSeed::PackageContractIssue(issue) => {
                let (condition, hint) = match &issue.status {
                    PackageContractIssueKind::Stale {
                        contract_version,
                        installed_version,
                    } => (
                        format!(
                            "has a reactivity contract for version {contract_version}, but version {installed_version} is installed"
                        ),
                        format!(
                            "The contract is evidence about a release this project no longer installs. Regenerate it with `solid-checker contract generate --package-root node_modules/{package} --output .solid-checker/contracts/{package}/solid-reactivity.json`, then review the checklist written beside it. A project-owned contract overrides a package-published one, so this is the remedy for either.",
                            package = issue.package
                        ),
                    ),
                    PackageContractIssueKind::StaleBundled {
                        audited_version,
                        installed_version,
                    } => (
                        format!(
                            "is audited by this checker at version {audited_version}, but version {installed_version} is installed"
                        ),
                        format!(
                            "Bundled contracts describe one exact release. Install {} {audited_version}, or upgrade solid-checker to a release that audits {installed_version}.",
                            issue.package
                        ),
                    ),
                    PackageContractIssueKind::Unverified => (
                        "has only an unverified generated reactivity contract".to_owned(),
                        "Verify the generated contract against the exact package artifacts and behavioral probes, then record verified, reviewed, or attested evidence.".into(),
                    ),
                    PackageContractIssueKind::Missing => (
                        "has no reactivity contract".to_owned(),
                        format!(
                            "Create a local contract at {}, or pass one explicitly with --contract <PATH>. If you maintain {}, ship solid-reactivity.json in the package root so every consumer gets it. See docs/package-contracts.md for the format.",
                            issue.contract_path, issue.package
                        ),
                    ),
                };
                FindingWording::new(
                    Rule::PackageContractMissing.metadata(),
                    format!(
                        "imported Solid package {:?} {condition}; solid-checker cannot rely on its export summaries, so every use of them is uncertifiable",
                        issue.package
                    ),
                    hint,
                )
            }
            FindingSeed::Action(_) | FindingSeed::AsyncRead(_) => {
                panic!("Solid 1.x projector received a disabled 2.0-only finding seed")
            }
        }
    }
}

fn owned_write_wording(write: &solid_reactive_ir::ReactiveWrite) -> FindingWording {
    match write.operation {
        ReactiveWriteOperation::Refresh => {
            panic!("Solid 1.x analysis emitted a 2.0-only refresh write")
        }
        ReactiveWriteOperation::Setter => {}
    }
    let context = if write.context.is_empty() {
        "owned scope"
    } else {
        &write.context
    };
    let source = match write.source_kind {
        ReactiveSourceKind::Accessor => "accessor",
        ReactiveSourceKind::Store => "store",
    };
    FindingWording::new(
        Rule::ReactiveWriteInOwnedScope.metadata(),
        format!(
            "{source} setter {:?} is called inside owned scope {context}; writes during the tracking phase re-trigger the computation that made them and can loop the reactive graph",
            write.setter
        ),
        "Derive the value instead of writing it back: replace compute-then-set with a createMemo. If the write is genuinely imperative, move it to an event handler, onMount, or a callback that runs after the current computation.",
    )
    .with_evidence(vec![
        EvidenceStep {
            message: format!(
                "{:?} is paired with a source proven to be a Solid {source}",
                write.setter
            ),
            location: Some(write.declaration.clone()),
        },
        EvidenceStep {
            message: "this scope is owned (tracking phase); writes are only allowed in event handlers, onMount, untracked or deferred callbacks, and directive bodies".into(),
            location: Some(write.location.clone()),
        },
    ])
}

fn static_violation_wording(violation: &solid_reactive_ir::StaticViolation) -> FindingWording {
    let rule = Rule::from_identity(&violation.id, &violation.rule).unwrap_or_else(|| {
        panic!(
            "diagnostic identity is missing from the v1 rule catalog: {} [{}]",
            violation.id, violation.rule
        )
    });
    let evidence = match rule {
        Rule::EventHandlers => {
            "the native JSX attribute spelling does not match Solid's event-handler contract"
        }
        Rule::JsxNoDuplicateProps => "the same JSX property is assigned more than once",
        Rule::JsxNoScriptUrl => "the statically resolved URL uses the javascript: scheme",
        Rule::JsxNoUndef => "the JSX name has no value-space binding in lexical scope",
        Rule::NoArrayHandlers if violation.uncertain => {
            "the native event attribute's value may be a plain function or a bound-handler array"
        }
        Rule::NoArrayHandlers => "the native event attribute receives an array-valued handler",
        Rule::NoInnerhtml => "the JSX attribute writes markup through an HTML injection sink",
        Rule::NoProxyApis => "the import or call requires Proxy-backed Solid APIs",
        Rule::NoReactDeps => "a Solid reactive primitive received a React-style dependency array",
        Rule::NoReactSpecificProps => "the JSX attribute uses a React-specific property spelling",
        Rule::NoUnknownNamespaces => "the JSX namespace is outside Solid's known vocabulary",
        Rule::PreferClasslist => "a class helper call matches the configured classList preference",
        Rule::PreferFor => "an array map call is used directly in a JSX rendering position",
        Rule::PreferShow => "a conditional JSX expression matches the configured Show preference",
        Rule::SelfClosingComp => {
            "the element's child and closing-tag shape conflicts with the configured policy"
        }
        Rule::StyleProp => "the style attribute shape conflicts with Solid's style contract",
        Rule::NoAsyncTrackedScope => {
            "the tracked callback is syntactically async and can continue after an await"
        }
        Rule::StrictReadUntracked
        | Rule::ReactiveReadAfterAwait
        | Rule::NoDestructure
        | Rule::ComponentsReturnOnce
        | Rule::ReactiveWriteInOwnedScope
        | Rule::NoOwnerEffect
        | Rule::NoOwnerCleanup
        | Rule::NoOwnerBoundary
        | Rule::MissingEffectFunction
        | Rule::UncalledAccessor
        | Rule::ExpectedFunctionGotExpression
        | Rule::NoDirectMutation
        | Rule::ReactiveSourceUncaptured
        | Rule::ReactiveDispatchUnresolved
        | Rule::PreferComponentSyntax
        | Rule::NoImplicitDraggable
        | Rule::ValidJsxNesting
        | Rule::JsxUsesVars
        | Rule::PackageContractExportMissing
        | Rule::PackageContractCallbackMissing
        | Rule::PackageContractMissing
        | Rule::ExecutionMapIncomplete => panic!(
            "v1 rule {} is not emitted through the static-violation channel",
            rule.metadata().name
        ),
    };
    FindingWording::new(
        rule.metadata(),
        violation.message.clone(),
        violation.hint.clone(),
    )
    .with_evidence(vec![EvidenceStep {
        message: evidence.into(),
        location: Some(violation.location.clone()),
    }])
}

fn static_defect_wording(defect: &StaticDefect) -> FindingWording {
    let rule = match &defect.kind {
        StaticDefectKind::ExecutionMapIncomplete => Rule::ExecutionMapIncomplete,
        StaticDefectKind::ReactiveObjectDestructure { .. } => Rule::NoDestructure,
        StaticDefectKind::ReactiveReadAfterAwait { .. } => Rule::ReactiveReadAfterAwait,
        StaticDefectKind::ComponentReturnsConditionally => Rule::ComponentsReturnOnce,
        StaticDefectKind::PreferComponentSyntax { .. } => Rule::PreferComponentSyntax,
        StaticDefectKind::ImplicitDraggableBoolean { .. } => Rule::NoImplicitDraggable,
        StaticDefectKind::InvalidJsxNesting { .. } => Rule::ValidJsxNesting,
        StaticDefectKind::PackageContractExportMissing { .. } => Rule::PackageContractExportMissing,
        StaticDefectKind::PackageContractEnvironmentDependent { .. } => {
            Rule::PackageContractExportMissing
        }
        StaticDefectKind::UnknownCallbackExecution { .. } => Rule::PackageContractCallbackMissing,
        StaticDefectKind::MissingEffectFunction => Rule::MissingEffectFunction,
        StaticDefectKind::ReactiveSourceUncaptured { .. } => Rule::ReactiveSourceUncaptured,
        StaticDefectKind::ReactiveDispatchUnresolved { .. }
        | StaticDefectKind::ReactiveCallbackUnresolved { .. }
        | StaticDefectKind::StructuredReturnUnresolved { .. } => Rule::ReactiveDispatchUnresolved,
        StaticDefectKind::ReactiveHandlerRead { .. }
        | StaticDefectKind::HandlerValueUnresolved { .. } => Rule::ExpectedFunctionGotExpression,
        StaticDefectKind::UncalledAccessor { .. } => Rule::UncalledAccessor,
        StaticDefectKind::DirectMutation { .. } => Rule::NoDirectMutation,
    };
    let text = solid_reactive_ir::static_defect_text(defect, &V1_STATIC_TERMS);
    FindingWording::new(rule.metadata(), text.message, text.hint).with_evidence(vec![
        EvidenceStep {
            message: text.evidence.into(),
            location: Some(defect.location.clone()),
        },
    ])
}

const V1_STATIC_TERMS: solid_reactive_ir::StaticDefectTerms =
    solid_reactive_ir::StaticDefectTerms {
        props_destructure_hint: "Keep the props object intact and read props.<name> inside JSX or a tracked computation; the property access is what tracks. To split or default props, use splitProps(props, ...keys) and mergeProps(defaults, props) instead of destructuring.",
        reactive_object_destructure_hint: "Keep the reactive object intact and read object.<name> inside JSX or a tracked computation. A property access made there remains subscribed; a setup-time destructuring binding does not.",
        missing_effect_message: "createEffect is called without an effect function; the signature is createEffect(fn, value?), where fn tracks dependencies and runs the side effect, and the optional value seeds the previous value passed to fn on its first run",
        missing_effect_hint: "Pass the effect function as the first argument. Reads inside it are tracked, and cleanup is registered with onCleanup rather than returned.",
        store_mutation_hint: v1_store_mutation_hint,
        // The removed-API migration map is a 2.0 concept: nothing has been
        // removed *from* the 1.x surface this catalog describes.
        removed_export_hint: v1_no_removed_export_hint,
    };

fn v1_no_removed_export_hint(_module: &str, _export: &str) -> Option<String> {
    None
}

fn v1_store_mutation_hint(name: &str) -> String {
    format!(
        "Write through the store's setter: setStore(\"key\", value), or produce(draft => ...) for an in-place update. Direct assignment to {name} does not notify subscribers."
    )
}

#[cfg(test)]
mod strict_read_hint_tests {
    use super::strict_read_hint;

    #[test]
    fn component_props_hint_does_not_present_untrack_as_a_reactivity_fix() {
        let hint = strict_read_hint("component-props");
        assert!(hint.contains("read props.<name> directly in JSX"));
        assert!(hint.contains("Do not use untrack() to make a prop reactive"));
    }
}
