//! The table-transition encoder (Go: `wire_transition.go`): validates a
//! transition's inputs, plans its operations, and writes the packed frame.

use std::fmt;

use super::plan::{
    PathOpRef, Slot, SymbolBody, SymbolOpRef, plan_exact_path_delta, plan_exact_symbol_delta,
    plan_fallback_path_delta, plan_fallback_symbol_delta, plan_full, plan_sparse_paths,
};
use super::rows::{write_path_op, write_symbol_op};
use super::table::ProducerTable;
use super::writer::{RowWriter, append_packed_dictionary};
use crate::v3::{
    self, SUPPORTED_TABLE_SCHEMAS, TABLE_TRANSITION_VERSION, TYPE_FACTS_SCHEMA_V1,
    TYPE_FACTS_TABLE_SCHEMA_V24, TransitionMode,
};
use crate::v3::{PathOp, SlotOp, SymbolOp};

/// Why a transition could not be encoded.
#[derive(Debug)]
pub enum EncodeError {
    /// A sparse transition needs the target to carry an exact manifest. The
    /// caller rebuilds the table in full and encodes that instead.
    SparseUnavailable,
    /// The inputs contradict the wire format; nothing was emitted.
    Invalid(String),
    /// The sink refused a write.
    Sink(String),
}

impl EncodeError {
    pub(super) fn invalid(message: String) -> Self {
        Self::Invalid(message)
    }

    pub(super) fn within(self, context: &str) -> Self {
        match self {
            Self::Invalid(message) => Self::Invalid(format!("{context}: {message}")),
            other => other,
        }
    }
}

impl fmt::Display for EncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SparseUnavailable => {
                formatter.write_str("sparse transition requires an exact manifest")
            }
            Self::Invalid(message) | Self::Sink(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for EncodeError {}

/// Receives one packed transition without taking ownership of the producer's
/// memory (Go: `TransitionArena`). `append_rows` gets bounded chunks of the
/// row run in order; `finish` gets the header and dictionary that logically
/// precede those rows.
pub trait TransitionSink {
    fn append_rows(&mut self, rows: &[u8]) -> std::io::Result<()>;
    fn finish(&mut self, prefix: &[u8]) -> std::io::Result<()>;
}

/// The complete state needed to encode one transition. `base` is `None` for a
/// full transition; a delta names the client's retained state by
/// `base_state_token`.
#[derive(Clone, Copy)]
pub struct TransitionInput<'a> {
    pub project_id: &'a str,
    pub base_state_token: &'a str,
    pub base: Option<&'a ProducerTable>,
    pub target: &'a ProducerTable,
    /// Emit unconditional upserts and removals for exactly the target's
    /// manifest paths. The client owns the base table, so the producer need
    /// not retain expanded base rows.
    pub sparse: bool,
}

#[derive(Debug)]
pub struct EncodedTransition {
    /// The frame. Empty for an empty same-generation delta (reuse) and when
    /// the frame went to a sink.
    pub bytes: Vec<u8>,
    pub mode: TransitionMode,
    pub path_operations: usize,
    pub symbol_operations: usize,
}

/// Encodes transitions at one Wire table schema.
#[derive(Clone, Copy, Debug)]
pub struct TransitionEncoder {
    table_schema: u64,
}

impl Default for TransitionEncoder {
    fn default() -> Self {
        Self::new(0)
    }
}

impl TransitionEncoder {
    /// `table_schema` 0 means the newest schema.
    #[must_use]
    pub const fn new(table_schema: u64) -> Self {
        Self {
            table_schema: if table_schema == 0 {
                TYPE_FACTS_TABLE_SCHEMA_V24
            } else {
                table_schema
            },
        }
    }

    pub fn encode(&self, input: TransitionInput<'_>) -> Result<EncodedTransition, EncodeError> {
        self.encode_with(input, None)
    }

    /// Streams the rows to `sink` and hands it the header last.
    pub fn encode_into(
        &self,
        input: TransitionInput<'_>,
        sink: &mut dyn TransitionSink,
    ) -> Result<EncodedTransition, EncodeError> {
        self.encode_with(input, Some(sink))
    }

    fn encode_with(
        &self,
        input: TransitionInput<'_>,
        sink: Option<&mut dyn TransitionSink>,
    ) -> Result<EncodedTransition, EncodeError> {
        if !SUPPORTED_TABLE_SCHEMAS.contains(&self.table_schema) {
            return Err(EncodeError::invalid(format!(
                "unsupported Wire table schema {}",
                self.table_schema
            )));
        }
        let (mode, base_generation) = validate(&input)?;
        let target = input.target;

        let (path_operations, symbol_operations) = match (mode, input.base) {
            (TransitionMode::Delta, Some(base)) => {
                if input.sparse {
                    let manifest = target
                        .transport
                        .as_ref()
                        .filter(|manifest| manifest.exact)
                        .ok_or(EncodeError::SparseUnavailable)?;
                    // Expanded path rows moved to the client. The active
                    // manifest carries path changes only; the client derives
                    // symbol and reference closure and patches its table.
                    (
                        plan_sparse_paths(target, manifest),
                        plan_exact_symbol_delta(base, target, manifest),
                    )
                } else if let Some(manifest) = target.transport.as_ref().filter(|manifest| {
                    manifest.exact
                        && base.state_id != 0
                        && manifest.base_generation == base.generation
                        && manifest.base_state_id == base.state_id
                }) {
                    (
                        plan_exact_path_delta(base, target, manifest),
                        plan_exact_symbol_delta(base, target, manifest),
                    )
                } else {
                    (
                        plan_fallback_path_delta(base, target),
                        plan_fallback_symbol_delta(base, target),
                    )
                }
            }
            _ => (
                plan_full(target)?,
                target
                    .symbols
                    .iter()
                    .map(|fact| SymbolOpRef {
                        id: &fact.id,
                        body: SymbolBody::Replace(fact),
                    })
                    .collect(),
            ),
        };
        let counts = (path_operations.len(), symbol_operations.len());
        if mode == TransitionMode::Delta && base_generation == target.generation && counts == (0, 0)
        {
            return Ok(EncodedTransition {
                bytes: Vec::new(),
                mode,
                path_operations: 0,
                symbol_operations: 0,
            });
        }
        let bytes = write_transition(
            &Plan {
                mode,
                table_schema: self.table_schema,
                base_generation,
                target_generation: target.generation,
                project_id: input.project_id,
                base_state_token: input.base_state_token,
                paths: &path_operations,
                symbols: &symbol_operations,
            },
            sink,
        )?;
        Ok(EncodedTransition {
            bytes,
            mode,
            path_operations: counts.0,
            symbol_operations: counts.1,
        })
    }
}

fn validate(input: &TransitionInput<'_>) -> Result<(TransitionMode, u64), EncodeError> {
    let invalid = |message: String| Err(EncodeError::invalid(message));
    if input.project_id.is_empty() {
        return invalid("wire transition project ID is empty".to_owned());
    }
    let target = input.target;
    if target.schema != TYPE_FACTS_SCHEMA_V1 {
        return invalid(format!(
            "wire transition target schema = {}, want {TYPE_FACTS_SCHEMA_V1}",
            target.schema
        ));
    }
    if target.generation == 0 {
        return invalid("wire transition target generation is zero".to_owned());
    }
    let Some(base) = input.base else {
        if !input.base_state_token.is_empty() {
            return invalid("full wire transition carries a base state token".to_owned());
        }
        return Ok((TransitionMode::Full, 0));
    };
    if base.schema != TYPE_FACTS_SCHEMA_V1 {
        return invalid(format!(
            "wire transition base schema = {}, want {TYPE_FACTS_SCHEMA_V1}",
            base.schema
        ));
    }
    if input.base_state_token.is_empty() {
        return invalid("delta wire transition has no base state token".to_owned());
    }
    if base.generation == 0 {
        return invalid("delta wire transition base generation is zero".to_owned());
    }
    if target.generation < base.generation {
        return invalid(format!(
            "wire transition target generation {} precedes base generation {}",
            target.generation, base.generation
        ));
    }
    Ok((TransitionMode::Delta, base.generation))
}

/// A transition ready to write: its identity and every operation.
struct Plan<'a, 'b> {
    mode: TransitionMode,
    table_schema: u64,
    base_generation: u64,
    target_generation: u64,
    project_id: &'a str,
    base_state_token: &'a str,
    paths: &'b [PathOpRef<'a>],
    symbols: &'b [SymbolOpRef<'a>],
}

fn write_transition(
    plan: &Plan<'_, '_>,
    mut sink: Option<&mut dyn TransitionSink>,
) -> Result<Vec<u8>, EncodeError> {
    let mut rows = RowWriter::new(
        sink.as_mut()
            .map(|sink| &mut **sink as &mut dyn TransitionSink),
    );
    rows.text(plan.project_id);
    rows.text(plan.base_state_token);
    // Canonical keys sit next to one another in the prefix-coded dictionary,
    // before replacement bodies introduce less-local strings.
    for operation in plan.paths {
        rows.intern(operation.path);
    }
    for operation in plan.symbols {
        rows.intern(operation.id);
    }
    for operation in plan.symbols {
        if let SymbolBody::ReplaceReferencePath { path, .. } = operation.body {
            rows.intern(path);
        }
    }
    rows.u64(plan.paths.len() as u64);
    for (index, operation) in plan.paths.iter().enumerate() {
        write_path_op(&mut rows, operation, plan.table_schema)
            .map_err(|error| error.within(&format!("encode path operation {index}")))?;
    }
    rows.u64(plan.symbols.len() as u64);
    for (index, operation) in plan.symbols.iter().enumerate() {
        write_symbol_op(&mut rows, plan.mode, operation)
            .map_err(|error| error.within(&format!("encode symbol operation {index}")))?;
    }
    let (row_bytes, dictionary) = rows.finish()?;

    let mut frame = Vec::with_capacity(64 + row_bytes.len());
    v3::push_uvarint(&mut frame, TABLE_TRANSITION_VERSION);
    v3::push_uvarint(
        &mut frame,
        match plan.mode {
            TransitionMode::Full => 0,
            TransitionMode::Delta => 1,
        },
    );
    v3::push_uvarint(&mut frame, plan.table_schema);
    v3::push_uvarint(&mut frame, plan.base_generation);
    v3::push_uvarint(&mut frame, plan.target_generation);
    append_packed_dictionary(&mut frame, &dictionary);
    if let Some(sink) = sink {
        sink.finish(&frame)
            .map_err(|error| EncodeError::Sink(format!("finish transition arena: {error}")))?;
        return Ok(Vec::new());
    }
    frame.extend_from_slice(&row_bytes);
    Ok(frame)
}

/// Decodes `input` and encodes the result again at the schema it names. A
/// frame the Go producer wrote comes back byte for byte.
pub fn reencode_table_transition(input: &[u8]) -> Result<Vec<u8>, String> {
    let transition = v3::decode_table_transition(input)?;
    let paths: Vec<PathOpRef<'_>> = transition.paths.iter().map(path_ref).collect();
    let symbols: Vec<SymbolOpRef<'_>> = transition.symbols.iter().map(symbol_ref).collect();
    write_transition(
        &Plan {
            mode: transition.mode,
            table_schema: transition.table_schema,
            base_generation: transition.base_generation,
            target_generation: transition.target_generation,
            project_id: &transition.project_id,
            base_state_token: &transition.base_state_token,
            paths: &paths,
            symbols: &symbols,
        },
        None,
    )
    .map_err(|error| error.to_string())
}

fn path_ref(operation: &PathOp) -> PathOpRef<'_> {
    fn slot<T, U: ?Sized>(op: &SlotOp<T>, borrow: impl FnOnce(&T) -> &U) -> Slot<&U> {
        match op {
            SlotOp::Unchanged => Slot::Unchanged,
            SlotOp::Replace(value) => Slot::Replace(borrow(value)),
            SlotOp::Remove => Slot::Remove,
        }
    }
    PathOpRef {
        path: &operation.path,
        source: slot(&operation.source, |hash| hash),
        entities: slot(&operation.entities, |rows| rows.as_slice()),
        file: slot(&operation.file, |file| file),
    }
}

fn symbol_ref(operation: &SymbolOp) -> SymbolOpRef<'_> {
    match operation {
        SymbolOp::Replace(fact) => SymbolOpRef {
            id: &fact.id,
            body: SymbolBody::Replace(fact),
        },
        SymbolOp::Remove { id } => SymbolOpRef {
            id,
            body: SymbolBody::Remove,
        },
        SymbolOp::ReplaceReferencePath {
            id,
            path,
            references,
        } => SymbolOpRef {
            id,
            body: SymbolBody::ReplaceReferencePath { path, references },
        },
    }
}

/// What a decoded transition declares about itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionSummary {
    pub mode: TransitionMode,
    pub table_schema: u64,
    pub base_generation: u64,
    pub target_generation: u64,
    pub project_id: String,
    pub base_state_token: String,
    pub path_operations: usize,
    pub symbol_operations: usize,
    /// The id of every symbol operation, in wire order.
    pub symbol_ids: Vec<String>,
    /// The distinct symbols entity rows name, ascending.
    pub entity_symbol_ids: Vec<String>,
}

/// The symbol rows of a full transition (a `symbols` answer), decoded and
/// materialized with the client's own decoder; for tests, not for applying
/// a transition.
pub fn symbol_facts_of_full_transition(input: &[u8]) -> Result<Vec<crate::SymbolFact>, String> {
    let transition = v3::decode_table_transition(input)?;
    if transition.mode != TransitionMode::Full {
        return Err("transition is not a full table".into());
    }
    let table = crate::FactTable::materialize_full(transition);
    Ok(table
        .symbols()
        .map(|symbol| crate::SymbolFact {
            id: symbol.id().into(),
            references: symbol.references().cloned().collect::<Vec<_>>().into(),
            alias_target: symbol.alias_target().into(),
            declarations: symbol.declarations().to_vec().into(),
        })
        .collect())
}

/// Fully decodes `input` with the client's own decoder and reports its header
/// and operation counts; for tests and tracing, not for applying a transition.
pub fn inspect_table_transition(input: &[u8]) -> Result<TransitionSummary, String> {
    let transition = v3::decode_table_transition(input)?;
    let mut entity_symbol_ids = std::collections::BTreeSet::new();
    for operation in &transition.paths {
        if let SlotOp::Replace(entities) = &operation.entities {
            entity_symbol_ids.extend(
                entities
                    .iter()
                    .filter(|entity| !entity.symbol.is_empty())
                    .map(|entity| entity.symbol.to_string()),
            );
        }
    }
    Ok(TransitionSummary {
        mode: transition.mode,
        table_schema: transition.table_schema,
        base_generation: transition.base_generation,
        target_generation: transition.target_generation,
        project_id: transition.project_id.to_string(),
        base_state_token: transition.base_state_token.to_string(),
        path_operations: transition.paths.len(),
        symbol_operations: transition.symbols.len(),
        symbol_ids: transition
            .symbols
            .iter()
            .map(|operation| operation.id().to_string())
            .collect(),
        entity_symbol_ids: entity_symbol_ids.into_iter().collect(),
    })
}
