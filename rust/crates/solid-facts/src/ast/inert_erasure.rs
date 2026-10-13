//! Complete syntax whitelists for ADRs 0026, 0028 and 0030. This is a preservation premise,
//! not an implementation census or permission to certify a domain.

use oxc_allocator::Allocator;
use oxc_ast::{
    ast::{
        CallExpression, Declaration, ExportNamedDeclaration, Expression, FormalParameter,
        ImportDeclarationSpecifier, ImportExpression, MetaProperty, Statement, TSType,
    },
    ast_kind::AstKind,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InertErasure {
    pub export_name: String,
    pub output: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportFreeErasure {
    pub output: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelativeImportErasure {
    pub output: String,
    pub imports: Vec<String>,
}

/// Computes a byte-position-preserving strip-only module for ADR 0028.
/// Runtime syntax is never generated: parser-owned type spans are replaced by
/// ASCII spaces, and the result must parse as JavaScript. The pinned Node
/// transformer independently has to produce these exact bytes at execution.
pub fn import_free_erasure(source: &str, export_name: &str) -> Option<ImportFreeErasure> {
    let erased = strip_only_erasure(source, Some(export_name), false)?;
    Some(ImportFreeErasure {
        output: erased.output,
    })
}

/// Computes one module of ADR 0030's package-local graph. The caller must bind
/// every returned import literal to the authenticated snapshot resolver before
/// these bytes can execute.
pub fn relative_import_erasure(
    source: &str,
    required_export: Option<&str>,
) -> Option<RelativeImportErasure> {
    strip_only_erasure(source, required_export, true)
}

fn strip_only_erasure(
    source: &str,
    required_export: Option<&str>,
    allow_relative_imports: bool,
) -> Option<RelativeImportErasure> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::ts()).parse();
    if parsed.panicked
        || !parsed.errors.is_empty()
        || !parsed.program.directives.is_empty()
        || parsed.program.hashbang.is_some()
        || required_export
            .is_some_and(|export_name| !direct_callable_export(&parsed.program.body, export_name))
    {
        return None;
    }
    let mut imports = Vec::new();
    let mut erasure = ErasureMask::new(source.len());
    erasure.allow_static_imports = allow_relative_imports;
    for statement in &parsed.program.body {
        if let Statement::ImportDeclaration(import) = statement {
            let specifier = import.source.value.as_str();
            if !allow_relative_imports
                || import.import_kind.is_type()
                || import.phase.is_some()
                || import.with_clause.is_some()
                || !(specifier.starts_with("./") || specifier.starts_with("../"))
                || specifier.contains(['?', '#', '\\'])
                || import.specifiers.iter().flatten().any(|specifier| {
                    matches!(specifier, ImportDeclarationSpecifier::ImportSpecifier(specifier) if specifier.import_kind.is_type())
                })
            {
                return None;
            }
            imports.push(specifier.to_owned());
        }
        if let Statement::ExportNamedDeclaration(export) = statement
            && matches!(
                export.declaration,
                Some(
                    Declaration::TSTypeAliasDeclaration(_) | Declaration::TSInterfaceDeclaration(_)
                )
            )
        {
            erasure.erase(export.span);
        }
    }
    erasure.visit_program(&parsed.program);
    if erasure.refused {
        return None;
    }
    let mut output = source.as_bytes().to_vec();
    for (index, byte) in output.iter_mut().enumerate() {
        if erasure.bytes[index] && *byte != b'\r' && *byte != b'\n' {
            if !byte.is_ascii() {
                return None;
            }
            *byte = b' ';
        }
    }
    let output = String::from_utf8(output).ok()?;
    let parsed_output = Parser::new(&allocator, &output, SourceType::mjs()).parse();
    if parsed_output.panicked || !parsed_output.errors.is_empty() {
        return None;
    }
    Some(RelativeImportErasure { output, imports })
}

fn direct_callable_export(body: &[Statement<'_>], name: &str) -> bool {
    body.iter().any(|statement| {
        let Statement::ExportNamedDeclaration(export) = statement else {
            return false;
        };
        match &export.declaration {
            Some(Declaration::FunctionDeclaration(function)) => function
                .id
                .as_ref()
                .is_some_and(|identifier| identifier.name == name && function.body.is_some()),
            Some(Declaration::VariableDeclaration(declaration)) => {
                declaration.declarations.iter().any(|declarator| {
                    declarator
                        .id
                        .get_binding_identifiers()
                        .into_iter()
                        .any(|identifier| identifier.name == name)
                        && matches!(
                            declarator.init.as_ref(),
                            Some(
                                Expression::ArrowFunctionExpression(_)
                                    | Expression::FunctionExpression(_)
                            )
                        )
                })
            }
            _ => false,
        }
    })
}

struct ErasureMask {
    bytes: Vec<bool>,
    refused: bool,
    allow_static_imports: bool,
}

impl ErasureMask {
    fn new(length: usize) -> Self {
        Self {
            bytes: vec![false; length],
            refused: false,
            allow_static_imports: false,
        }
    }

    fn erase(&mut self, span: Span) {
        let Some(bytes) = self.bytes.get_mut(span.start as usize..span.end as usize) else {
            self.refused = true;
            return;
        };
        bytes.fill(true);
    }
}

impl<'a> Visit<'a> for ErasureMask {
    fn enter_node(&mut self, kind: AstKind<'a>) {
        match kind {
            AstKind::TSTypeAnnotation(_)
            | AstKind::TSTypeParameterDeclaration(_)
            | AstKind::TSTypeParameterInstantiation(_)
            | AstKind::TSTypeAliasDeclaration(_)
            | AstKind::TSInterfaceDeclaration(_) => self.erase(kind.span()),
            AstKind::TSAsExpression(expression) => {
                self.erase(Span::new(
                    expression.expression.span().end,
                    expression.span.end,
                ));
            }
            AstKind::TSSatisfiesExpression(expression) => {
                self.erase(Span::new(
                    expression.expression.span().end,
                    expression.span.end,
                ));
            }
            AstKind::TSTypeAssertion(expression) => {
                self.erase(Span::new(
                    expression.span.start,
                    expression.expression.span().start,
                ));
            }
            AstKind::TSNonNullExpression(expression) => {
                self.erase(Span::new(
                    expression.expression.span().end,
                    expression.span.end,
                ));
            }
            AstKind::TSInstantiationExpression(expression) => {
                self.erase(Span::new(
                    expression.expression.span().end,
                    expression.span.end,
                ));
            }
            AstKind::TSEnumDeclaration(_)
            | AstKind::TSModuleDeclaration(_)
            | AstKind::TSGlobalDeclaration(_)
            | AstKind::TSImportEqualsDeclaration(_)
            | AstKind::TSExportAssignment(_)
            | AstKind::TSNamespaceExportDeclaration(_)
            | AstKind::TSThisParameter(_)
            | AstKind::Decorator(_)
            | AstKind::JSXElement(_)
            | AstKind::JSXFragment(_) => self.refused = true,
            _ => {}
        }
    }

    fn visit_export_named_declaration(&mut self, declaration: &ExportNamedDeclaration<'a>) {
        if declaration.source.is_some()
            || (declaration.declaration.is_none() && !declaration.specifiers.is_empty())
        {
            self.refused = true;
        }
        walk::walk_export_named_declaration(self, declaration);
    }

    fn visit_import_declaration(&mut self, declaration: &oxc_ast::ast::ImportDeclaration<'a>) {
        if !self.allow_static_imports {
            self.refused = true;
        }
        walk::walk_import_declaration(self, declaration);
    }

    fn visit_import_expression(&mut self, expression: &ImportExpression<'a>) {
        self.refused = true;
        walk::walk_import_expression(self, expression);
    }

    fn visit_meta_property(&mut self, property: &MetaProperty) {
        self.refused = true;
        walk::walk_meta_property(self, property);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if matches!(&call.callee, Expression::Identifier(identifier) if identifier.name == "require" || identifier.name == "eval")
        {
            self.refused = true;
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_formal_parameter(&mut self, parameter: &FormalParameter<'a>) {
        if parameter.accessibility.is_some() || parameter.readonly || parameter.r#override {
            self.refused = true;
        }
        if parameter.optional {
            let start = parameter.pattern.span().end as usize;
            let end = parameter
                .type_annotation
                .as_ref()
                .map_or(parameter.span.end as usize, |annotation| {
                    annotation.span.start as usize
                });
            // The parser records the optional marker semantically but has no
            // node for the token. It is the only `?` between the binding and
            // its type annotation in this admitted grammar.
            if start < end {
                self.bytes[start..end].fill(true);
            } else {
                self.refused = true;
            }
        }
        walk::walk_formal_parameter(self, parameter);
    }
}

/// Recognizes one complete, inert ESM module and computes the only permissible
/// derived bytes. The runtime transformer must independently match them.
pub fn inert_erasure(source: &str) -> Option<InertErasure> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::ts()).parse();
    if parsed.panicked
        || !parsed.errors.is_empty()
        || !parsed.program.directives.is_empty()
        || parsed.program.hashbang.is_some()
    {
        return None;
    }
    let [Statement::ExportNamedDeclaration(export)] = parsed.program.body.as_slice() else {
        return None;
    };
    let Some(Declaration::FunctionDeclaration(function)) = &export.declaration else {
        return None;
    };
    if function.r#async
        || function.generator
        || function.declare
        || function.type_parameters.is_some()
        || function.this_param.is_some()
        || !function.params.items.is_empty()
        || function.params.rest.is_some()
        || export.source.is_some()
        || !export.specifiers.is_empty()
    {
        return None;
    }
    let body = function.body.as_ref()?;
    if !body.directives.is_empty()
        || !matches!(
            body.statements.as_slice(),
            [] | [Statement::ReturnStatement(_)]
        )
        || matches!(body.statements.first(), Some(Statement::ReturnStatement(statement)) if statement.argument.is_some())
    {
        return None;
    }
    let mut output = source.as_bytes().to_vec();
    if let Some(annotation) = &function.return_type {
        if !matches!(annotation.type_annotation, TSType::TSVoidKeyword(_)) {
            return None;
        }
        // Non-ASCII annotation trivia is outside this initial byte-preservation
        // premise. Comments outside the erased span remain byte-identical.
        let erased =
            output.get_mut(annotation.span.start as usize..annotation.span.end as usize)?;
        if !erased.is_ascii() {
            return None;
        }
        for byte in erased {
            if *byte != b'\r' && *byte != b'\n' {
                *byte = b' ';
            }
        }
    }
    Some(InertErasure {
        export_name: function.id.as_ref()?.name.to_string(),
        output: String::from_utf8(output).ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_inert_erasure_is_a_complete_whitelist() {
        for source in [
            "export function noop() {}",
            "export function noop(): void { return; }",
        ] {
            let erased = inert_erasure(source).unwrap();
            assert_eq!(erased.export_name, "noop");
            assert_eq!(erased.output.len(), source.len());
            assert!(!erased.output.contains(": void"));
            assert_eq!(erased.output, source.replace(": void", "      "));
        }
        for source in [
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../fixtures/package-contracts/restricted-type-erasure/reflect.ts"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../fixtures/package-contracts/restricted-type-erasure/inert-import.ts"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../fixtures/package-contracts/restricted-type-erasure/inert-unsupported.ts"
            )),
        ] {
            assert!(inert_erasure(source).is_none());
        }
        for source in [
            "export function noop() { sideEffect(); }",
            "export function noop() { return undefined; }",
            "export function noop(x = sideEffect()) {}",
            "export function noop(...args: unknown[]) {}",
            "export async function noop() {}",
            "export function* noop() {}",
            "export function noop<T>() {}",
            "export function noop(this: unknown) {}",
            "export function noop() { 'use server'; }",
            "'use client'; export function noop() {}",
            "export function noop(): never {}",
            "export function noop();",
            "export function noop() {} globalThis.x = 1;",
            "import type { X } from './x'; export function noop() {}",
            "import './x'; export function noop() {}",
            "export function noop() { import('./x'); }",
            "export function noop() { require('./x'); }",
            "export function noop() { noop.toString(); }",
            "export enum X { A }",
            "export const noop = () => {};",
            "export function noop() { <div/>; }",
        ] {
            assert!(inert_erasure(source).is_none(), "{source}");
        }
    }

    #[test]
    fn import_free_erasure_preserves_runtime_tokens_and_direct_export() {
        let source = "export type Point = [number, number];\n\
export function point<T>(value?: T): T | undefined { return value as T; }\n\
export const arrow = (value: number) => value!;\n";
        let erased = import_free_erasure(source, "point").unwrap();
        assert_eq!(erased.output.len(), source.len());
        assert!(
            erased
                .output
                .contains("export function point   (value    )"),
            "{}",
            erased.output
        );
        assert!(erased.output.contains("return value     ;"));
        assert!(
            erased
                .output
                .contains("export const arrow = (value        ) => value ")
        );
        assert!(import_free_erasure(source, "missing").is_none());
    }

    #[test]
    fn import_free_erasure_refuses_resolution_and_transforming_syntax() {
        for source in [
            "import { x } from './x'; export function subject() { return x; }",
            "export { subject } from './x';",
            "export function subject() { return import('./x'); }",
            "export function subject() { return require('./x'); }",
            "export function subject() { return import.meta.url; }",
            "export enum Subject { One }",
            "namespace Subject { export const one = 1; } export function subject() {}",
            "export function subject() { return eval('1'); }",
            "export function subject(this: unknown) {}",
            "class Subject { constructor(public value: number) {} } export function subject() {}",
            "'use client'; export function subject() {}",
        ] {
            assert!(import_free_erasure(source, "subject").is_none(), "{source}");
        }
    }

    #[test]
    fn probe_relative_import_erasure_names_only_runtime_relative_edges() {
        let source = "import { helper } from './helper';\n\
                      export function subject(value: number) { return helper(value); }";
        let erased = relative_import_erasure(source, Some("subject")).unwrap();
        assert_eq!(erased.imports, vec!["./helper"]);
        assert_eq!(erased.output.len(), source.len());
        assert!(erased.output.contains("subject(value        )"));

        let helper = relative_import_erasure(
            "export function helper(value: number) { return value; }",
            None,
        )
        .unwrap();
        assert!(helper.imports.is_empty());
        for source in [
            "import type { X } from './x'; export function subject() {}",
            "import { type X } from './x'; export function subject() {}",
            "import { x } from 'pkg'; export function subject() { return x; }",
            "import { x } from './x?raw'; export function subject() { return x; }",
            "export { subject } from './x';",
            "export function subject() { return import('./x'); }",
        ] {
            assert!(
                relative_import_erasure(source, Some("subject")).is_none(),
                "{source}"
            );
        }
    }
}
