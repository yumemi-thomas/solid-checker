//! Static module request specifiers without full fact extraction.
//!
//! Cache-input enumeration needs only the top-level value import and
//! re-export specifiers of each candidate source. Running full extraction
//! (including host-execution facts) for that alone dominated whole runs.

use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::ast::{ExportAllDeclaration, ExportNamedDeclaration, ImportDeclaration, Statement};
use oxc_parser::Parser;
use oxc_span::SourceType;

/// The specifiers `extract` reports as non-type-only imports and re-exports,
/// in source order. `None` when the source does not parse.
pub fn static_module_requests(path: &Path, source: &str) -> Option<Vec<String>> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).ok()?.with_module(true);
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    let mut requests = Vec::new();
    for statement in &parsed.program.body {
        match statement {
            Statement::ImportDeclaration(import) => {
                let ImportDeclaration {
                    source,
                    import_kind,
                    ..
                } = &**import;
                if !import_kind.is_type() {
                    requests.push(source.value.to_string());
                }
            }
            Statement::ExportNamedDeclaration(export) => {
                let ExportNamedDeclaration {
                    source,
                    export_kind,
                    ..
                } = &**export;
                if let Some(source) = source
                    && !export_kind.is_type()
                {
                    requests.push(source.value.to_string());
                }
            }
            Statement::ExportAllDeclaration(export) => {
                let ExportAllDeclaration {
                    source,
                    export_kind,
                    ..
                } = &**export;
                if !export_kind.is_type() {
                    requests.push(source.value.to_string());
                }
            }
            _ => {}
        }
    }
    Some(requests)
}
