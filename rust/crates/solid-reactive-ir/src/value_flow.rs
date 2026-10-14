//! ADR 0309: a reactive value's flow is one fact.
//!
//! Private observations over settled proven seeds, consumed only by opt-in
//! audit and timing output. Call, Get, construct, and protocol records describe
//! source forms; they establish no runtime invocation, tracking, ownership,
//! cardinality, or completeness. Function topology is lexical evidence only.
//!
//! Every observed reference is classified or explicitly unknown. Missing facts
//! remain gaps. Reference-table presence never attests completeness: every seed
//! remains open, including empty, excluded-only, and import-only records.
//! Transfers are not followed, read rows remain unbound, and no terminal or
//! certification consumer exists. No shared lookup cache is queried.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use serde::Serialize;
use solid_facts::ast::{
    ArrayElementKind, BindingShape, CoerciveOperandKind, ExportKind, FunctionKind, IdentifierRole,
    ImportKind, ModuleHazardKind, PatternSlot,
};
use solid_facts::core::Span;
use solid_facts::{FileFacts, TypeScriptSymbol};

use crate::ReactiveSourceKind;
use crate::identity::SymbolId;
use crate::pipeline::AnalysisContext;

/// The environment variable naming the file the shadow audit is written to.
const AUDIT_VARIABLE: &str = "SOLID_CHECKER_VALUE_FLOW_AUDIT";

/// Transparent wrappers are peeled at most this many levels deep. The bound is
/// never a reason to stop silently: an exhausted climb records
/// [`UnknownReason::WrapperDepth`].
const WRAPPER_DEPTH: usize = 64;

/// A bounded private descent through exact normalized wrapper spans. No shared
/// indexes or semantic resolution are consulted; missing/deep evidence stays open.
fn peel_wrapper(ast: &solid_facts::ast::AstFacts, mut at: Span) -> Option<Span> {
    for _ in 0..=WRAPPER_DEPTH {
        let Ok(index) = ast
            .transparent_wrappers
            .binary_search_by_key(&at, |wrapper| wrapper.span)
        else {
            return Some(at);
        };
        let inner = ast.transparent_wrappers[index].inner;
        if inner == at {
            return None;
        }
        at = inner;
    }
    None
}

// ---------------------------------------------------------------------------
// Identities
// ---------------------------------------------------------------------------

/// A durable source address. Generation symbol IDs differ between cold and
/// retained runs, so the fact never names a node by one.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub(crate) struct SourceAddress {
    pub(crate) path: Arc<str>,
    pub(crate) span: Span,
}

impl SourceAddress {
    fn new(path: &Arc<str>, span: Span) -> Self {
        Self {
            path: path.clone(),
            span,
        }
    }
}

/// What makes a seed a proven reactive value. Typed candidates (ADR 0294) are
/// never seeds.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SeedKind {
    Accessor,
    Store,
    PropRoot,
}

/// An abstract allocation site.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AllocationKind {
    /// The props object a JSX element builds for its component.
    JsxProps,
}

/// A property key, in the one spelling ToPropertyKey gives it: an array
/// element's index, `{0: v}`, `{"0": v}`, `{[0]: v}` and `o[0]` are all
/// `Named("0")`. A key the syntax does not state is [`Self::Summarized`]: the
/// container then has one summarized element, which is may-flow and never
/// selects a value.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub(crate) enum PropertyKey {
    Named(Box<str>),
    Summarized,
}

impl PropertyKey {
    /// An array index, spelled as the property key it is.
    fn index(index: u32) -> Self {
        Self::Named(index.to_string().into())
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(crate) enum FlowNodeKind {
    /// The proven reactive value's own binding.
    Seed { seed: SeedKind },
    /// A variable binding, addressed by its pattern.
    Binding,
    /// A formal parameter slot, addressed by its function.
    Parameter { index: u32 },
    /// The return slot of a function, addressed by the function.
    Return,
    /// An actual argument slot of an invocation, addressed by the invocation.
    /// A tagged template's substitutions are its arguments from index 1.
    ActualArgument { index: u32 },
    /// An allocation site, addressed by the expression that allocates it.
    Allocation { allocation: AllocationKind },
    /// A property slot. Addressed by the property's own definition when the
    /// syntax states no allocation (an object literal property), and by the
    /// allocation otherwise.
    Property { key: PropertyKey },
    /// The value of a value-preserving expression (a conditional or logical
    /// expression), addressed by that expression.
    Value,
    /// A module export slot, addressed by the export specifier.
    Export { name: Box<str> },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FlowNode {
    pub(crate) address: SourceAddress,
    #[serde(flatten)]
    pub(crate) kind: FlowNodeKind,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum FlowEdgeKind {
    /// `const alias = value`, `alias = value`.
    Alias,
    /// `f(value)`, `new C(value)`. Which callee receives it (a project
    /// function, `Proxy`, `Array.from`, a callback slot) is the target's
    /// question, not the edge's.
    Argument,
    /// `function f(v = value)`, `{ v = value } = o`.
    DefaultInitializer,
    /// `const { key: local } = owner` where `owner.key` is the value: the
    /// pattern Gets it into its target.
    Destructure,
    /// `` tag`${value}` ``: the tag receives the substitution as an argument.
    TaggedTemplateSubstitution,
    /// `<C prop={value} />`.
    JsxAttribute,
    /// `<C {...value} />`.
    JsxSpread,
    /// `return value`, `() => value`.
    Return,
    /// `{ key: value }`, `{ value }`, `o.key = value`.
    PropertyStore,
    /// `[value]`: an element store into an array allocation.
    ContainerStore,
    /// `c ? value : other`.
    ConditionalBranch,
    /// `value || other`, `value ?? other`, `other && value`.
    LogicalOperand,
    /// `export { value }`.
    Export,
    /// `import { value } from "…"`. The import binding is an alias of the
    /// seed, and its references are censused in the same seed record.
    Import,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FlowEdge {
    pub(crate) from: FlowNode,
    pub(crate) to: FlowNode,
    pub(crate) kind: FlowEdgeKind,
    /// The reference that carries the value along this edge.
    pub(crate) at: SourceAddress,
}

/// Where code can read the value: calls, and the Gets and protocol steps that
/// run code.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(crate) enum ReadEventKind {
    Call,
    Construct,
    /// The tag of a tagged template is called by the template.
    TaggedTemplateCall,
    /// A Get on a store or a props object, which runs its getter.
    PropertyGet {
        key: PropertyKey,
    },
    /// Object destructuring Gets every key its pattern names.
    DestructuringGet,
    /// The iteration protocol: `for…of`, array spread or destructuring.
    Iteration,
    /// Own-property enumeration and Gets: object spread, an object rest
    /// pattern.
    Enumeration,
    /// `await value` Gets `then`.
    ThenLookup,
}

/// The invocation context a read event runs under, from existing facts only.
/// The stack and deferral contexts of the execution-role owners are bound
/// later; until then `Unbound` is the honest answer and never certifies.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum InvocationContext {
    Unbound,
}

/// A recognized operation whose terminal row (does it invoke the value or
/// not) belongs to the terminal table. None has a row yet.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(crate) enum OperationKind {
    /// `if (value)`, `value ? a : b`: a truthiness or switch test.
    Test,
    /// `value == null`.
    NullishTest,
    /// ToPrimitive/ToString/ToNumber/ToBoolean: string concatenation, an
    /// untagged template substitution, `!value`, `+value`.
    Coercion,
    /// A Get on an accessor function object: `value.call`, `value.length`.
    FunctionMemberGet { key: PropertyKey },
    /// Object destructuring of an accessor function object.
    FunctionDestructuring,
    /// A JSX child expression, which the compiler lowers.
    JsxChild,
    /// An attribute value of an intrinsic (DOM) element. The compiler lowers
    /// it to a runtime operation on the element: `ref` and event handlers
    /// invoke the value, other attributes assign or coerce it. There is no
    /// props object to transfer into.
    IntrinsicAttribute { name: Box<str> },
}

/// Positive evidence that a reference is not a runtime use of the value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ExclusionReason {
    /// Inside a TypeScript type query, which is erased.
    TypeOnly,
    /// The name of a property definition, not a value.
    PropertyName,
}

/// Why a use, or a seed, has no closed flow. These are explicit escapes; the
/// census never answers "clean" by omission.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum UnknownReason {
    /// The runtime transform's execution of this exact source value is open.
    RuntimeExecution,
    /// Normalized syntax cannot establish the eval binding/call semantics.
    EvalBindingUnattested,
    /// An exact declaration, location, alias, or seed kind is missing/ambiguous.
    IdentityUnattested,
    /// No normalized fact table claims the reference's parent syntax.
    UnrecordedParent,
    /// Two fact tables claim different parents for the same reference.
    AmbiguousParent,
    /// Transparent wrappers nested deeper than [`WRAPPER_DEPTH`].
    WrapperDepth,
    /// The seed binding itself is written.
    BindingWritten,
    /// A projection whose key is not static: `value[key]`.
    ComputedKey,
    /// A write or delete through the seed: `store.key = v`.
    WriteThroughSeed,
    /// The reference is in a file outside the analysis.
    ReferenceOutsideAnalysis,
    /// The reference names no recorded identifier or property in its file.
    NotAReference,
    /// A return outside any function.
    ReturnOutsideFunction,
    /// The seed has no declaration in an analyzed file.
    DeclarationOutsideAnalysis,
    /// An `arguments` object of the function whose parameter the seed is
    /// aliases it.
    ArgumentsObject,
    /// `{ __proto__: value }` installs the value as a prototype, not as an
    /// own property.
    PrototypeInstallation,
    /// `using x = value`: the declaration Gets the value's dispose method and
    /// calls it at block exit.
    DisposalProtocol,
    /// A module namespace that exports the seed is accessed by a computed key
    /// or escapes as a whole (`ns[k]`, `Object.values(ns)`, `f(ns)`).
    NamespaceEscape,
    /// Reference completeness is unattested, even when every table is present.
    ReferenceCensusIncomplete,
    /// The seed is exported or reaches an export: published (decision 2).
    Published,
    /// Shadow mode: the edge is recorded but not followed.
    EdgeNotFollowed,
    /// Shadow mode: the read event is not bound to a read row and execution
    /// evidence.
    ReadRowUnbound,
    /// Shadow mode: the operation has no terminal-table row.
    NoTerminalRow,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "disposition", rename_all = "kebab-case")]
pub(crate) enum UseDisposition {
    Transfer {
        edge: FlowEdgeKind,
        to: FlowNode,
    },
    Read {
        event: ReadEventKind,
        at: SourceAddress,
        context: InvocationContext,
    },
    Operation {
        operation: OperationKind,
        at: SourceAddress,
    },
    Excluded {
        reason: ExclusionReason,
    },
    Unknown {
        reason: UnknownReason,
    },
}

/// Syntactic default-initializer containment, with no execution authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "region", rename_all = "kebab-case")]
pub(crate) enum UseRegion {
    /// No exact default-initializer containment is established.
    Unattested,
    /// Exact default-initializer syntax, with no intervening lexical function.
    /// `parameters` describes lexical placement before that function's body;
    /// it does not assert when or whether the initializer executes.
    DefaultInitializer { initializer: Span, parameters: bool },
}

/// One observed reference of a seed (or of an alias binding of it).
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UseRecord {
    pub(crate) reference: SourceAddress,
    /// Lexical function containment only; not an execution frame or owner.
    #[serde(rename = "lexicalFunction")]
    pub(crate) frame: Option<Span>,
    /// Capture is unattested, including class/default/implicit uses.
    pub(crate) captured: Option<bool>,
    pub(crate) region: UseRegion,
    pub(crate) disposition: UseDisposition,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "coverage", rename_all = "kebab-case")]
pub(crate) enum Coverage {
    /// The first escape. "First" is the shortest transfer path, then
    /// canonical source order (decision 7); every use is one step long in
    /// shadow mode, so it is the first open use in source order.
    Open {
        first: Option<SourceAddress>,
        reason: UnknownReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SeedRecord {
    pub(crate) seed: Option<FlowNode>,
    pub(crate) identity_gaps: Vec<IdentityGap>,
    /// Import bindings whose references are this seed's references.
    pub(crate) aliases: Vec<SourceAddress>,
    pub(crate) published: bool,
    pub(crate) uses: Vec<UseRecord>,
    pub(crate) coverage: Coverage,
}

/// A raw location is retained even when it cannot be represented as a span.
/// It is gap evidence, never an alias promoted to the seed's declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IdentityGap {
    pub(crate) location: Option<typefacts::Location>,
    pub(crate) reason: UnknownReason,
}

/// The shadow fact: every seed's census and the edges it records.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ValueFlowShadow {
    pub(crate) seeds: Vec<SeedRecord>,
    pub(crate) edges: Vec<FlowEdge>,
    pub(crate) indexed_files: usize,
}

// ---------------------------------------------------------------------------
// The per-file reverse-use index
// ---------------------------------------------------------------------------

/// The syntactic parent of a value expression, as one normalized fact table
/// states it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Parent {
    Callee {
        call: Span,
        construct: bool,
    },
    Argument {
        call: Span,
        index: u32,
        spread: bool,
    },
    MemberObject {
        member: Span,
    },
    /// `object[expression]`, `{ [expression]: v }`, `{ [expression]: t } = o`:
    /// the key operand, converted with ToPropertyKey.
    ComputedKeyOperand {
        member: Span,
    },
    TemplateSubstitution {
        template: Span,
    },
    TaggedTemplateTag {
        template: Span,
    },
    TaggedTemplateSubstitution {
        template: Span,
        index: u32,
    },
    ArrayElement {
        array: Span,
        index: Option<u32>,
        spread: bool,
    },
    PatternDefault {
        target: Span,
    },
    AwaitOperand,
    JsxChild,
    JsxAttribute {
        element: Span,
        name: Span,
    },
    JsxSpread {
        element: Span,
    },
    ObjectPropertyValue {
        property: usize,
    },
    Return,
    BindingInitializer {
        binding: usize,
    },
    ParameterDefault {
        function: usize,
        index: u32,
    },
    AssignmentValue {
        assignment: usize,
    },
    ConditionalTest {
        conditional: Span,
    },
    ConditionalBranch {
        conditional: Span,
    },
    LogicalOperand {
        logical: Span,
    },
    ExportSpecifier {
        export: usize,
        specifier: usize,
    },
    /// `export default expression`.
    DefaultExport {
        export: Span,
    },
    /// The value of `{ __proto__: value }`.
    PrototypeValue,
    /// The initializer of a `using` declarator.
    DisposalInitializer,
}

/// Facts about the operation the parent performs on the expression, which
/// several tables state for the same span. They refine a parent; they never
/// replace one.
#[derive(Clone, Copy, Debug, Default)]
struct Annotations {
    iterated: bool,
    coerced: bool,
    nullish_test: bool,
    test: bool,
    spread: bool,
    written: bool,
    deleted: bool,
}

/// The reverse-use index of one file: for each value-expression span, the
/// parents and annotations the normalized fact tables state for it. Built
/// once per file, only for files a seed reaches.
pub(crate) struct FileUseIndex<'a> {
    file: &'a FileFacts,
    path: Arc<str>,
    parents: HashMap<Span, Vec<Parent>>,
    annotations: HashMap<Span, Annotations>,
    wrappers: HashMap<Span, Span>,
    member_properties: HashMap<Span, Span>,
    /// Every member expression's property span and, for a computed member,
    /// the literal key it states (`None` when it states none).
    members: HashMap<Span, (Span, Option<Option<PropertyKey>>)>,
    property_keys: HashMap<Span, usize>,
    import_locals: HashMap<Span, Span>,
    references: BTreeSet<Span>,
    binder_references: HashMap<Span, Vec<Span>>,
    /// Function spans sorted by start, for the innermost-frame query.
    frames: Vec<Span>,
    /// Function spans, by function index.
    function_spans: Vec<Span>,
    /// Function bodies, by function span.
    bodies: HashMap<Span, Span>,
    /// Every default initializer: parameter defaults and pattern defaults.
    defaults: Vec<Span>,
    /// Object pattern keys whose targets differ from them, to their targets.
    pattern_keys: HashMap<Span, Span>,
    /// Object patterns with a rest element.
    rest_patterns: std::collections::HashSet<Span>,
    /// Direct `eval(…)` calls: `ModuleHazardKind::Eval`.
    evals: Vec<(Span, UnknownReason)>,
    /// References to an `arguments` object: unresolved identifier references
    /// spelled `arguments` (a module is strict, so no binding has the name).
    arguments_references: Vec<Span>,
    /// Non-arrow functions, which own an `arguments` object: span and body.
    argument_owners: Vec<(Span, Span)>,
    /// Elements of intrinsic (DOM) JSX elements.
    intrinsic_elements: std::collections::HashSet<Span>,
}

impl<'a> FileUseIndex<'a> {
    pub(crate) fn new(file: &'a FileFacts) -> Self {
        let ast = &*file.ast;
        let mut index = Self {
            file,
            path: Arc::from(file.path.as_str()),
            parents: HashMap::new(),
            annotations: HashMap::new(),
            wrappers: HashMap::new(),
            member_properties: HashMap::new(),
            members: HashMap::new(),
            property_keys: HashMap::new(),
            import_locals: HashMap::new(),
            references: BTreeSet::new(),
            binder_references: HashMap::new(),
            frames: Vec::new(),
            function_spans: ast.functions.iter().map(|function| function.span).collect(),
            bodies: ast
                .functions
                .iter()
                .map(|function| (function.span, function.body))
                .collect(),
            defaults: Vec::new(),
            pattern_keys: HashMap::new(),
            rest_patterns: std::collections::HashSet::new(),
            evals: ast
                .calls
                .iter()
                .filter_map(|call| {
                    if call.construct {
                        return None;
                    }
                    let callee = peel_wrapper(ast, call.callee)?;
                    let identifier = ast.identifiers.iter().find(|identifier| {
                        identifier.span == callee && identifier.role == IdentifierRole::Reference
                    })?;
                    // Exact syntax identity plus a positively resolved local declaration
                    // can reject a lexical eval. Absence alone cannot prove a global.
                    if file.source_text(identifier.span) != Some("eval")
                        || ast.reference_declaration(identifier.span).is_some()
                    {
                        return None;
                    }
                    Some((call.span, UnknownReason::EvalBindingUnattested))
                })
                .collect(),
            arguments_references: ast
                .identifiers
                .iter()
                .filter(|identifier| {
                    identifier.role == IdentifierRole::Reference
                        && file.source_text(identifier.span) == Some("arguments")
                        && ast.reference_declaration(identifier.span).is_none()
                })
                .map(|identifier| identifier.span)
                .collect(),
            argument_owners: ast
                .functions
                .iter()
                .filter(|function| function.kind != FunctionKind::Arrow)
                .map(|function| (function.span, function.body))
                .collect(),
            intrinsic_elements: ast
                .jsx_elements
                .iter()
                .filter(|element| {
                    element.member_object.is_none()
                        && file
                            .source_text(element.name.span)
                            .and_then(|name| name.chars().next())
                            .is_some_and(|first| first.is_ascii_lowercase())
                })
                .map(|element| element.span)
                .collect(),
        };
        for wrapper in &ast.transparent_wrappers {
            if wrapper.inner != wrapper.span {
                index.wrappers.insert(wrapper.inner, wrapper.span);
            }
        }
        for call in &ast.calls {
            index.parent(
                call.callee,
                Parent::Callee {
                    call: call.span,
                    construct: call.construct,
                },
            );
            for (position, argument) in call.arguments.iter().enumerate() {
                let position = u32::try_from(position).unwrap_or(u32::MAX);
                index.parent(
                    argument.span,
                    Parent::Argument {
                        call: call.span,
                        index: position,
                        spread: argument.spread,
                    },
                );
            }
        }
        // A spread argument's span is the spread element; the spread value
        // is its argument.
        let spread_elements = ast
            .spreads
            .iter()
            .map(|spread| (spread.span, spread.argument))
            .collect::<HashMap<_, _>>();
        for call in &ast.calls {
            for (position, argument) in call.arguments.iter().enumerate() {
                if let Some(inner) = argument
                    .spread
                    .then(|| spread_elements.get(&argument.span))
                    .flatten()
                {
                    let position = u32::try_from(position).unwrap_or(u32::MAX);
                    index.parent(
                        *inner,
                        Parent::Argument {
                            call: call.span,
                            index: position,
                            spread: true,
                        },
                    );
                }
            }
        }
        let computed = ast
            .computed_members
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        let literal_keys = ast
            .literal_computed_members
            .iter()
            .map(|member| (member.span, member.key.as_str()))
            .collect::<HashMap<_, _>>();
        for member in &ast.members {
            index.parent(
                member.object,
                Parent::MemberObject {
                    member: member.span,
                },
            );
            if computed.contains(&member.span) {
                index.parent(
                    member.property,
                    Parent::ComputedKeyOperand {
                        member: member.span,
                    },
                );
                let key = literal_keys
                    .get(&member.span)
                    .map(|key| PropertyKey::Named((*key).into()));
                index
                    .members
                    .insert(member.span, (member.property, Some(key)));
            } else {
                index.member_properties.insert(member.property, member.span);
                index.members.insert(member.span, (member.property, None));
            }
        }
        for template in &ast.template_literals {
            for expression in &template.expressions {
                index.parent(
                    *expression,
                    Parent::TemplateSubstitution {
                        template: template.span,
                    },
                );
            }
        }
        for template in &ast.tagged_templates {
            index.parent(
                template.tag,
                Parent::TaggedTemplateTag {
                    template: template.span,
                },
            );
            for (position, substitution) in template.substitutions.iter().enumerate() {
                index.parent(
                    *substitution,
                    Parent::TaggedTemplateSubstitution {
                        template: template.span,
                        index: u32::try_from(position + 1).unwrap_or(u32::MAX),
                    },
                );
            }
        }
        for array in &ast.array_literal_elements {
            // An element's index is static until the first spread.
            let mut static_index = Some(0u32);
            for element in &array.elements {
                match element.kind {
                    ArrayElementKind::Hole => {}
                    ArrayElementKind::Value => index.parent(
                        element.span,
                        Parent::ArrayElement {
                            array: array.span,
                            index: static_index,
                            spread: false,
                        },
                    ),
                    ArrayElementKind::Spread => {
                        index.parent(
                            element.span,
                            Parent::ArrayElement {
                                array: array.span,
                                index: None,
                                spread: true,
                            },
                        );
                        static_index = None;
                    }
                }
                static_index = static_index.and_then(|position| position.checked_add(1));
            }
        }
        for element in &ast.pattern_elements {
            if let Some(default) = element.default {
                index.parent(
                    default,
                    Parent::PatternDefault {
                        target: element.target,
                    },
                );
                index.defaults.push(default);
            }
            if element.slot == PatternSlot::Rest {
                index.rest_patterns.insert(element.pattern);
            }
            if element.slot == PatternSlot::ComputedKey {
                // A computed key's span is its key expression, which is a
                // value converted with ToPropertyKey, never the property the
                // pattern Gets into its target.
                if let Some(key) = element.key {
                    index.parent(
                        key,
                        Parent::ComputedKeyOperand {
                            member: element.pattern,
                        },
                    );
                }
            } else if let Some(key) = element.key
                && key != element.target
                && !element.assignment
            {
                index.pattern_keys.insert(key, element.target);
            }
        }
        for operand in &ast.await_operands {
            index.parent(*operand, Parent::AwaitOperand);
        }
        for child in &ast.jsx_child_expressions {
            index.parent(*child, Parent::JsxChild);
        }
        for element in &ast.jsx_elements {
            for attribute in &element.attributes {
                if let Some(expression) = attribute.expression {
                    index.parent(
                        expression,
                        Parent::JsxAttribute {
                            element: element.span,
                            name: attribute.name,
                        },
                    );
                }
            }
            for spread in &element.spreads {
                index.parent(
                    spread.argument,
                    Parent::JsxSpread {
                        element: element.span,
                    },
                );
            }
        }
        let prototype_installations = ast
            .module_hazards
            .iter()
            .filter(|hazard| hazard.kind == ModuleHazardKind::RuntimeAccessorInstallation)
            .map(|hazard| hazard.span)
            .collect::<std::collections::HashSet<_>>();
        for (position, property) in ast.object_properties.iter().enumerate() {
            let shorthand = property.key == property.value;
            if property.computed {
                // A computed key's span is its key expression: a value
                // converted with ToPropertyKey, not a property name.
                index.parent(
                    property.key,
                    Parent::ComputedKeyOperand {
                        member: property.span,
                    },
                );
            } else if !shorthand {
                index.property_keys.insert(property.key, position);
            }
            // `{ __proto__: v }` sets the literal's prototype; the shorthand,
            // computed and method spellings define an own property.
            let prototype = !property.computed
                && !shorthand
                && property.data
                && prototype_installations.contains(&property.span);
            index.parent(
                property.value,
                if prototype {
                    Parent::PrototypeValue
                } else {
                    Parent::ObjectPropertyValue { property: position }
                },
            );
        }
        for returned in &ast.returns {
            if let Some(argument) = returned.argument {
                index.parent(argument, Parent::Return);
            }
        }
        for (position, function) in ast.functions.iter().enumerate() {
            if let Some(argument) = function
                .expression_return
                .as_ref()
                .and_then(|returned| returned.argument)
            {
                index.parent(argument, Parent::Return);
            }
            for (slot, parameter) in function.parameters.iter().enumerate() {
                if let Some(initializer) = parameter.initializer {
                    index.defaults.push(initializer);
                    index.parent(
                        initializer,
                        Parent::ParameterDefault {
                            function: position,
                            index: u32::try_from(slot).unwrap_or(u32::MAX),
                        },
                    );
                }
            }
        }
        for (position, binding) in ast.bindings.iter().enumerate() {
            if let Some(initializer) = binding.initializer {
                index.parent(
                    initializer,
                    Parent::BindingInitializer { binding: position },
                );
            }
        }
        for (position, assignment) in ast.assignments.iter().enumerate() {
            index.parent(
                assignment.value_span,
                Parent::AssignmentValue {
                    assignment: position,
                },
            );
        }
        for conditional in &ast.conditional_expressions {
            index.parent(
                conditional.test,
                Parent::ConditionalTest {
                    conditional: conditional.span,
                },
            );
            for branch in [conditional.consequent, conditional.alternate] {
                index.parent(
                    branch,
                    Parent::ConditionalBranch {
                        conditional: conditional.span,
                    },
                );
            }
        }
        for logical in &ast.logical_expressions {
            for operand in [logical.left, logical.right] {
                index.parent(
                    operand,
                    Parent::LogicalOperand {
                        logical: logical.span,
                    },
                );
            }
        }
        for (position, export) in ast.exports.iter().enumerate() {
            if export.module.is_none() && export.kind == ExportKind::Default {
                for declaration in &export.declarations {
                    index.parent(
                        declaration.local.span,
                        Parent::DefaultExport {
                            export: export.span,
                        },
                    );
                }
            }
            if export.module.is_none() {
                for (slot, specifier) in export.specifiers.iter().enumerate() {
                    index.parent(
                        specifier.local.span,
                        Parent::ExportSpecifier {
                            export: position,
                            specifier: slot,
                        },
                    );
                }
            }
        }
        for initializer in &ast.disposal_initializers {
            index.parent(*initializer, Parent::DisposalInitializer);
        }
        for import in &ast.imports {
            for binding in &import.bindings {
                index
                    .import_locals
                    .insert(binding.local.span, binding.local.span);
            }
        }
        for span in &ast.iterated_operands {
            index.annotate(*span).iterated = true;
        }
        for span in &ast.coercing_operands {
            index.annotate(*span).coerced = true;
        }
        for operand in &ast.coercive_operands {
            let annotation = index.annotate(operand.span);
            match operand.kind {
                CoerciveOperandKind::StringConcatenation
                | CoerciveOperandKind::LogicalNot
                | CoerciveOperandKind::NumericCoercion => annotation.coerced = true,
            }
        }
        for span in &ast.nullish_test_operands {
            index.annotate(*span).nullish_test = true;
        }
        for span in &ast.conditional_tests {
            index.annotate(*span).test = true;
        }
        for spread in &ast.spreads {
            index.annotate(spread.argument).spread = true;
        }
        for span in ast
            .write_targets
            .iter()
            .chain(&ast.iteration_targets)
            .chain(ast.assignments.iter().map(|assignment| &assignment.target))
        {
            index.annotate(*span).written = true;
        }
        for span in &ast.deleted_targets {
            index.annotate(*span).deleted = true;
        }
        for identifier in &ast.identifiers {
            if identifier.role == IdentifierRole::Reference {
                index.references.insert(identifier.span);
            }
        }
        for (reference, declaration) in &ast.reference_declarations {
            index
                .binder_references
                .entry(*declaration)
                .or_default()
                .push(*reference);
        }
        index.frames = index.function_spans.clone();
        index
            .frames
            .sort_by_key(|span| (span.start, std::cmp::Reverse(span.end)));
        index
    }

    fn parent(&mut self, span: Span, parent: Parent) {
        let parents = self.parents.entry(span).or_default();
        if !parents.contains(&parent) {
            parents.push(parent);
        }
    }

    fn annotate(&mut self, span: Span) -> &mut Annotations {
        self.annotations.entry(span).or_default()
    }

    /// The binder's references to the declaration at `declaration`, in
    /// source order.
    pub(crate) fn binder_references(&self, declaration: Span) -> Vec<Span> {
        let mut references = self
            .binder_references
            .get(&declaration)
            .cloned()
            .unwrap_or_default();
        references.sort_unstable();
        references.dedup();
        references
    }

    /// The innermost function whose span contains `span`. Function spans nest
    /// properly, so the containing function with the latest start is the
    /// innermost one.
    pub(crate) fn frame(&self, span: Span) -> Option<Span> {
        let end = self
            .frames
            .partition_point(|frame| frame.start <= span.start);
        self.frames[..end]
            .iter()
            .rev()
            .find(|frame| frame.contains(span) && **frame != span)
            .copied()
    }

    /// The exact default-initializer syntax containing `span`, if any.
    pub(crate) fn region(&self, span: Span) -> UseRegion {
        let frame = self.frame(span);
        let Some(initializer) = self
            .defaults
            .iter()
            .filter(|default| default.contains(span) && !self.function_between(**default, span))
            .min_by_key(|default| default.end - default.start)
            .copied()
        else {
            return UseRegion::Unattested;
        };
        let parameters = frame
            .and_then(|frame| self.bodies.get(&frame).map(|body| (frame, *body)))
            .is_some_and(|(frame, body)| {
                frame.start <= initializer.start && initializer.end <= body.start
            });
        UseRegion::DefaultInitializer {
            initializer,
            parameters,
        }
    }

    /// Whether a function boundary lies between `outer` and `inner`.
    fn function_between(&self, outer: Span, inner: Span) -> bool {
        self.function_spans
            .iter()
            .any(|function| outer.contains(*function) && function.contains(inner))
    }

    fn context(&self, _at: Span) -> InvocationContext {
        InvocationContext::Unbound
    }

    /// Held lowering premises cannot establish runtime execution. This is a
    /// private uncertainty guard, independent of shared deletion policy/mode.
    fn runtime_uncertain(&self, at: Span) -> bool {
        let ast = &*self.file.ast;
        let compiler = &self.file.compiler;
        ast.jsx_elements
            .iter()
            .any(|element| element.span.contains(at))
            || ast.jsx_fragments.iter().any(|span| span.contains(at))
            || compiler
                .discarded_regions
                .iter()
                .chain(&compiler.tracked_regions)
                .chain(&compiler.untracked_regions)
                .any(|region| region.span.contains(at))
    }

    fn excluded(&self, reference: Span) -> Option<ExclusionReason> {
        let ast = &*self.file.ast;
        if ast.imports.iter().any(|import| {
            import.bindings.iter().any(|binding| {
                (import.type_only || binding.type_only) && binding.local.span.contains(reference)
            })
        }) || ast.exports.iter().any(|export| {
            export.specifiers.iter().any(|specifier| {
                (export.type_only || specifier.type_only)
                    && specifier.local.span.contains(reference)
            })
        }) {
            return Some(ExclusionReason::TypeOnly);
        }
        if ast
            .type_queries
            .iter()
            .any(|query| query.contains(reference))
        {
            return Some(ExclusionReason::TypeOnly);
        }
        None
    }

    fn address(&self, span: Span) -> SourceAddress {
        SourceAddress::new(&self.path, span)
    }

    fn node(&self, span: Span, kind: FlowNodeKind) -> FlowNode {
        FlowNode {
            address: self.address(span),
            kind,
        }
    }

    fn text(&self, span: Span) -> Option<&str> {
        self.file.source.get(span.start as usize..span.end as usize)
    }

    /// The key a static property name states: an identifier, or a literal
    /// [`Self::literal_key`] spells. Anything else is summarized.
    fn static_key(&self, span: Span) -> PropertyKey {
        if let Some(key) = self.literal_key(span) {
            return PropertyKey::Named(key.into());
        }
        let Some(text) = self.text(span) else {
            return PropertyKey::Summarized;
        };
        let identifier = text
            .chars()
            .next()
            .is_some_and(|first| !first.is_ascii_digit())
            && text
                .chars()
                .all(|character| character.is_alphanumeric() || matches!(character, '_' | '$'));
        if identifier {
            PropertyKey::Named(text.into())
        } else {
            PropertyKey::Summarized
        }
    }

    /// The key a literal at `span` names, in ToPropertyKey's spelling: a
    /// quoted string of plain characters, or a decimal integer in canonical
    /// form (`0`, `12`; not `012`, `0x1`, `1e3` or `1.0`). `None` for anything
    /// else, including an identifier.
    fn literal_key(&self, span: Span) -> Option<String> {
        let text = self.text(span)?;
        let quoted = text
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
            .or_else(|| {
                text.strip_prefix('\'')
                    .and_then(|rest| rest.strip_suffix('\''))
            });
        if let Some(unquoted) = quoted {
            let plain = !unquoted.is_empty()
                && unquoted
                    .chars()
                    .all(|character| character.is_alphanumeric() || matches!(character, '_' | '$'));
            return plain.then(|| unquoted.to_string());
        }
        let canonical = !text.is_empty()
            && text.len() <= 9
            && text.chars().all(|character| character.is_ascii_digit())
            && (text == "0" || !text.starts_with('0'));
        canonical.then(|| text.to_string())
    }

    /// Classifies one observed source reference of a value of kind `seed`.
    pub(crate) fn classify(&self, reference: Span, seed: SeedKind) -> UseDisposition {
        if let Some(reason) = self.excluded(reference) {
            return UseDisposition::Excluded { reason };
        }
        // Only a positively identified definition name precedes the uncertainty
        // guard. Computed and shorthand keys are values, not names.
        if self.property_keys.contains_key(&reference) {
            return UseDisposition::Excluded {
                reason: ExclusionReason::PropertyName,
            };
        }
        if self.runtime_uncertain(reference) {
            return UseDisposition::Unknown {
                reason: UnknownReason::RuntimeExecution,
            };
        }
        // A Type Facts reference at a member's property name is a use of the
        // whole member expression: `owner.value` produces the value.
        let expression = if let Some(member) = self.member_properties.get(&reference) {
            *member
        } else if let Some(target) = self.pattern_keys.get(&reference) {
            return self.transfer(FlowEdgeKind::Destructure, *target, FlowNodeKind::Binding);
        } else if self.property_keys.contains_key(&reference) {
            return UseDisposition::Excluded {
                reason: ExclusionReason::PropertyName,
            };
        } else if self.import_locals.contains_key(&reference) {
            return UseDisposition::Transfer {
                edge: FlowEdgeKind::Import,
                to: self.node(reference, FlowNodeKind::Binding),
            };
        } else if self.references.contains(&reference) {
            reference
        } else {
            return UseDisposition::Unknown {
                reason: UnknownReason::NotAReference,
            };
        };
        let mut parents = Vec::new();
        let mut annotations = Annotations::default();
        let mut at = expression;
        let mut depth = 0;
        loop {
            if let Some(found) = self.parents.get(&at) {
                for parent in found {
                    if !parents.contains(parent) {
                        parents.push(*parent);
                    }
                }
            }
            if let Some(found) = self.annotations.get(&at) {
                annotations.iterated |= found.iterated;
                annotations.coerced |= found.coerced;
                annotations.nullish_test |= found.nullish_test;
                annotations.test |= found.test;
                annotations.spread |= found.spread;
                annotations.written |= found.written;
                annotations.deleted |= found.deleted;
            }
            let Some(outer) = self.wrappers.get(&at) else {
                break;
            };
            depth += 1;
            if depth > WRAPPER_DEPTH {
                return UseDisposition::Unknown {
                    reason: UnknownReason::WrapperDepth,
                };
            }
            at = *outer;
        }
        if annotations.written || annotations.deleted {
            return UseDisposition::Unknown {
                reason: UnknownReason::BindingWritten,
            };
        }
        // A `using` initializer is also a binding initializer; the disposal
        // protocol is the use that decides.
        if parents.contains(&Parent::DisposalInitializer) {
            return UseDisposition::Unknown {
                reason: UnknownReason::DisposalProtocol,
            };
        }
        match parents.as_slice() {
            [] => self.annotated(at, annotations),
            [parent] => self.dispose(*parent, at, seed, annotations),
            _ => UseDisposition::Unknown {
                reason: UnknownReason::AmbiguousParent,
            },
        }
    }

    /// A reference with no parent table but an operation table: a `for…of`
    /// operand, an `if` test, a coerced operand, an object spread.
    fn annotated(&self, at: Span, annotations: Annotations) -> UseDisposition {
        let address = self.address(at);
        if annotations.iterated {
            return self.read(ReadEventKind::Iteration, at);
        }
        if annotations.spread {
            return self.read(ReadEventKind::Enumeration, at);
        }
        let operation = if annotations.coerced {
            OperationKind::Coercion
        } else if annotations.nullish_test {
            OperationKind::NullishTest
        } else if annotations.test {
            OperationKind::Test
        } else {
            return UseDisposition::Unknown {
                reason: UnknownReason::UnrecordedParent,
            };
        };
        UseDisposition::Operation {
            operation,
            at: address,
        }
    }

    fn read(&self, event: ReadEventKind, at: Span) -> UseDisposition {
        UseDisposition::Read {
            event,
            at: self.address(at),
            context: self.context(at),
        }
    }

    fn transfer(&self, edge: FlowEdgeKind, span: Span, kind: FlowNodeKind) -> UseDisposition {
        UseDisposition::Transfer {
            edge,
            to: self.node(span, kind),
        }
    }

    fn dispose(
        &self,
        parent: Parent,
        at: Span,
        seed: SeedKind,
        annotations: Annotations,
    ) -> UseDisposition {
        let ast = &*self.file.ast;
        match parent {
            Parent::Callee { call, construct } => self.read(
                if construct {
                    ReadEventKind::Construct
                } else {
                    ReadEventKind::Call
                },
                call,
            ),
            Parent::Argument { spread: true, .. } => self.read(ReadEventKind::Iteration, at),
            Parent::Argument { call, index, .. } => self.transfer(
                FlowEdgeKind::Argument,
                call,
                FlowNodeKind::ActualArgument { index },
            ),
            Parent::MemberObject { member } => {
                if self
                    .annotations
                    .get(&member)
                    .is_some_and(|member| member.written || member.deleted)
                {
                    return UseDisposition::Unknown {
                        reason: UnknownReason::WriteThroughSeed,
                    };
                }
                let Some(key) = self.member_key(member) else {
                    return UseDisposition::Unknown {
                        reason: UnknownReason::ComputedKey,
                    };
                };
                match seed {
                    SeedKind::Accessor => UseDisposition::Operation {
                        operation: OperationKind::FunctionMemberGet { key },
                        at: self.address(member),
                    },
                    SeedKind::Store | SeedKind::PropRoot => {
                        self.read(ReadEventKind::PropertyGet { key }, member)
                    }
                }
            }
            Parent::ComputedKeyOperand { member } => UseDisposition::Operation {
                operation: OperationKind::Coercion,
                at: self.address(member),
            },
            Parent::TemplateSubstitution { template } => UseDisposition::Operation {
                operation: OperationKind::Coercion,
                at: self.address(template),
            },
            Parent::TaggedTemplateTag { template } => {
                self.read(ReadEventKind::TaggedTemplateCall, template)
            }
            Parent::TaggedTemplateSubstitution { template, index } => self.transfer(
                FlowEdgeKind::TaggedTemplateSubstitution,
                template,
                FlowNodeKind::ActualArgument { index },
            ),
            Parent::ArrayElement { spread: true, .. } => self.read(ReadEventKind::Iteration, at),
            Parent::ArrayElement { array, index, .. } => self.transfer(
                FlowEdgeKind::ContainerStore,
                array,
                FlowNodeKind::Property {
                    key: index.map_or(PropertyKey::Summarized, PropertyKey::index),
                },
            ),
            Parent::PatternDefault { target } => self.transfer(
                FlowEdgeKind::DefaultInitializer,
                target,
                FlowNodeKind::Binding,
            ),
            Parent::AwaitOperand => self.read(ReadEventKind::ThenLookup, at),
            Parent::JsxChild => UseDisposition::Operation {
                operation: OperationKind::JsxChild,
                at: self.address(at),
            },
            Parent::JsxAttribute { element, name }
                if self.intrinsic_elements.contains(&element) =>
            {
                UseDisposition::Operation {
                    operation: OperationKind::IntrinsicAttribute {
                        name: self.text(name).unwrap_or_default().into(),
                    },
                    at: self.address(at),
                }
            }
            Parent::JsxAttribute { element, name } => self.transfer(
                FlowEdgeKind::JsxAttribute,
                element,
                FlowNodeKind::Property {
                    key: self.text(name).map_or(PropertyKey::Summarized, |name| {
                        PropertyKey::Named(name.into())
                    }),
                },
            ),
            Parent::JsxSpread { element } => self.transfer(
                FlowEdgeKind::JsxSpread,
                element,
                FlowNodeKind::Allocation {
                    allocation: AllocationKind::JsxProps,
                },
            ),
            Parent::ObjectPropertyValue { property } => {
                let property = &ast.object_properties[property];
                let key = if property.computed {
                    self.literal_key(property.key)
                        .map_or(PropertyKey::Summarized, |key| {
                            PropertyKey::Named(key.into())
                        })
                } else {
                    self.static_key(property.key)
                };
                self.transfer(
                    FlowEdgeKind::PropertyStore,
                    property.span,
                    FlowNodeKind::Property { key },
                )
            }
            Parent::Return => match self.frame(at) {
                Some(function) => {
                    self.transfer(FlowEdgeKind::Return, function, FlowNodeKind::Return)
                }
                None => UseDisposition::Unknown {
                    reason: UnknownReason::ReturnOutsideFunction,
                },
            },
            Parent::BindingInitializer { binding } => {
                let binding = &ast.bindings[binding];
                match binding.shape {
                    BindingShape::Identifier => {
                        self.transfer(FlowEdgeKind::Alias, binding.pattern, FlowNodeKind::Binding)
                    }
                    BindingShape::Array => self.read(ReadEventKind::Iteration, at),
                    BindingShape::Object => match seed {
                        SeedKind::Accessor => UseDisposition::Operation {
                            operation: OperationKind::FunctionDestructuring,
                            at: self.address(binding.pattern),
                        },
                        SeedKind::Store | SeedKind::PropRoot
                            if self.rest_patterns.contains(&binding.pattern) =>
                        {
                            self.read(ReadEventKind::Enumeration, binding.pattern)
                        }
                        SeedKind::Store | SeedKind::PropRoot => {
                            self.read(ReadEventKind::DestructuringGet, binding.pattern)
                        }
                    },
                }
            }
            Parent::ParameterDefault { function, index } => self.transfer(
                FlowEdgeKind::DefaultInitializer,
                self.function_spans[function],
                FlowNodeKind::Parameter { index },
            ),
            Parent::AssignmentValue { assignment } => {
                let target = ast.assignments[assignment].target;
                if self.members.contains_key(&target) {
                    // A __proto__ setter may install a prototype, rather than
                    // defining an own property. Without receiver authority it
                    // must remain an unknown, never an exact store edge.
                    if matches!(self.member_key(target), Some(PropertyKey::Named(key)) if &*key == "__proto__")
                    {
                        return UseDisposition::Unknown {
                            reason: UnknownReason::PrototypeInstallation,
                        };
                    }
                    // A store under a key the syntax does not state goes to
                    // the summarized element: may-flow, never a selection.
                    self.transfer(
                        FlowEdgeKind::PropertyStore,
                        target,
                        FlowNodeKind::Property {
                            key: self.member_key(target).unwrap_or(PropertyKey::Summarized),
                        },
                    )
                } else if self.references.contains(&target) {
                    self.transfer(FlowEdgeKind::Alias, target, FlowNodeKind::Binding)
                } else if annotations.iterated {
                    self.read(ReadEventKind::Iteration, at)
                } else {
                    UseDisposition::Unknown {
                        reason: UnknownReason::UnrecordedParent,
                    }
                }
            }
            Parent::ConditionalTest { conditional } => UseDisposition::Operation {
                operation: OperationKind::Test,
                at: self.address(conditional),
            },
            Parent::ConditionalBranch { conditional } => self.transfer(
                FlowEdgeKind::ConditionalBranch,
                conditional,
                FlowNodeKind::Value,
            ),
            Parent::LogicalOperand { logical } => {
                self.transfer(FlowEdgeKind::LogicalOperand, logical, FlowNodeKind::Value)
            }
            Parent::DefaultExport { export } => self.transfer(
                FlowEdgeKind::Export,
                export,
                FlowNodeKind::Export {
                    name: "default".into(),
                },
            ),
            Parent::PrototypeValue => UseDisposition::Unknown {
                reason: UnknownReason::PrototypeInstallation,
            },
            Parent::DisposalInitializer => UseDisposition::Unknown {
                reason: UnknownReason::DisposalProtocol,
            },
            Parent::ExportSpecifier { export, specifier } => {
                let specifier = &ast.exports[export].specifiers[specifier];
                self.transfer(
                    FlowEdgeKind::Export,
                    specifier.local.span,
                    FlowNodeKind::Export {
                        name: specifier.exported.as_str().into(),
                    },
                )
            }
        }
    }

    /// The key a member expression states: its static name, or a computed
    /// member's literal key. `None` for a computed key the syntax does not
    /// state.
    fn member_key(&self, member: Span) -> Option<PropertyKey> {
        match self.members.get(&member)? {
            (property, None) => Some(self.static_key(*property)),
            (_, Some(literal)) => literal.clone(),
        }
    }

    /// The uses no reference spells that can reach the binding declared at
    /// `declaration`: a direct `eval` anywhere in its scope (which can name
    /// it), and, for a parameter, every `arguments` reference of its function
    /// (which aliases it). The scope is the innermost function containing the
    /// declaration, or the module.
    fn dynamic_scope_uses(&self, declaration: Span) -> Vec<(Span, UnknownReason)> {
        let scope = self.frame(declaration);
        let in_scope = |span: Span| scope.is_none_or(|scope| scope.contains(span));
        let mut uses = self
            .evals
            .iter()
            .filter(|(at, _)| in_scope(*at))
            .copied()
            .collect::<Vec<_>>();
        let owner = self
            .argument_owners
            .iter()
            .filter(|(function, body)| {
                function.start <= declaration.start && declaration.end <= body.start
            })
            .max_by_key(|(function, _)| function.start);
        if let Some((function, _)) = owner {
            uses.extend(
                self.arguments_references
                    .iter()
                    .filter(|reference| {
                        function.contains(**reference)
                            && !self.argument_owners.iter().any(|(inner, _)| {
                                inner != function
                                    && function.contains(*inner)
                                    && inner.contains(**reference)
                            })
                    })
                    .map(|reference| (*reference, UnknownReason::ArgumentsObject)),
            );
        }
        uses
    }

    /// The runtime references of the namespace binding declared at
    /// `declaration` that are not a static member Get (`ns.name`, whose name
    /// Type Facts resolves to the export it reads). Each can reach every
    /// export of the module.
    fn namespace_escapes(&self, declaration: Span) -> Vec<Span> {
        self.binder_references(declaration)
            .into_iter()
            .filter(|reference| self.excluded(*reference).is_none())
            .filter(|reference| {
                let static_member = self.parents.get(reference).is_some_and(|parents| {
                    matches!(
                        parents.as_slice(),
                        [Parent::MemberObject { member }]
                            if matches!(self.members.get(member), Some((_, None)))
                    )
                });
                !static_member
            })
            .collect()
    }

    /// Whether the declaration is published by an export of this module.
    /// A declaration inside a local export and outside its nested functions
    /// is published, whatever its pattern. Over-answering keeps a seed open.
    fn published(&self, declaration: Span) -> bool {
        let ast = &*self.file.ast;
        ast.module_level_exports()
            .filter(|export| export.module.is_none())
            .any(|export| {
                export
                    .declarations
                    .iter()
                    .chain(&export.specifiers)
                    .any(|specifier| specifier.local.span == declaration)
                    || (export.span.contains(declaration)
                        && !self.function_between(export.span, declaration))
            })
    }
}

// ---------------------------------------------------------------------------
// The census
// ---------------------------------------------------------------------------

/// One proven reactive value, as the census sees it.
struct Seed<'a> {
    symbol: &'a SymbolId,
    kind: Option<SeedKind>,
}

/// Namespace bindings by the module they name (`None`: any module), as the
/// importing file and the binding's declaration.
type NamespaceImports<'a> = HashMap<Option<&'a str>, Vec<(&'a str, Span)>>;

/// The project-level reverse-use index: per-file indexes built on demand.
struct ProjectUses<'a> {
    files: HashMap<&'a str, &'a FileFacts>,
    indexes: HashMap<&'a str, FileUseIndex<'a>>,
    /// Runtime namespace imports (`import * as ns`), by the module file they
    /// name: the importing file and the binding's declaration. Built on the
    /// first published seed. `None` keys a namespace that may expose any
    /// module: Type Facts does not resolve it, or it names a file outside the
    /// analysis or one that re-exports (a barrel can expose a seed under its
    /// own namespace).
    namespaces: Option<NamespaceImports<'a>>,
}

/// Exact local namespace bindings initialized by `await import(...)`, through
/// transparent TS wrappers. A promise or a helper result is not a namespace.
fn dynamic_namespace_declarations(file: &FileFacts) -> Vec<Span> {
    let ast = &*file.ast;
    ast.bindings
        .iter()
        .filter_map(|binding| {
            if binding.shape != solid_facts::ast::BindingShape::Identifier {
                return None;
            }
            let initializer = peel_wrapper(ast, binding.initializer?)?;
            if !ast.awaits.contains(&initializer) {
                return None;
            }
            // The first operand is this await's own operand, not an inner
            // await inside a helper call. Containment alone would mistake
            // `await consume(await import(...))` for a namespace binding.
            let operand = ast
                .await_operands
                .iter()
                .filter(|operand| initializer.contains(**operand))
                .min_by_key(|operand| operand.start)?;
            let operand = peel_wrapper(ast, *operand)?;
            let direct_import = ast.module_loads.iter().any(|load| {
                load.kind == solid_facts::ast::ModuleLoadKind::DynamicImport && operand == load.span
            });
            direct_import
                .then(|| binding.names.first().map(|name| name.span))
                .flatten()
        })
        .collect()
}

impl<'a> ProjectUses<'a> {
    fn new(files: &'a [FileFacts]) -> Self {
        Self {
            files: files
                .iter()
                .map(|file| (file.path.as_str(), file))
                .collect(),
            indexes: HashMap::new(),
            namespaces: None,
        }
    }

    fn namespace_imports(
        &mut self,
        entities: &crate::indexes::EntitySymbols,
        symbols: &HashMap<&'a str, Option<TypeScriptSymbol<'a>>>,
        module: &str,
    ) -> Vec<(&'a str, Span)> {
        let files = &self.files;
        let namespaces = self.namespaces.get_or_insert_with(|| {
            let mut namespaces = NamespaceImports::new();
            let exact = |target: &'a str| {
                files.get(target).is_some_and(|file| {
                    file.ast
                        .exports
                        .iter()
                        .all(|export| export.module.is_none())
                })
            };
            let mut paths = files.keys().copied().collect::<Vec<_>>();
            paths.sort_unstable();
            for path in paths {
                let file = files[path];
                // Type Facts has no exact module-alias target for dynamic
                // imports here. Keep their escapes unknown for every published
                // seed, as for an unresolved static namespace.
                namespaces.entry(None).or_default().extend(
                    dynamic_namespace_declarations(file)
                        .into_iter()
                        .map(|span| (path, span)),
                );
                for import in file.ast.imports.iter().filter(|import| !import.type_only) {
                    for binding in import.bindings.iter().filter(|binding| {
                        binding.kind == ImportKind::Namespace && !binding.type_only
                    }) {
                        let targets = entities
                            .at(path, binding.local.span)
                            .and_then(|id| symbols.get(id.as_str()).copied().flatten())
                            .and_then(|symbol| {
                                symbols.get(symbol.alias_target()).copied().flatten()
                            })
                            .and_then(|symbol| {
                                // Empty or multiple declarations are not exact resolution.
                                let [declaration] = symbol.declarations() else {
                                    return None;
                                };
                                let at = span_of(&declaration.location)?;
                                let target = declaration.location.path.as_ref();
                                if at.start > at.end
                                    || files
                                        .get(target)?
                                        .source
                                        .get(at.start as usize..at.end as usize)
                                        .is_none()
                                {
                                    return None;
                                }
                                exact(target).then_some(vec![target])
                            });
                        match targets {
                            Some(targets) => {
                                for target in targets {
                                    namespaces
                                        .entry(Some(target))
                                        .or_default()
                                        .push((path, binding.local.span));
                                }
                            }
                            None => namespaces
                                .entry(None)
                                .or_default()
                                .push((path, binding.local.span)),
                        }
                    }
                }
            }
            namespaces
        });
        let mut imports = namespaces
            .iter()
            .filter(|(target, _)| target.is_none_or(|target| target == module))
            .flat_map(|(_, imports)| imports.iter().copied())
            .collect::<Vec<_>>();
        imports.sort_unstable();
        imports.dedup();
        imports
    }

    fn index(&mut self, path: &str) -> Option<&FileUseIndex<'a>> {
        let (&path, &file) = self.files.get_key_value(path)?;
        Some(
            self.indexes
                .entry(path)
                .or_insert_with(|| FileUseIndex::new(file)),
        )
    }
}

fn span_of(location: &typefacts::Location) -> Option<Span> {
    Some(Span::new(
        u32::try_from(location.start_byte).ok()?,
        u32::try_from(location.end_byte).ok()?,
    ))
}

/// Canonical source order (decision 7): project-relative path, then span.
/// Absolute paths would make the order, and so `coverage.first`, depend on
/// where the project is checked out; a path outside the project keeps its
/// absolute spelling, which sorts apart from every relative one.
struct Canon {
    root: String,
}

impl Canon {
    fn new(project_id: &str) -> Self {
        let root = std::path::Path::new(project_id)
            .parent()
            .map(|parent| parent.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            root: if root.is_empty() || root.ends_with('/') {
                root
            } else {
                format!("{root}/")
            },
        }
    }

    fn relative<'a>(&self, path: &'a str) -> &'a str {
        if self.root.is_empty() {
            path
        } else {
            path.strip_prefix(self.root.as_str()).unwrap_or(path)
        }
    }

    fn key<'a>(&self, address: &'a SourceAddress) -> (&'a str, Span) {
        (self.relative(&address.path), address.span)
    }

    fn node<'a>(&self, node: &'a FlowNode) -> ((&'a str, Span), &'a FlowNodeKind) {
        (self.key(&node.address), &node.kind)
    }

    fn sort_gaps(&self, gaps: &mut [IdentityGap]) {
        gaps.sort_by_key(|gap| {
            (
                gap.location.as_ref().map(|location| {
                    (
                        self.relative(&location.path).to_owned(),
                        location.start_byte,
                        location.end_byte,
                    )
                }),
                gap.reason,
            )
        });
    }

    /// Equal location/kind keys need a content tie-break: declaration-address
    /// uniqueness is not attested, and generation-local symbol IDs are not
    /// audit identities. Normalize only source-path fields, never dedup a seed.
    fn record_key(&self, record: &SeedRecord) -> String {
        fn normalize(value: &mut serde_json::Value, canon: &Canon) {
            match value {
                serde_json::Value::Object(fields) => {
                    if let Some(serde_json::Value::String(path)) = fields.get_mut("path") {
                        *path = canon.relative(path).to_owned();
                    }
                    for child in fields.values_mut() {
                        normalize(child, canon);
                    }
                }
                serde_json::Value::Array(items) => {
                    for child in items {
                        normalize(child, canon);
                    }
                }
                _ => {}
            }
        }
        let mut value = serde_json::to_value(record).unwrap_or_default();
        normalize(&mut value, self);
        value.to_string()
    }

    fn compare_records(&self, left: &SeedRecord, right: &SeedRecord) -> std::cmp::Ordering {
        let primary = match (&left.seed, &right.seed) {
            (Some(left), Some(right)) => self.node(left).cmp(&self.node(right)),
            (None, None) => {
                let gap_key = |record: &SeedRecord| {
                    record
                        .identity_gaps
                        .iter()
                        .filter_map(|gap| gap.location.as_ref())
                        .map(|location| {
                            (
                                self.relative(&location.path).to_owned(),
                                location.start_byte,
                                location.end_byte,
                            )
                        })
                        .chain(record.aliases.iter().map(|address| {
                            (
                                self.relative(&address.path).to_owned(),
                                u64::from(address.span.start),
                                u64::from(address.span.end),
                            )
                        }))
                        .collect::<Vec<_>>()
                };
                gap_key(left).cmp(&gap_key(right))
            }
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
        };
        primary.then_with(|| self.record_key(left).cmp(&self.record_key(right)))
    }
}

/// Builds the shadow fact over every proven seed.
pub(crate) fn build(ctx: &AnalysisContext<'_>) -> ValueFlowShadow {
    let mut seeds = BTreeMap::<&str, Seed<'_>>::new();
    for symbol in ctx.accessors.keys() {
        let kind = match ctx.source_kinds.get(symbol) {
            Some(ReactiveSourceKind::Store) => Some(SeedKind::Store),
            Some(ReactiveSourceKind::Accessor) => Some(SeedKind::Accessor),
            None => None,
        };
        seeds.insert(symbol.as_str(), Seed { symbol, kind });
    }
    for symbol in ctx.prop_sources.keys() {
        if !ctx.uncertain_prop_sources.contains(symbol) {
            seeds.entry(symbol.as_str()).or_insert(Seed {
                symbol,
                kind: Some(SeedKind::PropRoot),
            });
        }
    }
    let canon = Canon::new(&ctx.facts.project_id);
    let mut uses = ProjectUses::new(&ctx.facts.files);
    // Private per-analysis storage; duplicates are ambiguity, not last-writer wins.
    let mut symbols = HashMap::new();
    for symbol in ctx.facts.typescript.symbols() {
        symbols
            .entry(symbol.id())
            .and_modify(|entry| *entry = None)
            .or_insert(Some(symbol));
    }
    let mut records = Vec::with_capacity(seeds.len());
    for seed in seeds.values() {
        records.push(census(
            ctx.entities,
            ctx.symbols_by_root
                .get(seed.symbol)
                .map(Vec::as_slice)
                .unwrap_or_default(),
            &symbols,
            &mut uses,
            &canon,
            seed,
        ));
    }
    records.sort_by(|left, right| canon.compare_records(left, right));
    let mut edges = records
        .iter()
        .flat_map(|record| {
            record.uses.iter().filter_map(|use_record| {
                let UseDisposition::Transfer { edge, to } = &use_record.disposition else {
                    return None;
                };
                Some(FlowEdge {
                    from: record.seed.clone()?,
                    to: to.clone(),
                    kind: *edge,
                    at: use_record.reference.clone(),
                })
            })
        })
        .collect::<Vec<_>>();
    edges.sort_by(|left, right| {
        (
            canon.node(&left.from),
            canon.node(&left.to),
            left.kind,
            canon.key(&left.at),
        )
            .cmp(&(
                canon.node(&right.from),
                canon.node(&right.to),
                right.kind,
                canon.key(&right.at),
            ))
    });
    edges.dedup();
    ValueFlowShadow {
        seeds: records,
        edges,
        indexed_files: uses.indexes.len(),
    }
}

fn census<'a>(
    entities: &crate::indexes::EntitySymbols,
    aliases: &[SymbolId],
    private_symbols: &HashMap<&'a str, Option<TypeScriptSymbol<'a>>>,
    uses: &mut ProjectUses<'a>,
    canon: &Canon,
    seed: &Seed<'_>,
) -> SeedRecord {
    let symbol_fact = |id: &str| private_symbols.get(id).copied().flatten();
    // The seed and every symbol whose alias root it is: import bindings and
    // re-exports of the same value.
    let mut symbols = vec![seed.symbol.as_str()];
    symbols.extend(aliases.iter().map(SymbolId::as_str));
    symbols.sort_unstable();
    symbols.dedup();

    let mut declarations = BTreeSet::<SourceAddress>::new();
    let mut identity_gaps = Vec::new();
    let mut own = Vec::new();
    for symbol in &symbols {
        let candidates = symbol_fact(symbol)
            .map(|fact| fact.declarations())
            .unwrap_or_default();
        if candidates.len() != 1 {
            identity_gaps.push(IdentityGap {
                location: None,
                reason: UnknownReason::IdentityUnattested,
            });
            identity_gaps.extend(candidates.iter().map(|declaration| IdentityGap {
                location: Some(declaration.location.clone()),
                reason: UnknownReason::IdentityUnattested,
            }));
            continue;
        }
        for declaration in candidates {
            let location = &declaration.location;
            let valid = span_of(location)
                .filter(|span| span.start <= span.end)
                .filter(|span| {
                    uses.files.get(location.path.as_ref()).is_some_and(|file| {
                        file.source
                            .get(span.start as usize..span.end as usize)
                            .is_some()
                    })
                });
            if let Some(span) = valid {
                let address = SourceAddress {
                    path: location.path.clone(),
                    span,
                };
                declarations.insert(address.clone());
                if *symbol == seed.symbol.as_str() {
                    own.push(address);
                }
            } else {
                identity_gaps.push(IdentityGap {
                    location: Some(location.clone()),
                    reason: UnknownReason::DeclarationOutsideAnalysis,
                });
            }
        }
    }
    let mut declarations = declarations.into_iter().collect::<Vec<_>>();
    declarations.sort_by(|left, right| canon.key(left).cmp(&canon.key(right)));
    let primary = (own.len() == 1).then(|| own[0].clone());
    if primary.is_none() || seed.kind.is_none() {
        identity_gaps.push(IdentityGap {
            location: None,
            reason: UnknownReason::IdentityUnattested,
        });
        canon.sort_gaps(&mut identity_gaps);
        return SeedRecord {
            seed: None,
            aliases: declarations,
            published: false,
            uses: vec![],
            coverage: Coverage::Open {
                first: primary,
                reason: UnknownReason::IdentityUnattested,
            },
            identity_gaps,
        };
    }
    let primary = primary.unwrap();
    let kind = seed.kind.unwrap();

    // Every reference: the binder's census at each analyzed declaration, and
    // Type Facts' references of each symbol.
    let mut references = BTreeSet::<SourceAddress>::new();
    let mut published = false;
    // Uses no reference spells: a direct eval or `arguments` that reaches a
    // declaration.
    let mut implicit = Vec::<(SourceAddress, UnknownReason)>::new();
    for declaration in &declarations {
        let Some(index) = uses.index(&declaration.path) else {
            continue;
        };
        published |= index.published(declaration.span);
        for reference in index.binder_references(declaration.span) {
            references.insert(SourceAddress::new(&declaration.path, reference));
        }
        for (span, reason) in index.dynamic_scope_uses(declaration.span) {
            implicit.push((SourceAddress::new(&declaration.path, span), reason));
        }
    }
    // Require reference-table presence for every symbol. This API does not
    // independently attest completeness; do not call presence completeness.
    let reference_census_present = symbols.iter().all(|symbol| symbol_fact(symbol).is_some());
    for symbol in &symbols {
        for location in symbol_fact(symbol)
            .into_iter()
            .flat_map(|fact| fact.references())
        {
            let Some(span) = span_of(location).filter(|span| span.start <= span.end) else {
                identity_gaps.push(IdentityGap {
                    location: Some(location.clone()),
                    reason: UnknownReason::IdentityUnattested,
                });
                continue;
            };
            if uses.files.get(location.path.as_ref()).is_some_and(|file| {
                file.source
                    .get(span.start as usize..span.end as usize)
                    .is_none()
            }) {
                identity_gaps.push(IdentityGap {
                    location: Some(location.clone()),
                    reason: UnknownReason::IdentityUnattested,
                });
                continue;
            }
            references.insert(SourceAddress {
                path: location.path.clone(),
                span,
            });
        }
    }
    for declaration in &declarations {
        references.remove(declaration);
    }

    let mut records = Vec::with_capacity(references.len() + implicit.len());
    for reference in references {
        let Some(index) = uses.index(&reference.path) else {
            records.push(UseRecord {
                reference,
                frame: None,
                captured: None,
                region: UseRegion::Unattested,
                disposition: UseDisposition::Unknown {
                    reason: UnknownReason::ReferenceOutsideAnalysis,
                },
            });
            continue;
        };
        let frame = index.frame(reference.span);
        let captured = None;
        let disposition = index.classify(reference.span, kind);
        let region = index.region(reference.span);
        records.push(UseRecord {
            reference,
            frame,
            captured,
            region,
            disposition,
        });
    }
    // A reference that transfers the value into an export publishes it,
    // whichever declaration it names (`export default value`).
    published |= records.iter().any(|use_record| {
        matches!(
            use_record.disposition,
            UseDisposition::Transfer {
                edge: FlowEdgeKind::Export,
                ..
            }
        )
    });
    // A published value is reachable through every namespace of its module:
    // each escape of one is a use (decision 5: computed access is unknown).
    if published {
        for declaration in &declarations {
            for (path, namespace) in
                uses.namespace_imports(entities, private_symbols, &declaration.path)
            {
                let Some(index) = uses.index(path) else {
                    continue;
                };
                for reference in index.namespace_escapes(namespace) {
                    implicit.push((
                        SourceAddress::new(&index.path, reference),
                        UnknownReason::NamespaceEscape,
                    ));
                }
            }
        }
    }
    for (reference, reason) in implicit {
        let (frame, region) = uses
            .index(&reference.path)
            .map_or((None, UseRegion::Unattested), |index| {
                (index.frame(reference.span), index.region(reference.span))
            });
        records.push(UseRecord {
            reference,
            frame,
            captured: None,
            region,
            disposition: UseDisposition::Unknown { reason },
        });
    }
    records.sort_by(|left, right| {
        (canon.key(&left.reference), &left.disposition)
            .cmp(&(canon.key(&right.reference), &right.disposition))
    });
    records.dedup();
    canon.sort_gaps(&mut identity_gaps);
    let mut coverage = coverage(&primary, published, reference_census_present, &records);
    if !identity_gaps.is_empty()
        && matches!(
            coverage,
            Coverage::Open {
                reason: UnknownReason::ReferenceCensusIncomplete,
                ..
            }
        )
    {
        coverage = Coverage::Open {
            first: Some(primary.clone()),
            reason: UnknownReason::IdentityUnattested,
        };
    }
    SeedRecord {
        seed: Some(FlowNode {
            address: primary.clone(),
            kind: FlowNodeKind::Seed { seed: kind },
        }),
        identity_gaps,
        aliases: declarations
            .into_iter()
            .filter(|declaration| *declaration != primary)
            .collect(),
        published,
        uses: records,
        coverage,
    }
}

/// The coverage query in shadow mode.
///
/// Every seed remains open; reference presence is not completeness.
fn coverage(
    seed: &SourceAddress,
    published: bool,
    _reference_census_present: bool,
    uses: &[UseRecord],
) -> Coverage {
    let first = uses.iter().find_map(|use_record| {
        let reason = match &use_record.disposition {
            UseDisposition::Excluded { .. }
            | UseDisposition::Transfer {
                edge: FlowEdgeKind::Import,
                ..
            } => return None,
            UseDisposition::Transfer {
                edge: FlowEdgeKind::Export,
                ..
            } => UnknownReason::Published,
            UseDisposition::Transfer { .. } => UnknownReason::EdgeNotFollowed,
            UseDisposition::Read { .. } => UnknownReason::ReadRowUnbound,
            UseDisposition::Operation { .. } => UnknownReason::NoTerminalRow,
            UseDisposition::Unknown { reason } => *reason,
        };
        Some((use_record.reference.clone(), reason))
    });
    match (published, first) {
        (true, _) => Coverage::Open {
            first: Some(seed.clone()),
            reason: UnknownReason::Published,
        },
        (false, Some((first, reason))) => Coverage::Open {
            first: Some(first),
            reason,
        },
        (false, None) => Coverage::Open {
            first: Some(seed.clone()),
            reason: UnknownReason::ReferenceCensusIncomplete,
        },
    }
}

impl ValueFlowShadow {
    /// The deterministic audit: one JSON object per seed, in canonical order,
    /// then one per edge.
    pub(crate) fn audit(&self) -> String {
        let mut lines = String::new();
        for record in &self.seeds {
            lines.push_str(&serde_json::to_string(record).unwrap_or_default());
            lines.push('\n');
        }
        for edge in &self.edges {
            lines.push_str(&serde_json::to_string(edge).unwrap_or_default());
            lines.push('\n');
        }
        lines
    }

    /// Stage counters for `SOLID_CHECKER_TIMINGS`.
    fn counters(&self) -> serde_json::Value {
        let mut dispositions = BTreeMap::<&str, usize>::new();
        let mut reasons = BTreeMap::<String, usize>::new();
        let mut uses = 0;
        for record in &self.seeds {
            for use_record in &record.uses {
                uses += 1;
                let name = match &use_record.disposition {
                    UseDisposition::Transfer { .. } => "transfer",
                    UseDisposition::Read { .. } => "read",
                    UseDisposition::Operation { .. } => "operation",
                    UseDisposition::Excluded { .. } => "excluded",
                    UseDisposition::Unknown { reason } => {
                        *reasons
                            .entry(serde_json::to_string(reason).unwrap_or_default())
                            .or_default() += 1;
                        "unknown"
                    }
                };
                *dispositions.entry(name).or_default() += 1;
            }
        }
        serde_json::json!({
            "valueFlowShadow": {
                "seeds": self.seeds.len(),
                "uses": uses,
                "edges": self.edges.len(),
                "indexedFiles": self.indexed_files,
                "dispositions": dispositions,
                "unknownReasons": reasons,
            }
        })
    }
}

/// The shadow stage. It builds the fact and hands it to the audit alone:
/// counters under `SOLID_CHECKER_TIMINGS`, and the complete audit when
/// `SOLID_CHECKER_VALUE_FLOW_AUDIT` names a file. A failed audit write is
/// ignored so ordinary diagnostics and public status remain unchanged.
pub(crate) fn enabled(emit_timings: bool) -> bool {
    emit_timings || std::env::var_os(AUDIT_VARIABLE).is_some()
}

pub(crate) fn shadow_stage(ctx: &AnalysisContext<'_>, emit_timings: bool) {
    if !enabled(emit_timings) {
        return;
    }
    let shadow = build(ctx);
    if emit_timings {
        eprintln!("{}", shadow.counters());
    }
    if let Some(path) = std::env::var_os(AUDIT_VARIABLE) {
        // Optional private I/O cannot affect ordinary diagnostics or status.
        let _ = std::fs::write(path, shadow.audit());
    }
}

#[cfg(test)]
mod tests;
