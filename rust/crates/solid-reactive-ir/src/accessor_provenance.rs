//! Exact project-owned callable transport. Shapes and declared Accessor types
//! are deliberately not evidence here. This join retains an implementation
//! or source identity; the interprocedural consumer decides execution.

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
#[path = "accessor_provenance/resolver_queries.rs"]
mod resolver_queries;
#[path = "accessor_provenance/resolver_values.rs"]
mod resolver_values;
pub(crate) use projection_helpers::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Callable {
    Accessor(SymbolId),
    Function {
        path: String,
        span: Span,
    },
    /// Exact synchronous element-member invocations at a built-in map/forEach.
    ArrayMembers(Vec<(String, Span)>),
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
    functions: HashSet<(String, Span)>,
    binding_locations: HashMap<SymbolId, Location>,
    parameter_inputs: HashMap<Location, (String, Span)>,
    declarations: HashMap<SymbolId, (SymbolId, Location)>,
    uses: HashMap<Location, ClosedUses>,
    providers: HashMap<Location, Vec<Option<(String, Span)>>>,
    source_origins: HashSet<SymbolId>,
    runtime_source_origins: HashSet<SymbolId>,
    blocked_contexts: HashSet<Location>,
    unkeyed_provider: bool,
    member_stability: std::borrow::Cow<'a, MemberStability>,
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

    pub(crate) fn index(facts: &ProjectFacts, lookup: &SemanticLookup<'_>) -> Self {
        let mut project = Self::default();
        for file in &facts.files {
            let index = match facts.dependency_source_root(file.path.as_str()) {
                Some(root) => project.dependencies.entry(root.to_owned()).or_default(),
                None => &mut project,
            };
            index.syntax(file);
            for call in &file.ast.calls {
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

    fn syntax(&mut self, file: &FileFacts) {
        // Normalized leaves exclude receiver Gets, destructuring keys/defaults
        // and reads inside computed keys. Updates and loop targets are included.
        let targets = file
            .ast
            .write_targets
            .iter()
            .copied()
            .chain(file.ast.deleted_targets.iter().copied())
            .map(|span| file.ast.peel_ts_sugar_span(span))
            .collect::<HashSet<_>>();
        let literal_keys = file
            .ast
            .literal_computed_members
            .iter()
            .map(|key| (key.span, key.key.as_str()))
            .collect::<HashMap<_, _>>();
        for member in &file.ast.members {
            if !targets.contains(&member.span) {
                continue;
            }
            let name = if file.ast.computed_members.contains(&member.span) {
                literal_keys.get(&member.span).copied()
            } else {
                file.source_text(member.property)
            };
            if let Some(name) = name {
                self.written
                    .entry(name.to_owned())
                    .or_insert_with(|| location(file.path.shared(), member.span));
            } else {
                self.unknown_receivers
                    .push(location(file.path.shared(), member.object));
            }
        }
    }
}

#[derive(Clone, Copy, Default)]
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
}

#[cfg(test)]
#[path = "accessor_provenance/tests.rs"]
mod tests;
