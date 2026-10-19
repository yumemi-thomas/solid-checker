//! Explicit package-source boundary obligations. No package contracts are read.

use solid_dialect::Dialect;
use solid_facts::ProjectFacts;
use typefacts::Location;

use super::{EntitySymbols, StaticDefect, StaticDefectKind, location};
use crate::package_boundary::UncertifiableImportReason;

/// A missing external-contract obligation must not invent a receipt
/// requirement for a primitive already modeled by the selected dialect.
/// Core contracts themselves are excluded at the analysis boundary.
fn native_vocabulary_outranks_contract(
    dialect: &dyn Dialect,
    module: &str,
    imported: &str,
) -> bool {
    dialect.owns_module(module) && dialect.declares_primitive(imported)
}

/// The analysis context of [`push_unmet_context_premise`], followed by the
/// premise exports the project provides.
pub(crate) const CONTEXT_PREMISE_UNMET_CONTEXT: &str = "context-premise-unmet:";

/// The package boundary's defects: one per demanded export of every import
/// the boundary's requirement markers name, in file and import order. Solid
/// core is never a marker: the dialect is its only authority.
pub(super) fn discover_boundary_defects(
    facts: &ProjectFacts,
    inputs: crate::AnalysisInputs<'_>,
    entities: &EntitySymbols,
    dialect: &dyn Dialect,
) -> Vec<StaticDefect> {
    let mut defects = Vec::new();
    for file in &facts.files {
        for import in &file.ast.imports {
            if import.type_only
                || solid_dialect::core_runtime_contract_reference("", &import.module)
            {
                continue;
            }
            let Some(reason) = inputs
                .boundary
                .legacy_reason(file.path.as_str(), &import.module)
            else {
                continue;
            };
            push_missing_accepted_import(
                &mut defects,
                file,
                import,
                entities,
                dialect,
                reason,
                None,
            );
        }
    }
    defects
}

fn push_missing_accepted_import(
    missing: &mut Vec<StaticDefect>,
    file: &solid_facts::FileFacts,
    import: &solid_facts::ast::ImportFact,
    entities: &EntitySymbols,
    dialect: &dyn Dialect,
    reason: UncertifiableImportReason,
    refusal: Option<&str>,
) {
    for binding in &import.bindings {
        if binding.type_only || !binding.runtime_referenced {
            continue;
        }
        if binding.kind == solid_facts::ast::ImportKind::Namespace {
            let namespace_location = location(file.path.shared(), binding.local.span);
            let Some(namespace_symbol) = entities.get(&namespace_location) else {
                continue;
            };
            for member in file.ast.members.iter().filter(|member| {
                file.ast
                    .computed_members
                    .binary_search(&member.span)
                    .is_err()
                    && entities.get(&location(file.path.shared(), member.object))
                        == Some(namespace_symbol)
            }) {
                let export = file.source_text(member.property).unwrap_or_default();
                if !native_vocabulary_outranks_contract(dialect, &import.module, export) {
                    push_missing_accepted_export(
                        missing,
                        &import.module,
                        export,
                        location(file.path.shared(), member.property),
                        reason,
                        refusal,
                    );
                }
            }
            continue;
        }
        let Some(export) = binding.imported.as_deref().or_else(|| {
            (binding.kind == solid_facts::ast::ImportKind::Default).then_some("default")
        }) else {
            continue;
        };
        if !native_vocabulary_outranks_contract(dialect, &import.module, export) {
            push_missing_accepted_export(
                missing,
                &import.module,
                export,
                location(file.path.shared(), import.span),
                reason,
                refusal,
            );
        }
    }
}

/// The analysis context of the **acceptance** gate: this project accepted no
/// contract for the import at all, so nothing export-specific has been read
/// yet.
///
/// Named rather than spelled twice because three places have to agree on it —
/// the producer below, the evidence wording, and the per-package collapse in
/// `projection` — and two of them are deciding whether a finding is about the
/// package or about one of its exports. A drift between them would silently
/// re-scatter the collapse or mislabel the evidence.
pub(crate) const UNACCEPTED_IMPORT_CONTEXT: &str =
    "no receipt-accepted contract matches this exact import";

fn push_missing_accepted_export(
    missing: &mut Vec<StaticDefect>,
    module: &str,
    export: &str,
    location: Location,
    reason: UncertifiableImportReason,
    refusal: Option<&str>,
) {
    missing.push(StaticDefect {
        kind: StaticDefectKind::PackageContractExportMissing {
            module: module.into(),
            export: export.into(),
            reexported: false,
            site: crate::ContractDefectSite::Import,
            admission_refusal: refusal.map(str::to_owned),
        },
        location,
        analysis_context: match reason {
            UncertifiableImportReason::Unspecified => UNACCEPTED_IMPORT_CONTEXT,
            UncertifiableImportReason::ObsoletePolicy1 => {
                "obsolete-policy1-receipt: policy 1 cannot authorize analyzer semantics"
            }
        }
        .into(),
        fixes: vec![],
        uncertain: false,
    });
}
