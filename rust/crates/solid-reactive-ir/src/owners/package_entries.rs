//! ADR 0949: the project sites that enter package code and that ADR 0290's call
//! walk cannot name.
//!
//! A read of a context created with no default, in a package analyzed as source
//! (ADR 0943), is a `provider-not-proven-above` obligation at the project site that
//! runs it, because a finding in package source is not reported (ADR 0284). ADR
//! 0290 finds those sites by walking owner edges back to a *call* written in a
//! project function. A package function is also entered where no call stands:
//!
//! - a JSX tag renders it as a component, under a root of its own (ADR 0869), and
//!   the component's owner context is a seed, not an edge;
//! - a project hands it on as a value (`createEffect(useUi)`, `run(useUi)`,
//!   `<Dynamic component={Panel}>`, `<Layout comp={Panel}>`, `{Badge}`, a lazy
//!   loader's `m.Panel`);
//! - a holder of it is used (`new Reader().read()`, `registry.current`,
//!   `<Ui.Panel>`, a namespace's member): the function is a method, a getter or a
//!   member that no reference to its own name enters;
//! - the package's own closure or helper reaches it by a hop the call walk stops at.
//!
//! Each of those sites got no row, so the project was certified clean while Solid
//! threw `ContextNotFoundError` (CONTEXT_NOT_FOUND). The requirement is now
//! reported at each such site, as the same obligation (uncertifiable, never a
//! violation: the package may provide its own context):
//!
//! 1. **Renders.** The walk goes back from the function that holds the read through
//!    every owner edge, the exact call and tag sites the call graph names for it
//!    (`SemanticLookup::function_call_sites`, the resolution that proves component
//!    identity) and a nested function's enclosing function, through package
//!    functions only. A project JSX element that renders a function the walk
//!    reached is a site ([`DependencyEntry::Render`]), whether its tag is the
//!    function's own name, an import alias, a namespace member, or an exact local
//!    alias (`const P2 = Panel; <P2/>`, ADR 0331).
//! 2. **Calls** into a reached function that the call walk did not visit (it stops at
//!    an edge a package function creates, and at a callback edge that names no call)
//!    are sites too, when the caller's owner context may carry an owner.
//! 3. **References.** Any other project reference to a symbol the package declares,
//!    whose package has such a read, is a site ([`DependencyEntry::Reference`]),
//!    unless it is the callee of a call or the tag of an element that resolves
//!    exactly to a function (that entry is decided by 1 and 2), an import or export
//!    specifier, a type, or a context binding (the project's own read is its own
//!    row). Which function a handed-on value runs is not followed: the package
//!    holds a read, and the reference may run it.
//!
//! A project function that no owner edge reaches (context zero) runs no entry
//! written in it, as for the call walk. Module scope runs.

use super::*;

use solid_facts::ast::JsxElementFact;
use typefacts::Location;

/// The elements that render a function through an exact local alias, by the
/// function's path and span: each element's path and opening tag.
type AliasTags = HashMap<(String, Span), Vec<(String, Span)>>;

/// How a project entry is found, before it becomes a requirement.
struct Entry {
    path: String,
    span: Span,
    kind: crate::DependencyEntry,
}

/// The requirements of the project sites that enter package functions that hold
/// a [`PackageRead`], added to `requirements` (each location and operation once).
///
/// `walked` is what the call walk visited (`attribute_dependency_requirements`),
/// `provider_free` the census verdict of each context, `reads` the reads of every
/// package file.
#[allow(clippy::too_many_arguments)]
pub(super) fn attribute_package_entries(
    requirements: &mut Vec<OwnerRequirement>,
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    nodes: &[OwnerNode],
    contexts: &[u8],
    outgoing: &[Vec<(usize, OwnerEdgeKind)>],
    walked: &HashSet<usize>,
    provider_free: &HashMap<Location, ContextVerdict>,
    reads: &[PackageRead],
) {
    if reads.is_empty() || facts.dependency_source_roots.is_empty() {
        return;
    }
    let in_dependency = |path: &str| facts.dependency_source_root(path).is_some();
    let reach = crate::execution_role::SiteReach::new(lookup);
    let mut reads = reads.to_vec();
    reads.sort_by(|left, right| {
        (
            &left.location.path,
            left.location.start_byte,
            left.location.end_byte,
        )
            .cmp(&(
                &right.location.path,
                right.location.start_byte,
                right.location.end_byte,
            ))
    });
    // The function nodes by path, and the owner edges back.
    let mut nodes_by_path = HashMap::<&str, Vec<usize>>::default();
    for (index, node) in nodes.iter().enumerate() {
        nodes_by_path
            .entry(node.path.as_str())
            .or_default()
            .push(index);
    }
    let mut incoming = vec![Vec::<(usize, OwnerEdgeKind)>::new(); nodes.len()];
    for (source, edges) in outgoing.iter().enumerate() {
        for &(target, kind) in edges {
            incoming[target].push((source, kind));
        }
    }
    // The innermost function whose body is written around `span`.
    let node_at = |path: &str, span: Span| {
        nodes_by_path
            .get(path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| nodes[*index].body.contains(span))
            .min_by_key(|index| nodes[*index].body.end - nodes[*index].body.start)
    };
    // A project function an owner edge reaches runs its entries. One no edge reaches
    // (context zero) is still run by whatever the graph does not follow (a wrapper
    // it was handed to, a timer, a handler), unless it is proven never to run:
    // as `emit_file`'s `caller_unplaced`, an entry written in it stands. Only that
    // proof silences an entry; a function or a file the facts do not name is none.
    let never_runs = |node: usize| {
        lookup
            .file_by_path(nodes[node].path.as_str())
            .and_then(|file| {
                file.ast
                    .function_at(nodes[node].span)
                    .map(|function| !reach.function_runs(file, function))
            })
            .is_some_and(|never| never)
    };
    let may_run = |node: usize| contexts[node] != 0 || !never_runs(node);
    let mut seen = requirements
        .iter()
        .map(|requirement| {
            (
                requirement.location.path.to_string(),
                requirement.location.start_byte,
                requirement.location.end_byte,
                operation_name(requirement.operation).to_owned(),
            )
        })
        .collect::<HashSet<_>>();
    // Which package a read belongs to, and the first read of each: the evidence
    // a reference to one of its symbols names.
    let root_of = |path: &str| facts.dependency_source_root(path).map(str::to_owned);
    let mut first_read_of_root = HashMap::<String, &PackageRead>::default();
    for read in &reads {
        if let Some(root) = root_of(read.location.path.as_ref()) {
            first_read_of_root.entry(root).or_insert(read);
        }
    }

    // 1 and 2: the walk back from each read.
    let mut sites: Vec<(Entry, &PackageRead)> = Vec::new();
    // Local alias tags (`const P2 = Panel; <P2/>`), by the function they render.
    let mut alias_tags: Option<AliasTags> = None;
    for read in &reads {
        let Some(seed) = nodes_by_path
            .get(read.location.path.as_ref())
            .into_iter()
            .flatten()
            .copied()
            .find(|index| nodes[*index].span == read.function)
        else {
            continue;
        };
        let mut visited = HashSet::<usize>::default();
        let mut queue = VecDeque::from([seed]);
        visited.insert(seed);
        while let Some(node) = queue.pop_front() {
            let node_path = nodes[node].path.as_str();
            let node_span = nodes[node].span;
            // Every owner edge into the function, whatever its kind.
            for &(caller, kind) in &incoming[node] {
                if in_dependency(nodes[caller].path.as_str()) {
                    if visited.insert(caller) {
                        queue.push_back(caller);
                    }
                    continue;
                }
                // A project function that calls it: the call walk decided every
                // function it visited, with the call's arguments.
                if walked.contains(&node) || !may_run(caller) {
                    continue;
                }
                let Some(caller_file) = lookup.file_by_path(nodes[caller].path.as_str()) else {
                    continue;
                };
                let carries = owner_edge_context(
                    if kind == OwnerEdgeKind::MayRun {
                        OwnerEdgeKind::Preserve
                    } else {
                        kind
                    },
                    contexts[caller],
                ) & OWNER_CONTEXT_RUNS_OWNED
                    != 0;
                if !carries {
                    continue;
                }
                for call in &caller_file.ast.calls {
                    if crate::owners::containing_body_function(&caller_file.ast, call.span)
                        .is_some_and(|function| function.span == nodes[caller].span)
                        && call_enters(lookup, caller_file, call, &nodes[node])
                    {
                        sites.push((
                            Entry {
                                path: caller_file.path.to_string(),
                                span: call.span,
                                kind: crate::DependencyEntry::Call,
                            },
                            read,
                        ));
                    }
                }
            }
            // The function a nested function is written in runs it, or hands it on.
            if let Some(parent) = nodes_by_path
                .get(node_path)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| {
                    *index != node
                        && nodes[*index].body.contains(node_span)
                        && nodes[*index].span != node_span
                })
                .min_by_key(|index| nodes[*index].body.end - nodes[*index].body.start)
                && in_dependency(nodes[parent].path.as_str())
                && visited.insert(parent)
            {
                queue.push_back(parent);
            }
            // The exact call and tag sites of the function.
            let mut tag_sites = lookup
                .function_call_sites(node_path, node_span)
                .into_iter()
                .filter_map(|(file, callee)| {
                    let element = file
                        .ast
                        .jsx_elements
                        .iter()
                        .find(|element| element.name.span == callee)?;
                    Some((file, element))
                })
                .map(|(file, element)| (file.path.to_string(), element_site(element)))
                .collect::<Vec<_>>();
            let aliases = alias_tags.get_or_insert_with(|| alias_rendered_tags(facts, lookup));
            if let Some(more) = aliases.get(&(node_path.to_owned(), node_span)) {
                tag_sites.extend(more.iter().cloned());
            }
            for (path, site) in tag_sites {
                let caller = node_at(&path, site);
                if in_dependency(&path) {
                    if let Some(caller) = caller
                        && visited.insert(caller)
                    {
                        queue.push_back(caller);
                    }
                    continue;
                }
                if caller.is_some_and(|caller| !may_run(caller)) {
                    continue;
                }
                sites.push((
                    Entry {
                        path,
                        span: site,
                        kind: crate::DependencyEntry::Render,
                    },
                    read,
                ));
            }
        }
    }

    // 3: references to a symbol a package declares, no call or render resolves.
    let mut roots = first_read_of_root
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    roots.sort_unstable();
    let mut references: Vec<(Entry, &PackageRead)> = Vec::new();
    for symbol in facts.typescript.symbols() {
        let Some(root) = symbol.declarations().iter().find_map(|declaration| {
            roots.iter().copied().find(|root| {
                std::path::Path::new(declaration.location.path.as_ref()).starts_with(root)
            })
        }) else {
            continue;
        };
        let read = first_read_of_root[root];
        for reference in symbol.references() {
            let path = reference.path.as_ref();
            if in_dependency(path) {
                continue;
            }
            let (Ok(start), Ok(end)) = (
                u32::try_from(reference.start_byte),
                u32::try_from(reference.end_byte),
            ) else {
                continue;
            };
            let Some(file) = lookup.file_by_path(path) else {
                continue;
            };
            let span = Span::new(start, end);
            if reference_is_not_an_entry(lookup, file, span, provider_free) {
                continue;
            }
            if node_at(path, span).is_some_and(|node| !may_run(node)) {
                continue;
            }
            references.push((
                Entry {
                    path: path.to_owned(),
                    span: entry_anchor(file, span),
                    kind: crate::DependencyEntry::Reference,
                },
                read,
            ));
        }
    }

    // The requirements, in path and span order.
    sites.extend(references);
    sites.sort_by(|(left, _), (right, _)| {
        (&left.path, left.span.start, left.span.end).cmp(&(
            &right.path,
            right.span.start,
            right.span.end,
        ))
    });
    for (entry, read) in sites {
        let Some(file) = lookup.file_by_path(&entry.path) else {
            continue;
        };
        let before = requirements.len();
        push_owner_requirement(
            requirements,
            &mut seen,
            "context-provider",
            file,
            entry.span,
            OwnerRequirementStatus {
                flight_error_unobserved: false,
                uncertain: true,
                runtime_uncertain: false,
                caller_uncertain: false,
                unplaced_callback: false,
                conditional_owner: false,
                owner_varies_by_caller: false,
                later_run_unowned: false,
                component_uncertain: false,
                provider_open: true,
                default_unproven: false,
                owned_run_throws: false,
                after_await: false,
                after_yield: false,
                context_unaccounted: false,
                report: true,
            },
            lookup,
        );
        if requirements.len() > before
            && let Some(pushed) = requirements.last_mut()
        {
            pushed.through_dependency = Some(read.location.clone());
            pushed.through_dependency_entry = entry.kind;
        }
    }
}

/// The span a render is reported at: the element's opening tag.
fn element_site(element: &JsxElementFact) -> Span {
    if element.opening.end > element.opening.start {
        element.opening
    } else {
        element.name.span
    }
}

/// Where a reference is reported: the call it is written in (its callee chain or an
/// argument, in the same function: `createEffect(useUi, …)`, `new Reader().read()`),
/// or the opening tag of the element it is an attribute of (`<Dynamic component={Panel}>`),
/// whichever is the innermost; the reference itself otherwise. Solid's stack frame for the
/// run sits at that call or element, which is where the entry happens.
fn entry_anchor(file: &solid_facts::FileFacts, reference: Span) -> Span {
    let ast = &file.ast;
    let here = crate::owners::owning_function(ast, reference).map(|function| function.span);
    let in_this_function = |span: Span| {
        crate::owners::owning_function(ast, span).map(|function| function.span) == here
    };
    let call = ast
        .calls
        .iter()
        .filter(|call| call.span.contains(reference) && in_this_function(call.span))
        .map(|call| call.span);
    let element = ast
        .jsx_elements
        .iter()
        .filter(|element| {
            element.opening.contains(reference)
                && !element.name.span.contains(reference)
                && in_this_function(element.span)
        })
        .map(|element| element.opening);
    call.chain(element)
        .min_by_key(|span| span.end - span.start)
        .unwrap_or(reference)
}

/// Whether the call enters the function `node`: an exact invocation of it, not
/// a member of it nor a replaced binding (as the call walk's `enters`).
fn call_enters(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    node: &OwnerNode,
) -> bool {
    crate::execution_role::project_callee_at_parameter(file, call, 0, lookup).is_some_and(
        |(callee_file, callee, exact)| {
            exact && *callee_file.path.as_str() == *node.path && callee.span == node.span
        },
    )
}

/// The elements whose tag names a function only through an exact alias
/// (`const P2 = Panel; <P2/>`), by the function they render: its path and
/// span, then each element's path and opening tag. The call graph keeps no
/// entry for them (ADR 0331).
fn alias_rendered_tags(facts: &ProjectFacts, lookup: &SemanticLookup<'_>) -> AliasTags {
    let mut tags = AliasTags::default();
    for file in crate::base::reads::all_files(facts) {
        for element in &file.ast.jsx_elements {
            if lookup
                .function_called_at(file.path.as_str(), element.name.span)
                .is_some()
            {
                continue;
            }
            if let Some((target_file, target)) =
                lookup.alias_rendered_tag_function(file, element.name.span)
            {
                tags.entry((target_file.path.to_string(), target.span))
                    .or_default()
                    .push((file.path.to_string(), element_site(element)));
            }
        }
    }
    tags
}

/// Whether a project reference to a symbol of a package is no entry the walks
/// leave open: an import or export specifier, a type, the callee of a call or
/// the tag of an element that resolves exactly to a function (decided by the
/// walks above), or a context binding (the project's own read is its own row).
fn reference_is_not_an_entry(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    span: Span,
    provider_free: &HashMap<Location, ContextVerdict>,
) -> bool {
    let ast = &file.ast;
    if ast.imports.iter().any(|import| import.span.contains(span))
        || ast.exports.iter().any(|export| {
            export.module.is_some() && export.span.contains(span)
                || export
                    .specifiers
                    .iter()
                    .any(|specifier| specifier.local.span.contains(span))
        })
        || ast.type_names.iter().any(|name| name.contains(span))
        || ast.type_queries.iter().any(|query| query.contains(span))
        || ast
            .type_imports
            .iter()
            .any(|import| import.span.contains(span))
    {
        return true;
    }
    // The closing tag names what the opening tag names: one entry per render.
    if ast.jsx_elements.iter().any(|element| {
        element
            .closing_name
            .is_some_and(|closing| closing.contains(span))
    }) {
        return true;
    }
    // The tag of an element that resolves exactly: sites 1 decides.
    if ast.jsx_elements.iter().any(|element| {
        (element.name.span.contains(span) || element.closing_name.is_some_and(|c| c.contains(span)))
            && (lookup
                .function_called_at(file.path.as_str(), element.name.span)
                .is_some()
                || lookup
                    .alias_rendered_tag_function(file, element.name.span)
                    .is_some())
    }) {
        return true;
    }
    // The callee of a call that resolves exactly: sites 2 and the call walk.
    if ast.calls.iter().any(|call| {
        ast.peel_ts_sugar_span(call.callee).contains(span)
            && crate::execution_role::project_callee_at_parameter(file, call, 0, lookup)
                .is_some_and(|(_, _, exact)| exact)
    }) {
        return true;
    }
    // A context binding: a read or a provider names it, and neither runs package code.
    context_key_at(lookup, file, span).is_some_and(|key| provider_free.contains_key(&key))
}
