//! Explicit package-source boundary obligations.

use solid_dialect::Dialect;
use solid_facts::ProjectFacts;
use typefacts::Location;

use super::{EntitySymbols, StaticDefect, StaticDefectKind, location};

/// A boundary obligation must not be raised for a primitive already modeled
/// by the selected dialect. Core runtime references themselves are excluded
/// at the analysis boundary.
fn native_vocabulary_outranks_package_boundary(
    dialect: &dyn Dialect,
    module: &str,
    imported: &str,
) -> bool {
    dialect.owns_module(module) && dialect.declares_primitive(imported)
}

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
            if import.type_only || solid_dialect::core_runtime_reference("", &import.module) {
                continue;
            }
            if !inputs
                .boundary
                .requires_legacy(file.path.as_str(), &import.module)
            {
                continue;
            }
            push_unavailable_import(&mut defects, file, import, entities, dialect);
        }
    }
    defects
}

fn push_unavailable_import(
    missing: &mut Vec<StaticDefect>,
    file: &solid_facts::FileFacts,
    import: &solid_facts::ast::ImportFact,
    entities: &EntitySymbols,
    dialect: &dyn Dialect,
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
                    && entities.value_symbol(file, member.object) == Some(namespace_symbol)
            }) {
                let export = file.source_text(member.property).unwrap_or_default();
                if !native_vocabulary_outranks_package_boundary(dialect, &import.module, export) {
                    push_unavailable_export(
                        missing,
                        &import.module,
                        export,
                        location(file.path.shared(), member.property),
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
        if !native_vocabulary_outranks_package_boundary(dialect, &import.module, export) {
            push_unavailable_export(
                missing,
                &import.module,
                export,
                location(file.path.shared(), import.span),
            );
        }
    }
}

/// The analysis context of a boundary obligation: no analyzed source covers
/// the import, so nothing export-specific has been read.
pub(crate) const UNANALYZED_IMPORT_CONTEXT: &str = "no analyzed source covers this exact import";

fn push_unavailable_export(
    missing: &mut Vec<StaticDefect>,
    module: &str,
    export: &str,
    location: Location,
) {
    missing.push(StaticDefect {
        kind: StaticDefectKind::PackageBehaviorUnavailable {
            module: module.into(),
            export: export.into(),
            reexported: false,
        },
        location,
        analysis_context: UNANALYZED_IMPORT_CONTEXT.into(),
        fixes: vec![],
        uncertain: false,
    });
}
