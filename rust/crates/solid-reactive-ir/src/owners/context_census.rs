//! The closed provider census behind `missing-context-provider` (ADR 0352).
//!
//! `useContext(Ctx)` throws `ContextNotFoundError` when no provider stores a
//! value for a context created with no default. A project context has no
//! provider in a closed program when no use of its binding can store one: no
//! `<Ctx value={…}>` element, and no reference that could hand the binding to
//! code that provides it. Then every read that runs under an owner throws. This
//! census names the contexts that hold for; it claims nothing beyond the program.

use super::*;
use solid_dialect::ContextRole;
use solid_facts::ast::{BindingShape, ExportKind, IdentifierRole, ImportKind};
use typefacts::Location;

/// The project contexts, created with no default, that no provider can supply.
///
/// Empty unless the program is closed. A namespace import or export, `export *`,
/// or module load (`require`, `import()`) reaches the exports of the module it
/// names without naming them in the reading file, so it closes the census for
/// every context that module exports. A context no export publishes is reachable
/// only through its own file's escapes, which the census checks.
pub(crate) fn provider_free_contexts(
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> HashSet<Location> {
    let mut free = HashSet::default();
    if !lookup.program_closed {
        return free;
    }
    let entities = lookup.entities();
    // Candidates: `const Ctx = createContext()` with no argument, an exact,
    // unwritten identifier binding (ADR 0352). Dependency source is not the
    // project's to close.
    // Each candidate's name, and whether its file exports it.
    let mut candidates = HashMap::<Location, (String, bool)>::default();
    for file in &facts.files {
        if facts.dependency_source_root(file.path.as_str()).is_some() {
            continue;
        }
        for call in &file.ast.calls {
            if !call.arguments.is_empty()
                || context_role_of(lookup, file, call, entities, symbol_names)
                    != Some(ContextRole::Create)
            {
                continue;
            }
            for binding in &file.ast.bindings {
                // A `const` cannot be assigned. The name-based redeclaration check of
                // `binding_written` would also count a shadowing declaration, which
                // is another binding, so it is not used here.
                if binding.call_initializer != Some(call.span)
                    || !binding.immutable
                    || binding.shape != BindingShape::Identifier
                    || binding.names.len() != 1
                {
                    continue;
                }
                let name = file.source_text(binding.names[0].span).unwrap_or_default();
                let exported = file.ast.exports.iter().any(|export| {
                    export
                        .specifiers
                        .iter()
                        .chain(&export.declarations)
                        .any(|item| item.local.span == binding.names[0].span)
                });
                candidates.insert(
                    location(file.path.shared(), binding.pattern),
                    (name.to_owned(), exported),
                );
            }
        }
    }
    if candidates.is_empty() {
        return free;
    }
    let reaches_unnamed = facts.files.iter().any(|file| {
        !file.ast.module_loads.is_empty()
            || file.ast.imports.iter().any(|import| {
                import
                    .bindings
                    .iter()
                    .any(|binding| binding.kind == ImportKind::Namespace)
            })
            || file
                .ast
                .exports
                .iter()
                .any(|export| export.kind == ExportKind::All || export.namespace.is_some())
    });
    if reaches_unnamed {
        candidates.retain(|_, (_, exported)| !*exported);
    }
    if candidates.is_empty() {
        return free;
    }
    // Reads: the one argument of Solid's `useContext`. A read stores nothing.
    let mut reads = HashSet::<(String, Span)>::default();
    for file in &facts.files {
        for call in &file.ast.calls {
            let [argument] = call.arguments.as_slice() else {
                continue;
            };
            if argument.spread
                || context_role_of(lookup, file, call, entities, symbol_names)
                    != Some(ContextRole::Read)
            {
                continue;
            }
            reads.insert((
                file.path.to_string(),
                file.ast.peel_ts_sugar_span(argument.span),
            ));
        }
    }
    // Providers: a JSX element whose tag resolves to the binding. Its name and
    // closing name are the element's own, not a use that escapes.
    let mut provided = HashSet::<Location>::default();
    let mut tag_names = HashSet::<(String, Span)>::default();
    let mut closing_names = HashSet::<(String, Span)>::default();
    for file in &facts.files {
        for element in &file.ast.jsx_elements {
            tag_names.insert((file.path.to_string(), element.name.span));
            if let Some(closing) = element.closing_name {
                tag_names.insert((file.path.to_string(), closing));
                closing_names.insert((file.path.to_string(), closing));
            }
            if let Some(key) = context_key_at(lookup, file, element.name.span) {
                provided.insert(key);
            }
        }
    }
    // Escapes: any other reference that resolves to a candidate. An import or
    // export specifier is publication, not a use: an importer's own references
    // are censused where they occur. A reference that resolves to nothing and
    // reads a candidate's name cannot be attributed, so that name is blocked.
    let mut escaped = HashSet::<Location>::default();
    let mut blocked_names = HashSet::<String>::default();
    let candidate_names = candidates
        .values()
        .map(|(name, _)| name.clone())
        .collect::<HashSet<_>>();
    for file in &facts.files {
        let specifiers = file
            .ast
            .imports
            .iter()
            .flat_map(|import| import.bindings.iter().map(|binding| binding.local.span))
            .chain(
                file.ast
                    .exports
                    .iter()
                    .flat_map(|export| export.specifiers.iter().map(|item| item.local.span)),
            )
            .collect::<HashSet<Span>>();
        for identifier in &file.ast.identifiers {
            if identifier.role != IdentifierRole::Reference {
                continue;
            }
            let Some((context_file, binding, _)) =
                lookup.binding_at_reference(file.path.as_str(), identifier.span)
            else {
                // A closing tag names the element its opening tag names, so it
                // is not an independent unresolved use; an opening tag is one.
                if closing_names.contains(&(file.path.to_string(), identifier.span)) {
                    continue;
                }
                if let Some(text) = file.source_text(identifier.span)
                    && candidate_names.contains(text)
                {
                    blocked_names.insert(text.to_owned());
                }
                continue;
            };
            let key = location(context_file.path.shared(), binding.pattern);
            if !candidates.contains_key(&key) {
                continue;
            }
            let position = (file.path.to_string(), identifier.span);
            if reads.contains(&position)
                || tag_names.contains(&position)
                || specifiers.contains(&identifier.span)
            {
                continue;
            }
            escaped.insert(key);
        }
    }
    for (key, (name, _)) in &candidates {
        if escaped.contains(key) || provided.contains(key) || blocked_names.contains(name) {
            continue;
        }
        free.insert(key.clone());
    }
    free
}

/// Whether an owner context holds an owner on every path: an owned bit, and no
/// unowned bit (ADR 0352). A context with any unowned bit may run with no owner.
pub(crate) const fn owned_on_every_path(context: u8) -> bool {
    context & OWNER_CONTEXT_UNOWNED == 0
        && context & (OWNER_CONTEXT_OWNED | OWNER_CONTEXT_LEAF_OWNED) != 0
}

/// The binding key of the context an expression at `span` names, when it names
/// a binding exactly.
pub(crate) fn context_key_at(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    span: Span,
) -> Option<Location> {
    let span = file.ast.peel_ts_sugar_span(span);
    let (context_file, binding, _) = lookup.binding_at_reference(file.path.as_str(), span)?;
    Some(location(context_file.path.shared(), binding.pattern))
}

fn context_role_of(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> Option<ContextRole> {
    let primitive = crate::call_primitive_name_cached(
        lookup,
        file,
        call,
        entities,
        symbol_names,
        lookup.dialect,
    )?
    .primitive()?;
    lookup
        .dialect
        .context_role(lookup.dialect.name_of(primitive)?)
}
