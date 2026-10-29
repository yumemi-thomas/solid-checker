//! Exact project-owned callable transport. Shapes and declared Accessor types
//! are deliberately not evidence here. This join retains an implementation
//! or source identity; the interprocedural consumer decides execution.

use std::sync::Arc;

use crate::{HashMap, HashSet};

use solid_dialect::ContextRole;
use solid_facts::{FileFacts, ProjectFacts, ast, core::Span};
use typefacts::Location;

use super::{SummaryRead, source_has_runtime_witness};
use crate::{SymbolId, indexes::SemanticLookup, location, owners::containing_ast_function};

#[path = "accessor_provenance/mutation_sites.rs"]
mod mutation_sites;
pub(crate) use mutation_sites::*;
#[path = "accessor_provenance/projection_helpers.rs"]
mod projection_helpers;
#[path = "accessor_provenance/resolver_dependencies.rs"]
mod resolver_dependencies;
#[path = "accessor_provenance/resolver_index.rs"]
mod resolver_index;
#[path = "accessor_provenance/resolver_memos.rs"]
mod resolver_memos;
#[path = "accessor_provenance/resolver_queries.rs"]
mod resolver_queries;
#[path = "accessor_provenance/resolver_values.rs"]
mod resolver_values;
#[path = "accessor_provenance/resolver_world.rs"]
mod resolver_world;
#[path = "accessor_provenance/transport_memos.rs"]
mod transport_memos;
pub(crate) use projection_helpers::*;
use resolver_dependencies::{Dependencies, dependency_member_stable};
use resolver_index::*;
pub(crate) use resolver_memos::{RESOLVER_TABLES, RESOLVER_TABLES_FROM, ResolverTables};
use resolver_queries::Queries;
use resolver_values::Values;
use resolver_world::{ProviderValues, World, parameter_typefacts_admit_in};
pub(crate) use transport_memos::{TRANSPORT_TABLES_END, TRANSPORT_TABLES_FROM, TransportTables};

/// Whether a recorded read of memo table `table` may be checked in this
/// build. The resolver's and transport's memos are the only ones that read
/// derived tables (their types are private to this module), and a derived
/// table is exact only for readers asked after its pass publishes in the
/// build (`base::memo::derived`). A clean memo is answered from its stamp
/// without running, so a proof that checked one before the resolver's memo
/// construction served could hold on a key the pass is about to move (final
/// review R2, finding 6a): until then such a read is undecided, and the
/// chunk reruns.
pub(crate) fn memo_read_decidable(
    store: &crate::base::memo::build::MemoStore,
    table: crate::base::memo::TableId,
) -> bool {
    let reads_derived = RESOLVER_TABLES.contains(&table.0)
        || (TRANSPORT_TABLES_FROM..TRANSPORT_TABLES_END).contains(&table.0);
    !reads_derived || resolver_memos::served_this_build(store)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Callable {
    Accessor(SymbolId),
    Function {
        path: String,
        span: Span,
    },
    /// Exact synchronous element-member invocations at a built-in map/forEach.
    ArrayMembers(Vec<(String, Span)>),
    /// The setter slot of a reactive tuple a dialect primitive created
    /// (`const [, set] = createSignal(0)`), by that binding's symbol: a call
    /// writes through it and reads nothing.
    Setter(SymbolId),
    /// The binding of an `action(…)` result, by its symbol: a call invokes
    /// the action.
    Action(SymbolId),
}

enum Value {
    Callable(Callable),
    /// A supported tuple closure whose iteration identity was withdrawn.
    VetoedTuple,
    Object {
        path: String,
        span: Span,
        context: bool,
    },
    Array {
        path: String,
        elements: Vec<Span>,
    },
    /// ADR 0282: the slots of the fresh array literal that a factory's one
    /// unconditional return builds. Only an exact `const` array pattern
    /// bound to that very call consumes it. Every other route (an alias, a
    /// parameter handoff, a context provider) refuses it, because there the
    /// array is no longer fresh: a holder could replace a slot.
    Tuple {
        path: String,
        factory: Span,
        elements: Vec<Span>,
    },
}

pub(super) struct Resolver<'a, 'facts> {
    pub facts: &'a ProjectFacts,
    pub lookup: &'a SemanticLookup<'facts>,
    pub accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub source_kinds: &'a HashMap<SymbolId, crate::ReactiveSourceKind>,
    pub names: &'a HashMap<SymbolId, SymbolId>,
    functions: Arc<HashSet<(String, Span)>>,
    /// The tables shared with the memo build's kept state, which patches
    /// them per moved key (S7b).
    binding_locations: Arc<HashMap<SymbolId, Location>>,
    parameter_inputs: HashMap<Location, (String, Span)>,
    declarations: Arc<HashMap<SymbolId, (SymbolId, Location)>>,
    uses: Arc<HashMap<Location, ClosedUses>>,
    providers: HashMap<Location, Vec<Option<(String, Span)>>>,
    source_origins: HashSet<SymbolId>,
    runtime_source_origins: HashSet<SymbolId>,
    blocked_contexts: HashSet<Location>,
    unkeyed_provider: bool,
    member_stability: &'a MemberStability,
    receiver_channels_opaque: bool,
}

/// ADR 0273: absence of a visible veto is an assumption, not a closed census.
#[derive(Clone, Default)]
pub(crate) struct MemberStability {
    written: HashMap<String, Location>,
    all: Option<Location>,
    /// Every wildcard site, retained so ADR 0300 can discharge one only by
    /// proving its target is a different fresh allocation.
    wildcard_sites: Vec<Location>,
    /// Dynamic-key syntax must pass the same complete exact-target proof as
    /// reflective wildcards. A failed may-derivation is never a discharge.
    unknown_receivers: Vec<Location>,
    /// ADR 0284: ordinary callable transport scopes wildcard vetoes written
    /// inside a package analyzed as source by its root. Dependency internals are outside ADR 0273's
    /// premise, as when only the package's declarations are seen, so a write
    /// that could name any member vetoes only members of objects that package
    /// creates. A write naming its member vetoes it everywhere (review
    /// round 1): the package may write it on an object the project handed it.
    dependencies: HashMap<String, MemberStability>,
    /// ADR 0282: a visible mutation that may change the array iteration
    /// protocol. Unknown receivers/keys refuse; spelling is not identity.
    tuple_iteration: bool,
}

pub(crate) enum Stability<'a> {
    Assumed,
    Vetoed(&'a Location),
}

impl MemberStability {
    fn wildcard(&mut self, site: Location) {
        self.all.get_or_insert_with(|| site.clone());
        if !self.wildcard_sites.contains(&site) {
            self.wildcard_sites.push(site);
        }
    }
    fn state(&self, name: &str) -> Stability<'_> {
        match self.all.as_ref().or_else(|| self.written.get(name)) {
            Some(site) => Stability::Vetoed(site),
            None => Stability::Assumed,
        }
    }

    /// [`Self::state`] for a member of an object created in `owner`: the
    /// project's vetoes, and those of the package analyzed as source that
    /// holds `owner`, if one does.
    pub(crate) fn state_of(&self, facts: &ProjectFacts, owner: &str, name: &str) -> Stability<'_> {
        match self.state(name) {
            Stability::Assumed => facts
                .dependency_source_root(owner)
                .and_then(|root| self.dependencies.get(root))
                .map_or(Stability::Assumed, |package| package.state(name)),
            vetoed => vetoed,
        }
    }

    /// The census over the lookup's files, replayed in file order from the
    /// base indexes: each file's write and `delete` targets
    /// ([`crate::base::MemberRole::StabilityWrite`]) and its reflective
    /// mutation candidates ([`crate::base::Hazard::MutationBuiltinCall`],
    /// the only calls [`mutation_builtin`] accepts).
    pub(crate) fn index(facts: &ProjectFacts, lookup: &SemanticLookup<'_>) -> Self {
        let mut project = Self::default();
        let members = lookup.slot_entries(lookup.base_members());
        let hazards = lookup.slot_entries(lookup.base_hazards());
        for ((file, members), (_, hazards)) in members.zip(hazards) {
            let index = match facts.dependency_source_root(file.path.as_str()) {
                Some(root) => project.dependencies.entry(root.to_owned()).or_default(),
                None => &mut project,
            };
            index.syntax(file, members);
            let calls = hazards
                .iter()
                .filter(|(hazard, _)| *hazard == crate::base::Hazard::MutationBuiltinCall)
                .map(|(_, call)| &file.ast.calls[*call as usize]);
            for call in calls {
                let Some(method) = mutation_builtin(file, call, lookup) else {
                    continue;
                };
                let site = location(file.path.shared(), call.span);
                // A spread argument shifts every later position (a tuple can
                // supply target, key and value at once), so no key position is
                // known: it could write any member.
                if call.arguments.iter().any(|argument| argument.spread) {
                    index.wildcard(site);
                    continue;
                }
                match method {
                    // A key the literal fact cannot name (a template, an
                    // identifier, a spread) could be any member, so it vetoes
                    // every transported member: over-vetoing only withholds.
                    "defineProperty" | "set" | "deleteProperty" => {
                        let name = call
                            .arguments
                            .get(1)
                            .filter(|argument| !argument.spread)
                            .and_then(|argument| match &argument.literal_value {
                                ast::ArgumentLiteralFact::String(name) => Some(name.to_string()),
                                ast::ArgumentLiteralFact::Integer(index) => Some(index.to_string()),
                                _ => None,
                            });
                        match name {
                            Some(name) => {
                                index.written.entry(name).or_insert(site);
                            }
                            None => {
                                index.wildcard(site);
                            }
                        }
                    }
                    "defineProperties" => {
                        // A literal descriptor map names its literal keys; a
                        // spread in it, or a map that is not a literal, could
                        // name any member. An unnamed computed descriptor
                        // key is also a wildcard for the integrity proof.
                        let shape = call
                            .arguments
                            .get(1)
                            .filter(|argument| !argument.spread)
                            .and_then(|argument| {
                                file.ast.object_get_shapes.iter().find(|shape| {
                                    shape.span == file.ast.peel_ts_sugar_span(argument.span)
                                })
                            })
                            .filter(|shape| {
                                !file
                                    .ast
                                    .spreads
                                    .iter()
                                    .any(|spread| shape.span.contains(spread.span))
                                    // An unnamed computed descriptor key is
                                    // a wildcard, even inside a literal map.
                                    && shape.properties.len() == file.ast.object_properties.iter().filter(|property| {
                                        shape.span.contains(property.span)
                                            && !file.ast.object_properties.iter().any(|parent| {
                                                parent.span != property.span
                                                    && shape.span.contains(parent.span)
                                                    && parent.span.contains(property.span)
                                            })
                                    }).count()
                            });
                        match shape {
                            Some(shape) => {
                                for property in &shape.properties {
                                    index
                                        .written
                                        .entry(property.key.to_string())
                                        .or_insert_with(|| site.clone());
                                }
                            }
                            None => {
                                index.wildcard(site);
                            }
                        }
                    }
                    "assign" => {
                        for (position, argument) in call.arguments.iter().enumerate() {
                            // A first-slot spread can supply both target and sources.
                            if argument.spread {
                                index.wildcard(site.clone());
                                continue;
                            }
                            if position == 0 {
                                continue;
                            }
                            let shape = file.ast.object_get_shapes.iter().find(|shape| {
                                shape.span == file.ast.peel_ts_sugar_span(argument.span)
                                    && shape.closed
                            });
                            if shape.is_none() {
                                index.wildcard(site.clone());
                            } else if let Some(shape) = shape {
                                for property in &shape.properties {
                                    index
                                        .written
                                        .entry(property.key.to_string())
                                        .or_insert_with(|| site.clone());
                                }
                            }
                        }
                    }
                    _ => unreachable!("mutation builtin catalog"),
                }
            }
        }
        for package in project.dependencies.values_mut() {
            for (name, site) in package.written.drain() {
                project.written.entry(name).or_insert(site);
            }
        }
        project.tuple_iteration = lookup.tuple_iteration_may_be_mutated();
        project
    }

    /// Normalized leaves exclude receiver Gets, destructuring keys/defaults
    /// and reads inside computed keys. Updates and loop targets are included
    /// ([`crate::base::contributions::members`]).
    fn syntax(
        &mut self,
        file: &FileFacts,
        members: &[(crate::base::MemberName, crate::base::MemberUse)],
    ) {
        use crate::base::{MemberName, MemberRole};
        for (name, used) in members {
            if used.role != MemberRole::StabilityWrite {
                continue;
            }
            let member = &file.ast.members[used.ordinal as usize];
            match name {
                MemberName::Named(name) => {
                    self.written
                        .entry(name.to_string())
                        .or_insert_with(|| location(file.path.shared(), member.span));
                }
                MemberName::Unnamed => {
                    self.unknown_receivers
                        .push(location(file.path.shared(), member.object));
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ClosedUses {
    object_escaped: bool,
    array_escaped: bool,
    callable_escaped: bool,
    context_escaped: bool,
    context_value_escaped: bool,
    destructured_escaped: bool,
}

impl ClosedUses {
    /// An exported binding's shared object can be replaced by any importer.
    fn export(&mut self) {
        self.object_escaped = true;
        self.array_escaped = true;
        self.context_value_escaped = true;
        self.destructured_escaped = true;
    }

    /// What a handoff hands back from its receiver to its argument binding:
    /// the receiver's escapes, or, for a destructuring receiver, its
    /// destructured escape as the object's and the context value's.
    fn handed_back(destructured: &bool, receiver: &Self) -> Self {
        if *destructured {
            Self {
                object_escaped: receiver.destructured_escaped,
                context_value_escaped: receiver.destructured_escaped,
                ..Self::default()
            }
        } else {
            Self {
                object_escaped: receiver.object_escaped,
                array_escaped: receiver.array_escaped,
                callable_escaped: receiver.callable_escaped,
                context_value_escaped: receiver.context_value_escaped,
                ..Self::default()
            }
        }
    }
}

/// The census is a join of escape flags.
impl crate::base::fixpoint::Lattice for ClosedUses {
    fn bottom() -> Self {
        Self::default()
    }

    fn join(&mut self, other: &Self) {
        self.object_escaped |= other.object_escaped;
        self.array_escaped |= other.array_escaped;
        self.callable_escaped |= other.callable_escaped;
        self.context_escaped |= other.context_escaped;
        self.context_value_escaped |= other.context_value_escaped;
        self.destructured_escaped |= other.destructured_escaped;
    }
}

impl crate::base::memo::Canonical for ClosedUses {}

/// Each handoff binding's joined use census ([`Resolver::new`]), published
/// for memo readers: the resolver's origins and object contexts.
struct HandoffUses;

impl crate::base::memo::derived::Derived for HandoffUses {
    type Key = Location;
    type Value = ClosedUses;
    const NAME: &'static str = "handoff_uses";
    const ID: crate::base::memo::derived::DerivedId = crate::base::memo::derived::DerivedId(224);
}

/// The kept solver of the handoff join.
const HANDOFF_SOLVER: crate::base::memo::derived::SolverId =
    crate::base::memo::derived::SolverId(224);

/// The resolver's tables over `lookup`, rendered in a canonical order, for
/// tests that compare constructions.
#[cfg(test)]
pub(crate) fn resolver_tables(
    facts: &ProjectFacts,
    lookup: &SemanticLookup<'_>,
    accessors: &HashMap<SymbolId, (SymbolId, Location)>,
) -> Vec<String> {
    let source_kinds = HashMap::default();
    let resolver = Resolver::new(
        facts,
        lookup,
        accessors,
        &source_kinds,
        lookup.symbol_names(),
        Arc::default(),
    );
    let mut lines = Vec::new();
    lines.extend(
        resolver
            .binding_locations
            .iter()
            .map(|(symbol, location)| format!("binding {symbol:?} {location:?}")),
    );
    lines.extend(
        resolver
            .declarations
            .iter()
            .map(|(symbol, declaration)| format!("declaration {symbol:?} {declaration:?}")),
    );
    lines.extend(
        resolver
            .parameter_inputs
            .iter()
            .map(|(parameter, argument)| format!("parameter {parameter:?} {argument:?}")),
    );
    lines.extend(
        resolver
            .uses
            .iter()
            .map(|(key, uses)| format!("uses {key:?} {uses:?}")),
    );
    lines.extend(
        resolver
            .providers
            .iter()
            .map(|(context, providers)| format!("providers {context:?} {providers:?}")),
    );
    lines.extend(
        resolver
            .source_origins
            .iter()
            .map(|symbol| format!("source origin {symbol:?}")),
    );
    lines.extend(
        resolver
            .runtime_source_origins
            .iter()
            .map(|symbol| format!("runtime origin {symbol:?}")),
    );
    lines.extend(
        resolver
            .blocked_contexts
            .iter()
            .map(|context| format!("blocked {context:?}")),
    );
    lines.push(format!("unkeyed {}", resolver.unkeyed_provider));
    lines.sort();
    lines
}

#[cfg(test)]
#[path = "accessor_provenance/tests.rs"]
mod tests;
