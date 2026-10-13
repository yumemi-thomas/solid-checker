//! Exact "this import binding is only re-exported" facts, without package or
//! receipt authority (ADR 0133).

use crate::core::Span;
use oxc_allocator::Allocator;
use oxc_ast::ast::{ImportDeclarationSpecifier, ImportOrExportKind, ModuleExportName, Statement};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use oxc_syntax::reference::ReferenceId;
use std::collections::BTreeSet;
use std::path::Path;

/// The public names a module-level value import binding is published under,
/// when publishing it is the *only* thing the module does with it.
///
/// `import { x } from "m"; export { x, x as y };` answers `["x", "y"]`. The
/// answer is `None` unless every one of these holds:
///
/// - `binding` is exactly the local identifier span of a value import
///   specifier (named, default, or namespace) of a module-level import;
/// - the binding has at least one resolved reference, and every resolved
///   reference is the local of a value specifier in a module-level
///   `export { … }` with no `from` clause;
/// - the source parses as a module with no parse or semantic error, and
///   nothing in it references `eval` (dynamic lexical access is outside this
///   proof).
///
/// An import used anywhere else -- in a function, in a module-level
/// expression, as a class heritage, in a nested export -- answers `None`.
#[must_use]
pub fn reexport_only_import_names(path: &Path, source: &str, binding: Span) -> Option<Vec<String>> {
    if source.len() > 4 * 1024 * 1024 {
        return None;
    }
    let source_type = SourceType::from_path(path).ok()?.with_module(true);
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    let built = SemanticBuilder::new().build(&parsed.program);
    if !built.errors.is_empty() {
        return None;
    }
    let mut dynamic = EvalReference(false);
    dynamic.visit_program(&parsed.program);
    if dynamic.0 {
        return None;
    }
    let scoping = built.semantic.scoping();

    let mut symbol = None;
    let mut publications = Vec::<(ReferenceId, String)>::new();
    for statement in &parsed.program.body {
        match statement {
            Statement::ImportDeclaration(import) => {
                if import.import_kind == ImportOrExportKind::Type {
                    continue;
                }
                for specifier in import.specifiers.iter().flatten() {
                    let local = match specifier {
                        ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                            if specifier.import_kind == ImportOrExportKind::Type {
                                continue;
                            }
                            &specifier.local
                        }
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                            &specifier.local
                        }
                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                            &specifier.local
                        }
                    };
                    if local.span.start == binding.start && local.span.end == binding.end {
                        symbol = local.symbol_id.get();
                    }
                }
            }
            Statement::ExportNamedDeclaration(export)
                if export.source.is_none()
                    && export.declaration.is_none()
                    && export.export_kind != ImportOrExportKind::Type =>
            {
                for specifier in &export.specifiers {
                    if specifier.export_kind == ImportOrExportKind::Type {
                        continue;
                    }
                    let ModuleExportName::IdentifierReference(local) = &specifier.local else {
                        continue;
                    };
                    if let Some(reference) = local.reference_id.get() {
                        publications.push((reference, specifier.exported.name().to_string()));
                    }
                }
            }
            _ => {}
        }
    }
    let symbol = symbol?;
    if !scoping.symbol_redeclarations(symbol).is_empty() {
        return None;
    }
    let references = scoping.get_resolved_reference_ids(symbol);
    if references.is_empty() {
        return None;
    }
    let mut names = BTreeSet::new();
    for reference in references {
        let (_, name) = publications
            .iter()
            .find(|(published, _)| published == reference)?;
        names.insert(name.clone());
    }
    Some(names.into_iter().collect())
}

struct EvalReference(bool);

impl<'a> Visit<'a> for EvalReference {
    fn visit_identifier_reference(&mut self, identifier: &oxc_ast::ast::IdentifierReference<'a>) {
        self.0 |= identifier.name == "eval";
        walk::walk_identifier_reference(self, identifier);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(source: &str, binding: &str) -> Option<Vec<String>> {
        let start = u32::try_from(source.find(binding).unwrap()).unwrap();
        let end = start + u32::try_from(binding.len()).unwrap();
        reexport_only_import_names(Path::new("index.js"), source, Span { start, end })
    }

    #[test]
    fn an_import_published_by_the_export_list_alone_names_its_public_names() {
        assert_eq!(
            names(
                "import { value } from \"dep\";\nexport { value, value as alias };\n",
                "value"
            ),
            Some(vec!["alias".to_string(), "value".to_string()])
        );
        assert_eq!(
            names(
                "import value from \"dep\";\nexport { value as default };\n",
                "value"
            ),
            Some(vec!["default".to_string()])
        );
        assert_eq!(
            names(
                "import * as space from \"dep\";\nexport { space };\n",
                "space"
            ),
            Some(vec!["space".to_string()])
        );
        // The shape of `@tanstack/solid-router`'s `dist/esm/index.js`.
        assert_eq!(
            names(
                "import { a, b } from \"dep\";\nimport { c } from \"./c.js\";\nexport { a, b, c };\n",
                "b"
            ),
            Some(vec!["b".to_string()])
        );
    }

    #[test]
    fn any_other_use_of_the_binding_refuses() {
        for (source, binding) in [
            // Used in a function.
            (
                "import { value } from \"dep\";\nexport function f() { return value(); }\nexport { value };\n",
                "value",
            ),
            // Used at module level.
            (
                "import { value } from \"dep\";\nconst held = value;\nexport { value, held };\n",
                "value",
            ),
            // Class heritage runs at module evaluation.
            (
                "import { Base } from \"dep\";\nclass Local extends Base {}\nexport { Base, Local };\n",
                "Base",
            ),
            // Not published at all.
            (
                "import { value } from \"dep\";\nexport const other = 1;\n",
                "value",
            ),
            // A re-export with a `from` clause is a different binding.
            (
                "import { value } from \"dep\";\nexport { value as again } from \"dep\";\n",
                "value",
            ),
            // Type-only import.
            (
                "import type { value } from \"dep\";\nexport { value };\n",
                "value",
            ),
            // Dynamic lexical access.
            (
                "import { value } from \"dep\";\nexport { value };\neval(\"value\");\n",
                "value",
            ),
        ] {
            assert_eq!(names(source, binding), None, "{source}");
        }
        // Not an import binding's span.
        assert_eq!(
            names("import { value } from \"dep\";\nexport { value };\n", "dep"),
            None
        );
    }
}
