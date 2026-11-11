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

/// How many project helpers deep a package function handed on as an argument is followed to the call that
/// runs it ([`WalkCtx::helper_calls`]): `run(useUi)` in `outer(f) { run(f) }` in `App` is two.
const HELPER_DEPTH: usize = 3;

/// How a project entry is found, before it becomes a requirement.
struct Entry {
    path: String,
    span: Span,
    /// Where the entry's reference is written, before it is anchored at its call or element
    /// (`span`): the reference itself for a reference, the same as `span` for the rest.
    reference: Span,
    kind: crate::DependencyEntry,
}

/// The owner graph's dense view, kept past the owner stage for the package entry table
/// ([`package_entry_table`]).
pub(crate) struct OwnerView {
    pub(super) nodes: Vec<OwnerNode>,
    pub(super) contexts: Vec<u8>,
    pub(super) outgoing: Vec<Vec<(usize, OwnerEdgeKind)>>,
    /// The census verdict of each context, for the references that name a context binding.
    pub(super) provider_free: HashMap<Location, ContextVerdict>,
}

/// Which project calls the walk takes as entries into a package function it reaches.
#[derive(Clone, Copy)]
enum CallMode<'w> {
    /// A call along an edge that may carry an owner, into a function the call walk
    /// (`attribute_dependency_requirements`) did not visit: that walk decided the rest,
    /// with the call's arguments, for a `context-provider` requirement.
    OwnedUnwalked(&'w HashSet<usize>),
    /// Every call that enters it (ADR 0949's table: any rule's finding runs wherever the
    /// function runs, owned or not).
    Every,
}

/// The facts the walk back from a package function reads, built once per pass.
struct WalkCtx<'a, 'f> {
    facts: &'a ProjectFacts,
    lookup: &'a SemanticLookup<'f>,
    nodes: &'a [OwnerNode],
    contexts: &'a [u8],
    incoming: Vec<Vec<(usize, OwnerEdgeKind)>>,
    nodes_by_path: HashMap<&'a str, Vec<usize>>,
    reach: crate::execution_role::SiteReach<'a, 'f>,
    /// Local alias tags (`const P2 = Panel; <P2/>`), by the function they render.
    alias_tags: std::cell::OnceCell<AliasTags>,
}

impl<'a, 'f> WalkCtx<'a, 'f> {
    fn new(
        facts: &'a ProjectFacts,
        lookup: &'a SemanticLookup<'f>,
        nodes: &'a [OwnerNode],
        contexts: &'a [u8],
        outgoing: &'a [Vec<(usize, OwnerEdgeKind)>],
    ) -> Self {
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
        Self {
            facts,
            lookup,
            nodes,
            contexts,
            incoming,
            nodes_by_path,
            reach: crate::execution_role::SiteReach::new(lookup),
            alias_tags: std::cell::OnceCell::new(),
        }
    }

    fn in_dependency(&self, path: &str) -> bool {
        self.facts.dependency_source_root(path).is_some()
    }

    /// The facts of the file at `path`: the one read of another file the passes below make by path.
    fn file(&self, path: &str) -> Option<&'a solid_facts::FileFacts> {
        self.lookup.file_by_path(path)
    }

    /// The innermost function whose body is written around `span`.
    fn node_at(&self, path: &str, span: Span) -> Option<usize> {
        self.nodes_by_path
            .get(path)
            .into_iter()
            .flatten()
            .copied()
            .filter(|index| self.nodes[*index].body.contains(span))
            .min_by_key(|index| self.nodes[*index].body.end - self.nodes[*index].body.start)
    }

    /// The node of the function at exactly `function`.
    fn seed_of(&self, path: &str, function: Span) -> Option<usize> {
        self.nodes_by_path
            .get(path)
            .into_iter()
            .flatten()
            .copied()
            .find(|index| self.nodes[*index].span == function)
    }

    /// A project function an owner edge reaches runs its entries. One no edge reaches
    /// (context zero) is still run by whatever the graph does not follow (a wrapper
    /// it was handed to, a timer, a handler), unless it is proven never to run:
    /// as `emit_file`'s `caller_unplaced`, an entry written in it stands. Only that
    /// proof silences an entry; a function or a file the facts do not name is none.
    fn may_run(&self, node: usize) -> bool {
        let never_runs = self
            .file(self.nodes[node].path.as_str())
            .and_then(|file| {
                file.ast
                    .function_at(self.nodes[node].span)
                    .map(|function| !self.reach.function_runs(file, function))
            })
            .is_some_and(|never| never);
        self.contexts[node] != 0 || !never_runs
    }

    /// Whether the reference at `entry` runs the function where it is written (a call, `new`, a tag,
    /// `f.call`), in a project function that holds an owner on every run: the owner an effect or a
    /// cleanup needs is current there. A value handed on (`setTimeout(f)`, `onClick={f}`, a holder's
    /// member) runs where something else runs it, whatever owner this function holds.
    fn runs_owned_here(&self, entry: &Entry) -> bool {
        let Some(file) = self.file(&entry.path) else {
            return false;
        };
        // Code after an `await`, or on a later `next()`, runs with no owner whatever the function was entered
        // under: an async function or a generator around the reference never holds an owner on every run.
        if file
            .ast
            .functions_owning(entry.reference)
            .any(|function| function.r#async || function.generator)
        {
            return false;
        }
        matches!(
            crate::local_access::hand_off_at(file, entry.reference, self.lookup),
            crate::local_access::HandOff::Call | crate::local_access::HandOff::Inline(_)
        ) && self
            .node_at(&entry.path, entry.reference)
            .is_some_and(|node| {
                let context = self.contexts[node];
                context != 0
                    && context
                        & (OWNER_CONTEXT_UNOWNED
                            | OWNER_CONTEXT_PROVEN_UNOWNED
                            | OWNER_CONTEXT_LEAVES_REGION)
                        == 0
            })
    }

    /// The project sites that enter the package function `seed`: a walk back through the
    /// package's own callers (every owner edge, the exact call and tag sites of the
    /// function, a nested function's enclosing function) to the project's calls and
    /// renders.
    fn entries_of(&self, seed: usize, calls: CallMode<'_>) -> Vec<Entry> {
        let mut found = Vec::new();
        let mut visited = HashSet::<usize>::default();
        let mut queue = VecDeque::from([seed]);
        visited.insert(seed);
        while let Some(node) = queue.pop_front() {
            let node_path = self.nodes[node].path.as_str();
            let node_span = self.nodes[node].span;
            // Every owner edge into the function, whatever its kind.
            for &(caller, kind) in &self.incoming[node] {
                if self.in_dependency(self.nodes[caller].path.as_str()) {
                    if visited.insert(caller) {
                        queue.push_back(caller);
                    }
                    continue;
                }
                let CallMode::OwnedUnwalked(walked) = calls else {
                    self.push_calls(&mut found, caller, node);
                    continue;
                };
                // A project function that calls it: the call walk decided every
                // function it visited, with the call's arguments.
                if walked.contains(&node) || !self.may_run(caller) {
                    continue;
                }
                let carries = owner_edge_context(
                    if kind == OwnerEdgeKind::MayRun {
                        OwnerEdgeKind::Preserve
                    } else {
                        kind
                    },
                    self.contexts[caller],
                ) & OWNER_CONTEXT_RUNS_OWNED
                    != 0;
                if carries {
                    self.push_calls(&mut found, caller, node);
                }
            }
            // The function a nested function is written in runs it, or hands it on.
            if let Some(parent) = self
                .nodes_by_path
                .get(node_path)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| {
                    *index != node
                        && self.nodes[*index].body.contains(node_span)
                        && self.nodes[*index].span != node_span
                })
                .min_by_key(|index| self.nodes[*index].body.end - self.nodes[*index].body.start)
                && self.in_dependency(self.nodes[parent].path.as_str())
                && visited.insert(parent)
            {
                queue.push_back(parent);
            }
            // The exact call and tag sites of the function.
            let mut tag_sites = self
                .lookup
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
            let aliases = self
                .alias_tags
                .get_or_init(|| alias_rendered_tags(self.facts, self.lookup));
            if let Some(more) = aliases.get(&(node_path.to_owned(), node_span)) {
                tag_sites.extend(more.iter().cloned());
            }
            for (path, site) in tag_sites {
                let caller = self.node_at(&path, site);
                if self.in_dependency(&path) {
                    if let Some(caller) = caller
                        && visited.insert(caller)
                    {
                        queue.push_back(caller);
                    }
                    continue;
                }
                if caller.is_some_and(|caller| !self.may_run(caller)) {
                    continue;
                }
                found.push(Entry {
                    path,
                    span: site,
                    reference: site,
                    kind: crate::DependencyEntry::Render,
                });
            }
        }
        found
    }

    /// The calls written directly in the project function `caller` that enter `node`.
    fn push_calls(&self, found: &mut Vec<Entry>, caller: usize, node: usize) {
        if !self.may_run(caller) {
            return;
        }
        let Some(caller_file) = self.file(self.nodes[caller].path.as_str()) else {
            return;
        };
        for call in &caller_file.ast.calls {
            if crate::owners::containing_body_function(&caller_file.ast, call.span)
                .is_some_and(|function| function.span == self.nodes[caller].span)
                && call_enters(self.lookup, caller_file, call, &self.nodes[node])
            {
                found.push(Entry {
                    path: caller_file.path.to_string(),
                    span: call.span,
                    reference: call.span,
                    kind: crate::DependencyEntry::Call,
                });
            }
        }
    }

    /// ADR 0949, the calls inside a project helper that run a package function the project hands it:
    /// `run(useUi)` with `function run(f) { return f(); }` runs the package function at `f()`, where a run's own
    /// stack frame stands (the Chrome site of `pk-hook-via-project-helper`). `reference` is the written reference
    /// to the package function in `file`; it must be a whole argument of a call the call graph resolves exactly to
    /// a project function, whose parameter there is a plain identifier nothing assigns. Each call of that parameter
    /// written directly in the helper is an exact entry ([`crate::DependencyEntry::Call`]); the parameter handed on
    /// whole to another such helper is followed `depth` more times. A helper that stores the parameter, returns it, or
    /// passes it on in a shape this does not read adds no entry: the reference's own site stands for it.
    fn helper_calls(
        &self,
        file: &solid_facts::FileFacts,
        reference: Span,
        depth: usize,
        found: &mut Vec<Entry>,
    ) {
        if depth == 0 {
            return;
        }
        let ast = &file.ast;
        let Some((call, index)) = ast.arguments_containing(reference).find(|(call, index)| {
            let argument = &call.arguments[*index];
            !argument.spread && ast.peel_ts_sugar_span(argument.span) == reference
        }) else {
            return;
        };
        let Some((helper_file, helper, exact)) =
            crate::execution_role::project_callee_at_parameter(file, call, index, self.lookup)
        else {
            return;
        };
        if !exact || self.in_dependency(helper_file.path.as_str()) {
            return;
        }
        let Some(parameter) = helper.parameters.get(index) else {
            return;
        };
        let Some(name) = parameter
            .names
            .first()
            .filter(|_| parameter.shape == solid_facts::ast::BindingShape::Identifier)
        else {
            return;
        };
        if crate::indexes::binding_written(helper_file, name.span) {
            return;
        }
        let Some(symbol) = self
            .lookup
            .entities()
            .at(helper_file.path.as_str(), name.span)
        else {
            return;
        };
        let helper_path = helper_file.path.as_str();
        // A helper no execution reaches runs none of its calls (`may_run`: only a proof of that says no).
        if self
            .seed_of(helper_path, helper.span)
            .is_some_and(|node| !self.may_run(node))
        {
            return;
        }
        for inner in &helper_file.ast.calls {
            if crate::owners::owned_by_body(&helper_file.ast, helper, inner.span)
                && self.lookup.callee_symbol(helper_file, inner.callee) == Some(symbol.as_str())
            {
                found.push(Entry {
                    path: helper_path.to_owned(),
                    span: inner.span,
                    reference: inner.callee,
                    kind: crate::DependencyEntry::Call,
                });
            }
        }
        // The parameter handed on whole, in the helper's own body, to another helper.
        for (written, declaration) in &helper_file.ast.reference_declarations {
            if *declaration == name.span
                && crate::owners::written_directly_in(&helper_file.ast, helper, *written)
            {
                self.helper_calls(helper_file, *written, depth - 1, found);
            }
        }
    }

    /// The project references to a symbol a package of `roots` declares that no call or render
    /// resolves, with the root each belongs to.
    fn references_to(
        &self,
        roots: &[&str],
        provider_free: &HashMap<Location, ContextVerdict>,
    ) -> Vec<(String, Entry)> {
        let mut found = Vec::new();
        for symbol in self.facts.typescript.symbols() {
            let Some(root) = symbol.declarations().iter().find_map(|declaration| {
                roots.iter().copied().find(|root| {
                    std::path::Path::new(declaration.location.path.as_ref()).starts_with(root)
                })
            }) else {
                continue;
            };
            for reference in symbol.references() {
                let path = reference.path.as_ref();
                if self.in_dependency(path) {
                    continue;
                }
                let (Ok(start), Ok(end)) = (
                    u32::try_from(reference.start_byte),
                    u32::try_from(reference.end_byte),
                ) else {
                    continue;
                };
                let Some(file) = self.file(path) else {
                    continue;
                };
                let span = Span::new(start, end);
                if reference_is_not_an_entry(self.lookup, file, span, provider_free, true) {
                    continue;
                }
                if self
                    .node_at(path, span)
                    .is_some_and(|node| !self.may_run(node))
                {
                    continue;
                }
                found.push((
                    root.to_owned(),
                    Entry {
                        path: path.to_owned(),
                        span: entry_anchor(file, span),
                        reference: span,
                        kind: crate::DependencyEntry::Reference,
                    },
                ));
                // Handed whole to a project helper that calls it: the calls there enter it too.
                let mut helper_entries = Vec::new();
                self.helper_calls(file, span, HELPER_DEPTH, &mut helper_entries);
                found.extend(
                    helper_entries
                        .into_iter()
                        .map(|entry| (root.to_owned(), entry)),
                );
            }
        }
        found
    }
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
    owner_needing: &[PackageRequirement],
) {
    if (reads.is_empty() && owner_needing.is_empty()) || facts.dependency_source_roots.is_empty() {
        return;
    }
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
    let walk = WalkCtx::new(facts, lookup, nodes, contexts, outgoing);
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
    let mut first_read_of_root = HashMap::<String, &PackageRead>::default();
    for read in &reads {
        if let Some(root) = facts.dependency_source_root(read.location.path.as_ref()) {
            first_read_of_root.entry(root.to_owned()).or_insert(read);
        }
    }

    // 1 and 2: the walk back from each read.
    let mut sites: Vec<(Entry, &PackageRead)> = Vec::new();
    for read in &reads {
        let Some(seed) = walk.seed_of(read.location.path.as_ref(), read.function) else {
            continue;
        };
        sites.extend(
            walk.entries_of(seed, CallMode::OwnedUnwalked(walked))
                .into_iter()
                .map(|entry| (entry, read)),
        );
    }
    // 3: references to a symbol a package declares, no call or render resolves.
    let mut roots = first_read_of_root
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    roots.sort_unstable();
    sites.extend(
        walk.references_to(&roots, provider_free)
            .into_iter()
            .map(|(root, entry)| (entry, first_read_of_root[root.as_str()])),
    );

    // Operations that need an owner, in a package function no owner edge reaches (ADR 0949): the
    // call walk follows owner edges, and none reaches it, so no project site was asked. A render
    // runs the component under an owner of its own; a call or a reference may run it with none.
    let mut owner_sites: Vec<(Entry, &PackageRequirement)> = Vec::new();
    let mut first_of_root = std::collections::BTreeMap::<String, &PackageRequirement>::new();
    for seed in owner_needing {
        let Some(node) = walk.seed_of(seed.location.path.as_ref(), seed.function) else {
            continue;
        };
        if let Some(root) = facts.dependency_source_root(seed.location.path.as_ref()) {
            first_of_root.entry(root.to_owned()).or_insert(seed);
        }
        owner_sites.extend(
            walk.entries_of(node, CallMode::Every)
                .into_iter()
                .filter(|entry| entry.kind != crate::DependencyEntry::Render)
                .map(|entry| (entry, seed)),
        );
    }
    let owner_roots = first_of_root.keys().map(String::as_str).collect::<Vec<_>>();
    owner_sites.extend(
        walk.references_to(&owner_roots, provider_free)
            .into_iter()
            .filter(|(_, entry)| !walk.runs_owned_here(entry))
            .map(|(root, entry)| (entry, first_of_root[root.as_str()])),
    );
    owner_sites.sort_by(|(left, _), (right, _)| {
        (&left.path, left.span.start, left.span.end).cmp(&(
            &right.path,
            right.span.start,
            right.span.end,
        ))
    });
    for (entry, seed) in owner_sites {
        let Some(file) = walk.file(&entry.path) else {
            continue;
        };
        let before = requirements.len();
        push_owner_requirement(
            requirements,
            &mut seen,
            seed.operation,
            file,
            entry.span,
            OwnerRequirementStatus {
                flight_error_unobserved: false,
                uncertain: true,
                runtime_uncertain: false,
                caller_uncertain: false,
                // Whoever enters the function from here is not followed: no owner is proven current.
                unplaced_callback: true,
                conditional_owner: false,
                owner_varies_by_caller: false,
                later_run_unowned: false,
                component_uncertain: false,
                provider_open: false,
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
            pushed.through_dependency = Some(seed.location.clone());
            pushed.through_dependency_entry = entry.kind;
        }
    }

    // The requirements, in path and span order.
    sites.sort_by(|(left, _), (right, _)| {
        (&left.path, left.span.start, left.span.end).cmp(&(
            &right.path,
            right.span.start,
            right.span.end,
        ))
    });
    for (entry, read) in sites {
        let Some(file) = walk.file(&entry.path) else {
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

/// ADR 0949, the one table the ADR 0284 drop reads: for every package function that holds a row
/// of the final program, the project sites that may run it (`PackageEntries`), and per package the
/// project references no exact call or tag decides (`PackageReferences`). A finding the drop pass
/// discards in a package is reported at those sites, never silent.
pub(crate) fn package_entry_table(
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    view: &OwnerView,
    program: &crate::Program,
    mentions: &PackageMentions,
) -> (Vec<crate::PackageEntries>, Vec<crate::PackageReferences>) {
    // Every row of the program located in a package.
    let mut rows = Vec::<&Location>::new();
    rows.extend(program.reads.iter().map(|row| &row.location));
    rows.extend(program.writes.iter().map(|row| &row.location));
    rows.extend(program.actions.iter().map(|row| &row.location));
    rows.extend(program.leaf_operations.iter().map(|row| &row.location));
    rows.extend(program.static_violations.iter().map(|row| &row.location));
    rows.extend(program.static_defects.iter().map(|row| &row.location));
    rows.extend(program.directive_creations.iter().map(|row| &row.location));
    rows.extend(program.missing_owners.iter().map(|row| &row.location));
    rows.extend(program.async_reads.iter().map(|row| &row.location));
    rows.retain(|location| {
        facts
            .dependency_source_root(location.path.as_ref())
            .is_some()
    });
    if rows.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let walk = WalkCtx::new(facts, lookup, &view.nodes, &view.contexts, &view.outgoing);
    let mut functions = std::collections::BTreeMap::<(String, u32, u32), usize>::new();
    let mut roots = std::collections::BTreeSet::<String>::new();
    for location in rows {
        if let Some(root) = facts.dependency_source_root(location.path.as_ref()) {
            roots.insert(root.to_owned());
        }
        let (Ok(start), Ok(end)) = (
            u32::try_from(location.start_byte),
            u32::try_from(location.end_byte),
        ) else {
            continue;
        };
        if let Some(node) = walk.node_at(location.path.as_ref(), Span::new(start, end)) {
            let span = view.nodes[node].span;
            functions
                .entry((location.path.to_string(), span.start, span.end))
                .or_insert(node);
        }
    }
    let site = |entry: Entry| crate::PackageEntrySite {
        location: crate::location(
            walk.file(&entry.path)
                .map_or_else(|| entry.path.as_str().into(), |file| file.path.shared()),
            entry.span,
        ),
        kind: entry.kind,
    };
    let mut entries = Vec::new();
    for ((path, start, end), node) in functions {
        let mut sites = walk
            .entries_of(node, CallMode::Every)
            .into_iter()
            .map(site)
            .collect::<Vec<_>>();
        sites.sort_by(|left, right| crate::location_order(&left.location, &right.location));
        sites.dedup_by(|left, right| left.location == right.location);
        entries.push(crate::PackageEntries {
            function: crate::location(path.as_str(), Span::new(start, end)),
            sites,
        });
    }
    let root_list = roots.iter().map(String::as_str).collect::<Vec<_>>();
    let mut by_root = std::collections::BTreeMap::<String, Vec<crate::PackageEntrySite>>::new();
    for (root, entry) in walk.references_to(&root_list, &view.provider_free) {
        by_root.entry(root).or_default().push(site(entry));
    }
    // A use of a binding that carries a package (`const Lazy = dynamic(() => Panel)`; `<Lazy />`) runs its code
    // wherever it is used: a site of every finding of the package it mentions.
    for (path, carriers) in &mentions.carriers {
        let Some(file) = walk.file(path) else {
            continue;
        };
        for (at, declaration) in &file.ast.reference_declarations {
            let Some(carried) = carriers.get(declaration) else {
                continue;
            };
            if reference_is_not_an_entry(lookup, file, *at, &view.provider_free, false)
                || walk
                    .node_at(path, *at)
                    .is_some_and(|node| !walk.may_run(node))
            {
                continue;
            }
            // The tag of an element is entered at the element's opening tag (where a render's frame stands), any
            // other use at the call or element it is an argument or attribute of.
            let anchor = file
                .ast
                .jsx_elements
                .iter()
                .find(|element| element.name.span.contains(*at))
                .map_or_else(|| entry_anchor(file, *at), element_site);
            for &root in carried {
                by_root
                    .entry(mentions.roots[root].clone())
                    .or_default()
                    .push(site(Entry {
                        path: path.clone(),
                        span: anchor,
                        reference: *at,
                        kind: crate::DependencyEntry::Render,
                    }));
            }
        }
    }
    let references = by_root
        .into_iter()
        .map(|(root, mut sites)| {
            sites.sort_by(|left, right| crate::location_order(&left.location, &right.location));
            sites.dedup_by(|left, right| left.location == right.location);
            crate::PackageReferences { root, sites }
        })
        .collect();
    (entries, references)
}

/// Where a project mentions a package analyzed as source (ADR 0949, addendum 4): the references to the symbols a
/// package declares, and the project bindings that carry one (`const Lazy = dynamic(() => Panel)`: using `Lazy` runs
/// `Panel`). A site that mentions a package may run its code, by a path no call walk follows.
pub(crate) struct PackageMentions {
    /// The package roots, by index.
    roots: Vec<String>,
    /// Per project path, the spans of the references to a symbol a package declares, with the package's root, sorted.
    refs: HashMap<String, Vec<(Span, usize)>>,
    /// Per project path, the declaration (name) span of each immutable binding whose initializer mentions a package,
    /// directly or through another such binding, with the roots it mentions.
    carriers: HashMap<String, HashMap<Span, Vec<usize>>>,
}

impl PackageMentions {
    pub(crate) fn build(facts: &ProjectFacts) -> Self {
        let roots = facts.dependency_source_roots.clone();
        let root_index: HashMap<&str, usize> = roots
            .iter()
            .enumerate()
            .map(|(index, root)| (root.as_str(), index))
            .collect();
        // The root a declaration's path is under: its longest directory prefix that is a root.
        let mut by_path = HashMap::<&str, Option<usize>>::default();
        let mut refs = HashMap::<String, Vec<(Span, usize)>>::default();
        for symbol in facts.typescript.symbols() {
            let Some(root) = symbol.declarations().iter().find_map(|declaration| {
                *by_path
                    .entry(declaration.location.path.as_ref())
                    .or_insert_with(|| {
                        let mut prefix = declaration.location.path.as_ref();
                        loop {
                            if let Some(index) = root_index.get(prefix) {
                                return Some(*index);
                            }
                            prefix = prefix.rsplit_once('/')?.0;
                        }
                    })
            }) else {
                continue;
            };
            for reference in symbol.references() {
                if facts
                    .dependency_source_root(reference.path.as_ref())
                    .is_some()
                {
                    continue;
                }
                let (Ok(start), Ok(end)) = (
                    u32::try_from(reference.start_byte),
                    u32::try_from(reference.end_byte),
                ) else {
                    continue;
                };
                refs.entry(reference.path.to_string())
                    .or_default()
                    .push((Span::new(start, end), root));
            }
        }
        for list in refs.values_mut() {
            list.sort_unstable();
            list.dedup();
        }
        let mut mentions = Self {
            roots,
            refs,
            carriers: HashMap::default(),
        };
        // A binding whose initializer mentions a package carries it: three rounds follow `const B = A`.
        for _ in 0..3 {
            let mut found = Vec::<(String, Span, Vec<usize>)>::new();
            for file in crate::base::reads::all_files(facts) {
                if facts.dependency_source_root(file.path.as_str()).is_some() {
                    continue;
                }
                for binding in &file.ast.bindings {
                    if !binding.immutable
                        || binding.shape != solid_facts::ast::BindingShape::Identifier
                    {
                        continue;
                    }
                    let (Some(name), Some(initializer)) =
                        (binding.names.first(), binding.initializer)
                    else {
                        continue;
                    };
                    let roots = mentions.roots_in(file, initializer);
                    if !roots.is_empty()
                        && mentions
                            .carriers
                            .get(file.path.as_str())
                            .is_none_or(|known| known.get(&name.span) != Some(&roots))
                    {
                        found.push((file.path.to_string(), name.span, roots));
                    }
                }
            }
            if found.is_empty() {
                break;
            }
            for (path, name, roots) in found {
                mentions
                    .carriers
                    .entry(path)
                    .or_default()
                    .insert(name, roots);
            }
        }
        mentions
    }

    /// The package roots (indexes) mentioned within `span` of `file`: a reference to a symbol a package declares, or a
    /// use of a binding that carries one.
    fn roots_in(&self, file: &solid_facts::FileFacts, span: Span) -> Vec<usize> {
        let path = file.path.as_str();
        let mut found = Vec::new();
        if let Some(list) = self.refs.get(path) {
            let from = list.partition_point(|(at, _)| at.start < span.start);
            for (at, root) in &list[from..] {
                if at.start >= span.end {
                    break;
                }
                if span.contains(*at) {
                    found.push(*root);
                }
            }
        }
        if let Some(carriers) = self.carriers.get(path) {
            let uses = &file.ast.reference_declarations;
            let from = uses.partition_point(|(at, _)| at.start < span.start);
            for (at, declaration) in &uses[from..] {
                if at.start >= span.end {
                    break;
                }
                if span.contains(*at)
                    && let Some(roots) = carriers.get(declaration)
                {
                    found.extend(roots.iter().copied());
                }
            }
        }
        found.sort_unstable();
        found.dedup();
        found
    }
}

/// The reactive capabilities of the program, by symbol: what a project can hand to code the checker does not follow.
pub(crate) struct Capabilities<'a> {
    pub(crate) setters: &'a HashMap<
        SymbolId,
        (
            SymbolId,
            Location,
            crate::OwnedWriteOption,
            crate::ReactiveSourceKind,
        ),
    >,
    pub(crate) accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) actions: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) source_kinds: &'a HashMap<SymbolId, crate::ReactiveSourceKind>,
    pub(crate) async_sources: &'a HashSet<SymbolId>,
}

/// What a value is that a project hands to code the checker does not follow.
#[derive(Clone, Copy)]
enum HandedValue {
    Setter,
    Accessor { asynchronous: bool },
    Store,
    Action,
}

impl Capabilities<'_> {
    fn of(&self, symbol: &SymbolId) -> Option<HandedValue> {
        if self.setters.contains_key(symbol) {
            Some(HandedValue::Setter)
        } else if self.actions.contains_key(symbol) {
            Some(HandedValue::Action)
        } else if self.source_kinds.get(symbol) == Some(&crate::ReactiveSourceKind::Store)
            || self.accessors.get(symbol).is_some_and(|(source, _)| {
                self.source_kinds.get(source) == Some(&crate::ReactiveSourceKind::Store)
            })
        {
            // A store's root is read like an accessor and written through its properties.
            Some(HandedValue::Store)
        } else {
            self.accessors
                .get(symbol)
                .map(|(source, _)| HandedValue::Accessor {
                    asynchronous: self.async_sources.contains(symbol)
                        || self.async_sources.contains(source),
                })
        }
    }
}

/// The reactive values a project hands code in a package in any shape (ADR 0949, addendum 4).
///
/// A package analyzed as source is entered by a project call, `new`, render or tagged template, or run by a project
/// value that carries one (`dynamic(() => Panel)`, `createComponent(Panel, props)`, `Reflect.apply(fn, null, [set])`).
/// A call walk follows what the call hands only where it resolves the callee to a function and reads the argument; a
/// package's constructor, its returned object's getters, a hop through a primitive, a carrier, `Reflect.apply`, a
/// JSX prop and any value inside a larger one are not followed. The package may call, write or read what it is
/// handed, and where that happens in an owned scope Solid's dev build says so. Every site that mentions a package
/// (its callee, tag or tag function, or any argument, names a symbol a package declares or a binding that carries one)
/// and hands an exact capability in an argument, a JSX attribute or a template substitution is an uncertifiable row
/// of each rule that claims the runtime code the capability's use throws: a setter, `REACTIVE_WRITE_IN_OWNED_SCOPE`
/// (SC2001); an accessor, `STRICT_READ_UNTRACKED` (SC1001); a store, `STORE_WRITE_OUTSIDE_SETTER` (SC2003) and SC1001; an action, `ACTION_CALLED_IN_OWNED_SCOPE`
/// (SC2002). Never a violation: whether the package uses it, and where, is not proven here; a violation the call walk
/// proves at the same site stands beside it.
///
/// Only references that are values count: a callee (`setN(1)` is a call: the project's own read or write) and a
/// reference inside a function written in the value (that function's own site, judged by its role) are not handed.
/// Children (`<Pkg>{setN}</Pkg>`) are not read: a limit (docs/open-work.md).
pub(crate) fn package_handed_value_claims(
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    view: &OwnerView,
    mentions: &PackageMentions,
    capabilities: &Capabilities<'_>,
) -> Vec<crate::StaticViolation> {
    if mentions.refs.is_empty() && mentions.carriers.is_empty() {
        return Vec::new();
    }
    let walk = WalkCtx::new(facts, lookup, &view.nodes, &view.contexts, &view.outgoing);
    let mut claims = Vec::<crate::StaticViolation>::new();
    for file in crate::base::reads::all_files(facts) {
        if facts.dependency_source_root(file.path.as_str()).is_some()
            || !(mentions.refs.contains_key(file.path.as_str())
                || mentions.carriers.contains_key(file.path.as_str()))
        {
            continue;
        }
        let ast = &file.ast;
        let mut hand = |at: Span, callee: Span, values: &[Span]| {
            // A site no execution reaches hands nothing (`may_run`: only a proof of that says no).
            if walk
                .node_at(file.path.as_str(), at)
                .is_some_and(|node| !walk.may_run(node))
            {
                return;
            }
            if mentions.roots_in(file, callee).is_empty()
                && values
                    .iter()
                    .all(|value| mentions.roots_in(file, *value).is_empty())
            {
                return;
            }
            for &value in values {
                for (reference, declaration, shorthand) in value_references(ast, value) {
                    // The reference's own entity where the facts demanded one (an argument, a property
                    // value), else the entity of the declaration it names (`[setN]`, `c ? setN : off`);
                    // a shorthand's is the declaration's alone.
                    let own = if shorthand {
                        None
                    } else {
                        lookup.entities().at(file.path.as_str(), reference)
                    };
                    let Some(kind) = own
                        .or_else(|| lookup.entities().at(file.path.as_str(), declaration))
                        .and_then(|symbol| capabilities.of(symbol))
                    else {
                        continue;
                    };
                    claims.extend(handed_value_claims(file, at, kind));
                }
            }
        };
        for element in &ast.jsx_elements {
            let values = element
                .attributes
                .iter()
                .filter_map(|attribute| attribute.expression)
                .chain(element.spreads.iter().map(|spread| spread.argument))
                .collect::<Vec<_>>();
            hand(element_site(element), element.name.span, &values);
        }
        for call in &ast.calls {
            let values = call
                .arguments
                .iter()
                .map(|argument| argument.span)
                .collect::<Vec<_>>();
            hand(call.span, call.callee, &values);
        }
        for template in &ast.tagged_templates {
            hand(template.span, template.tag, &template.substitutions);
        }
    }
    claims.sort_by(|left, right| {
        crate::location_order(&left.location, &right.location).then_with(|| left.id.cmp(&right.id))
    });
    claims.dedup_by(|left, right| left.location == right.location && left.id == right.id);
    claims
}

/// The references inside `value` that are values the project hands on: not a callee (`setN(1)` is a call, and a
/// read or a write at the project's own site) and not inside a function written in the value (that function's own
/// site, judged by its role).
fn value_references(
    ast: &solid_facts::ast::AstFacts,
    value: Span,
) -> impl Iterator<Item = (Span, Span, bool)> + '_ {
    let around = crate::owners::containing_ast_function(ast, value).map(|function| function.span);
    let from = ast
        .reference_declarations
        .partition_point(|(reference, _)| reference.start < value.start);
    // A shorthand property (`{ setN }`) is no entry of `reference_declarations`: its binding is recorded on it, and the
    // symbol the facts answer at its span is the property's, never the value binding's (the flag).
    let shorthands = ast.object_properties.iter().filter_map(|property| {
        property
            .shorthand_binding
            .map(|declaration| (property.value, declaration, true))
    });
    ast.reference_declarations[from..]
        .iter()
        .take_while(move |(reference, _)| reference.start < value.end)
        .map(|(reference, declaration)| (*reference, *declaration, false))
        .chain(shorthands)
        .filter(move |(reference, _, _)| {
            value.contains(*reference)
                && ast.calls_with_peeled_callee(*reference).next().is_none()
                && crate::owners::containing_ast_function(ast, *reference)
                    .map(|function| function.span)
                    == around
        })
}

/// The uncertifiable rows, one per rule that claims the runtime code the use of `kind` throws, at the project site
/// `at` that hands it to code in a package ([`package_handed_value_claims`]).
fn handed_value_claims(
    file: &solid_facts::FileFacts,
    at: Span,
    kind: HandedValue,
) -> Vec<crate::StaticViolation> {
    const SETTER: (&str, &str, &str, &str) = (
        "SC2001",
        "no-write-in-pure-computation",
        "a signal's setter",
        "a write in an owned scope (a component body, a computation) throws `REACTIVE_WRITE_IN_OWNED_SCOPE` in the dev build",
    );
    const ACCESSOR: (&str, &str, &str, &str) = (
        "SC1001",
        "strict-read-untracked",
        "a reactive accessor",
        "a read where nothing tracks it (a component body, an owned setup) logs `STRICT_READ_UNTRACKED` in the dev build",
    );
    const STORE_WRITE: (&str, &str, &str, &str) = (
        "SC2003",
        "no-store-mutation-outside-setter",
        "a store",
        "a write through the store outside its setter logs `STORE_WRITE_OUTSIDE_SETTER` in the dev build",
    );
    const STORE_READ: (&str, &str, &str, &str) = (
        "SC1001",
        "strict-read-untracked",
        "a store",
        "a read where nothing tracks it (a component body, an owned setup) logs `STRICT_READ_UNTRACKED` in the dev build",
    );
    const ACTION: (&str, &str, &str, &str) = (
        "SC2002",
        "action-called-in-owned-scope",
        "an action",
        "a call in an owned scope throws `ACTION_CALLED_IN_OWNED_SCOPE` in the dev build",
    );
    let (rules, blocker): (&[(&str, &str, &str, &str)], &str) = match kind {
        HandedValue::Setter => (&[SETTER], crate::blockers::SETTER_HANDED_TO_PACKAGE),
        HandedValue::Accessor {
            asynchronous: false,
        } => (&[ACCESSOR], crate::blockers::ACCESSOR_HANDED_TO_PACKAGE),
        // An async source's untracked read is SC5001's (`PENDING_ASYNC_UNTRACKED_READ`), a rule the dialect emits only
        // through the async-read channel, never as a static violation (`solve` panics on one): not claimed here yet
        // (docs/open-work.md); the accessor's own claim stands.
        HandedValue::Accessor { asynchronous: true } => {
            (&[ACCESSOR], crate::blockers::ACCESSOR_HANDED_TO_PACKAGE)
        }
        HandedValue::Store => (
            &[STORE_WRITE, STORE_READ],
            crate::blockers::STORE_HANDED_TO_PACKAGE,
        ),
        HandedValue::Action => (&[ACTION], crate::blockers::ACTION_HANDED_TO_PACKAGE),
    };
    rules
        .iter()
        .map(|(id, rule, value, throws)| crate::StaticViolation {
            id: (*id).into(),
            rule: (*rule).into(),
            message: format!(
                "this hands {value} to code in a package analyzed as source, by a path no walk of the project \
                 follows (an argument or attribute of a call, `new`, render or tagged template that names the \
                 package, or of a carrier of it); the package may use it, and {throws}; whether it does, and \
                 whether that runs in such a scope, cannot be proven here"
            ),
            hint: "Use it in the project's own code where it is legal (an event handler, an effect's apply \
                   function, a tracking scope), and hand the package a value or a callback that does."
                .into(),
            location: crate::location(file.path.shared(), at),
            analysis_context: blocker.to_owned(),
            fixes: Vec::new(),
            uncertain: true,
        })
        .collect()
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
    exact_decides: bool,
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
    // The tag of an element that resolves exactly: sites 1 decides. A use of a carrier (`const Lazy = dynamic(() => Panel)`)
    // may resolve exactly through the primitive's wrapper too, and stands wherever it is used (`exact_decides` false).
    if exact_decides
        && ast.jsx_elements.iter().any(|element| {
            (element.name.span.contains(span)
                || element.closing_name.is_some_and(|c| c.contains(span)))
                && (lookup
                    .function_called_at(file.path.as_str(), element.name.span)
                    .is_some()
                    || lookup
                        .alias_rendered_tag_function(file, element.name.span)
                        .is_some())
        })
    {
        return true;
    }
    // The callee of a call that resolves exactly: sites 2 and the call walk.
    if exact_decides
        && ast.calls.iter().any(|call| {
            ast.peel_ts_sugar_span(call.callee).contains(span)
                && crate::execution_role::project_callee_at_parameter(file, call, 0, lookup)
                    .is_some_and(|(_, _, exact)| exact)
        })
    {
        return true;
    }
    // A context binding: a read or a provider names it, and neither runs package code.
    context_key_at(lookup, file, span).is_some_and(|key| provider_free.contains_key(&key))
}
