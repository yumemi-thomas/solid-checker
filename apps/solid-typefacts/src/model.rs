//! The closure's working types (Go: the demand-run half of `model.go`, plus
//! the symbol-identity helpers of `semantic_retained.go`). The fact types
//! themselves (`EntityFact`, `SymbolFact`, ...) are the client crate's.

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
    /// The retained allocation `demands` borrows from, when there is one:
    /// a kept pass compares it by identity instead of by value.
    pub shared: Option<&'a std::sync::Arc<[EntityDemand]>>,
}

/// One input run's aligned semantic answer (Go: `SemanticDemandRunResult`).
/// `durable` covers every symbol identity embedded anywhere in the result.
///
/// `entities` is index-aligned with the run's demands. In an entity,
/// `symbol_claim` carries what Go spreads over `SymbolCoversSpan` and
/// `SymbolUnresolved`: with a symbol, `Covering` is "covers span" and anything
/// else is the partial claim; without one, `Unresolved` is "symbol unresolved".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SemanticDemandRunResult {
    pub entities: Vec<EntityFact>,
    pub durable: bool,
}

/// One run's answer to a table: its aligned result, or the entity rows a
/// pass already prepared from it (`contributions::prepare_rows`) and
/// completed (ADR 0398).
#[derive(Clone, Debug, PartialEq)]
pub enum RunAnswer {
    Result(SemanticDemandRunResult),
    Rows {
        entities: Arc<[EntityFact]>,
        durable: bool,
    },
}

impl RunAnswer {
    /// The run's entity rows and whether every identity in them is
    /// binder-level.
    pub fn into_rows(
        self,
        path: &str,
        demands: &[EntityDemand],
    ) -> Result<(Arc<[EntityFact]>, bool), String> {
        match self {
            Self::Result(result) => Ok((
                crate::contributions::prepare_rows(path, demands, result.entities)?.into(),
                result.durable,
            )),
            Self::Rows { entities, durable } => Ok((entities, durable)),
        }
    }
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
        declared_keys: None,
        writable_optional_property: false,
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
/// are hashes (ADR 0001); the producer mints no other kind since every
/// derived symbol takes an exact key (ADR 0398). The empty id is trivially
/// durable (Go: `DurableSymbolID`).
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
