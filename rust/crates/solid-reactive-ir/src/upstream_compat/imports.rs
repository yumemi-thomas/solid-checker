//! `v1/imports` — a Solid name imported from a module that does not export it.
//!
//! Upstream's rule carries a hand-written map of name → canonical module. This
//! one asks the dialect's generated export index instead, which is extracted
//! from the installed package, so it covers every export rather than the
//! subset someone remembered, and it cannot recommend a module the installed
//! version does not have.
//!
//! Two properties the index gives that a map cannot:
//!
//! - **Multi-valued.** 1.x's `solid-js/web` re-exports the control-flow
//!   components, so `Show` legitimately resolves from two modules and neither
//!   is wrong.
//! - **Silence on unknown names.** A name the package does not export anywhere
//!   is not a wrong-module import — it is someone else's export, or a typo for
//!   another rule to report. The index answers empty and this rule says
//!   nothing.

use solid_dialect::ExportPosition;
use solid_facts::FileFacts;
use solid_facts::ast::ImportKind;

use super::UpstreamCompatContext;
use crate::{Fix, StaticViolation, TextEdit, location};

pub(super) fn check_file(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    let dialect = context.lookup.dialect;
    for import in &file.ast.imports {
        // Only the packages this dialect owns: a name that also exists in an
        // unrelated package is that package's business.
        if !dialect.owns_module(&import.module) {
            continue;
        }
        for binding in &import.bindings {
            // Namespace and default imports name no export, so there is no
            // export to locate.
            if binding.kind != ImportKind::Named {
                continue;
            }
            let Some(imported) = binding.imported.as_deref() else {
                continue;
            };
            let position = if import.type_only || binding.type_only {
                ExportPosition::Type
            } else {
                ExportPosition::Value
            };
            let modules = dialect.export_modules(imported, position);
            if modules.is_empty() || modules.contains(&import.module.as_str()) {
                continue;
            }
            let canonical = modules[0];
            let alternatives = if modules.len() > 1 {
                format!(
                    " (also exported from {})",
                    modules[1..]
                        .iter()
                        .map(|module| format!("{module:?}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            } else {
                String::new()
            };
            violations.push(StaticViolation {
                id: "SC8002".into(),
                rule: "imports".into(),
                message: format!(
                    "{imported:?} is imported from {:?}, which does not export it; {} exports it from {canonical:?}{alternatives}",
                    import.module,
                    package_of(&import.module),
                ),
                hint: format!(
                    "Import {imported:?} from {canonical:?}. The export lists are extracted from the installed package, so this is where that version actually exports the name."
                ),
                location: location(file.path.shared(), binding.local.span),
                analysis_context: String::new(),
                fixes: module_rewrite_fix(file, import, canonical)
                    .into_iter()
                    .collect(),
            });
        }
    }
}

/// The package a subpath belongs to, for the message: `solid-js/store` is
/// still `solid-js`.
fn package_of(module: &str) -> &str {
    if module.starts_with('@') {
        let mut boundaries = module.match_indices('/').map(|(index, _)| index);
        boundaries.next();
        if let Some(second) = boundaries.next() {
            return &module[..second];
        }
        return module;
    }
    module.split('/').next().unwrap_or(module)
}

/// Rewrites the module string of a single-specifier import.
///
/// Deliberately narrow: moving one specifier out of a multi-specifier import
/// means editing two declarations and deciding where the second one goes, and
/// a fix that guesses wrong about import ordering is worse than no fix. A
/// declaration importing only the misplaced name has one correct rewrite.
fn module_rewrite_fix(
    file: &FileFacts,
    import: &solid_facts::ast::ImportFact,
    canonical: &str,
) -> Option<Fix> {
    if import.bindings.len() != 1 {
        return None;
    }
    let text = file.source_text(import.span)?;
    let quote = if text.contains(&format!("\"{}\"", import.module)) {
        '"'
    } else if text.contains(&format!("'{}'", import.module)) {
        '\''
    } else {
        return None;
    };
    let quoted = format!("{quote}{}{quote}", import.module);
    let offset = text.find(&quoted)?;
    let start = import.span.start + u32::try_from(offset).ok()?;
    let end = start + u32::try_from(quoted.len()).ok()?;
    Some(Fix {
        message: format!("import from {canonical:?}"),
        applicability: "safe".into(),
        edits: vec![TextEdit {
            location: location(file.path.shared(), solid_facts::core::Span::new(start, end)),
            new_text: format!("{quote}{canonical}{quote}"),
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::package_of;

    #[test]
    fn a_subpath_still_belongs_to_its_package() {
        assert_eq!(package_of("solid-js"), "solid-js");
        assert_eq!(package_of("solid-js/store"), "solid-js");
        assert_eq!(package_of("solid-js/web"), "solid-js");
        assert_eq!(package_of("@solidjs/web"), "@solidjs/web");
        assert_eq!(package_of("@solidjs/router/data"), "@solidjs/router");
    }
}
