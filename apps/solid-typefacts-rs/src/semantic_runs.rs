//! `SemanticDemandRuns` over tsrs (Go: `tsgo/semantic_runs.go`).
//!
//! One call resolves canonically ordered per-file runs under one checker.
//! Results are index-aligned with `runs` and with each run's demands; no
//! caller has to flatten or repartition file ownership.
//!
//! # What is answered, and what is unavailable
//!
//! Answered end to end: `symbol` (with its covering claim and unresolved
//! claim), `structuralAccessor`, `referenceSpace`, `typeDescriptor`,
//! `callability` (with `untypedCallable`), `constructability`,
//! `runtimeValueDomain`, `primitiveValueDomain` and
//! `primitiveLiteralCandidates` (see `type_facts`). Not ported yet:
//! `resolvedCall`, `parameterObjectShape` and `callResultDomain` (they need
//! the call resolver, whose validity check `callResultDomain` shares),
//! `constantValue`, `arrayShape`, `tupleShape`, `libraryTypes`,
//! `runtimeIdentity` and the runtime binding kind. For a demanded entity
//! they are reported unavailable the one way the wire has: the
//! field is absent (`None`, the empty string, or the all-zero primitive
//! domain). The client reads an absent field as "no fact here" -- the same
//! row the Go producer writes when a span carries no expression to classify
//! -- never as a negative proof, so the result is uncertifiable, never a
//! guess. Each such omission is counted in [`Unavailable`].

use crate::backend::BackendError;
use crate::context::Context;
use crate::gocompat::{absolute, quote};
use crate::identity::Env;
use crate::model::{
    SemanticDemandRun, SemanticDemandRunResult, SemanticScope, SymbolId, blank_entity,
    durable_symbol_id,
};
use crate::nodes::{NodeCursor, jsx_tag_name_at, symbol_span_covers};
use crate::references::ReferenceIndex;
use crate::type_facts::{
    DescriptorMemo, callability_of_type, constructability_of_type,
    primitive_literal_candidates_of_type, primitive_value_domain_of_type,
    runtime_value_domain_of_type,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use tsrs_ast::{Node, SourceFile, Symbol};
use tsrs_checker::types::Type;
use tsrs_core::P;
use typefacts::v3::EntityDemand;
use typefacts::{ReferenceSpace, SymbolClaim, TypeDescriptor};

/// How many demanded facts were left absent because their family is not
/// ported, by family name (the wire field's name).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Unavailable(pub BTreeMap<&'static str, u64>);

impl Unavailable {
    fn note(&mut self, family: &'static str) {
        *self.0.entry(family).or_default() += 1;
    }

    /// How many demands left `family` absent.
    #[must_use]
    pub fn count(&self, family: &str) -> u64 {
        self.0.get(family).copied().unwrap_or(0)
    }
}

/// Go: `semanticEvidence`: built alongside facts so retained-state ownership
/// does not require a second walk through every nested descriptor.
struct Evidence {
    path: String,
    dependencies: Vec<String>,
    durable: bool,
}

impl Evidence {
    fn new(path: &str) -> Self {
        Self {
            path: path.to_owned(),
            dependencies: Vec::new(),
            durable: true,
        }
    }

    fn symbol(&mut self, id: &str) {
        if !durable_symbol_id(id) {
            self.durable = false;
        }
    }

    fn descriptor(&mut self, descriptor: Option<&TypeDescriptor>) {
        let Some(descriptor) = descriptor else {
            return;
        };
        for declaration in descriptor.alias_declarations.iter() {
            let path: &str = &declaration.location.path;
            if !path.is_empty() && path != self.path {
                self.dependencies.push(path.to_owned());
            }
        }
    }

    fn finish(mut self) -> (Vec<String>, bool) {
        self.dependencies.sort();
        self.dependencies.dedup();
        (self.dependencies, self.durable)
    }
}

/// The families past `symbol`, `referenceSpace` and a seeded `typeDescriptor`.
/// Go skips an entity that already has a symbol and demands none of these.
fn wants_beyond_symbol(demand: &EntityDemand) -> bool {
    demand.type_descriptor
        || demand.resolved_call
        || demand.callability
        || demand.constructability
        || demand.runtime_value_domain
        || demand.primitive_value_domain
        || demand.primitive_literal_candidates
        || demand.parameter_object_shape
        || demand.call_result_domain
        || demand.constant_value
        || demand.array_shape
        || demand.tuple_shape
        || demand.library_types
        || demand.reference_space
        || demand.runtime_identity
}

/// Names every demanded family this slice cannot answer. `callResultDomain`
/// is counted by [`run`], which knows whether a call node exists.
fn note_unported(demand: &EntityDemand, unavailable: &mut Unavailable) {
    let families: [(bool, &'static str); 8] = [
        (demand.resolved_call, "resolvedCall"),
        (demand.parameter_object_shape, "parameterObjectShape"),
        (demand.constant_value, "constantValue"),
        (demand.array_shape, "arrayShape"),
        (demand.tuple_shape, "tupleShape"),
        (demand.library_types, "libraryTypes"),
        (demand.runtime_identity, "runtimeIdentity"),
        (
            demand.runtime_identity && demand.callability && demand.constructability,
            "runtimeBindingKind",
        ),
    ];
    for (demanded, family) in families {
        if demanded {
            unavailable.note(family);
        }
    }
}

fn check(ctx: &Context) -> Result<(), BackendError> {
    match ctx.err() {
        Some(reason) => Err(BackendError(reason.to_owned())),
        None => Ok(()),
    }
}

/// Go: `SemanticDemandRuns`, after the project lock and checker are held.
#[allow(clippy::too_many_lines)]
pub fn run(
    ctx: &Context,
    env: &mut Env<'_>,
    references: &mut ReferenceIndex,
    runs: &[SemanticDemandRun<'_>],
    scope: &SemanticScope<'_>,
    unavailable: &mut Unavailable,
) -> Result<Vec<SemanticDemandRunResult>, BackendError> {
    check(ctx)?;
    let mut results: Vec<SemanticDemandRunResult> = Vec::with_capacity(runs.len());
    let mut files: Vec<Option<P<SourceFile>>> = Vec::with_capacity(runs.len());
    let mut evidence: Vec<Evidence> = Vec::with_capacity(runs.len());
    let mut nodes: Vec<Vec<Option<P<Node>>>> = Vec::with_capacity(runs.len());
    let mut symbols: Vec<Vec<Option<P<Symbol>>>> = Vec::with_capacity(runs.len());
    let mut batch_structural: HashSet<SymbolId> = HashSet::new();

    for run in runs {
        let path = run.path;
        let mut run_evidence = Evidence::new(path);
        let count = run.demands.len();
        let mut result = SemanticDemandRunResult {
            entities: Vec::with_capacity(count),
            structural: vec!["".into(); count],
            ..SemanticDemandRunResult::default()
        };
        let mut run_nodes: Vec<Option<P<Node>>> = vec![None; count];
        let mut run_symbols: Vec<Option<P<Symbol>>> = vec![None; count];
        // `sourceFileFor(Location{Path: path})`: an absent file leaves every
        // entity of the run as its bare location.
        let file = absolute(path)
            .ok()
            .and_then(|absolute_path| env.program.get_source_file(&absolute_path));
        let mut cursor = NodeCursor::new(file);
        for (index, demand) in run.demands.iter().enumerate() {
            if *demand.location.path != *path {
                return Err(BackendError(format!(
                    "semantic demand run {} contains location from {}",
                    quote(path),
                    quote(&demand.location.path)
                )));
            }
            if let Some(query) = &demand.query_location
                && *query.path != *path
            {
                return Err(BackendError(format!(
                    "semantic demand run {} contains query location from {}",
                    quote(path),
                    quote(&query.path)
                )));
            }
            let mut location = demand.location.clone();
            location.path = path.into();
            result.entities.push(blank_entity(location.clone()));
            let Some(file) = file else {
                continue;
            };
            let node = cursor.at(location.start_byte);
            run_nodes[index] = node;
            if !demand.structural_accessor {
                continue;
            }
            let Some(node) = node else {
                continue;
            };
            if let Some(symbol) = env.checker.get_symbol_at_location_exported(node) {
                let id = env.id_for(symbol);
                result.structural[index] = id.clone();
                let entity = &mut result.entities[index];
                entity.symbol = id.clone();
                entity.symbol_claim =
                    if symbol_span_covers(file, node, location.start_byte, location.end_byte) {
                        SymbolClaim::Covering
                    } else {
                        SymbolClaim::Partial
                    };
                if jsx_tag_name_at(node, location.start_byte, location.end_byte) == node {
                    run_symbols[index] = Some(symbol);
                }
                run_evidence.symbol(&id);
                batch_structural.insert(id);
            }
        }
        results.push(result);
        files.push(file);
        evidence.push(run_evidence);
        nodes.push(run_nodes);
        symbols.push(run_symbols);
    }

    // Go: `batchDescriptors` and `project.typeDescriptors` (the latter held
    // per call here, so no type address outlives its checker).
    let mut batch_descriptors: HashMap<SymbolId, Arc<TypeDescriptor>> = HashMap::new();
    let mut memo = DescriptorMemo::default();
    let suppressed =
        |symbol: &SymbolId| batch_structural.contains(symbol) || scope.suppression.contains(symbol);

    for (run_index, run) in runs.iter().enumerate() {
        let Some(file) = files[run_index] else {
            let (dependencies, durable) =
                std::mem::replace(&mut evidence[run_index], Evidence::new(run.path)).finish();
            results[run_index].dependencies = dependencies;
            results[run_index].durable = durable;
            continue;
        };
        let mut query_cursor = NodeCursor::new(Some(file));
        for (index, demand) in run.demands.iter().enumerate() {
            check(ctx)?;
            let entity = &mut results[run_index].entities[index];
            if !entity.symbol.is_empty() && !wants_beyond_symbol(demand) {
                continue;
            }
            let location = entity.location.clone();
            let result_node = nodes[run_index][index];
            let mut result_symbol: Option<P<Symbol>> = None;
            if let Some(node) = result_node
                && (demand.symbol || demand.reference_space || demand.runtime_identity)
            {
                let tag = jsx_tag_name_at(node, location.start_byte, location.end_byte);
                result_symbol = symbols[run_index][index];
                if result_symbol.is_none() {
                    result_symbol = env.checker.get_symbol_at_location_exported(tag);
                }
                if demand.symbol && entity.symbol.is_empty() {
                    if let Some(symbol) = result_symbol {
                        entity.symbol = env.id_for(symbol);
                        entity.symbol_claim = if symbol_span_covers(
                            file,
                            tag,
                            location.start_byte,
                            location.end_byte,
                        ) {
                            SymbolClaim::Covering
                        } else {
                            SymbolClaim::Partial
                        };
                    } else {
                        entity.symbol_claim = SymbolClaim::Unresolved;
                    }
                }
            }
            evidence[run_index].symbol(&entity.symbol);
            if demand.reference_space {
                entity.reference_space = Some(ReferenceSpace::Neither);
                if let Some(symbol) = result_symbol {
                    let id = env.id_for(symbol);
                    entity.reference_space = Some(references.space_for(env, &id));
                }
            }

            let query = demand
                .query_location
                .as_ref()
                .map_or((location.start_byte, location.end_byte), |query| {
                    (query.start_byte, query.end_byte)
                });
            let query_node = query_cursor.covering(query.0, query.1);
            let mut query_type: Option<P<Type>> = None;
            if demand.type_descriptor
                && !suppressed(&entity.symbol)
                && let Some(node) = query_node
            {
                // A descriptor the batch already holds answers any entity of
                // that symbol; the scope's seed is compiler-independent.
                let held = batch_descriptors
                    .get(&entity.symbol)
                    .or_else(|| scope.descriptor_seed.get(&entity.symbol))
                    .cloned();
                let descriptor = if let Some(descriptor) = held {
                    descriptor
                } else {
                    let value = env.checker.get_type_at_location(node);
                    query_type = Some(value);
                    let descriptor = memo.descriptor(env.checker, value);
                    if !entity.symbol.is_empty() {
                        batch_descriptors.insert(entity.symbol.clone(), Arc::clone(&descriptor));
                    }
                    descriptor
                };
                evidence[run_index].descriptor(Some(&descriptor));
                entity.type_descriptor = Some(descriptor);
            }
            if (demand.callability || demand.constructability || demand.runtime_value_domain)
                && let Some(node) = query_node
            {
                let value = match query_type {
                    Some(value) => value,
                    None => {
                        let value = env.checker.get_type_at_location(node);
                        query_type = Some(value);
                        value
                    }
                };
                if demand.callability {
                    entity.callability = Some(callability_of_type(env.checker, value));
                }
                if demand.constructability {
                    entity.constructability = Some(constructability_of_type(env.checker, value));
                }
                if demand.runtime_value_domain {
                    entity.runtime_value_domain =
                        Some(runtime_value_domain_of_type(env.checker, value));
                }
            }
            if (demand.primitive_value_domain || demand.primitive_literal_candidates)
                && let Some(node) = query_cursor.exact_expression_at(query.0, query.1)
            {
                // The query node's type is reused only when it is the
                // same node (Go: `primitiveNode != queryNode`).
                let value = match query_type {
                    Some(value) if query_node == Some(node) => value,
                    _ => env.checker.get_type_at_location(node),
                };
                if demand.primitive_value_domain {
                    entity.primitive_value_domain =
                        primitive_value_domain_of_type(env.checker, value);
                }
                if demand.primitive_literal_candidates {
                    entity.primitive_literal_candidates = Some(Arc::new(
                        primitive_literal_candidates_of_type(env.checker, value),
                    ));
                }
            }
            if demand.call_result_domain
                && query_cursor.exact_call_like_at(query.0, query.1).is_some()
            {
                // Its validity guard is the call resolver's (resolved calls).
                unavailable.note("callResultDomain");
            }
            note_unported(demand, unavailable);
        }
        let (dependencies, durable) =
            std::mem::replace(&mut evidence[run_index], Evidence::new(run.path)).finish();
        results[run_index].dependencies = dependencies;
        results[run_index].durable = durable;
    }
    Ok(results)
}
