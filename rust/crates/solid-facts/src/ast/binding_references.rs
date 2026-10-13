//! Exact lexical references of one module-level import binding, without
//! package or receipt authority (ADR 0135).

use crate::core::Span;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    IdentifierReference, ImportDeclarationSpecifier, ImportOrExportKind, Statement,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};
use std::path::Path;

/// The span of every resolved reference to the value import binding whose
/// local identifier is exactly `binding`, or `None`.
///
/// `None` unless the module parses with no error, has no semantic error,
/// references no `eval`, and `binding` is exactly the local of a value import
/// specifier (named, default or namespace) of a module-level import. The
/// references are Oxc's resolved lexical references, so a shadowing local of
/// the same name is never one of them.
#[must_use]
pub fn import_binding_references(path: &Path, source: &str, binding: Span) -> Option<Vec<Span>> {
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
    let mut symbol = None;
    for statement in &parsed.program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
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
                ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => &specifier.local,
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => &specifier.local,
            };
            if local.span.start == binding.start && local.span.end == binding.end {
                symbol = local.symbol_id.get();
            }
        }
    }
    let symbol = symbol?;
    let scoping = built.semantic.scoping();
    if !scoping.symbol_redeclarations(symbol).is_empty() {
        return None;
    }
    let nodes = built.semantic.nodes();
    Some(
        scoping
            .get_resolved_references(symbol)
            .map(|reference| {
                let span = nodes.get_node(reference.node_id()).kind().span();
                Span {
                    start: span.start,
                    end: span.end,
                }
            })
            .collect(),
    )
}

struct EvalReference(bool);

impl<'a> Visit<'a> for EvalReference {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.0 |= identifier.name == "eval";
        walk::walk_identifier_reference(self, identifier);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn references(source: &str) -> Option<Vec<(u32, u32)>> {
        let start = u32::try_from(source.find("value").unwrap()).unwrap();
        import_binding_references(
            Path::new("m.js"),
            source,
            Span {
                start,
                end: start + 5,
            },
        )
        .map(|spans| {
            spans
                .into_iter()
                .map(|span| (span.start, span.end))
                .collect()
        })
    }

    #[test]
    fn references_are_exact_and_lexical() {
        let source = "import { value } from \"dep\";\nfunction a() { return value(); }\nfunction b(value) { return value; }\n";
        let expected = u32::try_from(source.find("value()").unwrap()).unwrap();
        assert_eq!(references(source), Some(vec![(expected, expected + 5)]));
        assert_eq!(references("import { value } from \"dep\";\n"), Some(vec![]));
        assert_eq!(
            references("import { value } from \"dep\";\neval(\"value\");\n"),
            None
        );
        assert_eq!(references("const value = 1;\n"), None);
    }
}
