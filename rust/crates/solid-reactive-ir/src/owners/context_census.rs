//! The closed provider census behind `missing-context-provider` (ADR 0352).
//!
//! `useContext(Ctx)` throws `ContextNotFoundError` when no provider stores a
//! value for a context created with no default. A project context has no
//! provider in a closed program when no use of its binding can store one: no
//! `<Ctx value={…}>` element, and no reference that could hand the binding to
//! code that provides it. Then every read that runs under an owner throws. This
//! census names the contexts that hold for; it claims nothing beyond the program.

use super::*;
use crate::pipeline::parallel_balanced_results;
use solid_dialect::ContextRole;
use solid_facts::ast::{BindingShape, ExportKind, IdentifierRole, ImportKind};
use typefacts::Location;

/// The project contexts, created with no default, that no provider can supply,
/// each with whether its lack of a default is proven (ADR 0575): `true` for
/// `createContext()` and `createContext(undefined)` (rc.17 `getContext` treats
/// an `undefined` default as none), `false` for a default whose value the
/// syntax does not prove is not `undefined` (a read is then an obligation).
///
/// In an open program the same census runs, and what it finds is an
/// obligation rather than a proof (ADR 0517). A namespace import or export, `export *`,
/// or module load (`require`, `import()`) reaches the exports of the module it
/// names without naming them in the reading file, so it closes the census for
/// every context that module exports. A context no export publishes is reachable
/// only through its own file's escapes, which the census checks.
///
/// What a file contributes is a function of that file and the lookup, so the
/// two passes run per file on the analysis workers and their results are
/// merged in file order. Every reference of every file is censused: a name
/// that does not match a candidate's can still resolve to its binding (a
/// renamed import), so no reference is skipped by its spelling.
pub(crate) fn provider_free_contexts(
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    symbol_names: &HashMap<SymbolId, SymbolId>,
) -> HashMap<Location, ContextVerdict> {
    // ADR 0517: an open program is censused too. What the census finds
    // provider-free there may still be provided by code the analysis does not
    // see, so the read is an obligation (`owner_emission`), not silence.
    let mut free = HashMap::default();
    let entities = lookup.entities();
    // Candidates: `const Ctx = createContext()` with no argument, an exact,
    // unwritten identifier binding (ADR 0352). Dependency source is not the
    // project's to close.
    // Each candidate's name, whether its file exports it, and whether its
    // lack of a default is proven.
    let mut candidates = HashMap::<Location, (String, bool, bool)>::default();
    for found in parallel_balanced_results(&facts.files, |file| {
        file_candidates(facts, lookup, symbol_names, entities, file)
    }) {
        for (key, candidate, verdict) in found {
            // ADR 0713: a binding the census cannot close (`let`, a pattern)
            // is still a context with no default: its reads are open. One
            // with a proven default, or a package's, is silent.
            match verdict {
                None => {
                    candidates.insert(key, candidate);
                }
                Some(verdict) => {
                    free.insert(key, verdict);
                }
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
        // ADR 0713: an export a namespace or module load may reach is open,
        // not silent.
        candidates.retain(|key, (_, exported, _)| {
            if *exported {
                free.insert(key.clone(), ContextVerdict::Open);
            }
            !*exported
        });
    }
    if candidates.is_empty() {
        return free;
    }
    let candidate_names = candidates
        .values()
        .map(|(name, _, _)| name.clone())
        .collect::<HashSet<_>>();
    let mut provided = HashSet::<Location>::default();
    // Escapes: any other reference that resolves to a candidate. A reference
    // that resolves to nothing and reads a candidate's name cannot be
    // attributed, so that name is blocked.
    let mut escaped = HashSet::<Location>::default();
    let mut blocked_names = HashSet::<String>::default();
    for census in parallel_balanced_results(&facts.files, |file| {
        file_census(
            lookup,
            symbol_names,
            entities,
            &candidates,
            &candidate_names,
            file,
        )
    }) {
        provided.extend(census.provided);
        escaped.extend(census.escaped);
        blocked_names.extend(census.blocked);
    }
    for (key, (name, _, proven)) in &candidates {
        // ADR 0713: a context provided somewhere, or handed to code that may
        // provide it, is open: whether a provider is above a read is the
        // read's own proof (`owner_emission`), never silence.
        let verdict =
            if escaped.contains(key) || provided.contains(key) || blocked_names.contains(name) {
                ContextVerdict::Open
            } else {
                ContextVerdict::Free(*proven)
            };
        free.insert(key.clone(), verdict);
    }
    free
}

/// The census's answer for a project context created with no default
/// (ADR 0713).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ContextVerdict {
    /// No provider can supply it; whether its lack of a default is proven.
    Free(bool),
    /// A provider may supply it somewhere: each read needs its own proof.
    Open,
    /// Created with a default the syntax proves, or in a package: a read
    /// never throws for want of a provider here (unchanged by ADR 0713).
    Silent,
}

/// A candidate's name, whether its file exports it, and whether its lack of
/// a default is proven.
type Candidate = (String, bool, bool);

/// The candidates `file` declares, in the order the file declares them, each
/// with its verdict when the census need not close it (ADR 0713).
fn file_candidates(
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    entities: &EntitySymbols,
    file: &solid_facts::FileFacts,
) -> Vec<(Location, Candidate, Option<ContextVerdict>)> {
    let mut found = Vec::new();
    let dependency = facts.dependency_source_root(file.path.as_str()).is_some();
    for call in &file.ast.calls {
        if context_role_of(lookup, file, call, entities, symbol_names) != Some(ContextRole::Create)
        {
            continue;
        }
        let proven = default_absent(lookup, file, call);
        if dependency || proven.is_none() {
            for binding in &file.ast.bindings {
                if binding.call_initializer == Some(call.span) {
                    found.push((
                        location(file.path.shared(), binding.pattern),
                        (String::new(), false, false),
                        Some(ContextVerdict::Silent),
                    ));
                }
            }
            continue;
        }
        let Some(proven) = proven else {
            continue;
        };
        for binding in &file.ast.bindings {
            // A `const` cannot be assigned. The name-based redeclaration check of
            // `binding_written` would also count a shadowing declaration, which
            // is another binding, so it is not used here.
            if binding.call_initializer != Some(call.span) {
                continue;
            }
            let closed = binding.immutable
                && binding.shape == BindingShape::Identifier
                && binding.names.len() == 1;
            if !closed {
                found.push((
                    location(file.path.shared(), binding.pattern),
                    (String::new(), false, proven),
                    Some(ContextVerdict::Open),
                ));
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
            found.push((
                location(file.path.shared(), binding.pattern),
                (name.to_owned(), exported, proven),
                None,
            ));
        }
    }
    found
}

/// Whether `createContext`'s call leaves the context with no default
/// (ADR 0575). rc.17 `getContext` reads `context.defaultValue` when no
/// provider stored a value and throws `ContextNotFoundError` when it is
/// `undefined`, so an `undefined` default is none.
///
/// `Some(true)`: no argument, or the global `undefined` (through assertions
/// and parentheses). `None`: a value the syntax proves is not `undefined` (a
/// literal, `null`, a function, object or array literal, or a `const`
/// initialized with one). `Some(false)`: anything else, which may be
/// `undefined` at runtime.
fn default_absent(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
) -> Option<bool> {
    use solid_facts::ast::{ArgumentValueKind, RuntimeValueKind};
    let Some(default) = call.arguments.first() else {
        return Some(true);
    };
    if default.spread {
        return Some(false);
    }
    let value = |kind: RuntimeValueKind| {
        matches!(
            kind,
            RuntimeValueKind::Primitive
                | RuntimeValueKind::Function
                | RuntimeValueKind::Object
                | RuntimeValueKind::Array
        )
    };
    let peeled = file.ast.peel_ts_sugar_span(default.span);
    match default.value {
        ArgumentValueKind::Undefined => return Some(true),
        ArgumentValueKind::Null => return None,
        _ => {}
    }
    if default.runtime_value_kind == RuntimeValueKind::Nullish {
        // Only `null` and the global `undefined` are nullish literals.
        return if file.source_text(peeled) == Some("null") {
            None
        } else {
            Some(true)
        };
    }
    if value(default.runtime_value_kind) {
        return None;
    }
    if default.value == ArgumentValueKind::Identifier
        && let Some((_, binding, _)) = lookup.binding_at_reference(file.path.as_str(), peeled)
        && binding.immutable
        && binding.shape == BindingShape::Identifier
        && value(binding.initializer_value_kind)
    {
        return None;
    }
    Some(false)
}

/// Whether the JSX element provides `undefined` and nothing else: one `value`
/// attribute, the global `undefined`, and no spread. rc.17's provider stores
/// `context.defaultValue` for it, so for a context with no default it is no
/// provider (ADR 0575).
fn provides_undefined(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
) -> bool {
    if !element.spreads.is_empty() {
        return false;
    }
    let mut values = element.attributes.iter().filter(|attribute| {
        attribute.namespace.is_none() && file.source_text(attribute.name) == Some("value")
    });
    let (Some(value), None) = (values.next(), values.next()) else {
        return false;
    };
    value.expression.is_some_and(|expression| {
        let expression = file.ast.peel_ts_sugar_span(expression);
        file.source_text(expression) == Some("undefined")
            && file.ast.identifiers.iter().any(|identifier| {
                identifier.span == expression
                    && identifier.role == IdentifierRole::Reference
                    && file.ast.reference_declaration(expression).is_none()
            })
    })
}

/// What one file adds to the census of the candidates.
#[derive(Default)]
struct FileCensus {
    /// The contexts a JSX element of the file provides.
    provided: Vec<Location>,
    /// The candidates a use of the file hands to code that may provide them.
    escaped: Vec<Location>,
    /// The candidate names a reference of the file reads but resolves to
    /// nothing.
    blocked: Vec<String>,
}

/// One file's reads, providers and escapes. A position is excused from being an
/// escape by its own file's reads and tag names only, so the sets of positions
/// are the file's own, keyed by span.
fn file_census(
    lookup: &SemanticLookup<'_>,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    entities: &EntitySymbols,
    candidates: &HashMap<Location, (String, bool, bool)>,
    candidate_names: &HashSet<String>,
    file: &solid_facts::FileFacts,
) -> FileCensus {
    let mut census = FileCensus::default();
    // Reads: the one argument of Solid's `useContext`. A read stores nothing.
    let mut reads = HashSet::<Span>::default();
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
        reads.insert(file.ast.peel_ts_sugar_span(argument.span));
    }
    // Providers: a JSX element whose tag resolves to the binding. Its name and
    // closing name are the element's own, not a use that escapes.
    let mut tag_names = HashSet::<Span>::default();
    let mut closing_names = HashSet::<Span>::default();
    for element in &file.ast.jsx_elements {
        tag_names.insert(element.name.span);
        if let Some(closing) = element.closing_name {
            tag_names.insert(closing);
            closing_names.insert(closing);
        }
        if let Some(key) = context_key_at(lookup, file, element.name.span)
            && !provides_undefined(file, element)
        {
            census.provided.push(key);
        }
    }
    // Escapes: any other reference that resolves to a candidate. An import or
    // export specifier is publication, not a use: an importer's own references
    // are censused where they occur. A reference that resolves to nothing and
    // reads a candidate's name cannot be attributed, so that name is blocked.
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
            if closing_names.contains(&identifier.span) {
                continue;
            }
            if let Some(text) = file.source_text(identifier.span)
                && candidate_names.contains(text)
            {
                census.blocked.push(text.to_owned());
            }
            continue;
        };
        let key = location(context_file.path.shared(), binding.pattern);
        if !candidates.contains_key(&key) {
            continue;
        }
        if reads.contains(&identifier.span)
            || tag_names.contains(&identifier.span)
            || specifiers.contains(&identifier.span)
        {
            continue;
        }
        census.escaped.push(key);
    }
    census
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

/// How many functions [`provided_above`] climbs through callers.
const PROVIDED_DEPTH: usize = 8;

/// ADR 0713: whether every run of the function `reading` (in `file`) is
/// under a provider of the context `key`: a named, non-async, non-generator
/// function every reference to which is a call or render site the call
/// graph accounts for (none exported in an open program, no `eval`), with at
/// least one site, each written in the children of a `<Ctx value>` element
/// (not `value={undefined}`), or of a wrapper that renders its
/// `props.children` there unconditionally, with no function layer between,
/// or directly in a function that is itself provided for `key`.
pub(crate) fn provided_above(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    reading: &solid_facts::ast::FunctionFact,
    key: &Location,
) -> bool {
    function_provided(
        lookup,
        file,
        reading,
        key,
        PROVIDED_DEPTH,
        &mut rustc_hash::FxHashMap::default(),
    )
}

/// One function, asked at one remaining depth, for one `key`.
type ProvidedMemo = rustc_hash::FxHashMap<(String, Span, usize), bool>;

/// Asks each function once per remaining depth: a function many sites reach
/// costs one walk, not one per path, and the answer is the same (`walk_memo`).
fn function_provided(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    key: &Location,
    depth: usize,
    memo: &mut ProvidedMemo,
) -> bool {
    crate::walk_memo::memoized(
        memo,
        (file.path.to_string(), function.span, depth),
        |memo| function_provided_once(lookup, file, function, key, depth, memo),
    )
}

fn function_provided_once(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    key: &Location,
    depth: usize,
    memo: &mut ProvidedMemo,
) -> bool {
    let Some(depth) = depth.checked_sub(1) else {
        return false;
    };
    if function.r#async
        || function.generator
        || lookup.program_has_eval()
        || !lookup.function_value_is_current(file, function)
        || (!lookup.program_closed && crate::execution_role::module_exports(file, function, lookup))
    {
        return false;
    }
    let path = file.path.as_str();
    let Some(name) = crate::owners::function_binding_name(file, function) else {
        return false;
    };
    let Some(symbol) = lookup.entities().at(path, name.span) else {
        return false;
    };
    let Some(references) = lookup.symbol_references_if_present(symbol.as_str()) else {
        return false;
    };
    let accounted = lookup
        .function_call_site_references(path, function.span)
        .into_iter()
        .map(|(caller, callee)| (caller.path.as_str().to_owned(), callee.start, callee.end))
        .collect::<HashSet<_>>();
    let every_reference_a_site = references.iter().all(|reference| {
        let (Ok(start), Ok(end)) = (
            u32::try_from(reference.start_byte),
            u32::try_from(reference.end_byte),
        ) else {
            return false;
        };
        (reference.path.as_ref() == path && start == name.span.start && end == name.span.end)
            || accounted.contains(&(reference.path.as_ref().to_owned(), start, end))
    });
    let sites = lookup.function_call_sites(path, function.span);
    every_reference_a_site
        && !sites.is_empty()
        && sites
            .iter()
            .all(|(caller, site)| site_provided(lookup, caller, *site, key, depth, memo))
}

/// Whether the call or render at `site` runs under a provider of `key`.
fn site_provided(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    site: Span,
    key: &Location,
    depth: usize,
    memo: &mut ProvidedMemo,
) -> bool {
    let ast = &file.ast;
    let layer = containing_ast_function(ast, site);
    let mut around = ast
        .jsx_elements
        .iter()
        .filter(|element| {
            element.span.contains(site)
                && element.name.span != site
                && layer.is_none_or(|layer| layer.span.contains(element.span))
        })
        .collect::<Vec<_>>();
    around.sort_by_key(|element| element.span.end - element.span.start);
    for element in around {
        // An attribute's value is built by the element's parent, not under it.
        if !element.children.iter().any(|child| child.contains(site)) {
            continue;
        }
        if context_key_at(lookup, file, element.name.span).as_ref() == Some(key) {
            if provides_undefined(file, element) {
                return false;
            }
            return true;
        }
        if wrapper_provides(lookup, file, element, key) {
            return true;
        }
    }
    layer.is_some_and(|layer| function_provided(lookup, file, layer, key, depth, memo))
}

/// ADR 0713: the component `element` renders is a wrapper that returns, on
/// its one `return` (or as its expression body), a `<Ctx value>` element
/// providing `key` whose direct child is its props parameter's `children`,
/// with no function, `?:` or logical expression between.
fn wrapper_provides(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
    key: &Location,
) -> bool {
    if crate::execution_role::intrinsic_element(file, element) {
        return false;
    }
    let Some((wrapper_file, wrapper)) =
        lookup.function_called_at(file.path.as_str(), element.name.span)
    else {
        return false;
    };
    let ast = &wrapper_file.ast;
    let Some(parameter) = wrapper.parameters.first() else {
        return false;
    };
    let [props] = parameter.names.as_slice() else {
        return false;
    };
    if parameter.shape != BindingShape::Identifier || wrapper.r#async || wrapper.generator {
        return false;
    }
    let returned = if wrapper.expression_body {
        vec![Some(wrapper.body)]
    } else {
        ast.returns
            .iter()
            .filter(|returned| {
                containing_ast_function(ast, returned.span)
                    .is_some_and(|owner| owner.span == wrapper.span)
            })
            .map(|returned| returned.argument)
            .collect()
    };
    let [Some(returned)] = returned.as_slice() else {
        return false;
    };
    let in_wrapper_code = |span: Span| {
        containing_ast_function(ast, span).is_some_and(|owner| owner.span == wrapper.span)
    };
    let branch_between = |outer: Span, inner: Span| {
        ast.conditional_expressions
            .iter()
            .any(|branch| outer.contains(branch.span) && branch.span.contains(inner))
            || ast
                .logical_expressions
                .iter()
                .any(|branch| outer.contains(branch.span) && branch.span.contains(inner))
    };
    ast.jsx_elements.iter().any(|provider| {
        returned.contains(provider.span)
            && in_wrapper_code(provider.span)
            && !branch_between(*returned, provider.span)
            && context_key_at(lookup, wrapper_file, provider.name.span).as_ref() == Some(key)
            && !provides_undefined(wrapper_file, provider)
            && provider.children.iter().any(|child| {
                ast.members.iter().any(|member| {
                    child.contains(member.span)
                        && in_wrapper_code(member.span)
                        && !branch_between(*child, member.span)
                        && ast.reference_declaration(ast.peel_ts_sugar_span(member.object))
                            == Some(props.span)
                        && wrapper_file.source_text(member.property) == Some("children")
                        && !ast.jsx_elements.iter().any(|nested| {
                            nested.span != provider.span
                                && provider.span.contains(nested.span)
                                && nested.span.contains(member.span)
                        })
                })
            })
    })
}
