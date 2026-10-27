//! Missing-owner discovery: the project-wide scan for reactive reads and creations that run with no owner.

use super::*;

/// The function symbol a call runs, for an owner edge. An entity at a member
/// callee's composite span can name its receiver (`f.reset()`, `f.bind(…)`
/// carry `f`'s symbol), and such a call does not run the receiver
/// (ADR 0290 review round 1): it gets no edge, unless the member is the
/// standard library's `call` or `apply`, which run it before they return.
pub(super) fn call_edge_symbol<'e>(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    entities: &'e EntitySymbols,
    lookup: &SemanticLookup<'_>,
) -> Option<&'e SymbolId> {
    let symbol = entities.get(&location(file.path.shared(), call.callee))?;
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    let receiver = file
        .ast
        .member_at(callee)
        .map(|member| file.ast.peel_ts_sugar_span(member.object));
    if receiver
        .is_some_and(|object| entities.get(&location(file.path.shared(), object)) == Some(symbol))
    {
        let invokes_receiver = lookup
            .resolved_callee_call(file, call.callee)
            .and_then(|resolved| resolved.declaration.as_ref())
            .is_some_and(|declaration| {
                declaration.standard_library
                    && matches!(
                        declaration.qualified_name.as_ref(),
                        "Function.call"
                            | "Function.apply"
                            | "CallableFunction.call"
                            | "CallableFunction.apply"
                            | "NewableFunction.call"
                            | "NewableFunction.apply"
                    )
            });
        return invokes_receiver.then_some(symbol);
    }
    Some(symbol)
}

/// ADR 0290: report an owner requirement located in a package analyzed as
/// source (ADR 0284) at the project's call that runs it there.
///
/// A requirement inside a package is reported because some owner context
/// reached its function; when that context came from a project caller, the
/// defect is the project's call, and dropping it with the package's own
/// findings would leave the project silent. The walk goes back from the
/// operation's function along the edges that carry the reporting bit, through
/// package functions only, and stops at:
///
/// - an edge from a project function: each call written directly in it whose
///   callee is the package function it enters becomes a requirement at that
///   call;
/// - an edge a package function creates the bit on itself (it schedules the
///   callback): that context is the package's own, not a caller's.
///
/// Only a requirement proven unowned (`OWNER_CONTEXT_PROVEN_UNOWNED`, not
/// uncertain) walks the proven bit and can stay a violation at the call, and
/// only when the call is on its project function's every-completion path:
/// that function's unowned context reaches a call behind a branch or an owner
/// guard on some invocations only. Every other one walks the plain unowned
/// bit and becomes an obligation there: the call may run it unowned.
///
/// An operation after an `await` inside the package is unowned whatever the
/// caller did, so it walks every edge. A project call at module scope has no
/// edge; it is found by scanning and attributed as an obligation. A bit
/// seeded inside the package (its own event handlers or module scope) has no
/// project call and is not attributed.
pub(crate) fn attribute_dependency_requirements(
    requirements: &mut Vec<OwnerRequirement>,
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    nodes: &[OwnerNode],
    contexts: &[u8],
    outgoing: &[Vec<(usize, OwnerEdgeKind)>],
) {
    if facts.dependency_source_roots.is_empty() {
        return;
    }
    let mut incoming = vec![Vec::<(usize, OwnerEdgeKind)>::new(); nodes.len()];
    for (source, edges) in outgoing.iter().enumerate() {
        for &(target, kind) in edges {
            incoming[target].push((source, kind));
        }
    }
    let in_dependency = |index: usize| facts.dependency_source_root(&nodes[index].path).is_some();
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
    let mut attributed = Vec::new();
    for requirement in requirements.iter() {
        if !requirement.report
            || requirement.through_dependency.is_some()
            || facts
                .dependency_source_root(&requirement.location.path)
                .is_none()
        {
            continue;
        }
        let (Ok(start), Ok(end)) = (
            u32::try_from(requirement.location.start_byte),
            u32::try_from(requirement.location.end_byte),
        ) else {
            continue;
        };
        let span = Span::new(start, end);
        let Some(owner) = nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| {
                *node.path == *requirement.location.path && node.body.contains(span)
            })
            .min_by_key(|(_, node)| node.body.end - node.body.start)
            .map(|(index, _)| index)
        else {
            continue;
        };
        // After an `await` the operation is proven unowned on its own; its
        // function's entry context does not matter.
        let proven = !requirement.uncertain
            && (requirement.after_await || contexts[owner] & OWNER_CONTEXT_PROVEN_UNOWNED != 0);
        let mask = if proven {
            OWNER_CONTEXT_PROVEN_UNOWNED
        } else {
            OWNER_CONTEXT_UNOWNED
        };
        let Some(owner_file) = lookup.file_by_path(&nodes[owner].path) else {
            continue;
        };
        // ADR 0275 at the call boundary: the operation counts for a caller
        // only on its function's every-completion path, and each hop's call
        // on its caller's. The operation's own site is its call.
        let operation_site = owner_file
            .ast
            .calls
            .iter()
            .find(|call| call.callee == span || call.span == span)
            .map(|call| call.span);
        // Per package function on a path to the operation: the site in it
        // that leads there, whether that site is on the function's
        // every-completion path, and whether every hop below it is.
        // Per package function on a path to the operation: the sites in it
        // that lead there, whether one of them is on the function's
        // every-completion path, and whether every hop below it is.
        let mut best = HashMap::<usize, (Vec<Span>, bool, bool)>::default();
        let per_site = operation_site.is_some_and(|site| {
            crate::interproc::site_on_every_completion(lookup, owner_file, nodes[owner].span, site)
        });
        // Joint seed: the operation's sites here may be alternatives that
        // together cover every normal completion (`if (c) createEffect(…);
        // else createRenderEffect(…);`), though none of them does alone. The
        // sites are the calls written directly in this function whose exact
        // callee is a reportable requirement of this file, not through a
        // dependency, with the same operation and uncertainty. The function's
        // own site must be one, no `never` call may precede one (as for a single
        // site), and the cover must hold on every explicit exit.
        let owner_function = owner_file.ast.function_at(nodes[owner].span);
        let sibling_callees = requirements
            .iter()
            .filter(|sibling| {
                sibling.report
                    && sibling.through_dependency.is_none()
                    && sibling.operation == requirement.operation
                    && sibling.uncertain == requirement.uncertain
                    && sibling.location.path == requirement.location.path
            })
            .map(|sibling| (sibling.location.start_byte, sibling.location.end_byte))
            .collect::<HashSet<_>>();
        let sites = owner_file
            .ast
            .calls
            .iter()
            .filter(|call| {
                !per_site
                    && sibling_callees
                        .contains(&(u64::from(call.callee.start), u64::from(call.callee.end)))
                    && owner_function.is_some_and(|function| {
                        written_directly_in(&owner_file.ast, function, call.span)
                    })
            })
            .map(|call| call.span)
            .collect::<Vec<_>>();
        let joint = !per_site
            && sites.len() > 1
            && operation_site.is_some_and(|site| sites.contains(&site))
            && owner_function.is_some_and(|function| {
                sites
                    .iter()
                    .all(|&site| !lookup.never_returning_call_before(owner_file, function, site))
            })
            && solid_facts::ast::completion_call_cover_including_throws(
                std::path::Path::new(owner_file.path.as_str()),
                &owner_file.source,
                nodes[owner].body,
                &sites,
            ) == Some(true);
        best.insert(
            owner,
            (
                if joint {
                    sites
                } else {
                    vec![operation_site.unwrap_or(span)]
                },
                per_site || joint,
                true,
            ),
        );
        // An operation after an `await` in the package runs unowned whatever
        // context its function was entered with (review round 1 of ADR 0290,
        // Sol): every project call that runs it is attributed.
        let carries = |kind: OwnerEdgeKind, caller: usize| {
            requirement.after_await || owner_edge_context(kind, contexts[caller]) & mask != 0
        };
        let enters =
            |file: &solid_facts::FileFacts, call: &solid_facts::ast::CallFact, node: usize| {
                // An exact invocation of the function: not a member of it
                // (`f.reset()`, `f.bind(…)`) nor a replaced binding.
                crate::execution_role::project_callee_at_parameter(file, call, 0, lookup)
                    .is_some_and(|(callee_file, callee, exact)| {
                        exact
                            && *callee_file.path.as_str() == *nodes[node].path
                            && callee.span == nodes[node].span
                    })
            };
        // ADR 0280 on the first hop: what the call's arguments decide about
        // the callee's sites. Reached when one site is guaranteed, dropped
        // when every site is unreachable, possible otherwise.
        let first_hop = |file: &solid_facts::FileFacts,
                         call: &solid_facts::ast::CallFact,
                         node_file: &'_ solid_facts::FileFacts,
                         node_function: &solid_facts::ast::FunctionFact,
                         sites: &[Span]| {
            crate::argument_values::parameter_values(file, call, node_file, node_function, lookup)
                .filter(|values| !values.is_empty())
                .and_then(|values| {
                    solid_facts::ast::argument_site_standings(
                        std::path::Path::new(node_file.path.as_str()),
                        &node_file.source,
                        node_function.span,
                        &values,
                        &lookup.never_returning_calls_within(node_file, node_function),
                        sites,
                    )
                })
                .map(|standings| {
                    if standings.contains(&solid_facts::ast::SiteStanding::Guaranteed) {
                        solid_facts::ast::SiteStanding::Guaranteed
                    } else if standings
                        .iter()
                        .all(|standing| *standing == solid_facts::ast::SiteStanding::Unreachable)
                    {
                        solid_facts::ast::SiteStanding::Unreachable
                    } else {
                        solid_facts::ast::SiteStanding::Possible
                    }
                })
        };
        let mut queue = VecDeque::from([owner]);
        while let Some(node) = queue.pop_front() {
            let (sites, own, below) = best[&node].clone();
            let Some(node_file) = lookup.file_by_path(&nodes[node].path) else {
                continue;
            };
            let Some(node_function) = node_file.ast.function_at(nodes[node].span) else {
                continue;
            };
            for &(caller, kind) in &incoming[node] {
                if !carries(kind, caller) {
                    continue;
                }
                let created = owner_edge_context(kind, 0) & mask != 0;
                let Some(file) = lookup.file_by_path(&nodes[caller].path) else {
                    continue;
                };
                // Only calls written directly in the caller that enter this
                // function: a callback edge names no call to report at.
                let calls = file
                    .ast
                    .calls
                    .iter()
                    .filter(|call| {
                        crate::owners::containing_body_function(&file.ast, call.span)
                            .is_some_and(|function| function.span == nodes[caller].span)
                            && enters(file, call, node)
                    })
                    .collect::<Vec<_>>();
                if calls.is_empty() {
                    continue;
                }
                if in_dependency(caller) {
                    if created {
                        continue;
                    }
                    let completing = calls.iter().any(|call| {
                        crate::interproc::site_on_every_completion(
                            lookup,
                            file,
                            nodes[caller].span,
                            call.span,
                        )
                    });
                    let entry = (
                        calls.iter().map(|call| call.span).collect::<Vec<_>>(),
                        completing,
                        own && below,
                    );
                    let improves = best
                        .get(&caller)
                        .is_none_or(|previous| !(previous.1 && previous.2) && entry.1 && entry.2);
                    if improves {
                        best.insert(caller, entry);
                        queue.push_back(caller);
                    }
                    continue;
                }
                // The project caller creates the bit itself (it schedules the
                // package function, `setTimeout(f)`): the unowned run is that
                // reference's, not a direct call's (review round 1).
                if created && !requirement.after_await {
                    continue;
                }
                for call in calls {
                    let reached = match first_hop(file, call, node_file, node_function, &sites) {
                        Some(solid_facts::ast::SiteStanding::Unreachable) => continue,
                        Some(solid_facts::ast::SiteStanding::Guaranteed) => below,
                        Some(solid_facts::ast::SiteStanding::Possible) => false,
                        None => own && below,
                    };
                    // The unowned context is the project function's; it
                    // reaches this call only when the call is on that
                    // function's every-completion path (review round 1, Sol):
                    // a call behind a branch or an owner guard may run only on
                    // owned invocations.
                    let call_completes =
                        frontier_call_completes(lookup, file, nodes[caller].span, call.span);
                    attributed.push((
                        file,
                        call.span,
                        requirement.clone(),
                        proven && reached && call_completes,
                    ));
                }
            }
        }
        // A project call at module scope has no owner and no function node,
        // so no edge records it. It is attributed as an obligation: whether a
        // module-scope statement runs is not proven here.
        for &node in best.keys() {
            let (sites, _, _) = &best[&node];
            let Some(node_file) = lookup.file_by_path(&nodes[node].path) else {
                continue;
            };
            let Some(node_function) = node_file.ast.function_at(nodes[node].span) else {
                continue;
            };
            for file in &facts.files {
                if facts.dependency_source_root(file.path.as_str()).is_some() {
                    continue;
                }
                for call in &file.ast.calls {
                    // ADR 0454: a call in a parameter list is in a function.
                    if crate::owners::owning_function(&file.ast, call.span).is_some()
                        || !enters(file, call, node)
                        || first_hop(file, call, node_file, node_function, sites)
                            == Some(solid_facts::ast::SiteStanding::Unreachable)
                    {
                        continue;
                    }
                    attributed.push((file, call.span, requirement.clone(), false));
                }
            }
        }
    }
    for (file, span, requirement, proven) in attributed {
        let before = requirements.len();
        push_owner_requirement(
            requirements,
            &mut seen,
            operation_name(requirement.operation),
            file,
            span,
            OwnerRequirementStatus {
                uncertain: !proven,
                runtime_uncertain: requirement.runtime_uncertain,
                caller_uncertain: requirement.caller_uncertain,
                conditional_owner: requirement.conditional_owner,
                later_run_unowned: requirement.later_run_unowned,
                component_uncertain: requirement.component_uncertain,
                after_await: false,
                report: true,
            },
            lookup,
        );
        if requirements.len() > before
            && let Some(pushed) = requirements.last_mut()
        {
            pushed.through_dependency = Some(requirement.location.clone());
        }
    }
}

/// Whether `site`, a call written directly in the project function at
/// `function_span`, runs on that function's every completion (ADR 0276's
/// completion sites, minus an earlier `never` guard). Like
/// `interproc::site_on_every_completion`, except that only JSX, a class or a
/// pattern written inside this function defers the site: an inline handler
/// sits inside its component's JSX, and that JSX does not defer its own body.
pub(super) fn frontier_call_completes(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    function_span: Span,
    site: Span,
) -> bool {
    let Some(function) = file.ast.function_at(function_span) else {
        return false;
    };
    let inside = |span: Span| function.body.contains(span) && span.contains(site);
    file.ast.completion_sites.binary_search(&site).is_ok()
        && !function.generator
        && written_directly_in(&file.ast, function, site)
        && !file
            .ast
            .jsx_elements
            .iter()
            .any(|element| inside(element.span))
        && !file.ast.classes.iter().any(|class| inside(class.span))
        && !file
            .ast
            .bindings
            .iter()
            .any(|binding| inside(binding.pattern))
        && !file
            .ast
            .assignments
            .iter()
            .any(|assignment| inside(assignment.target))
        && !lookup.never_returning_call_before(file, function, site)
}

pub(super) const fn operation_name(operation: OwnerRequirementOperation) -> &'static str {
    match operation {
        OwnerRequirementOperation::Effect => "effect",
        OwnerRequirementOperation::Cleanup => "cleanup",
        OwnerRequirementOperation::Boundary => "boundary",
        OwnerRequirementOperation::SettledCleanup => "settled-cleanup",
        OwnerRequirementOperation::ContextRead => "context-read",
        OwnerRequirementOperation::ContextProvider => "context-provider",
    }
}
