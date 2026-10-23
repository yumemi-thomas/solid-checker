//! The closure's working types (Go: the demand-run half of `model.go`, plus
//! the symbol-identity helpers of `semantic_retained.go`). The fact types
//! themselves (`EntityFact`, `SymbolFact`, ...) are the client crate's.

use std::collections::HashSet;
use std::sync::Arc;
use typefacts::v3::EntityDemand;
use typefacts::{EntityFact, Location, PrimitiveValueDomain, SymbolClaim, SymbolFact};

/// A symbol identity (Go: `SymbolID`).
pub type SymbolId = Arc<str>;

/// One source file's canonically ordered demands. `path` is the ownership key
/// for every demand and query location in the run (Go: `SemanticDemandRun`).
#[derive(Clone, Copy, Debug)]
pub struct SemanticDemandRun<'a> {
    pub path: &'a str,
    pub demands: &'a [EntityDemand],
}

/// The batch-wide state that makes a recomputed subset equivalent to resolving
/// the complete demand closure (Go: `SemanticScope`). Type descriptors are not
/// part of it: a descriptor is a function of its demanded location alone
/// (ADR 0367), never shared from another entity of the same symbol.
#[derive(Clone, Copy, Debug)]
pub struct SemanticScope<'a> {
    pub suppression: &'a HashSet<SymbolId>,
}

/// One input run's aligned semantic answer (Go: `SemanticDemandRunResult`).
/// `dependencies` excludes the run's own path and `durable` covers every
/// symbol identity embedded anywhere in the result.
///
/// `entities` and `structural` are index-aligned with the run's demands. In
/// an entity, `symbol_claim` carries what Go spreads over `SymbolCoversSpan`
/// and `SymbolUnresolved`: with a symbol, `Covering` is "covers span" and
/// anything else is the partial claim; without one, `Unresolved` is
/// "symbol unresolved".
#[derive(Clone, Debug, Default)]
pub struct SemanticDemandRunResult {
    pub entities: Vec<EntityFact>,
    pub structural: Vec<SymbolId>,
    pub dependencies: Vec<String>,
    pub durable: bool,
}

/// An entity row carrying only its location.
#[must_use]
pub fn blank_entity(location: Location) -> EntityFact {
    EntityFact {
        location,
        symbol: "".into(),
        symbol_claim: SymbolClaim::Absent,
        type_descriptor: None,
        resolved_call: None,
        callability: None,
        constructability: None,
        runtime_binding_kind: None,
        runtime_value_domain: None,
        primitive_value_domain: PrimitiveValueDomain::default(),
        primitive_literal_candidates: None,
        call_result_domain: None,
        constant_value: None,
        array_shape: None,
        tuple_shape: None,
        library_types: None,
        reference_space: None,
        runtime_identity: "".into(),
    }
}

/// A symbol row carrying only its identity.
#[must_use]
pub fn blank_symbol(id: SymbolId) -> SymbolFact {
    SymbolFact {
        id,
        alias_target: "".into(),
        declarations: Arc::from(Vec::new()),
        references: Arc::from(Vec::new()),
    }
}

/// Reports whether an identity survives its minting generation: durable ids
/// hash the declaration (ADR 0001), while generation-scoped counter ids are
/// meaningless outside the generation that issued them. The empty id is
/// trivially durable (Go: `DurableSymbolID`).
#[must_use]
pub fn durable_symbol_id(id: &str) -> bool {
    id.is_empty() || id.starts_with("symbol:h:")
}

/// Go's `%+v` of a `Location`, as the closure's error messages print it.
#[must_use]
pub fn location_debug(location: &Location) -> String {
    format!(
        "{{Path:{} StartByte:{} EndByte:{}}}",
        location.path, location.start_byte, location.end_byte
    )
}
