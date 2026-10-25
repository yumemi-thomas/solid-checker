//! `SemanticDemandRuns` over tsrs (Go: `tsgo/semantic_runs.go`).
//!
//! One call resolves one file's run under that file's own checker (its pass,
//! `passes`). Results are index-aligned with the run's demands.
//!
//! # What is answered, and what is unavailable
//!
//! Every demand family is answered: `symbol` (with its covering claim and
//! unresolved claim), `structuralAccessor`, `referenceSpace`, `typeDescriptor`,
//! `callability` (with `untypedCallable`), `constructability`,
//! `runtimeValueDomain`, `primitiveValueDomain`, `primitiveLiteralCandidates`
//! and `arrayShape`/`tupleShape`/`libraryTypes` (see `type_facts`),
//! `constantValue` (see `constants`), `runtimeIdentity` and the runtime
//! binding kind (see `runtime_facts`), and `resolvedCall` with the
//! `parameterObjectShape` of its parameters and `callResultDomain` (see
//! `resolved_calls`).
//!
//! A family that is not ported would be listed in [`UNPORTED`] and reported
//! unavailable the one way the wire has: the field is absent (`None`, the
//! empty string, or the all-zero primitive domain). The client reads an absent
//! field as "no fact here", never as a negative proof, so the result is
//! uncertifiable, never a guess. Each such omission is counted in
//! [`Unavailable`]; the list is empty and a test keeps it, and the counter,
//! at zero (the precondition for making this producer the default).

use crate::backend::BackendError;
use crate::constants::constant_value_at;
use crate::context::Context;
use crate::gocompat::{absolute, quote};
use crate::identity::Env;
use crate::model::{
    SemanticDemandRun, SemanticDemandRunResult, SymbolId, blank_entity, durable_symbol_id,
};
use crate::nodes::{NodeCursor, jsx_tag_name_at, symbol_span_covers};
use crate::resolved_calls::{CallCaches, CallDemand, CallRun};
use crate::runtime_facts::{runtime_binding_kind, runtime_identity};
use crate::type_facts::{
    DescriptorMemo, array_shape_at, callability_of_type, constructability_of_type,
    library_types_at, primitive_literal_candidates_of_type, primitive_value_domain_of_type,
    runtime_value_domain_of_type, tuple_shape_at,
};
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use tsrs_ast::{Node, Symbol, is_identifier};
use tsrs_checker::types::Type;
use tsrs_core::P;
use typefacts::v3::EntityDemand;
use typefacts::{ParameterFact, ReferenceSpace, ResolvedDeclaration, SymbolClaim};

/// How many demanded facts were left absent because their family is not
/// ported, by family name (the wire field's name).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Unavailable(pub BTreeMap<&'static str, u64>);

impl Unavailable {
    pub fn note(&mut self, family: &'static str) {
        *self.0.entry(family).or_default() += 1;
    }

    /// How many demands left `family` absent.
    #[must_use]
    pub fn count(&self, family: &str) -> u64 {
        self.0.get(family).copied().unwrap_or(0)
    }
}

/// Go: `semanticEvidence`: whether every symbol identity a run embeds is a
/// binder-level one (`symbol:h:`), built alongside the facts so it needs no
/// second walk through every nested call and descriptor. A run holding a
/// counter id is counted in the timings' `nonDurableFiles`.
pub struct Evidence {
    pub(crate) durable: bool,
}

impl Evidence {
    fn new() -> Self {
        Self { durable: true }
    }

    pub(crate) fn symbol(&mut self, id: &str) {
        if !durable_symbol_id(id) {
            self.durable = false;
        }
    }

    pub(crate) fn declaration(&mut self, declaration: Option<&ResolvedDeclaration>) {
        let Some(declaration) = declaration else {
            return;
        };
        self.symbol(&declaration.symbol);
        for owner in declaration.owners.iter() {
            self.symbol(&owner.symbol);
        }
    }

    pub(crate) fn parameter(&mut self, parameter: &ParameterFact) {
        self.symbol(&parameter.symbol);
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

/// A family and the demand flag that asks for it, for families this producer
/// has no port of. Empty: every family is answered.
type Unported = (fn(&EntityDemand) -> bool, &'static str);
pub const UNPORTED: &[Unported] = &[];

/// Counts every demanded family that [`UNPORTED`] names.
fn note_unported(demand: &EntityDemand, unavailable: &mut Unavailable) {
    for (demanded, family) in UNPORTED {
        if demanded(demand) {
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

/// The answers of one file's run that read every file of the program. Inside
/// the file's pass only its own contribution exists, so they are recorded
/// here, by entity index, and completed once every file was scanned
/// (`per_file`), from pointer-free state only.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Deferrals {
    /// Entities whose reference space is the union of every file's spaces of
    /// the id.
    pub spaces: Vec<(usize, SymbolId)>,
    /// Entities whose default-library binding stands only if no reference of
    /// the target id, in any file, is a write (the census of
    /// `CallRun::default_library_binding`).
    pub library: Vec<(usize, SymbolId)>,
    /// The structural-accessor symbols of the run: the type descriptor of an
    /// entity of such a symbol is suppressed across the whole table, so the
    /// union over every file decides it.
    pub structural: HashSet<SymbolId>,
    /// The program-wide syntactic census of property-write names
    /// (`CallCaches::library_property_writes`), shared by every pass instead
    /// of recomputed per file.
    pub library_writes: Option<Arc<HashSet<String>>>,
    /// The census questions the run asked, with their answers
    /// (`CallCaches::take_census_reads`): what a reused pass must re-check.
    pub census: Vec<(String, bool)>,
}

/// Go: `SemanticDemandRuns` for one file's run, inside that file's pass
/// (its own checker). The answers that read every file are left to the
/// caller ([`Deferrals`]).
#[allow(clippy::too_many_lines)]
pub fn run(
    ctx: &Context,
    env: &mut Env<'_>,
    run: &SemanticDemandRun<'_>,
    unavailable: &mut Unavailable,
    deferrals: &mut Deferrals,
) -> Result<SemanticDemandRunResult, BackendError> {
    check(ctx)?;
    let path = run.path;
    let mut evidence = Evidence::new();
    let count = run.demands.len();
    let mut result = SemanticDemandRunResult {
        entities: Vec::with_capacity(count),
        ..SemanticDemandRunResult::default()
    };
    let mut nodes: Vec<Option<P<Node>>> = vec![None; count];
    let mut symbols: Vec<Option<P<Symbol>>> = vec![None; count];
    let mut structural: HashSet<SymbolId> = HashSet::new();
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
        // Its path equals the run's (checked above): the clone shares it.
        let location = demand.location.clone();
        result.entities.push(blank_entity(location.clone()));
        let Some(file) = file else {
            continue;
        };
        let node = cursor.at(location.start_byte);
        nodes[index] = node;
        if !demand.structural_accessor {
            continue;
        }
        let Some(node) = node else {
            continue;
        };
        if let Some(symbol) = env.checker.get_symbol_at_location_exported(node) {
            let id = env.id_for(symbol);
            // No exact key: no symbol (a structural accessor without one is
            // not suppressed batch-wide either).
            if id.is_empty() {
                continue;
            }
            let entity = &mut result.entities[index];
            entity.symbol = id.clone();
            entity.symbol_claim =
                if symbol_span_covers(file, node, location.start_byte, location.end_byte) {
                    SymbolClaim::Covering
                } else {
                    SymbolClaim::Partial
                };
            if jsx_tag_name_at(node, location.start_byte, location.end_byte) == node {
                symbols[index] = Some(symbol);
            }
            evidence.symbol(&id);
            structural.insert(id);
        }
    }
    let Some(file) = file else {
        result.durable = evidence.durable;
        return Ok(result);
    };

    // Go: `project.typeDescriptors`, held per call here, so no type address
    // outlives its checker. Go's `batchDescriptors` (the first descriptor of a
    // symbol answered every later entity of it) is gone: a descriptor depends
    // on its location only.
    let mut memo = DescriptorMemo::default();
    let mut caches = CallCaches::default();
    caches.defer_library_census(deferrals.library_writes.take());
    deferrals.structural.clone_from(&structural);
    // The structural-accessor symbols of the whole table suppress
    // descriptors too; the caller completes that across files.
    let suppressed = |symbol: &SymbolId| structural.contains(symbol);

    let mut query_cursor = NodeCursor::new(Some(file));
    let mut call_demands: Vec<CallDemand> = Vec::new();
    for (index, demand) in run.demands.iter().enumerate() {
        check(ctx)?;
        let entity = &mut result.entities[index];
        if !entity.symbol.is_empty() && !wants_beyond_symbol(demand) {
            continue;
        }
        let location = entity.location.clone();
        let result_node = nodes[index];
        let mut result_symbol: Option<P<Symbol>> = None;
        if let Some(node) = result_node
            && (demand.symbol || demand.reference_space || demand.runtime_identity)
        {
            let tag = jsx_tag_name_at(node, location.start_byte, location.end_byte);
            result_symbol = symbols[index];
            if result_symbol.is_none() {
                result_symbol = env.checker.get_symbol_at_location_exported(tag);
            }
            let answer = match result_symbol {
                Some(symbol) => {
                    let claim =
                        if symbol_span_covers(file, tag, location.start_byte, location.end_byte) {
                            SymbolClaim::Covering
                        } else {
                            SymbolClaim::Partial
                        };
                    let id = env.id_for(symbol);
                    // A symbol with no exact key carries no id: the entity
                    // says it could not name one.
                    if id.is_empty() {
                        (id, SymbolClaim::Unresolved)
                    } else {
                        (id, claim)
                    }
                }
                None => ("".into(), SymbolClaim::Unresolved),
            };
            if demand.symbol && entity.symbol.is_empty() {
                entity.symbol = answer.0;
                entity.symbol_claim = answer.1;
            }
        }
        evidence.symbol(&entity.symbol);
        if demand.reference_space {
            entity.reference_space = Some(ReferenceSpace::Neither);
            if let Some(id) = result_symbol
                .map(|symbol| env.id_for(symbol))
                .filter(|id| !id.is_empty())
            {
                deferrals.spaces.push((index, id));
            }
        }
        if demand.runtime_identity
            && let Some(symbol) = result_symbol
        {
            let identity = runtime_identity(env, symbol);
            entity.runtime_identity = identity.as_str().into();
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
            let value = env.checker.get_type_at_location(node);
            query_type = Some(value);
            entity.type_descriptor = Some(memo.descriptor(env.checker, value));
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
        if demand.runtime_identity && demand.callability && demand.constructability {
            let mut binding_node = query_cursor.exact_expression_at(query.0, query.1);
            if binding_node.is_none()
                && demand.query_location.is_none()
                && let Some(node) = result_node
                && is_identifier(node)
            {
                // Export/import specifiers are exact identifier demands but
                // are not expression nodes in the TypeScript AST. The already
                // located result node has the same demanded boundaries and
                // exact symbol.
                binding_node = Some(node);
            }
            if let Some(node) = binding_node {
                entity.runtime_binding_kind = runtime_binding_kind(env, node);
            }
        }
        if (demand.primitive_value_domain || demand.primitive_literal_candidates)
            && let Some(node) = query_cursor.exact_expression_at(query.0, query.1)
        {
            // The query node's type is reused only when it is the same node
            // (Go: `primitiveNode != queryNode`).
            let value = match query_type {
                Some(value) if query_node == Some(node) => value,
                _ => env.checker.get_type_at_location(node),
            };
            if demand.primitive_value_domain {
                entity.primitive_value_domain = primitive_value_domain_of_type(env.checker, value);
            }
            if demand.primitive_literal_candidates {
                entity.primitive_literal_candidates = Some(Arc::new(
                    primitive_literal_candidates_of_type(env.checker, value),
                ));
            }
        }
        if demand.call_result_domain
            && let Some(call_node) = query_cursor.exact_call_like_at(query.0, query.1)
        {
            let domain = CallRun {
                ctx,
                env: &mut *env,
                caches: &mut caches,
                memo: &mut memo,
                file,
                evidence: &mut evidence,
            }
            .call_result_domain(call_node)?;
            entity.call_result_domain = Some(domain);
        }
        if demand.constant_value
            && let Some(node) = query_cursor.exact_expression_at(query.0, query.1)
        {
            entity.constant_value = constant_value_at(env, node);
        }
        if demand.array_shape
            && let Some(node) = query_cursor.exact_expression_at(query.0, query.1)
        {
            entity.array_shape = Some(array_shape_at(env.checker, node));
        }
        if demand.tuple_shape
            && let Some(node) = query_cursor.exact_expression_at(query.0, query.1)
        {
            entity.tuple_shape = tuple_shape_at(env.checker, node);
        }
        if demand.library_types
            && let Some(node) = query_cursor.exact_expression_at(query.0, query.1)
        {
            entity.library_types = library_types_at(env.checker, env.program, node).map(Arc::new);
        }
        note_unported(demand, unavailable);
        if demand.resolved_call {
            // Call lookup is anchored at the first token and walks outward;
            // callers may demand a statement-sized range including a trailing
            // semicolon. Type/value domains above instead classify the
            // complete demanded expression range.
            call_demands.push(CallDemand {
                entity_index: index,
                node: query_cursor.resolved_call_like_at(query.0, query.1),
                object_shape: demand.parameter_object_shape,
            });
        }
    }
    if !call_demands.is_empty() {
        CallRun {
            ctx,
            env: &mut *env,
            caches: &mut caches,
            memo: &mut memo,
            file,
            evidence: &mut evidence,
        }
        .resolve(&call_demands, &mut result.entities)?;
    }
    result.durable = evidence.durable;
    let (writes, library) = caches.take_deferred_library();
    deferrals.library_writes = writes;
    deferrals.library = library;
    deferrals.census = caches.take_census_reads();
    Ok(result)
}
