//! Demand-shaped invocation proof transcripts.
//!
//! These facts are read directly from one live TypeScript-Go generation. They
//! are intentionally not retained entity-table rows: package proof callers pay
//! for callable trees and censuses, ordinary editor analysis does not.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    CallKind, CallTargetSet, Callability, Declaration, Location, PrimitiveLiteralCandidate,
    ResolvedCallValidity, ResolvedDeclaration, TypeDescriptor,
};

pub const MAX_INVOCATION_CALLABLE_DEPTH: usize = 8;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InvocationDomain {
    Signature,
    Bindings,
    Omissions,
    Parameters,
    Result,
    Uses,
    ControlFlow,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InvocationCompleteness(pub Vec<InvocationDomain>);

impl InvocationCompleteness {
    #[must_use]
    pub fn contains(&self, domain: InvocationDomain) -> bool {
        self.0.contains(&domain)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvocationDemand {
    pub location: Location,
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub callable_depth: usize,
    #[serde(default, skip_serializing_if = "is_false")]
    pub census: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArgumentBindingDisposition {
    Direct,
    ExactTupleSpread,
    UnknownLengthSpread,
    Unmapped,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpandedArgumentSlot {
    pub expanded_index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tuple_index: Option<usize>,
    pub parameter_index: usize,
    #[serde(default, skip_serializing_if = "is_false")]
    pub rest: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FormalRange {
    pub start: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_exclusive: Option<usize>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub unbounded: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArgumentBinding {
    pub argument_index: usize,
    pub location: Location,
    pub disposition: ArgumentBindingDisposition,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<ExpandedArgumentSlot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub possible: Option<FormalRange>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub reason: Arc<str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ValueProtocol {
    Plain,
    Promise,
    AsyncIterable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InvocationConstructability {
    Constructable,
    NonConstructable,
    Mixed,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PathSegmentKind {
    Property,
    Tuple,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PathSegment {
    pub kind: PathSegmentKind,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub property: Arc<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PathPresence {
    Required,
    Optional,
    Absent,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Discriminant {
    pub property: Arc<str>,
    pub value: PrimitiveLiteralCandidate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValueAlternative {
    pub index: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub discriminants: Vec<Discriminant>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_reasons: Vec<Arc<str>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CallablePathFact {
    pub alternative: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<PathSegment>,
    pub presence: PathPresence,
    pub callability: Callability,
    pub constructability: InvocationConstructability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declaration: Option<Declaration>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub complete: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_reasons: Vec<Arc<str>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FinitePartitionAxis {
    Literal,
    Callability,
    Protocol,
    Tuple,
    Discriminant,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FiniteCase {
    pub kind: Arc<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub literal: Option<PrimitiveLiteralCandidate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<ValueProtocol>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tuple_length: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub discriminants: Vec<Discriminant>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FinitePartition {
    pub axis: FinitePartitionAxis,
    #[serde(default, skip_serializing_if = "is_false")]
    pub complete: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cases: Vec<FiniteCase>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValuePrimitiveDomain {
    #[serde(default, skip_serializing_if = "is_false")]
    pub may_be_string: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub may_be_number: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub may_be_boolean: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub may_be_big_int: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub may_be_symbol: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub may_be_null: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub may_be_undefined: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub may_be_object: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub numbers_finite: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub unknown: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvocationValueFact {
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub type_descriptor: Option<TypeDescriptor>,
    pub callability: Callability,
    pub constructability: InvocationConstructability,
    pub primitive: ValuePrimitiveDomain,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<ValueAlternative>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub partitions: Vec<FinitePartition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_reasons: Vec<Arc<str>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectedParameter {
    pub index: usize,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub symbol: Arc<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declaration: Option<Declaration>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub rest: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub defaulted: bool,
    pub value: InvocationValueFact,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub callable_paths: Vec<CallablePathFact>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectedSignature {
    pub identity: Arc<str>,
    pub declaration: ResolvedDeclaration,
    pub overload_ordinal: usize,
    pub minimum_argument_count: usize,
    #[serde(default, skip_serializing_if = "is_false")]
    pub has_rest: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<SelectedParameter>,
    pub result: InvocationValueFact,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub result_callable_paths: Vec<CallablePathFact>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParameterUseKind {
    DirectCall,
    AliasCall,
    ArgumentKnown,
    ArgumentUnknown,
    PropertyAccess,
    Return,
    Storage,
    Capture,
    UnknownEscape,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParameterUse {
    pub parameter_index: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub binding_path: Vec<PathSegment>,
    pub location: Location,
    pub kind: ParameterUseKind,
    #[serde(default, skip_serializing_if = "is_false")]
    pub alias: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub captured: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Reachability {
    Reachable,
    Unreachable,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReturnSite {
    pub location: Location,
    pub reach: Reachability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<InvocationValueFact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub captures: Vec<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThrowSite {
    pub location: Location,
    pub reach: Reachability,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BranchSite {
    pub location: Location,
    pub reach: Reachability,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub partitions: Vec<FinitePartition>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlFlowCensus {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub returns: Vec<ReturnSite>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub throws: Vec<ThrowSite>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub branches: Vec<BranchSite>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unsupported: Vec<Arc<str>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvocationTranscript {
    pub location: Location,
    pub validity: ResolvedCallValidity,
    pub kind: CallKind,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub target: Arc<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targets: Option<CallTargetSet>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_signature: Option<SelectedSignature>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bindings: Vec<ArgumentBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub omitted_parameters: Vec<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameter_uses: Vec<ParameterUse>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_flow: Option<ControlFlowCensus>,
    #[serde(rename = "complete", default)]
    pub completeness: InvocationCompleteness,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_reasons: Vec<Arc<str>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TranscriptSourceDigest {
    pub path: Arc<str>,
    pub sha256: Arc<str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvocationEnvelope {
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub project_id: Arc<str>,
    pub generation: u64,
    pub demand_sha256: Arc<str>,
    pub module_graph_sha256: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub schema_sha256: Arc<str>,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub producer_build: Arc<str>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<TranscriptSourceDigest>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_reasons: Vec<Arc<str>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvocationAnswer {
    pub transcripts: Vec<InvocationTranscript>,
    pub envelope: InvocationEnvelope,
}

const fn is_false(value: &bool) -> bool {
    !*value
}

const fn is_zero_usize(value: &usize) -> bool {
    *value == 0
}
