//! ADR 0299: exact calls through immutable, source-owned object bindings.
//! Export/import is a channel into the same receiver census, not an escape
//! waiver. No structural signature or composite entity supplies a target.

use crate::{HashMap, HashSet};

use solid_facts::{FileFacts, ProjectFacts, ast, core::Span};
use typefacts::Location;

use crate::{
    SymbolId,
    indexes::{SemanticLookup, binding_written},
    interproc::accessor_provenance::{MemberStability, Stability},
    location,
};

pub(crate) type Targets = HashMap<Location, (String, Span)>;

pub(crate) fn index(facts: &ProjectFacts, lookup: &SemanticLookup<'_>) -> Targets {
    if facts.files.iter().any(|file| {
        file.ast
            .module_hazards
            .iter()
            .any(|hazard| hazard.kind == ast::ModuleHazardKind::Eval)
    }) {
        return HashMap::default();
    }
    let stability = MemberStability::index(facts, lookup);
    let modules = ModuleCensus::new(facts, lookup);
    let mut targets = HashMap::default();
    let mut closed = HashMap::default();
    for file in &facts.files {
        let members = file
            .ast
            .members
            .iter()
            .map(|member| (member.span, member))
            .collect::<HashMap<_, _>>();
        let references = references(file);
        for call in &file.ast.calls {
            if call.construct {
                continue;
            }
            let callee = file.ast.peel_ts_sugar_span(call.callee);
            let Some(member) = members.get(&callee) else {
                continue;
            };
            if file.ast.computed_members.contains(&callee) {
                continue;
            }
            let receiver = file.ast.peel_ts_sugar_span(member.object);
            if !references.contains(&receiver) {
                continue;
            }
            let Some(symbol) = lookup.entities().at(file.path.as_str(), receiver) else {
                continue;
            };
            let Some((owner, literal, declaration, frozen)) =
                receiver_literal(file, receiver, symbol, lookup)
            else {
                continue;
            };
            let stable = *closed
                .entry((owner.path.to_string(), declaration, symbol.clone()))
                .or_insert_with(|| {
                    receiver_closed(facts, owner, literal, declaration, symbol, lookup, &modules)
                });
            if !stable {
                continue;
            }
            let Some(name) = file.source_text(member.property) else {
                continue;
            };
            if !matches!(
                stability.state_of(facts, owner.path.as_str(), name),
                Stability::Assumed
            ) {
                continue;
            }
            if frozen
                && !matches!(
                    stability.state_of(facts, owner.path.as_str(), "freeze"),
                    Stability::Assumed
                )
            {
                continue;
            }
            let Some(function) = literal_function(owner, literal, name, lookup) else {
                continue;
            };
            if !receiver_inert(function.0, function.1) {
                continue;
            }
            // Both receiver and own callable are now proven non-nullish;
            // optional links therefore select the same function.
            targets.insert(
                location(file.path.shared(), call.callee),
                (function.0.path.to_string(), function.1.span),
            );
        }
    }
    targets
}

fn references(file: &FileFacts) -> HashSet<Span> {
    file.ast
        .identifiers
        .iter()
        .filter(|id| id.role == ast::IdentifierRole::Reference)
        .map(|id| id.span)
        .collect()
}

/// Every member expression whose receiver is an identifier bound to an exact
/// object literal, as `(path, member span)`. Built once per build: the
/// predicate below is asked for every call by several passes.
pub(crate) fn literal_receiver_members(lookup: &SemanticLookup<'_>) -> HashSet<(String, Span)> {
    let mut found = HashSet::default();
    for file in lookup.files() {
        let references = references(file);
        for member in &file.ast.members {
            let receiver = file.ast.peel_ts_sugar_span(member.object);
            if references.contains(&receiver)
                && lookup
                    .entities()
                    .at(file.path.as_str(), receiver)
                    .is_some_and(|symbol| {
                        receiver_literal(file, receiver, symbol, lookup).is_some()
                    })
            {
                found.insert((file.path.to_string(), member.span));
            }
        }
    }
    found
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

fn receiver_symbols(facts: &ProjectFacts, symbol: &SymbolId) -> HashSet<String> {
    let mut symbols = HashSet::from_iter([symbol.to_string()]);
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
pub(crate) fn namespace_exposed_modules(
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
) -> Option<HashSet<String>> {
    let census = ModuleCensus::new(facts, lookup);
    (!census.unknown_load).then_some(census.exposed)
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
#[derive(Default)]
pub(crate) struct ModuleCensus {
    exposed: HashSet<String>,
    cyclic: HashSet<String>,
    unknown_load: bool,
    /// Static import and re-export edges between analyzed modules.
    graph: HashMap<String, HashSet<String>>,
    /// The edges ES evaluation runs before the importer's own body: static
    /// imports and `export … from`.
    hoisted: HashMap<String, HashSet<String>>,
    /// Loads that run where they are written (`require(…)`, `import x =
    /// require(…)`) at module level: the targets and where the load ends.
    in_place: HashMap<String, Vec<(HashSet<String>, u32)>>,
}

impl ModuleCensus {
    /// Whether the module is on an import cycle, or imports (transitively) a
    /// module on one, or has a relative import the census could not resolve.
    pub(crate) fn is_cyclic(&self, path: &str) -> bool {
        self.cyclic.contains(path)
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

    /// Whether `from` has loaded `to` by the time code at module-level
    /// position `position` runs (`None`: the position is not known): through
    /// a hoisted edge, or an in-place load that ends at or before it. What the
    /// first module loads is finished by then (the importer is on no cycle).
    pub(crate) fn imports_before(&self, from: &str, to: &str, position: Option<u32>) -> bool {
        let mut firsts = self.hoisted.get(from).cloned().unwrap_or_default();
        if let Some(position) = position {
            for (targets, end) in self.in_place.get(from).into_iter().flatten() {
                if *end <= position {
                    firsts.extend(targets.iter().cloned());
                }
            }
        }
        firsts
            .iter()
            .any(|first| first == to || self.statically_imports(first, to))
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
        let analyzed = lookup
            .files()
            .iter()
            .map(|file| file.path.as_str())
            .collect::<HashSet<_>>();
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
                    match solid_facts::resolve_relative_module_path(
                        file.path.as_str(),
                        module,
                        lookup.files().iter().map(|file| file.path.as_str()),
                    ) {
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
            if runs.iter().any(|run| !analyzed.contains(run.as_str())) {
                return Landing::Unknown;
            }
            Landing::Modules(paths)
        };
        let mut result = Self::default();
        let mut graph = HashMap::<String, HashSet<String>>::default();
        let mut hoisted = HashMap::<String, HashSet<String>>::default();
        let mut in_place_loads = HashMap::<String, Vec<(HashSet<String>, u32)>>::default();
        let mut barrels = HashMap::<String, HashSet<String>>::default();
        for file in lookup.files() {
            let path = file.path.to_string();
            for import in file.ast.imports.iter().filter(|import| !import.type_only) {
                let namespace = import.bindings.iter().any(|binding| {
                    !binding.type_only && binding.kind == ast::ImportKind::Namespace
                });
                let landed = landing(file, import.span, &import.module);
                if matches!(landed, Landing::Unknown) {
                    result.cyclic.insert(path.clone());
                    result.unknown_load |= namespace;
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
                    if file.ast.module_level_exports().next().is_some() {
                        barrels
                            .entry(path.clone())
                            .or_default()
                            .extend(targets.iter().cloned());
                    }
                    if namespace {
                        result.exposed.extend(targets);
                    }
                }
            }
            for export in file
                .ast
                .module_level_exports()
                .filter(|export| !export.type_only)
            {
                let Some(module) = export.module.as_deref() else {
                    continue;
                };
                match landing(file, export.span, module) {
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
                        if export.namespace.is_some() {
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
            for load in &file.ast.module_loads {
                let in_place = load.kind == ast::ModuleLoadKind::Require;
                let landed = match load.specifier.as_deref() {
                    Some(module) => landing(file, load.span, module),
                    None => Landing::Unknown,
                };
                match landed {
                    Landing::Modules(targets) => {
                        if in_place {
                            graph
                                .entry(path.clone())
                                .or_default()
                                .extend(targets.iter().cloned());
                            // Evaluated where it is written; a load inside a
                            // function runs whenever that function does.
                            if !file
                                .ast
                                .functions
                                .iter()
                                .any(|function| function.span.contains(load.span))
                            {
                                in_place_loads
                                    .entry(path.clone())
                                    .or_default()
                                    .push((targets.clone(), load.span.end));
                            }
                        }
                        result.exposed.extend(targets);
                    }
                    Landing::Installed => {}
                    Landing::Unknown => {
                        if in_place {
                            result.cyclic.insert(path.clone());
                        }
                        result.unknown_load = true;
                    }
                }
            }
            for import in file
                .ast
                .import_equals
                .iter()
                .filter(|import| !import.type_only)
            {
                match landing(file, import.span, &import.module) {
                    Landing::Modules(targets) => {
                        graph
                            .entry(path.clone())
                            .or_default()
                            .extend(targets.iter().cloned());
                        // `import x = require(…)` runs where it is written.
                        in_place_loads
                            .entry(path.clone())
                            .or_default()
                            .push((targets.clone(), import.span.end));
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
            if !file.ast.in_place_import_globs.is_empty() {
                result.unknown_load = true;
                result.cyclic.insert(path.clone());
            }
            // A call of a bare `require` the facts did not record as a load
            // (a `declare function require` shim shadows the global in the
            // binder, though the host `require` is what runs): an unmodeled
            // load, so the importer may be on a cycle.
            if file.ast.calls.iter().any(|call| {
                !call.construct
                    && file.source_text(file.ast.peel_ts_sugar_span(call.callee)) == Some("require")
                    && !file
                        .ast
                        .module_loads
                        .iter()
                        .any(|load| load.span == call.span)
            }) {
                result.unknown_load = true;
                result.cyclic.insert(path.clone());
            }
            // Any other mention of `require` (`(0, require)(x)`, an alias, an
            // argument) may load in place with no recorded load.
            if !file.ast.unmodeled_require_references.is_empty() {
                result.unknown_load = true;
                result.cyclic.insert(path.clone());
            }
            if file
                .ast
                .module_hazards
                .iter()
                .any(|hazard| hazard.kind == ast::ModuleHazardKind::NonliteralDynamicLoading)
            {
                result.unknown_load = true;
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
        result.in_place = in_place_loads;
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

fn module_initialization_closed(
    owner: &FileFacts,
    literal: Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    // A hoisted function can run before a module-scope const too, without an
    // import cycle. Refuse an executable prefix whose callbacks/entry order
    // are unmodeled. The existing dialect axiom for a signal with only literal
    // data, and the exact standard-library freeze of a literal, enter no user
    // function. Function bodies written in the prefix are deferred.
    owner.ast.calls.iter().enumerate().all(|(index, call)| {
        if call.span.start >= literal.start || enclosing_function(owner, call.span).is_some() {
            return true;
        }
        let data_signal = !call.construct
            && matches!(
                lookup.primitives(owner).calls[index],
                Some(crate::PrimitiveName::Known(
                    solid_dialect::Primitive::CreateSignal,
                    _
                ))
            )
            && call.arguments.len() <= 1
            && call.arguments.iter().all(|argument| {
                !argument.spread
                    && matches!(
                        argument.literal_value,
                        ast::ArgumentLiteralFact::Null
                            | ast::ArgumentLiteralFact::Boolean(_)
                            | ast::ArgumentLiteralFact::Integer(_)
                            | ast::ArgumentLiteralFact::String(_)
                    )
            });
        data_signal || (!call.construct && object_literal(owner, call.span, lookup).is_some())
    })
}

fn initialized_use(
    owner: &FileFacts,
    literal: Span,
    declaration: Span,
    file: &FileFacts,
    reference: Span,
    modules: &ModuleCensus,
) -> bool {
    let declared_function = enclosing_function(owner, declaration);
    let used_function = enclosing_function(file, reference);
    if let Some(function) = declared_function {
        // A captured binding has no entry-order proof. A switch's shared case
        // scope has no lexical dominance proof either; source text only vetoes.
        file.path == owner.path
            && used_function == Some(function)
            && reference.start >= literal.end
            && owner
                .source_text(function)
                .is_some_and(|text| !text.contains("switch"))
    } else {
        used_function.is_some() && !modules.cyclic.contains(owner.path.as_str())
    }
}

fn object_literal(
    file: &FileFacts,
    span: Span,
    lookup: &SemanticLookup<'_>,
) -> Option<(Span, bool)> {
    let span = file.ast.peel_ts_sugar_span(span);
    if file
        .ast
        .object_get_shapes
        .iter()
        .any(|shape| shape.span == span)
    {
        return Some((span, false));
    }
    let call = file.ast.call_at(span)?;
    let declaration = lookup.standard_library_declaration(file, call, typefacts::CallKind::Call)?;
    if declaration.qualified_name.as_ref() != "ObjectConstructor.freeze"
        || call.arguments.len() != 1
        || call.arguments[0].spread
    {
        return None;
    }
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    let member = file
        .ast
        .members
        .iter()
        .find(|m| m.span == callee && !file.ast.computed_members.contains(&callee))?;
    let receiver = file.ast.peel_ts_sugar_span(member.object);
    if file.source_text(member.property) != Some("freeze")
        || file.source_text(receiver) != Some("Object")
        || file.ast.reference_declaration(receiver).is_some()
    {
        return None;
    }
    let symbol = lookup.symbol_at(file.path.as_str(), receiver)?;
    if !symbol.alias_target().is_empty() {
        return None;
    }
    let declarations = symbol
        .declarations()
        .iter()
        .filter(|declaration| {
            declaration.kind.as_ref() == "variable" && declaration.name.as_ref() == "Object"
        })
        .collect::<Vec<_>>();
    if declarations.len() != 1
        || declarations[0].location.path != declaration.source_file
        || !declaration.source_file.ends_with("/lib.es5.d.ts")
    {
        return None;
    }
    let literal = file.ast.peel_ts_sugar_span(call.arguments[0].span);
    file.ast
        .object_get_shapes
        .iter()
        .any(|shape| shape.span == literal)
        .then_some((literal, true))
}

fn receiver_literal<'a>(
    file: &FileFacts,
    receiver: Span,
    symbol: &SymbolId,
    lookup: &SemanticLookup<'a>,
) -> Option<(&'a FileFacts, Span, Span, bool)> {
    if let Some((owner, binding, _)) = lookup.binding_at_reference(file.path.as_str(), receiver) {
        if !binding.immutable
            || binding.shape != ast::BindingShape::Identifier
            || binding.names.len() != 1
        {
            return None;
        }
        let declaration = binding.names[0].span;
        if binding_written(owner, declaration) {
            return None;
        }
        let (literal, frozen) = object_literal(owner, binding.initializer?, lookup)?;
        return Some((owner, literal, declaration, frozen));
    }
    // Anonymous `export default { … }` has an exact export declaration,
    // not a variable binding. Require that very expression location.
    let declarations = lookup.symbol_declarations(symbol.as_str())?;
    if declarations.len() != 1 {
        return None;
    }
    let declaration = &declarations[0].location;
    let owner = lookup.file_by_path(declaration.path.as_ref())?;
    let export = owner.ast.exports.iter().find(|export| {
        export.kind == ast::ExportKind::Default
            && ((u64::from(export.span.start) == declaration.start_byte
                && u64::from(export.span.end) == declaration.end_byte)
                || export.declarations.iter().any(|value| {
                    u64::from(value.local.span.start) == declaration.start_byte
                        && u64::from(value.local.span.end) == declaration.end_byte
                }))
    })?;
    let value = export.declarations.first()?.local.span;
    let (literal, frozen) = object_literal(owner, value, lookup)?;
    Some((owner, literal, value, frozen))
}

fn receiver_closed<'a>(
    facts: &ProjectFacts,
    owner: &'a FileFacts,
    literal: Span,
    declaration: Span,
    symbol: &SymbolId,
    lookup: &SemanticLookup<'a>,
    modules: &ModuleCensus,
) -> bool {
    let symbols = receiver_symbols(facts, symbol);
    if enclosing_function(owner, declaration).is_none()
        && (modules.unknown_load
            || modules.exposed.contains(owner.path.as_str())
            || !module_initialization_closed(owner, literal, lookup))
    {
        return false;
    }
    let mut references = HashSet::default();
    // Binder references remain essential when a dependency import's Type
    // Facts references still name its .d.ts symbol before runtime redirects.
    for file in lookup.files() {
        let mut bindings = HashSet::default();
        if file.path == owner.path {
            bindings.insert(declaration);
        }
        for import in &file.ast.imports {
            for binding in &import.bindings {
                if lookup
                    .entities()
                    .at(file.path.as_str(), binding.local.span)
                    .is_some_and(|symbol| symbols.contains(symbol.as_str()))
                {
                    if binding.imported.is_none() {
                        return false;
                    }
                    bindings.insert(binding.local.span);
                }
            }
        }
        for (reference, bound) in &file.ast.reference_declarations {
            if lookup
                .entities()
                .at(file.path.as_str(), *reference)
                .is_some_and(|symbol| symbols.contains(symbol.as_str()))
                && file.ast.imports.iter().any(|import| {
                    import
                        .bindings
                        .iter()
                        .any(|binding| binding.local.span == *bound && binding.imported.is_some())
                })
            {
                bindings.insert(*bound);
            }
        }
        for (reference, bound) in &file.ast.reference_declarations {
            if bindings.contains(bound) {
                references.insert(location(file.path.shared(), *reference));
            }
        }
        for id in &file.ast.identifiers {
            if id.role == ast::IdentifierRole::Reference
                && lookup
                    .entities()
                    .at(file.path.as_str(), id.span)
                    .is_some_and(|symbol| symbols.contains(symbol.as_str()))
            {
                references.insert(location(file.path.shared(), id.span));
            }
        }
    }
    for symbol in symbols {
        if let Some(semantic) = lookup.symbol_references_if_present(&symbol) {
            references.extend(semantic);
        }
    }
    for reference in references {
        let Some(file) = lookup.file_by_path(reference.path.as_ref()) else {
            return false;
        };
        let Ok(start) = u32::try_from(reference.start_byte) else {
            return false;
        };
        let Ok(end) = u32::try_from(reference.end_byte) else {
            return false;
        };
        let span = Span::new(start, end);
        if file
            .ast
            .type_queries
            .iter()
            .any(|query| query.contains(span))
        {
            continue;
        }
        // Import/export name tokens are the exact module channel. Namespace
        // accesses and any ordinary value use are deliberately refused.
        if file.ast.imports.iter().any(|import| {
            !import.type_only
                && import
                    .bindings
                    .iter()
                    .any(|binding| binding.imported.is_some() && binding.local.span == span)
        }) || file.ast.exports.iter().any(|export| {
            export.namespace.is_none()
                && export
                    .specifiers
                    .iter()
                    .any(|specifier| specifier.local.span == span)
        }) {
            continue;
        }
        if file.path == owner.path && span == declaration {
            continue;
        }
        if !initialized_use(owner, literal, declaration, file, span, modules) {
            return false;
        }
        let Some(member) = file.ast.members.iter().find(|member| {
            file.ast.peel_ts_sugar_span(member.object) == span
                && !file.ast.computed_members.contains(&member.span)
        }) else {
            return false;
        };
        // Merely extracting a member lets a method receive an opaque `this`.
        // Require immediate calls, rather than treating all member reads as
        // harmless receiver uses.
        if !file
            .ast
            .calls
            .iter()
            .any(|call| !call.construct && file.ast.peel_ts_sugar_span(call.callee) == member.span)
        {
            return false;
        }
        let Some((function_file, function)) = file
            .source_text(member.property)
            .and_then(|name| literal_function(owner, literal, name, lookup))
        else {
            return false;
        };
        if !receiver_inert(function_file, function) {
            return false;
        }
    }
    true
}

pub(crate) fn receiver_inert(file: &FileFacts, function: &ast::FunctionFact) -> bool {
    !function.generator
        && file.source_text(function.span).is_some_and(|text| {
            !["this", "arguments", "eval", "super", "\\"]
                .iter()
                .any(|spelling| text.contains(spelling))
        })
}

fn literal_function<'a>(
    file: &'a FileFacts,
    literal: Span,
    name: &str,
    lookup: &SemanticLookup<'a>,
) -> Option<(&'a FileFacts, &'a ast::FunctionFact)> {
    let shape = file.ast.object_get_shapes.iter().find(|shape| {
        shape.span == literal && shape.closed && shape.properties.iter().all(|p| p.getter.is_none())
    })?;
    let own = |property: &&ast::ObjectPropertyFact| {
        shape.span.contains(property.span)
            && !file.ast.object_get_shapes.iter().any(|nested| {
                nested.span != literal
                    && literal.contains(nested.span)
                    && nested.span.contains(property.span)
            })
    };
    let properties = file
        .ast
        .object_properties
        .iter()
        .filter(own)
        .collect::<Vec<_>>();
    // Methods can expose their receiver through `this` or `arguments` to
    // opaque code. This conservative spelling veto only withholds a target;
    // it never supplies identity (comments/strings can over-veto).
    if properties
        .iter()
        .any(|p| !p.data || p.value_kind == ast::RuntimeValueKind::Function)
        && file.source_text(literal).is_some_and(|text| {
            text.contains("this")
                || text.contains("arguments")
                || text.contains("eval")
                || text.contains("super")
        })
    {
        return None;
    }
    if properties.len() != shape.properties.len() {
        return None;
    }
    let property = properties
        .into_iter()
        .zip(&shape.properties)
        .find(|(property, key)| !property.computed && key.key == name)?
        .0;
    let value = file.ast.peel_ts_sugar_span(property.value);
    if let Some(function) = file.ast.function_at(value) {
        return Some((file, function));
    }
    // Shorthand is one declared function, not a callable type or an alias.
    let declaration = property.shorthand_binding?;
    if let Some(function) = file.ast.functions.iter().find(|function| {
        function.kind == ast::FunctionKind::Declaration
            && function
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration)
    }) {
        return lookup
            .function_value_is_current(file, function)
            .then_some((file, function));
    }
    let symbol = lookup.entities().at(file.path.as_str(), declaration)?;
    let (owner, function) = lookup.function_for_symbol(symbol.as_str())?;
    (function.kind == ast::FunctionKind::Declaration
        && lookup.function_value_is_current(owner, function))
    .then_some((owner, function))
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
