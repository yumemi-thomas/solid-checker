//! The one AST join between a recorded source-edge refusal and the facts it
//! is checked against. Validation and emission both read it, so they cannot
//! disagree about what an occurrence is.
//!
//! A record is joined to an occurrence by **exact membership** and by nothing
//! else: the importer's path, the specifier text, the form, and the byte range
//! of the whole statement or expression. No nearby span, no name and no
//! guessed target ever stands in for a missing match. Membership is not
//! provenance: a record carries no generation, so a record made against one
//! generation joins an equal range in another. Callers reacquire their
//! records for each coherent facts generation; this module checks only that
//! each recorded occurrence is present in the facts it is given.
//!
//! What counts as an occurrence is the runtime-load set of the facts:
//! an import whose binding is referenced as a value, an `export … from` with
//! a runtime specifier, an `import … = require`, a literal `import()` and a
//! literal, unshadowed `require()`. Type-only forms, side-effect imports and
//! imports nothing references are excluded from this API's supported
//! occurrence set; that says nothing about whether a module is loaded (a
//! side-effect import does load one). A runtime namespace import is not
//! supported, so it is never an occurrence and never shares a key whose
//! contract is denied. Neither does an import with an unreferenced non-type
//! binding that is not itself recorded: the contract projection answers every
//! non-type binding, referenced or not, so a key-wide denial would silently
//! withhold an independent obligation there. That collateral is rejected, not
//! preserved.

use solid_facts::{FileFacts, ProjectFacts, ast::AstFacts, core::Span};

use crate::package_boundary::{
    PackageBoundary, SourceEdgeForm, SourceEdgeMismatch, SourceEdgeSite,
};

/// One supported runtime occurrence of one specifier in one file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EdgeOccurrence {
    pub form: SourceEdgeForm,
    /// The whole statement or expression.
    pub span: Span,
    /// The names the occurrence demands, sorted and distinct; data for the
    /// defect, never printed: the imported names (`default` for a default
    /// binding), the exported names, `*`, or `* as ns`.
    pub labels: Vec<String>,
}

impl EdgeOccurrence {
    fn is_site(&self, site: &SourceEdgeSite) -> bool {
        self.form == site.form
            && u64::from(self.span.start) == site.start_byte
            && u64::from(self.span.end) == site.end_byte
    }
}

/// Everything one file's AST says about one specifier.
#[derive(Debug, Default)]
pub(crate) struct FileEdges {
    /// Supported occurrences, in source order.
    pub occurrences: Vec<EdgeOccurrence>,
    /// Runtime namespace imports, in source order: statements with at least
    /// one runtime-referenced namespace binding.
    pub unsupported: Vec<Span>,
    /// Statements of the key with no runtime binding but at least one
    /// non-type, non-side-effect binding (named, default or namespace) that
    /// nothing references as a value. Not occurrences, and not inert: the
    /// accepted-contract projection reads these bindings too.
    pub unreferenced: Vec<Span>,
}

/// The occurrences of `specifier` in `ast`, joined exactly as the contract
/// binder reads imports and exports.
pub(crate) fn runtime_edges(ast: &AstFacts, specifier: &str) -> FileEdges {
    use solid_facts::ast::{ExportKind, ImportKind, ModuleLoadKind};

    let mut edges = FileEdges::default();
    for import in &ast.imports {
        if import.type_only || import.module != specifier {
            continue;
        }
        let runtime = import
            .bindings
            .iter()
            .filter(|binding| {
                !binding.type_only
                    && binding.runtime_referenced
                    && binding.kind != ImportKind::SideEffect
            })
            .collect::<Vec<_>>();
        if runtime.is_empty() {
            // No supported occurrence, but not inert if a non-type value
            // binding is there: the contract projection reads it.
            if import
                .bindings
                .iter()
                .any(|binding| !binding.type_only && binding.kind != ImportKind::SideEffect)
            {
                edges.unreferenced.push(import.span);
            }
            continue;
        }
        if runtime
            .iter()
            .any(|binding| binding.kind == ImportKind::Namespace)
        {
            edges.unsupported.push(import.span);
            continue;
        }
        edges.occurrences.push(EdgeOccurrence {
            form: SourceEdgeForm::Import,
            span: import.span,
            labels: labels(
                runtime
                    .iter()
                    .map(|binding| binding.imported.as_deref().unwrap_or("default").to_owned()),
            ),
        });
    }
    for export in ast.module_level_exports() {
        if export.type_only || export.module.as_deref() != Some(specifier) {
            continue;
        }
        let exported = match export.kind {
            ExportKind::Named => labels(
                export
                    .specifiers
                    .iter()
                    .filter(|specifier| !specifier.type_only)
                    .map(|specifier| specifier.exported.to_string()),
            ),
            ExportKind::All => vec![match export.namespace.as_deref() {
                Some(namespace) => format!("* as {namespace}"),
                None => "*".to_owned(),
            }],
            // A default export has no module; nothing re-exported from one.
            ExportKind::Default => continue,
        };
        if exported.is_empty() {
            continue;
        }
        edges.occurrences.push(EdgeOccurrence {
            form: SourceEdgeForm::ExportFrom,
            span: export.span,
            labels: exported,
        });
    }
    for declaration in &ast.import_equals {
        if !declaration.type_only && declaration.module == specifier {
            edges.occurrences.push(EdgeOccurrence {
                form: SourceEdgeForm::ImportEquals,
                span: declaration.span,
                labels: vec!["*".to_owned()],
            });
        }
    }
    for load in &ast.module_loads {
        // A literal argument only: the fact records no specifier otherwise.
        if load.specifier.as_deref() != Some(specifier) || load.specifier_span.is_none() {
            continue;
        }
        edges.occurrences.push(EdgeOccurrence {
            form: match load.kind {
                ModuleLoadKind::DynamicImport => SourceEdgeForm::DynamicImport,
                ModuleLoadKind::Require => SourceEdgeForm::Require,
            },
            span: load.span,
            labels: vec!["*".to_owned()],
        });
    }
    // Different fact tables, one source order: by range, then form.
    edges
        .occurrences
        .sort_by_key(|occurrence| (occurrence.span.start, occurrence.span.end, occurrence.form));
    edges.unsupported.sort();
    edges.unreferenced.sort();
    edges
}

fn labels(names: impl Iterator<Item = String>) -> Vec<String> {
    let mut names = names.collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names
}

fn file_of<'a>(facts: &'a ProjectFacts, importer: &str) -> Option<&'a FileFacts> {
    facts
        .files
        .iter()
        .find(|file| file.path.as_str() == importer)
}

/// Whether every recorded refusal joins exactly to the facts, and the denial
/// it implies is complete. Returns the first mismatch in key order, site
/// order and then source order, so the answer is a function of the inputs.
///
/// A key holding a contract-bound site (an `import` or an `export … from`)
/// will have its accepted contract denied key-wide. That is only sound when
/// every such occurrence of the key is recorded, and when none is a
/// runtime namespace import the neutral finding cannot cover, nor an import
/// whose only bindings are unreferenced (the contract projection answers
/// those too): a partial record would silence an occurrence while giving it
/// neither semantics nor a refusal.
pub(crate) fn check(
    boundary: &PackageBoundary,
    facts: &ProjectFacts,
) -> Result<(), SourceEdgeMismatch> {
    for (key, sites) in boundary.source_edge_refusals() {
        let Some(file) = file_of(facts, &key.importer) else {
            return Err(SourceEdgeMismatch::UnknownImporter {
                importer: key.importer.clone(),
            });
        };
        // A finding inside a selected dependency root is dropped after the
        // solve, so a refusal recorded there would disappear with its
        // contract already denied.
        if facts.dependency_source_root(&key.importer).is_some() {
            return Err(SourceEdgeMismatch::ImporterInDependencySource {
                importer: key.importer.clone(),
            });
        }
        let edges = runtime_edges(&file.ast, &key.specifier);
        for site in sites {
            if edges.occurrences.iter().any(|edge| edge.is_site(site)) {
                continue;
            }
            let namespace = (site.form == SourceEdgeForm::Import)
                .then(|| {
                    edges.unsupported.iter().find(|span| {
                        u64::from(span.start) == site.start_byte
                            && u64::from(span.end) == site.end_byte
                    })
                })
                .flatten();
            return Err(if namespace.is_some() {
                unsupported(key, site.start_byte, site.end_byte)
            } else {
                SourceEdgeMismatch::NoSuchOccurrence {
                    importer: key.importer.clone(),
                    specifier: key.specifier.clone(),
                    form: site.form,
                    start_byte: site.start_byte,
                    end_byte: site.end_byte,
                }
            });
        }
        if !sites.iter().any(|site| site.form.is_contract_bound()) {
            continue;
        }
        if let Some(span) = edges.unsupported.first() {
            return Err(unsupported(key, u64::from(span.start), u64::from(span.end)));
        }
        for edge in &edges.occurrences {
            if edge.form.is_contract_bound() && !sites.iter().any(|site| edge.is_site(site)) {
                return Err(SourceEdgeMismatch::UnrecordedOccurrence {
                    importer: key.importer.clone(),
                    specifier: key.specifier.clone(),
                    form: edge.form,
                    start_byte: u64::from(edge.span.start),
                    end_byte: u64::from(edge.span.end),
                });
            }
        }
        // The accepted-contract projection answers every non-type binding of
        // the key, referenced or not. A statement no record can name, because
        // it is no supported occurrence, would lose that answer to the
        // key-wide denial without any record having described it.
        if let Some(span) = edges.unreferenced.first() {
            return Err(SourceEdgeMismatch::UnreferencedCollateral {
                importer: key.importer.clone(),
                specifier: key.specifier.clone(),
                start_byte: u64::from(span.start),
                end_byte: u64::from(span.end),
            });
        }
    }
    Ok(())
}

fn unsupported(
    key: &crate::package_boundary::ImportKey,
    start_byte: u64,
    end_byte: u64,
) -> SourceEdgeMismatch {
    SourceEdgeMismatch::UnsupportedOccurrence {
        importer: key.importer.clone(),
        specifier: key.specifier.clone(),
        start_byte,
        end_byte,
    }
}

/// The recorded occurrences of one file, each with its specifier, in source
/// order. Reads the boundary's keys for this importer only, and does nothing
/// at all for a boundary with no record.
pub(crate) fn recorded_occurrences(
    file: &FileFacts,
    boundary: &PackageBoundary,
) -> Vec<(String, EdgeOccurrence)> {
    if !boundary.has_source_edge_refusals() {
        return Vec::new();
    }
    let mut recorded = Vec::new();
    for (key, sites) in boundary.source_edges_of(file.path.as_str()) {
        let edges = runtime_edges(&file.ast, &key.specifier);
        recorded.extend(
            edges
                .occurrences
                .into_iter()
                .filter(|edge| sites.iter().any(|site| edge.is_site(site)))
                .map(|edge| (key.specifier.clone(), edge)),
        );
    }
    recorded.sort_by_key(|(_, edge)| (edge.span.start, edge.span.end, edge.form));
    recorded
}

#[cfg(test)]
pub(crate) mod tests;

#[cfg(test)]
mod denial_tests;
