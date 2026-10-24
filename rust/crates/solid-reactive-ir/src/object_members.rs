//! ADR 0299: exact calls through immutable, source-owned object bindings.
//! Export/import is a channel into the same receiver census, not an escape
//! waiver. No structural signature or composite entity supplies a target.

use crate::{HashMap, HashSet};

use solid_facts::{FileFacts, ProjectFacts, ast, core::Span};
use typefacts::Location;

use crate::{
    indexes::{SemanticLookup, binding_written},
    location,
};

#[path = "object_members/handlers.rs"]
mod handlers;
pub(crate) use handlers::{SpreadTargets, intrinsic_element, unresolved_handler_defects};
#[path = "object_members/assigned.rs"]
mod assigned;
pub(crate) use assigned::{AssignedMembersAnswer, PrototypeOwnerAt};
#[path = "object_members/memo.rs"]
mod memo;
pub(crate) use memo::{LiteralReceivers, ObjectMemberTargets, ReceiverClosed, SpreadTargetsAt};

pub(crate) type Targets = HashMap<Location, (String, Span)>;

/// The target of every call and event-handler member read of the project:
/// the per-slot [`ObjectMemberTargets`] answers, assembled in file order (a
/// later slot of a path overwrites an earlier one).
pub(crate) fn index(lookup: &SemanticLookup<'_>) -> Targets {
    let world = lookup.memo_world();
    let mut targets = HashMap::default();
    for (slot, file) in lookup.files().iter().enumerate() {
        let key = lookup.memo_slot_of(slot);
        for (read, path, function) in world.answer::<ObjectMemberTargets>(&key) {
            targets.insert(
                location(file.path.shared(), read),
                (path.to_string(), function),
            );
        }
    }
    targets
}

/// What every intrinsic element's spread argument supplies, assembled from
/// the per-slot [`SpreadTargetsAt`] answers (ADR 0358).
pub(crate) fn spread_index(lookup: &SemanticLookup<'_>) -> SpreadTargets {
    let world = lookup.memo_world();
    let mut spreads = HashMap::default();
    for (slot, file) in lookup.files().iter().enumerate() {
        let key = lookup.memo_slot_of(slot);
        for (argument, targets) in world.answer::<SpreadTargetsAt>(&key) {
            spreads.insert(
                location(file.path.shared(), argument),
                targets.map(|targets| {
                    targets
                        .into_iter()
                        .map(|(path, span)| (path.to_string(), span))
                        .collect()
                }),
            );
        }
    }
    spreads
}

fn references(file: &FileFacts) -> HashSet<Span> {
    file.ast
        .identifiers
        .iter()
        .filter(|id| id.role == ast::IdentifierRole::Reference)
        .map(|id| id.span)
        .collect()
}

/// The members whose receiver is bound to an exact object literal.
pub(crate) struct LiteralReceiverCensus {
    /// Every one, as `(path, member span)`.
    pub(crate) all: HashSet<(String, Span)>,
    /// The ones whose literal is declared in another file, sorted.
    pub(crate) across_files: Vec<(String, Span)>,
}

/// Every member expression whose receiver is an identifier bound to an exact
/// object literal, as `(path, member span)`: the per-slot
/// [`LiteralReceivers`] answers. The predicate below is asked for every call
/// by several passes.
pub(crate) fn literal_receiver_members(lookup: &SemanticLookup<'_>) -> LiteralReceiverCensus {
    let world = lookup.memo_world();
    let mut found = HashSet::default();
    let mut across_files = Vec::new();
    for (slot, file) in lookup.files().iter().enumerate() {
        let key = lookup.memo_slot_of(slot);
        for (span, across) in world.answer::<LiteralReceivers>(&key) {
            found.insert((file.path.to_string(), span));
            if across {
                across_files.push((file.path.to_string(), span));
            }
        }
    }
    across_files.sort_unstable();
    LiteralReceiverCensus {
        all: found,
        across_files,
    }
}

/// The older accessor-transport route must not bypass this receiver census.
/// This predicate only claims jurisdiction over an exact literal binding;
/// it does not decide that the receiver or any of its members is callable.
pub(crate) fn literal_receiver_call(
    file: &FileFacts,
    callee: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let callee = file.ast.peel_ts_sugar_span(callee);
    lookup
        .literal_receiver_members()
        .contains(&(file.path.to_string(), callee))
}

pub(crate) fn call_withheld(file: &FileFacts, callee: Span, lookup: &SemanticLookup<'_>) -> bool {
    literal_receiver_call(file, callee, lookup)
        && !lookup
            .object_member_targets()
            .contains_key(&location(file.path.shared(), callee))
}

/// `symbol` and the symbols runtime redirects tie to it, transitively.
pub(crate) fn receiver_symbols(facts: &ProjectFacts, symbol: &str) -> HashSet<String> {
    let mut symbols = HashSet::from_iter([symbol.to_owned()]);
    loop {
        let mut changed = false;
        for (declaration, runtime) in &facts.runtime_symbol_redirects {
            if symbols.contains(declaration) || symbols.contains(runtime) {
                changed |= symbols.insert(declaration.clone());
                changed |= symbols.insert(runtime.clone());
            }
        }
        if !changed {
            return symbols;
        }
    }
}

/// ADR 0335: the analyzed modules some file reaches as a namespace (a
/// namespace import, a namespace re-export, a dynamic load or `import =`,
/// through barrels), or `None` when a load or module hazard leaves that
/// unknown: what a namespace reaches, no identifier reference enumerates.
pub(crate) fn namespace_exposed_modules(lookup: &SemanticLookup<'_>) -> Option<HashSet<String>> {
    let census = lookup.module_census();
    (!census.unknown_load).then(|| census.exposed.clone())
}

/// Where one specifier loads.
enum Landing {
    /// Analyzed project modules (and the runtime modules a declaration
    /// redirects to).
    Modules(HashSet<String>),
    /// A real installed package (ADR 0339's premise).
    Installed,
    /// Code no analyzed file carries: an unresolved or ambient module, a
    /// declaration file with no runtime redirect, a workspace link.
    Unknown,
}

/// Namespace bindings and module loads are separate export channels: their
/// references need not contain a receiver identifier at all. Until those uses
/// can be censused exactly, refuse the entire exposed module (and its barrels).
#[derive(Clone, Default)]
pub(crate) struct ModuleCensus {
    exposed: HashSet<String>,
    cyclic: HashSet<String>,
    unknown_load: bool,
    /// Static import and re-export edges between analyzed modules.
    graph: HashMap<String, HashSet<String>>,
    /// The edges ES evaluation runs before the importer's own body: static
    /// imports and `export … from`.
    hoisted: HashMap<String, HashSet<String>>,
    /// `import x = require(…)` statements, which sit at module level (never
    /// inside a block, loop, branch or `try`) and run where they are written:
    /// the targets and where the statement ends. A `require()` call is not
    /// here: it may be skipped by the code around it (ADR 0339).
    equals_loads: HashMap<String, Vec<(HashSet<String>, u32)>>,
    /// Loads that bind a name to a module at the point they run, by module:
    /// the targets, where an `import x = require(…)` statement ends (`None`
    /// for a `require()` call or a dynamic `import()` at module level, which
    /// are never credited).
    alias_loads: HashMap<String, Vec<AliasLoad>>,
    /// Modules with a load whose targets the census cannot see.
    opaque_loads: HashSet<String>,
    /// Modules with a top-level `await` or `for await`.
    async_modules: HashSet<String>,
}

type AliasLoad = (HashSet<String>, Option<u32>);

impl ModuleCensus {
    /// Whether the module is on an import cycle, or imports (transitively) a
    /// module on one, or has a relative import the census could not resolve.
    pub(crate) fn is_cyclic(&self, path: &str) -> bool {
        self.cyclic.contains(path)
    }

    /// Whether some file reaches the module as a namespace (ADR 0335).
    pub(crate) fn is_exposed(&self, path: &str) -> bool {
        self.exposed.contains(path)
    }

    /// Whether some load the census could not resolve (a computed dynamic
    /// `import()`, an unresolved namespace import or re-export) may run any
    /// module's code, so no module's initialization order is certain.
    pub(crate) fn has_unknown_load(&self) -> bool {
        self.unknown_load
    }

    /// Whether `from` statically imports `to`, directly or through re-exports.
    pub(crate) fn statically_imports(&self, from: &str, to: &str) -> bool {
        let mut seen = HashSet::<&str>::default();
        let mut stack = vec![from];
        while let Some(module) = stack.pop() {
            for target in self.graph.get(module).into_iter().flatten() {
                if target == to {
                    return true;
                }
                if seen.insert(target.as_str()) {
                    stack.push(target.as_str());
                }
            }
        }
        false
    }

    /// Whether a module, or one it loads, has a top-level `await`:
    /// `require` of it throws `ERR_REQUIRE_ASYNC_MODULE`.
    fn loads_async_module(&self, module: &str) -> bool {
        self.async_modules.contains(module)
            || self
                .async_modules
                .iter()
                .any(|asynchronous| self.statically_imports(module, asynchronous))
    }

    /// Whether `to` is reached from `from` by `import` and `export … from`
    /// alone, the edges ES evaluation runs ahead of the importer's body.
    fn hoisted_reaches(&self, from: &str, to: &str) -> bool {
        let mut seen = HashSet::<&str>::default();
        let mut stack = vec![from];
        while let Some(module) = stack.pop() {
            for target in self.hoisted.get(module).into_iter().flatten() {
                if target == to {
                    return true;
                }
                if seen.insert(target.as_str()) {
                    stack.push(target.as_str());
                }
            }
        }
        false
    }

    /// Whether `from` has loaded `to` by the time code at module-level
    /// position `position` runs (`None`: the position is not known): through
    /// static imports and re-exports, or an `import x = require(…)` statement
    /// that ends at or before it. Nothing else is credited. What the first
    /// module loads is finished by then (the importer is on no cycle).
    ///
    /// A name bound to a module by a load that reaches `to` is itself in its
    /// temporal dead zone until the load runs, whatever the module's own
    /// initialization: a mention in the file reaching `to` is proved only
    /// after every such `import x = require(…)`, and is withheld where a
    /// `require()` call or a module-level `import()` may bind it, the
    /// module has a top-level `await` (`require` throws), or a load of
    /// unseen targets exists.
    pub(crate) fn imports_before(&self, from: &str, to: &str, position: Option<u32>) -> bool {
        if self.opaque_loads.contains(from) {
            return false;
        }
        for (targets, end) in self.alias_loads.get(from).into_iter().flatten() {
            if !targets
                .iter()
                .any(|target| target == to || self.statically_imports(target, to))
            {
                continue;
            }
            let (Some(position), Some(end)) = (position, end) else {
                return false;
            };
            if position < *end || targets.iter().any(|target| self.loads_async_module(target)) {
                return false;
            }
        }
        let mut firsts = self.hoisted.get(from).cloned().unwrap_or_default();
        if let Some(position) = position {
            for (targets, end) in self.equals_loads.get(from).into_iter().flatten() {
                if *end <= position {
                    firsts.extend(targets.iter().cloned());
                }
            }
        }
        firsts
            .iter()
            .any(|first| first == to || self.hoisted_reaches(first, to))
    }

    pub(crate) fn new(facts: &ProjectFacts, lookup: &SemanticLookup<'_>) -> Self {
        let mut redirects = HashMap::<String, HashSet<String>>::default();
        for (declaration, runtime) in &facts.runtime_symbol_redirects {
            let (Some(declarations), Some(implementations)) = (
                lookup.symbol_declarations(declaration),
                lookup.symbol_declarations(runtime),
            ) else {
                continue;
            };
            for declaration in declarations {
                for implementation in implementations {
                    redirects
                        .entry(declaration.location.path.to_string())
                        .or_default()
                        .insert(implementation.location.path.to_string());
                }
            }
        }
        let unresolved = |relative: bool, module: &str| {
            let package = module
                .split('/')
                .take(if module.starts_with('@') { 2 } else { 1 })
                .collect::<Vec<_>>()
                .join("/");
            if relative
                || !(module.starts_with("node:")
                    || lookup.dialect.owns_module(module)
                    || lookup.dialect.owns_module(&package))
            {
                // A project module, an ambient or virtual module, or a
                // specifier with no attested resolution: no followed source.
                // The runtime's own `node:` modules and the dialect's own
                // packages are installed code, as ADR 0339 assumes.
                Landing::Unknown
            } else {
                Landing::Installed
            }
        };
        let analyzed = lookup.file_table();
        // Where a specifier loads: analyzed project modules (graph edges), an
        // installed dependency, or code the census cannot see.
        let landing = |file: &FileFacts, span, module: &str| -> Landing {
            let relative = module.starts_with('.') || module.starts_with('/');
            let (path, declaration_only, installed) =
                if let Some(imports) = lookup.resolved_imports() {
                    let solid_facts::SpecifierAttestation::Attested(row) =
                        imports.specifier(file.path.as_str(), span, module)
                    else {
                        return unresolved(relative, module);
                    };
                    if row.resolved_path.is_empty() {
                        return unresolved(relative, module);
                    }
                    let path = if row.included_path.is_empty() {
                        row.resolved_path.to_string()
                    } else {
                        row.included_path.to_string()
                    };
                    // The resolver landed in a real installed package: realpath
                    // keeps a workspace symlink out of `node_modules`.
                    let installed = row.resolution == solid_facts::ImportResolution::NodeModules
                        && path.contains("/node_modules/");
                    let declaration_only = [".d.ts", ".d.mts", ".d.cts"]
                        .iter()
                        .any(|suffix| path.ends_with(suffix));
                    (path, declaration_only, installed)
                } else {
                    match lookup.relative_module_path(file.path.as_str(), module) {
                        Some(path) => (path.to_string(), false, false),
                        None => return unresolved(relative, module),
                    }
                };
            if installed {
                // ADR 0339's premise: an installed dependency's runtime code
                // does not import the project's own modules.
                return Landing::Installed;
            }
            let mut paths = HashSet::from_iter([path.clone()]);
            // What runs: the redirect's runtime modules, else the module itself.
            let mut runs = vec![path.clone()];
            if let Some(runtime) = redirects.get(&path) {
                paths.extend(runtime.iter().cloned());
                runs = runtime.iter().cloned().collect();
            } else if declaration_only {
                // Types only: the runtime module beside them is project code
                // no analyzed file carries.
                return Landing::Unknown;
            }
            // A project file no analyzed file carries (JavaScript outside
            // `allowJs`) has imports the census cannot see.
            if runs.iter().any(|run| !analyzed.contains_path(run.as_str())) {
                return Landing::Unknown;
            }
            Landing::Modules(paths)
        };
        let mut result = Self::default();
        let mut graph = HashMap::<String, HashSet<String>>::default();
        let mut hoisted = HashMap::<String, HashSet<String>>::default();
        let mut in_place_loads = HashMap::<String, Vec<(HashSet<String>, u32)>>::default();
        let mut alias_loads = HashMap::<String, Vec<AliasLoad>>::default();
        let mut barrels = HashMap::<String, HashSet<String>>::default();
        let modules = lookup.slot_entries(lookup.base_modules());
        let hazards = lookup.slot_entries(lookup.base_hazards());
        for ((file, edges), (_, flags)) in modules.zip(hazards) {
            use crate::base::{Hazard, ModuleEdge};
            let flagged = |hazard: Hazard| flags.iter().any(|(found, _)| *found == hazard);
            let path = file.path.to_string();
            for (module, edge) in edges {
                let ModuleEdge::Import {
                    span, namespace, ..
                } = edge
                else {
                    continue;
                };
                let module = module.specifier().expect("an import names its module");
                let landed = landing(file, *span, module);
                if matches!(landed, Landing::Unknown) {
                    result.cyclic.insert(path.clone());
                    result.unknown_load |= *namespace;
                }
                if let Landing::Modules(targets) = landed {
                    graph
                        .entry(path.clone())
                        .or_default()
                        .extend(targets.iter().cloned());
                    hoisted
                        .entry(path.clone())
                        .or_default()
                        .extend(targets.iter().cloned());
                    // Also covers `import { receiver }; export { receiver }`.
                    // Over-including other imports of a barrel only withholds.
                    if flagged(Hazard::ModuleLevelExport) {
                        barrels
                            .entry(path.clone())
                            .or_default()
                            .extend(targets.iter().cloned());
                    }
                    if *namespace {
                        result.exposed.extend(targets);
                    }
                }
            }
            for (module, edge) in edges {
                let ModuleEdge::Export {
                    span,
                    module_level: true,
                    namespace,
                    ..
                } = edge
                else {
                    continue;
                };
                let Some(module) = module.specifier() else {
                    continue;
                };
                match landing(file, *span, module) {
                    Landing::Modules(targets) => {
                        graph
                            .entry(path.clone())
                            .or_default()
                            .extend(targets.iter().cloned());
                        hoisted
                            .entry(path.clone())
                            .or_default()
                            .extend(targets.iter().cloned());
                        barrels
                            .entry(path.clone())
                            .or_default()
                            .extend(targets.iter().cloned());
                        if *namespace {
                            result.exposed.extend(targets);
                        }
                    }
                    Landing::Installed => {}
                    Landing::Unknown => {
                        result.cyclic.insert(path.clone());
                        result.unknown_load = true;
                    }
                }
            }
            // A `require` runs the module it loads in place, so it is an
            // edge of the evaluation order, and one the census cannot resolve
            // (a nonliteral specifier, an unresolved project module) leaves
            // the importer possibly on a cycle. A dynamic `import()` evaluates
            // its module in a later job, after the importer's own synchronous
            // evaluation, and a module still being evaluated is awaited, so it
            // orders nothing: it only exposes the module as a namespace.
            for (module, edge) in edges {
                let ModuleEdge::Load {
                    span,
                    require: in_place,
                    in_function,
                    ..
                } = edge
                else {
                    continue;
                };
                let landed = match module.specifier() {
                    Some(module) => landing(file, *span, module),
                    None => Landing::Unknown,
                };
                match landed {
                    Landing::Modules(targets) => {
                        if *in_place {
                            graph
                                .entry(path.clone())
                                .or_default()
                                .extend(targets.iter().cloned());
                        }
                        // A `require()` call may be skipped by the code around
                        // it, and what it binds is in its dead zone until it
                        // runs: it orders nothing and withholds mentions of
                        // its targets. A dynamic `import()` written at module
                        // level binds a name the same way.
                        if *in_place || !*in_function {
                            alias_loads
                                .entry(path.clone())
                                .or_default()
                                .push((targets.clone(), None));
                        }
                        result.exposed.extend(targets);
                    }
                    Landing::Installed => {}
                    Landing::Unknown => {
                        if *in_place {
                            result.cyclic.insert(path.clone());
                        }
                        result.opaque_loads.insert(path.clone());
                        result.unknown_load = true;
                    }
                }
            }
            for (module, edge) in edges {
                let ModuleEdge::ImportEquals {
                    span, module_level, ..
                } = edge
                else {
                    continue;
                };
                let module = module.specifier().expect("an `import =` names its module");
                match landing(file, *span, module) {
                    Landing::Modules(targets) => {
                        graph
                            .entry(path.clone())
                            .or_default()
                            .extend(targets.iter().cloned());
                        // `import x = require(…)` runs where it is written.
                        if *module_level {
                            in_place_loads
                                .entry(path.clone())
                                .or_default()
                                .push((targets.clone(), span.end));
                        }
                        alias_loads
                            .entry(path.clone())
                            .or_default()
                            .push((targets.clone(), module_level.then_some(span.end)));
                        result.exposed.extend(targets);
                    }
                    Landing::Installed => {}
                    Landing::Unknown => {
                        result.cyclic.insert(path.clone());
                        result.unknown_load = true;
                    }
                }
            }
            // A bundler turns an `import.meta.glob(…, { eager: true })` into
            // static imports of modules no specifier names: unknown loads in
            // place. A lazy glob is a dynamic import and orders nothing.
            if flagged(Hazard::InPlaceImportGlob) {
                result.opaque_loads.insert(path.clone());
                result.unknown_load = true;
                result.cyclic.insert(path.clone());
            }
            // A `require` the facts did not model as a load: an unmodeled
            // load, so the importer may be on a cycle.
            if flagged(Hazard::UnmodeledRequire) {
                result.unknown_load = true;
                result.cyclic.insert(path.clone());
            }
            if flagged(Hazard::NonliteralDynamicLoad) {
                result.opaque_loads.insert(path.clone());
                result.unknown_load = true;
            }
            if flagged(Hazard::TopLevelAwait) {
                result.async_modules.insert(path.clone());
            }
        }
        loop {
            let mut changed = false;
            for (barrel, targets) in &barrels {
                if result.exposed.contains(barrel) {
                    for target in targets {
                        changed |= result.exposed.insert(target.clone());
                    }
                }
            }
            if !changed {
                break;
            }
        }
        result.cyclic.extend(cyclic_modules(&graph));
        loop {
            let mut changed = false;
            for (path, targets) in &graph {
                if targets.iter().any(|target| result.cyclic.contains(target)) {
                    changed |= result.cyclic.insert(path.clone());
                }
            }
            if !changed {
                break;
            }
        }
        result.graph = graph;
        result.hoisted = hoisted;
        result.equals_loads = in_place_loads;
        result.alias_loads = alias_loads;
        result
    }
}

/// Every module on a cycle of `graph`: a member of a strongly connected
/// component with more than one module, or with an edge to itself. Tarjan's
/// algorithm, iterative, in time linear in the graph.
fn cyclic_modules(graph: &HashMap<String, HashSet<String>>) -> HashSet<String> {
    let mut nodes = graph.keys().collect::<Vec<_>>();
    for targets in graph.values() {
        nodes.extend(targets);
    }
    nodes.sort();
    nodes.dedup();
    let id = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (*node, index))
        .collect::<HashMap<_, _>>();
    let edges = nodes
        .iter()
        .map(|node| {
            let mut targets = graph
                .get(*node)
                .into_iter()
                .flatten()
                .map(|target| id[target])
                .collect::<Vec<_>>();
            targets.sort_unstable();
            targets
        })
        .collect::<Vec<_>>();
    let mut index = vec![usize::MAX; nodes.len()];
    let mut low = vec![0; nodes.len()];
    let mut on_stack = vec![false; nodes.len()];
    let mut stack = Vec::new();
    let mut next = 0;
    let mut cyclic = HashSet::default();
    for start in 0..nodes.len() {
        if index[start] != usize::MAX {
            continue;
        }
        let mut frames = vec![(start, 0usize)];
        index[start] = next;
        low[start] = next;
        next += 1;
        stack.push(start);
        on_stack[start] = true;
        while let Some(&mut (node, ref mut edge)) = frames.last_mut() {
            if let Some(&target) = edges[node].get(*edge) {
                *edge += 1;
                if index[target] == usize::MAX {
                    index[target] = next;
                    low[target] = next;
                    next += 1;
                    stack.push(target);
                    on_stack[target] = true;
                    frames.push((target, 0));
                } else if on_stack[target] {
                    low[node] = low[node].min(index[target]);
                }
                continue;
            }
            frames.pop();
            if let Some(&(parent, _)) = frames.last() {
                low[parent] = low[parent].min(low[node]);
            }
            if low[node] == index[node] {
                let mut component = Vec::new();
                while let Some(member) = stack.pop() {
                    on_stack[member] = false;
                    component.push(member);
                    if member == node {
                        break;
                    }
                }
                if component.len() > 1 || edges[node].contains(&node) {
                    cyclic.extend(component.into_iter().map(|member| nodes[member].clone()));
                }
            }
        }
    }
    cyclic
}

fn enclosing_function(file: &FileFacts, span: Span) -> Option<Span> {
    file.ast
        .functions
        .iter()
        .filter(|function| function.span.contains(span))
        .min_by_key(|function| function.span.end - function.span.start)
        .map(|function| function.span)
}

pub(crate) fn receiver_inert(file: &FileFacts, function: &ast::FunctionFact) -> bool {
    !function.generator
        && file.source_text(function.span).is_some_and(|text| {
            !["this", "arguments", "eval", "super", "\\"]
                .iter()
                .any(|spelling| text.contains(spelling))
        })
}

#[cfg(test)]
mod tests {
    use crate::{HashMap, HashSet};

    fn graph(edges: &[(&str, &str)]) -> HashMap<String, HashSet<String>> {
        let mut graph = HashMap::<String, HashSet<String>>::default();
        for (from, to) in edges {
            graph
                .entry((*from).to_owned())
                .or_default()
                .insert((*to).to_owned());
        }
        graph
    }

    fn sorted(set: HashSet<String>) -> Vec<String> {
        let mut values = set.into_iter().collect::<Vec<_>>();
        values.sort();
        values
    }

    #[test]
    fn cyclic_modules_are_exactly_the_modules_on_a_cycle() {
        // a -> b -> c -> a is a cycle; d -> a reaches it but is not on it;
        // e imports itself; f -> g is acyclic.
        let modules = super::cyclic_modules(&graph(&[
            ("a", "b"),
            ("b", "c"),
            ("c", "a"),
            ("d", "a"),
            ("e", "e"),
            ("f", "g"),
        ]));
        assert_eq!(sorted(modules), ["a", "b", "c", "e"]);
        assert!(super::cyclic_modules(&graph(&[("x", "y"), ("y", "z")])).is_empty());
    }
}
