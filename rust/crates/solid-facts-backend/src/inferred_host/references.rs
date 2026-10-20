//! Local references: which edges, bodies and imports a scope exactly reaches.

use super::*;

/// ADR 0220 is optional. A requested but missing/disagreeing row is unknown,
/// never permission to reuse the default congruence guess.
pub(super) fn local_edge(
    facts: &ProjectFacts,
    config: &Config,
    paths: &BTreeMap<String, Vec<String>>,
    importer: &str,
    span: Span,
    text: &str,
    target: &str,
) -> bool {
    if let Some(runtime) = &facts.runtime_resolutions {
        return matches!(runtime.outcome(importer, span, text),
            solid_facts::runtime_resolution::RuntimeOutcome::File { path, physical_path }
            if path.as_ref() == target && physical_path.as_ref() == target
                && fs::canonicalize(target).ok().as_deref() == Some(Path::new(target)));
    }
    !resolver_selection_required(config, text)
        && congruent_alias(config, paths, text)
        && exact_local_target(config, importer, text, target)
}

pub(super) fn body_symbol(
    facts: &ProjectFacts,
    file: &FileFacts,
    function: &FunctionFact,
    written_symbols: &BTreeSet<String>,
) -> Option<String> {
    if function.method_name.is_some()
        || function.generator
        || file
            .ast
            .classes
            .iter()
            .any(|class| class.span.contains(function.span))
        || file
            .ast
            .module_blocks
            .iter()
            .any(|block| block.contains(function.span))
        || file
            .ast
            .module_hazards
            .iter()
            .any(|hazard| hazard.kind != ModuleHazardKind::NonliteralDynamicLoading)
    {
        return None;
    }
    let name = function.name.as_ref().or_else(|| {
        file.ast
            .bindings
            .iter()
            .find(|binding| {
                binding.immutable
                    && binding.initializer_function
                    && binding.names.len() == 1
                    && binding
                        .initializer
                        .is_some_and(|span| unwrapped(file, span) == function.span)
            })
            .and_then(|binding| binding.names.first())
    })?;
    let symbol = exact_symbol(facts, file.path.as_str(), name.span)?;
    // Exact writes invalidate declaration-to-runtime identity, even in other files.
    if written_symbols.contains(&symbol) {
        return None;
    }
    Some(symbol)
}

pub(super) fn literal_body(file: &FileFacts, span: Span) -> Option<&FunctionFact> {
    let span = unwrapped(file, span);
    let mut functions = file.ast.functions.iter().filter(|function| {
        function.span == span
            && function.kind != FunctionKind::Declaration
            && function.method_name.is_none()
            && !function.generator
    });
    let result = functions.next()?;
    functions.next().is_none().then_some(result)
}

pub(super) fn value_reference(facts: &ProjectFacts, file: &FileFacts, span: Span) -> bool {
    if file
        .ast
        .type_queries
        .iter()
        .any(|query| query.contains(span))
    {
        return false;
    }
    let mut matches = facts
        .typescript
        .entities_for_path(file.path.as_str())
        .iter()
        .filter(|entity| {
            entity.location.start_byte == u64::from(span.start)
                && entity.location.end_byte == u64::from(span.end)
        });
    let Some(entity) = matches.next() else {
        return false;
    };
    matches.next().is_none()
        && matches!(
            entity.reference_space,
            Some(typefacts::ReferenceSpace::Value | typefacts::ReferenceSpace::Both)
        )
}

pub(super) fn import_references_are_client_only(
    ast: &solid_facts::ast::AstFacts,
    binding: Span,
) -> bool {
    // ReferenceSpace and runtime_referenced summarize a binding's uses. A
    // value use inside a removed server function cannot authenticate a type
    // use outside it. Mixed client/server bindings need per-use lowering
    // facts; do not infer the surviving import from the aggregate.
    !ast.reference_declarations
        .iter()
        .any(|(reference, declaration)| {
            *declaration == binding
                && ast.functions.iter().any(|function| {
                    function.span.contains(*reference) && function.has_directive("use server")
                })
        })
}

pub(super) fn runtime_import(facts: &ProjectFacts, file: &FileFacts, import: &ImportFact) -> bool {
    !import.type_only
        && (import.bindings.is_empty()
            || import.bindings.iter().any(|binding| {
                if binding.type_only {
                    return false;
                }
                if binding.kind == ImportKind::SideEffect {
                    return true;
                }
                binding.runtime_referenced
                    && import_references_are_client_only(&file.ast, binding.local.span)
                    && file
                        .ast
                        .reference_declarations
                        .iter()
                        .any(|(reference, declaration)| {
                            *declaration == binding.local.span
                                && value_reference(facts, file, *reference)
                                && !file.ast.functions.iter().any(|function| {
                                    function.span.contains(*reference)
                                        && function.has_directive("use server")
                                })
                        })
            }))
}
