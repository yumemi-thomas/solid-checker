//! Positive, source-bound recognition of a restricted inert JavaScript module.
//!
//! This answers a syntax/fact question, not package applicability or acceptance.
//! A consumer still owes authenticated source bytes and an exact ESM loading
//! premise. In particular, this is not the declaration-erasure judgment and an
//! empty export census cannot be substituted for this complete parse.

use oxc_allocator::Allocator;
use oxc_ast::ast::Statement;
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

const MAX_SOURCE_BYTES: usize = 1024 * 1024;

/// Constructible only by successful complete recognition of exact bytes.
/// Neither serialized nor accepted as a caller-supplied certificate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InertJavaScriptModule {
    source_sha256: String,
    source_bytes: usize,
    statement_count: usize,
}

impl InertJavaScriptModule {
    #[must_use]
    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    #[must_use]
    pub const fn statement_count(&self) -> usize {
        self.statement_count
    }

    /// A positive parse of one source never authorizes another source.
    #[must_use]
    pub fn matches_source(&self, source: &str) -> bool {
        source.len() == self.source_bytes
            && format!("{:x}", Sha256::digest(source.as_bytes())) == self.source_sha256
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum InertJavaScriptRefusal {
    #[error("inert JavaScript recognition exceeds its source bound")]
    SourceBound,
    #[error("runtime bytes do not parse as a JavaScript module")]
    Parse,
    #[error("module directive is outside the inert JavaScript proof at bytes {start}..{end}")]
    Directive { start: u32, end: u32 },
    #[error("statement is outside the inert JavaScript proof at bytes {start}..{end}")]
    Statement { start: u32, end: u32 },
}

/// Recognizes only comments/whitespace, empty statements and local `export {}`.
/// Every statement must be witnessed by that closed grammar. Imports (including
/// an empty reexport with a source), declarations, directives and parse failures
/// remain explicit refusals. No TypeScript or JSX parse fallback is attempted.
pub fn inert_javascript_module(
    source: &str,
) -> Result<InertJavaScriptModule, InertJavaScriptRefusal> {
    inert_javascript_module_with_policy(source, false)
}

/// Recognizes the same inert source grammar while optionally admitting bare
/// `export * from "…"` statements. The latter is only a source-shape proof:
/// evaluating a reexport still evaluates its target, so callers must establish
/// the target's exact inert contract before treating the returned proof as an
/// initialization claim. Namespace reexports and import attributes stay
/// refused here and relative targets remain the caller's responsibility.
pub fn inert_javascript_module_with_export_all(
    source: &str,
) -> Result<InertJavaScriptModule, InertJavaScriptRefusal> {
    inert_javascript_module_with_policy(source, true)
}

fn inert_javascript_module_with_policy(
    source: &str,
    allow_export_all: bool,
) -> Result<InertJavaScriptModule, InertJavaScriptRefusal> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(InertJavaScriptRefusal::SourceBound);
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::mjs()).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Err(InertJavaScriptRefusal::Parse);
    }
    if let Some(directive) = parsed.program.directives.first() {
        let span = directive.span();
        return Err(InertJavaScriptRefusal::Directive {
            start: span.start,
            end: span.end,
        });
    }
    for statement in &parsed.program.body {
        let admitted = match statement {
            Statement::EmptyStatement(_) => true,
            Statement::ExportNamedDeclaration(export) => {
                export.source.is_none()
                    && export.declaration.is_none()
                    && export.specifiers.is_empty()
                    && export.with_clause.is_none()
                    && !export.export_kind.is_type()
            }
            Statement::ExportAllDeclaration(export) if allow_export_all => {
                export.exported.is_none() && export.with_clause.is_none()
            }
            _ => false,
        };
        if !admitted {
            let span = statement.span();
            return Err(InertJavaScriptRefusal::Statement {
                start: span.start,
                end: span.end,
            });
        }
    }
    Ok(InertJavaScriptModule {
        source_sha256: format!("{:x}", Sha256::digest(source.as_bytes())),
        source_bytes: source.len(),
        statement_count: parsed.program.body.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inert_javascript_binds_complete_source_not_export_absence() {
        for source in ["", "// marker\n", "/* 日本語 */\n", ";;export {};\n"] {
            let proof = inert_javascript_module(source).unwrap();
            assert!(proof.matches_source(source));
            assert!(!proof.matches_source(&format!("{source}\n")));
            assert!(!proof.matches_source("globalThis.changed = true;"));
        }
        let empty = inert_javascript_module("").unwrap();
        assert_eq!(empty.statement_count(), 0);
        assert_eq!(
            empty.source_sha256(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            inert_javascript_module(";;export {};")
                .unwrap()
                .statement_count(),
            3
        );
    }

    #[test]
    fn inert_javascript_refuses_effects_loads_and_non_javascript() {
        for source in [
            "import './effect.js';",
            "export {} from './effect.js';",
            "globalThis.changed = true;",
            "register();",
            "const value = initialize();",
            "await initialize();",
            "function hidden() {}",
            "export const value = 1;",
            "export default 1;",
            "'use strict';",
            "interface Hidden {}",
            "export declare function hidden(): void;",
            "export type {};",
            "<div />;",
            "export {",
        ] {
            assert!(inert_javascript_module(source).is_err(), "{source}");
        }
        assert_eq!(
            inert_javascript_module(&" ".repeat(MAX_SOURCE_BYTES + 1)),
            Err(InertJavaScriptRefusal::SourceBound)
        );
    }

    #[test]
    fn reexport_shape_requires_a_separate_dependency_inertness_proof() {
        let source = "export * from '@scope/empty';\nexport {};";
        assert!(inert_javascript_module(source).is_err());
        assert!(inert_javascript_module_with_export_all(source).is_ok());
        assert!(
            inert_javascript_module_with_export_all("export * as ns from '@scope/empty';").is_err()
        );
        assert!(inert_javascript_module_with_export_all("export * from './effect.js';").is_ok());
    }
}
