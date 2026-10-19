//! Read-optimized project indexes used by every analysis stage.
//!
//! This module hides AST and TypeScript table layout from rule discovery. The
//! builder asks semantic questions here instead of repeatedly scanning facts.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, OnceLock},
};

use sha2::{Digest, Sha256};
use solid_facts::core::Span;
use solid_facts::{FileFacts, ProjectFacts, TypeScriptSymbol, TypeScriptTable};
use typefacts::{
    Callability, EntityFact, FileFact, Location, ResolvedCall, ResolvedCallValidity, TypeDescriptor,
};

use super::{SymbolId, SymbolName};
use crate::owners::function_binding_name;

/// The string a computed key names when it is a `const` bound to a string
/// literal in the same file (`const key = "run"; c[key] = f`).
fn constant_string_key(file: &FileFacts, key: Span) -> Option<String> {
    let key = file.ast.peel_ts_sugar_span(key);
    let declaration = file.ast.reference_declaration(key)?;
    let binding = file.ast.bindings.iter().find(|binding| {
        binding.immutable
            && binding.shape == solid_facts::ast::BindingShape::Identifier
            && binding
                .names
                .first()
                .is_some_and(|name| name.span == declaration)
    })?;
    if binding.initializer_value_kind != solid_facts::ast::RuntimeValueKind::Primitive {
        return None;
    }
    let text = file.source_text(file.ast.peel_ts_sugar_span(binding.initializer?))?;
    let unquoted = text
        .strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
        .or_else(|| {
            text.strip_prefix('\'')
                .and_then(|text| text.strip_suffix('\''))
        })?;
    (!unquoted.contains('\\')).then(|| unquoted.to_string())
}

/// Whether the binding declared at `declaration` is written anywhere in its
/// file after its declaration: as an assignment target or inside one (a
/// destructuring pattern), or as a loop head (`for (x of …)`). A write of a
/// member of it (`x.y = …`) does not rebind it.
/// Whether some function in `file` returns a bare identifier, or a `new` of a
/// callee spelled `Proxy` (ADR 0287): the only shapes whose callee syntax a
/// returned-source proof (ADR 0222) reads beyond the summaries and Type Facts
/// the caches compare. A same-file `return g(…)` forward (ADR 0287) proves
/// nothing unless its file also holds one of these, so it is bound with it.
pub(crate) fn returns_source_root(file: &FileFacts) -> bool {
    file.ast
        .returns
        .iter()
        .chain(
            file.ast
                .functions
                .iter()
                .filter_map(|function| function.expression_return.as_ref()),
        )
        .filter_map(|returned| returned.argument)
        .any(|argument| {
            let argument = file.ast.peel_ts_sugar_span(argument);
            file.ast.identifiers.iter().any(|identifier| {
                identifier.span == argument
                    && identifier.role == solid_facts::ast::IdentifierRole::Reference
            }) || file.ast.calls.iter().any(|call| {
                call.construct
                    && call.span == argument
                    && file.source_text(file.ast.peel_ts_sugar_span(call.callee)) == Some("Proxy")
            })
        })
}

pub(super) fn binding_written(file: &FileFacts, declaration: Span) -> bool {
    // A redeclaration (`var xs = …` over a parameter, a second `function f`)
    // writes the same binding without any reference. Any other declaration
    // of the same name in the scope counts, a shadowing one included.
    let name = file.source_text(declaration);
    let scope = file
        .ast
        .functions
        .iter()
        .filter(|function| function.span.contains(declaration))
        .min_by_key(|function| function.span.end - function.span.start)
        .map_or(Span::new(0, u32::MAX), |function| function.span);
    let redeclared = file.ast.bindings.iter().any(|binding| {
        scope.contains(binding.declaration)
            && binding
                .names
                .iter()
                .any(|other| other.span != declaration && file.source_text(other.span) == name)
    }) || file.ast.functions.iter().any(|function| {
        scope.contains(function.span)
            && function.kind == solid_facts::ast::FunctionKind::Declaration
            && function.name.as_ref().is_some_and(|other| {
                other.span != declaration && file.source_text(other.span) == name
            })
    });
    if redeclared {
        return true;
    }
    let targets = file
        .ast
        .assignments
        .iter()
        .map(|assignment| assignment.target)
        .chain(file.ast.iteration_targets.iter().copied())
        .collect::<Vec<_>>();
    file.ast
        .reference_declarations
        .iter()
        .filter(|(_, declared)| *declared == declaration)
        .any(|(reference, _)| {
            targets.iter().any(|target| target.contains(*reference))
                && !file
                    .ast
                    .members
                    .iter()
                    .any(|member| member.object == *reference)
        })
}

/// ADR 0209, 0314: the members assignments write, by name, and the
/// prototypes they write through under a key no proof names.
#[derive(Default)]
pub(super) struct AssignedMembers {
    names: HashSet<String>,
    /// A write through a prototype whose owner is not proven: every object's
    /// members may be anything.
    global: bool,
    /// Project constructors, by file and declaration, whose prototype is
    /// written: their instances' members may be anything.
    constructors: HashSet<(String, Span)>,
    /// Host constructors whose prototype is written, each with whether every
    /// declaration of it is in `lib.dom.d.ts`.
    hosts: Vec<(String, bool)>,
}

/// Whose instances a write through one `prototype` reaches (ADR 0314).
enum PrototypeOwner {
    /// The project constructor declared at this span of the writing file.
    Constructor(Span),
    /// These host constructors, through this static method's parameter.
    Hosts(String, Vec<(String, bool)>),
}

/// Whether some assignment writes the global binding `name` directly
/// (`HTMLCanvasElement = …`): an identifier target no local declaration
/// binds.
fn global_identifier_written(facts: &ProjectFacts, name: &str) -> bool {
    facts.files.iter().any(|file| {
        file.ast
            .assignments
            .iter()
            .map(|assignment| assignment.target)
            .chain(file.ast.iteration_targets.iter().copied())
            .any(|target| {
                // The whole target, or a leaf of a destructuring pattern; an
                // identifier inside a member target or a type assertion
                // (`(x as HTMLCanvasElement).width = …`) writes no binding.
                let target = file.ast.peel_ts_sugar_span(target);
                let pattern = file
                    .source_text(target)
                    .is_some_and(|text| text.starts_with(['[', '{']));
                file.ast.identifiers.iter().any(|identifier| {
                    (identifier.span == target || (pattern && target.contains(identifier.span)))
                        && file.source_text(identifier.span) == Some(name)
                        && file.ast.reference_declaration(identifier.span).is_none()
                })
            })
    })
}

/// Whether `text` is exactly one string, template (without nested
/// templates), numeric or boolean literal.
fn primitive_literal_text(text: &str) -> bool {
    if text == "true" || text == "false" {
        return true;
    }
    let bytes = text.as_bytes();
    let Some(&first) = bytes.first() else {
        return false;
    };
    if first.is_ascii_digit() {
        return bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_'));
    }
    if !matches!(first, b'"' | b'\'' | b'`') || bytes.len() < 2 || bytes[bytes.len() - 1] != first {
        return false;
    }
    let mut escaped = false;
    for &byte in &bytes[1..bytes.len() - 1] {
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == first {
            return false;
        }
    }
    !escaped
}

/// ADR 0320: standard-library functions whose result is a primitive other
/// than `undefined` for every argument (`ToString`, `ToNumber` and the like
/// run user code but return a primitive).
const PRIMITIVE_RESULT_FUNCTIONS: &[&str] = &[
    "StringConstructor.call",
    "NumberConstructor.call",
    "BooleanConstructor.call",
    "BigIntConstructor.call",
    "parseInt",
    "parseFloat",
    "isNaN",
    "isFinite",
    "NumberConstructor.parseInt",
    "NumberConstructor.parseFloat",
    "NumberConstructor.isFinite",
    "NumberConstructor.isInteger",
    "NumberConstructor.isNaN",
    "NumberConstructor.isSafeInteger",
    "encodeURIComponent",
    "decodeURIComponent",
    "encodeURI",
    "decodeURI",
    "DateConstructor.now",
    "ArrayConstructor.isArray",
    "Math.abs",
    "Math.ceil",
    "Math.floor",
    "Math.round",
    "Math.trunc",
    "Math.sign",
    "Math.max",
    "Math.min",
    "Math.pow",
    "Math.sqrt",
    "Math.cbrt",
    "Math.random",
    "Math.log",
    "Math.log2",
    "Math.log10",
    "Math.exp",
    "Math.hypot",
    "Math.sin",
    "Math.cos",
    "Math.tan",
    "Math.atan2",
];

/// ADR 0211: what a value's origin proves about the object it evaluates to.
#[derive(Clone, Copy, Debug)]
pub(super) enum ValueOrigin<'a> {
    /// A fresh object of a reviewed standard-library value class: an array
    /// literal, `new Date(…)`, `Array.from(…)`. Its members are its built-in
    /// prototype's, none of which reads reactive state.
    Builtin,
    /// An instance of exactly this project class.
    ProjectClass(&'a FileFacts, &'a solid_facts::ast::ClassFact),
}

/// The standard-library constructors whose instances are plain values: their
/// built-in methods read and write the instance and call only the callbacks
/// they are handed. `Proxy`, `Function`, `Promise` and every event target are
/// left out: their members run user code that was registered elsewhere.
const BUILTIN_VALUE_CONSTRUCTORS: &[&str] = &[
    "ArrayConstructor.construct",
    "DateConstructor.construct",
    "MapConstructor.construct",
    "SetConstructor.construct",
    "WeakMapConstructor.construct",
    "WeakSetConstructor.construct",
    "RegExpConstructor.construct",
    "ArrayBufferConstructor.construct",
    "DataViewConstructor.construct",
    "Int8ArrayConstructor.construct",
    "Uint8ArrayConstructor.construct",
    "Uint8ClampedArrayConstructor.construct",
    "Int16ArrayConstructor.construct",
    "Uint16ArrayConstructor.construct",
    "Int32ArrayConstructor.construct",
    "Uint32ArrayConstructor.construct",
    "Float32ArrayConstructor.construct",
    "Float64ArrayConstructor.construct",
    "BigInt64ArrayConstructor.construct",
    "BigUint64ArrayConstructor.construct",
];

/// Static standard-library functions that return a fresh array, when called on
/// the global they are declared for.
const FRESH_ARRAY_STATICS: &[&str] = &[
    "ArrayConstructor.from",
    "ArrayConstructor.of",
    "ObjectConstructor.keys",
    "ObjectConstructor.values",
    "ObjectConstructor.entries",
];

/// Members of a proven built-in receiver that return the receiver itself.
const RECEIVER_RETURNING_METHODS: &[&str] = &["sort", "reverse", "fill", "copyWithin"];

/// Members of a proven built-in receiver that return a fresh built-in value:
/// on an array, one built by its species; on a typed array, buffer or string,
/// one of its own class. No reviewed value class has a member of these names
/// that returns anything else.
const FRESH_VALUE_METHODS: &[&str] = &[
    "map",
    "filter",
    "slice",
    "concat",
    "flat",
    "flatMap",
    "toSorted",
    "toReversed",
    "toSpliced",
    "with",
    "split",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ComponentStatus {
    No,
    Uncertain,
    Proven,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct EntitySymbols {
    pub(super) by_path: HashMap<String, HashMap<(u64, u64), SymbolId>>,
}

impl EntitySymbols {
    pub(super) fn get(&self, location: &Location) -> Option<&SymbolId> {
        self.by_path
            .get(location.path.as_ref())
            .and_then(|entities| entities.get(&(location.start_byte, location.end_byte)))
    }

    pub(super) fn at(&self, path: &str, span: Span) -> Option<&SymbolId> {
        self.by_path
            .get(path)
            .and_then(|entities| entities.get(&(u64::from(span.start), u64::from(span.end))))
    }
}

type DeclarationSymbols<'a> = HashMap<(&'a str, u64, u64), &'a str>;

/// Symbols addressable by an exact declaration location.
///
/// TypeScript does not emit an entity at every class/object method key, but a
/// resolved member call carries the declaration location and the canonical
/// method symbol. Entities win over call declarations at the same location,
/// preserving the direct-then-declaration order of the per-method lookup this
/// replaces -- which rescanned every project entity twice per call.
fn declaration_symbols(typescript: &TypeScriptTable) -> DeclarationSymbols<'_> {
    let mut by_location = DeclarationSymbols::new();
    for entity in typescript
        .entities()
        .filter(|entity| !entity.symbol.is_empty())
    {
        by_location
            .entry((
                entity.location.path.as_ref(),
                entity.location.start_byte,
                entity.location.end_byte,
            ))
            .or_insert(entity.symbol.as_ref());
    }
    for declaration in typescript
        .entities()
        .filter_map(|entity| entity.resolved_call.as_deref())
        .filter_map(|call| call.declaration.as_ref())
        .filter(|declaration| !declaration.symbol.is_empty())
    {
        by_location
            .entry((
                declaration.location.path.as_ref(),
                declaration.location.start_byte,
                declaration.location.end_byte,
            ))
            .or_insert(declaration.symbol.as_ref());
    }
    by_location
}

pub(super) struct ProjectIndexes<'a> {
    pub(super) files_by_path: HashMap<&'a str, &'a FileFacts>,
    pub(super) ast_files_by_path: HashMap<&'a str, &'a CachedAstFileIndex>,
    typescript: &'a TypeScriptTable,
    pub(super) symbols_by_id: HashMap<&'a str, TypeScriptSymbol<'a>>,
    declaration_symbols: OnceLock<DeclarationSymbols<'a>>,
}

impl<'a> ProjectIndexes<'a> {
    pub(super) fn new(
        facts: &'a ProjectFacts,
        ast_indexes: &'a HashMap<solid_facts::core::SourcePath, CachedAstFileIndex>,
    ) -> Self {
        let files_by_path = facts
            .files
            .iter()
            .map(|file| (file.path.as_str(), file))
            .collect();
        let ast_files_by_path = facts
            .files
            .iter()
            .filter_map(|file| {
                ast_indexes
                    .get(file.path.as_str())
                    .map(|index| (file.path.as_str(), index))
            })
            .collect();
        let symbols_by_id = facts
            .typescript
            .symbols()
            .map(|symbol| (symbol.id(), symbol))
            .collect();
        Self {
            files_by_path,
            ast_files_by_path,
            typescript: &facts.typescript,
            symbols_by_id,
            declaration_symbols: OnceLock::new(),
        }
    }

    pub(super) fn typescript_file(&self, path: &str) -> Option<&'a FileFact> {
        self.typescript.file(path)
    }

    /// TypeScript does not emit an entity at every class/object method key,
    /// but a resolved member call carries the declaration location and the
    /// canonical method symbol. Use that exact declaration pair to name a
    /// structural method; never match by spelling alone.
    pub(super) fn method_symbol(&self, path: &str, span: Span) -> Option<SymbolId> {
        self.method_symbol_ref(path, span).map(SymbolId::from)
    }

    fn method_symbol_ref(&self, path: &str, span: Span) -> Option<&'a str> {
        self.declaration_symbols
            .get_or_init(|| declaration_symbols(self.typescript))
            .get(&(path, u64::from(span.start), u64::from(span.end)))
            .copied()
    }

    pub(super) fn entities_for_path(&self, path: &str) -> &'a [EntityFact] {
        self.typescript.entities_for_path(path)
    }
}

pub(super) struct CachedAstFileIndex {
    pub(super) ast: Arc<solid_facts::ast::AstFacts>,
    calls_by_span: HashMap<Span, usize>,
    calls_by_callee: HashMap<Span, Vec<usize>>,
    direct_calls_by_callee: HashMap<Span, usize>,
    functions_by_span: HashMap<Span, usize>,
    member_properties_by_span: HashMap<Span, Span>,
}

impl CachedAstFileIndex {
    pub(super) fn new(file: &FileFacts) -> Self {
        let mut calls_by_span = HashMap::new();
        let mut calls_by_callee = HashMap::<Span, Vec<_>>::new();
        let mut direct_calls_by_callee = HashMap::new();
        for (index, call) in file.ast.calls.iter().enumerate() {
            calls_by_span.entry(call.span).or_insert(index);
            calls_by_callee.entry(call.callee).or_default().push(index);
            if call.direct_callee {
                direct_calls_by_callee.entry(call.callee).or_insert(index);
            }
        }
        let mut functions_by_span = HashMap::new();
        for (index, function) in file.ast.functions.iter().enumerate() {
            functions_by_span.entry(function.span).or_insert(index);
        }
        let member_properties_by_span = file
            .ast
            .members
            .iter()
            .map(|member| (member.span, member.property))
            .collect();
        Self {
            ast: file.ast.clone(),
            calls_by_span,
            calls_by_callee,
            direct_calls_by_callee,
            functions_by_span,
            member_properties_by_span,
        }
    }

    fn call(&self, index: usize) -> &solid_facts::ast::CallFact {
        &self.ast.calls[index]
    }

    fn function(&self, index: usize) -> &solid_facts::ast::FunctionFact {
        &self.ast.functions[index]
    }

    pub(super) fn call_by_span(&self, span: Span) -> Option<&solid_facts::ast::CallFact> {
        self.calls_by_span.get(&span).map(|index| self.call(*index))
    }

    /// The position of the call with exactly this span in `file.ast.calls`.
    ///
    /// The primitive, execution-role, and owner tables are index-aligned with
    /// that array, so every classifier that starts from a `CallFact` and needs
    /// its resolved primitive has to translate a span back into an index. Doing
    /// that with `calls.iter().position(..)` is a linear scan per processed
    /// call, which is quadratic in a file's call count.
    pub(super) fn call_index_by_span(&self, span: Span) -> Option<usize> {
        self.calls_by_span.get(&span).copied()
    }

    pub(super) fn direct_call_by_callee(&self, span: Span) -> Option<&solid_facts::ast::CallFact> {
        self.direct_calls_by_callee
            .get(&span)
            .map(|index| self.call(*index))
    }

    pub(super) fn calls_by_callee(
        &self,
        span: Span,
    ) -> impl Iterator<Item = &solid_facts::ast::CallFact> {
        self.calls_by_callee
            .get(&span)
            .into_iter()
            .flatten()
            .map(|index| self.call(*index))
    }

    pub(super) fn call_by_callee(&self, span: Span) -> Option<&solid_facts::ast::CallFact> {
        self.calls_by_callee(span).next()
    }

    pub(super) fn function_by_span(&self, span: Span) -> Option<&solid_facts::ast::FunctionFact> {
        self.functions_by_span
            .get(&span)
            .map(|index| self.function(*index))
    }

    pub(super) fn member_property(&self, span: Span) -> Option<Span> {
        self.member_properties_by_span.get(&span).copied()
    }
}

/// A resolution of a checker symbol to the project function it names.
///
/// `Aborted` reproduces the legacy scan's early return: a matching
/// function-initialized binding without a recorded initializer span ends the
/// project search with no result, even if later files also match.
#[derive(Clone, Copy)]
enum SymbolFunction {
    Resolved { file: usize, function: usize },
    Aborted,
}

/// Whether any JSX call site renders a function. Boundary placement above a
/// render is resolved transitively by `owners::read_loading_cover`.
#[derive(Clone, Copy, Default)]
pub(super) struct CallSiteLoading {
    pub(super) any: bool,
}

#[derive(Clone)]
struct BindingResolution {
    file: usize,
    binding: usize,
    symbol: SymbolId,
}

#[derive(Clone, Copy)]
struct FunctionCallSite {
    file: usize,
    callee: Span,
    /// A second reference to the same function that this one call site
    /// accounts for, when the syntax of the site writes the callee's name
    /// twice: a JSX element with a closing tag. It is not a call site of its
    /// own — a render is one invocation — so only the escape test, which
    /// enumerates *references*, ever reads it.
    also_referenced: Option<Span>,
}

type BindingsByReference = HashMap<String, HashMap<(u64, u64), BindingResolution>>;

/// Rendering-prop flows by value reference: the render sites each reaches
/// ([`SemanticLookup::prop_render_sites_at`]).
type PropRenderSites<'a> = HashMap<(&'a str, Span), Vec<Span>>;

/// One argument a project wrapper forwards into a result-access slot
/// ([`SemanticLookup::result_access_forwarded_arguments`]).
#[derive(Clone, Copy, Debug)]
pub(super) struct ResultAccessForwarded {
    /// The argument at the wrapper's call site.
    pub(super) argument: Span,
    /// The primitive whose slot the wrapper forwards it into.
    pub(super) primitive: solid_dialect::Primitive,
}

/// Lazy project-wide lookups that replace repeated whole-project scans.
///
/// Every map is built at most once per build, on first use, in the exact
/// file/declaration order the scans it replaces used, so first-match and
/// first-writer results are unchanged. Warm builds that never ask a question
/// never pay for an index.
pub(super) struct SemanticLookup<'a> {
    object_members: OnceLock<crate::object_members::Targets>,
    literal_receiver_members: OnceLock<HashSet<(String, Span)>>,
    facts: &'a ProjectFacts,
    /// The Solid-version vocabulary this build analyzes with. Every consumer
    /// of the lookup shares one dialect; a build never mixes two.
    pub(super) dialect: &'a dyn solid_dialect::Dialect,
    /// Whether the user asserted that the analyzed files are the whole
    /// program. Build-wide like the dialect, and false unless selected: a
    /// build that was never told stays fail-closed.
    pub(super) program_closed: bool,
    /// What the backend read of the installed router (ADR 0324). Set by the
    /// pipeline from the runtime environment; empty otherwise.
    router_install: crate::router_params::RouterInstallEvidence,
    ast_indexes: &'a HashMap<solid_facts::core::SourcePath, CachedAstFileIndex>,
    entities: &'a EntitySymbols,
    symbol_names: &'a HashMap<SymbolId, SymbolName>,
    functions_by_symbol: OnceLock<HashMap<&'a str, SymbolFunction>>,
    /// ADR 0319: every literal's invocation-flow answer.
    invocation_flow_answers: OnceLock<HashMap<(String, Span), crate::ExecutionRole>>,
    /// The literals whose every invocation is a host event dispatch.
    unowned_event_literals: OnceLock<HashSet<(String, Span)>>,
    /// ADR 0320: whether some call may copy keys into an existing object.
    objects_assigned_into: OnceLock<bool>,
    router_params: OnceLock<crate::router_params::RouterParamsStatus>,
    /// ADR 0209: the property names some assignment in the project writes
    /// (`x.name = …`), and which prototypes it writes through (ADR 0314).
    assigned_member_names: OnceLock<AssignedMembers>,
    /// Review round 1 of ADR 0293: whether some file may define a property
    /// reflectively ([`Self::property_may_be_defined_reflectively`]).
    reflective_property_definition: OnceLock<bool>,
    entities_by_location: OnceLock<HashMap<(&'a str, u64, u64), &'a EntityFact>>,
    contained_entities_by_path: OnceLock<HashMap<&'a str, Vec<&'a EntityFact>>>,
    descriptors_by_symbol: OnceLock<HashMap<&'a str, &'a TypeDescriptor>>,
    callability_by_symbol: OnceLock<HashMap<&'a str, Callability>>,
    symbols_by_id: OnceLock<HashMap<&'a str, solid_facts::TypeScriptSymbol<'a>>>,
    context_symbols: OnceLock<HashSet<&'a str>>,
    jsx_call_sites: OnceLock<HashMap<(&'a str, Span), CallSiteLoading>>,
    declaration_symbols: OnceLock<DeclarationSymbols<'a>>,
    function_call_sites: OnceLock<HashMap<(&'a str, Span), Vec<FunctionCallSite>>>,
    prop_render_sites: OnceLock<PropRenderSites<'a>>,
    direct_value_aliases: OnceLock<HashSet<SymbolId>>,
    bindings_by_symbol: OnceLock<HashMap<SymbolId, BindingResolution>>,
    bindings_by_reference: OnceLock<BindingsByReference>,
    files_by_path: OnceLock<HashMap<&'a str, usize>>,
    file_primitives: OnceLock<Vec<OnceLock<FilePrimitives>>>,
    file_named_callback_roles: OnceLock<Vec<OnceLock<super::NamedCallbackRoles>>>,
    project_primitives: OnceLock<HashSet<solid_dialect::Primitive>>,
    callback_capabilities: OnceLock<DialectCallbackCapabilities>,
    returned_callback_proof_digest: OnceLock<Option<CrossFileProofDigest>>,
    project_has_component_type: OnceLock<bool>,
    component_functions: OnceLock<HashSet<(&'a str, Span)>>,
    result_access_forwarded: OnceLock<HashMap<&'a str, Vec<ResultAccessForwarded>>>,
    cross_file_proof_digest: OnceLock<Option<CrossFileProofDigest>>,
    tuple_iteration_veto: OnceLock<bool>,
    string_code_veto: OnceLock<bool>,
    /// ADR 0277: `(path, function span, parameter index)` for each parameter
    /// a synchronous project function calls on its every-completion path.
    parameter_completion_calls: OnceLock<HashSet<(&'a str, Span, usize)>>,
    /// ADR 0278: the calls TypeScript resolves to a signature returning
    /// `never`, or (ADR 0279) to an `asserts` signature, per path. Such a call never completes normally, so a caller
    /// that catches its throw carries on with the rest of the body skipped.
    never_returning_calls: OnceLock<HashMap<&'a str, Vec<Span>>>,
    /// ADR 0280: `(path, function span)` for each function that tests one
    /// of its own parameters in a condition or a logical operand, the only
    /// functions a call's arguments can decide anything in.
    parameter_testing_functions: OnceLock<HashSet<(&'a str, Span)>>,
    /// ADR 0292: `(path, function span, property span)` for each object
    /// property that is a proven component's one route, the `default` of the
    /// module a dialect component loader renders.
    lazy_default_render_witnesses: OnceLock<HashSet<(&'a str, Span, Span)>>,
    effect_apply_regions: OnceLock<crate::effect_execution::ApplyRegions>,
    effect_apply_calls: OnceLock<crate::effect_execution::ApplyCalls<'a>>,
}

/// Which parts of the returned-adapter contract this build's dialect can
/// actually answer, for the primitives this project names.
///
/// Solid 2.0 leaves every method behind these flags at its `false`/`None`
/// default: no primitive routes callbacks through a returned adapter, and none
/// stores a function argument as a value. The engine still has to *ask* those
/// questions per call, and the asking is not free — proving a returned adapter
/// is invoked walks binding chains across every project file. One probe of the
/// vocabulary per build lets the passes skip machinery that provably cannot
/// answer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct DialectCallbackCapabilities {
    /// Some primitive routes a callback through a function it returns, so
    /// call-site proofs of that returned function's use are meaningful.
    pub(super) returned_callbacks: bool,
    /// Some primitive stores a function argument instead of invoking it, so
    /// dormant-argument classification is meaningful.
    pub(super) stored_function_arguments: bool,
}

/// Digest of the project facts the cross-file returned-callback proofs read.
pub(super) type CrossFileProofDigest = [u8; 32];

/// Argument positions, argument counts, and tuple slots wide enough to cover
/// every Solid signature either dialect models. Each probe is one table
/// lookup, and the whole sweep runs once per build over the handful of
/// primitives a project actually names.
const PROBED_ARGUMENTS: usize = 8;
const PROBED_RESULT_SLOTS: usize = 4;

/// The longest access path [`SemanticLookup::member_callee_receiver`] reports.
///
/// A deeper chain is **truncated to its first segments from the root, never
/// dropped**. The reads domain is emitted `Complete` once it is known at all,
/// so answering `None` for an over-long chain would turn an unreported access
/// into the negative claim "this export performs no parameter read". Cutting
/// the path is the same fail-closed move an unnameable segment already makes
/// -- keep the longest exact prefix from the root -- and 32 matches the depth
/// bound `interproc::member_root_is_parameter` applies to the same chains.
const MEMBER_CALLEE_PATH_LIMIT: usize = 32;

/// The member fact at an exact span, if that span is a member expression.
///
/// `FileFacts` sorts `members` by span, the same invariant the sibling
/// `computed_members` binary searches rely on.
fn member_at(file: &FileFacts, span: Span) -> Option<&solid_facts::ast::MemberFact> {
    file.ast
        .members
        .binary_search_by_key(&span, |member| member.span)
        .ok()
        .map(|index| &file.ast.members[index])
}

/// Whether `path` lies inside an installation of one of `packages`: its
/// components contain the package name as consecutive directory names
/// (`solid-js`, or `@solidjs` then `web`). Exact components, never a
/// substring, so `my-solid-js-tools/` and `@solidjs/router/` are neither.
fn declared_in_package(path: &str, packages: &[&str]) -> bool {
    let components = path.split(['/', '\\']).collect::<Vec<_>>();
    packages.iter().any(|package| {
        let name = package.split('/').collect::<Vec<_>>();
        components.windows(name.len()).any(|window| window == name)
    })
}

/// Resolved Solid primitive names for one file's calls and JSX elements,
/// index-aligned with `file.ast.calls` / `file.ast.jsx_elements`. Computed
/// once per file per build so per-call classifier scans stop re-resolving
/// (and re-allocating) the same names.
pub(super) struct FilePrimitives {
    pub(super) calls: Vec<Option<super::PrimitiveName>>,
    pub(super) jsx: Vec<Option<super::PrimitiveName>>,
}

impl<'a> SemanticLookup<'a> {
    pub(crate) fn effect_apply_calls(&self) -> &crate::effect_execution::ApplyCalls<'a> {
        self.effect_apply_calls
            .get_or_init(|| crate::effect_execution::calls(self))
    }
    pub(crate) fn effect_apply_regions(&self) -> &crate::effect_execution::ApplyRegions {
        self.effect_apply_regions
            .get_or_init(|| crate::effect_execution::regions(self))
    }
    /// The installed router's evidence; see [`Self::router_install`].
    #[must_use]
    pub(super) fn with_router_install(
        mut self,
        install: crate::router_params::RouterInstallEvidence,
    ) -> Self {
        self.router_install = install;
        self
    }

    pub(super) fn new(
        facts: &'a ProjectFacts,
        ast_indexes: &'a HashMap<solid_facts::core::SourcePath, CachedAstFileIndex>,
        entities: &'a EntitySymbols,
        symbol_names: &'a HashMap<SymbolId, SymbolName>,
        dialect: &'a dyn solid_dialect::Dialect,
        program_closed: bool,
    ) -> Self {
        debug_assert!(
            facts
                .typescript
                .entities()
                .map(|entity| entity.location.path.as_ref())
                .is_sorted(),
            "entity table must be sorted by path for per-path containment slices"
        );
        Self {
            object_members: OnceLock::new(),
            literal_receiver_members: OnceLock::new(),
            facts,
            dialect,
            program_closed,
            router_install: crate::router_params::RouterInstallEvidence::default(),
            ast_indexes,
            entities,
            symbol_names,
            functions_by_symbol: OnceLock::new(),
            invocation_flow_answers: OnceLock::new(),
            unowned_event_literals: OnceLock::new(),
            objects_assigned_into: OnceLock::new(),
            router_params: OnceLock::new(),
            assigned_member_names: OnceLock::new(),
            reflective_property_definition: OnceLock::new(),
            entities_by_location: OnceLock::new(),
            contained_entities_by_path: OnceLock::new(),
            descriptors_by_symbol: OnceLock::new(),
            callability_by_symbol: OnceLock::new(),
            symbols_by_id: OnceLock::new(),
            context_symbols: OnceLock::new(),
            jsx_call_sites: OnceLock::new(),
            declaration_symbols: OnceLock::new(),
            function_call_sites: OnceLock::new(),
            prop_render_sites: OnceLock::new(),
            direct_value_aliases: OnceLock::new(),
            bindings_by_symbol: OnceLock::new(),
            bindings_by_reference: OnceLock::new(),
            files_by_path: OnceLock::new(),
            file_primitives: OnceLock::new(),
            file_named_callback_roles: OnceLock::new(),
            project_primitives: OnceLock::new(),
            callback_capabilities: OnceLock::new(),
            returned_callback_proof_digest: OnceLock::new(),
            project_has_component_type: OnceLock::new(),
            component_functions: OnceLock::new(),
            result_access_forwarded: OnceLock::new(),
            cross_file_proof_digest: OnceLock::new(),
            tuple_iteration_veto: OnceLock::new(),
            string_code_veto: OnceLock::new(),
            parameter_completion_calls: OnceLock::new(),
            never_returning_calls: OnceLock::new(),
            parameter_testing_functions: OnceLock::new(),
            lazy_default_render_witnesses: OnceLock::new(),
            effect_apply_regions: OnceLock::new(),
            effect_apply_calls: OnceLock::new(),
        }
    }

    /// The exact compiler symbol demanded at one span of one file, if any.
    ///
    /// Deliberately narrower than [`Self::callee_symbol`]: no wrapper peeling
    /// and no member fallback, because the callers are spans that *are* a
    /// binding name (an import's local name) rather than an arbitrary callee
    /// expression.
    pub(super) fn entity_symbol(&self, file: &FileFacts, span: Span) -> Option<&'a str> {
        self.entities
            .at(file.path.as_str(), span)
            .map(SymbolId::as_str)
    }

    /// Every primitive this build can resolve at a call site.
    ///
    /// `primitive_name` answers only through the symbol-name table, so a
    /// primitive absent from this set cannot appear at any call, JSX tag, or
    /// binding initializer in this project. That makes the set a sound domain
    /// for probing what the dialect models.
    fn project_primitives(&self) -> &HashSet<solid_dialect::Primitive> {
        self.project_primitives.get_or_init(|| {
            self.symbol_names
                .values()
                .filter_map(|name| self.dialect.primitive(name.as_str()))
                .collect()
        })
    }

    pub(super) fn callback_capabilities(&self) -> DialectCallbackCapabilities {
        *self.callback_capabilities.get_or_init(|| {
            let mut capabilities = DialectCallbackCapabilities::default();
            for primitive in self.project_primitives() {
                for argument in 0..PROBED_ARGUMENTS {
                    let semantics =
                        self.dialect
                            .callback_semantics_at(*primitive, argument, PROBED_ARGUMENTS);
                    capabilities.returned_callbacks |= semantics.requires_return_invocation;
                    capabilities.stored_function_arguments |= semantics.stores_as_value;
                    for count in 0..PROBED_ARGUMENTS {
                        for slot in std::iter::once(None).chain((0..PROBED_RESULT_SLOTS).map(Some))
                        {
                            let returned = self
                                .dialect
                                .returned_callback_semantics_at(*primitive, slot, argument, count);
                            capabilities.returned_callbacks |=
                                returned.execution.is_some() || returned.owner.is_some();
                        }
                    }
                }
            }
            capabilities
        })
    }

    /// Whether the dialect routes any callback of a primitive this project
    /// names through a function the primitive returns.
    pub(super) fn models_returned_callbacks(&self) -> bool {
        self.callback_capabilities().returned_callbacks
    }

    /// Whether the dialect stores any function argument of a primitive this
    /// project names as a plain value instead of invoking it.
    pub(super) fn models_stored_function_arguments(&self) -> bool {
        self.callback_capabilities().stored_function_arguments
    }

    /// Whether the cross-file proofs can read anything at all from this file.
    ///
    /// Every read either machine makes of a file *other* than the one it was
    /// asked about is drawn from one of four AST tables: `calls` (invocation
    /// sites and the `factory(...)()` shape), `jsx_elements` (a rendered
    /// adapter), `members` (`lazyResult.preload()`), and `bindings` (the factory
    /// seed, its destructured tuple slots, and every identifier alias the
    /// closure walks). A file with all four empty is never resolved against, so
    /// its TypeScript facts are never consulted either — and editing it cannot
    /// move any proof.
    fn participates_in_returned_callback_proofs(file: &FileFacts) -> bool {
        !file.ast.calls.is_empty()
            || !file.ast.jsx_elements.is_empty()
            || !file.ast.members.is_empty()
            || !file.ast.bindings.is_empty()
    }

    /// Identity of the whole-project facts that the cross-file
    /// returned-callback proofs read, or `None` when no such proof can exist.
    ///
    /// Those proofs — "is the function `mapArray` returned in file A ever
    /// invoked?" — scan every project file's calls, members, JSX tags and
    /// bindings, yet their answers land in *per-file* cache fragments keyed on
    /// that one file's source hash. Editing the only file that invokes the
    /// adapter would otherwise leave the factory file's fragment untouched and
    /// stale. Folding this digest into those fragments' reuse identity closes
    /// that hole.
    ///
    /// One thing narrows it, provable from the machinery's loop headers alone: a
    /// file contributes only if a proof can read anything from it at all (see
    /// [`Self::participates_in_returned_callback_proofs`]). Editing a
    /// declaration-only module therefore invalidates nothing.
    ///
    /// # Why the contribution is still the whole source hash
    ///
    /// The obvious next step — digest each participating file's *proof-relevant
    /// projection* (its call callees, member invocations and alias bindings)
    /// instead of its source — is not sound, for two independent reasons.
    ///
    /// The proofs resolve every position through the project-wide TypeScript
    /// symbol and reference tables: `returned_binding_reference` accepts a span
    /// because some adapter symbol's *reference list* covers it, and
    /// `returned_primitive_invocation` walks binding chains through
    /// `binding_at_reference`. Those tables are a whole-project product, so an
    /// edit to file A can change what a position in file B resolves to while B's
    /// own AST is byte-identical. A projection of B alone cannot see that.
    ///
    /// And the local-access fragments depend on more than whether a site exists:
    /// `returned_factory_callback_execution_role` classifies each site's span
    /// *in the using file*, so the answer folds in that file's whole execution
    /// context. A projection would have to carry the classifier's entire input,
    /// which is the file.
    ///
    /// Decomposing the digest per file therefore needs the project-wide reverse
    /// index from adapter bindings to their use sites that the machinery does
    /// not build. Until it exists, a participating file's contribution stays its
    /// source hash: coarse, but never stale.
    pub(super) fn returned_callback_proof_digest(&self) -> Option<CrossFileProofDigest> {
        *self.returned_callback_proof_digest.get_or_init(|| {
            if !self.models_returned_callbacks() {
                return None;
            }
            let mut inputs = self
                .facts
                .files
                .iter()
                .filter(|file| Self::participates_in_returned_callback_proofs(file))
                .map(|file| (file.path.as_str(), file.source_hash.as_str()))
                .collect::<Vec<_>>();
            // Facts arrive in configured-source order; sort so a reordered
            // source set cannot read as a changed one.
            inputs.sort_unstable();
            let mut hasher = Sha256::new();
            for (path, source_hash) in inputs {
                hasher.update(u64::try_from(path.len()).unwrap_or(u64::MAX).to_le_bytes());
                hasher.update(path.as_bytes());
                hasher.update(source_hash.as_bytes());
            }
            Some(hasher.finalize().into())
        })
    }

    /// Digest of every project-wide proof whose answer is baked into a
    /// per-file cache fragment.
    ///
    /// Component identity is a whole-project fact: a JSX call site in one
    /// file can make a function in another file an owned component. Returned
    /// callback reachability has the same shape. One digest gives every
    /// dependent cache family the same coherent generation marker.
    /// ADR 0320: [`Self::cross_file_proof_digest`] plus the program-wide
    /// member-replacement census member resolution reads (an assignment of a
    /// name or through an unknown key, a reflective definition, code copying
    /// keys into objects). Only the stages that resolve members -- the
    /// interprocedural graph and results, local accesses -- are keyed on it,
    /// so a write elsewhere does not discard source discovery.
    pub(super) fn member_resolution_digest(&self) -> Option<CrossFileProofDigest> {
        let base = self.cross_file_proof_digest()?;
        let assigned = self.assigned_members();
        let mut names = assigned.names.iter().collect::<Vec<_>>();
        names.sort_unstable();
        let mut hasher = Sha256::new();
        hasher.update(base);
        hasher.update(b"member-replacement\0");
        hasher.update([
            u8::from(assigned.global),
            u8::from(self.property_may_be_defined_reflectively()),
            u8::from(self.objects_may_be_assigned_into()),
        ]);
        for name in names {
            hasher.update(u64::try_from(name.len()).unwrap_or(u64::MAX).to_le_bytes());
            hasher.update(name.as_bytes());
        }
        Some(hasher.finalize().into())
    }

    pub(super) fn cross_file_proof_digest(&self) -> Option<CrossFileProofDigest> {
        *self.cross_file_proof_digest.get_or_init(|| {
            let components = self.component_functions();
            let mut component_keys = components.iter().copied().collect::<Vec<_>>();
            component_keys.sort_unstable();
            let mut hasher = Sha256::new();
            hasher.update(b"components\0");
            // ADR 0282: a mutation in another file can withdraw the tuple
            // callable edge without changing the consuming file's facts.
            // Projects with no supported tuple result cannot consume this
            // premise. Do not invalidate their unrelated object/source caches.
            if self.files().iter().any(|file| {
                file.ast.returns.iter().any(|returned| {
                    returned.argument.is_some_and(|argument| {
                        file.ast
                            .array_literals
                            .contains(&file.ast.peel_ts_sugar_span(argument))
                    })
                }) || file.ast.bindings.iter().any(|binding| {
                    binding.shape == solid_facts::ast::BindingShape::Array
                        && file
                            .ast
                            .functions
                            .iter()
                            .any(|function| function.body.contains(binding.pattern))
                        && binding
                            .call_initializer
                            .and_then(|span| self.primitive_at_call(file, span))
                            .is_some_and(|primitive| self.dialect.returns_reactive_tuple(primitive))
                })
            }) {
                hasher.update(b"tuple-iteration-veto\0");
                hasher.update([u8::from(self.tuple_iteration_may_be_mutated())]);
            }
            // ADR 0324: whether the router premise applies is a property of
            // every file (a history in one decides what another reads).
            hasher.update(b"router-params\0");
            hasher.update([u8::from(self.router_params().applies())]);
            // ADR 0325: an imported constant's primitive-ness is another
            // file's fact.
            hasher.update(b"imported-primitive-consts\0");
            for (path, span) in self.exported_primitive_consts() {
                hasher.update(u64::try_from(path.len()).unwrap_or(u64::MAX).to_le_bytes());
                hasher.update(path.as_bytes());
                hasher.update(span.start.to_le_bytes());
                hasher.update(span.end.to_le_bytes());
            }
            // ADR 0302: a compute helper or source in another file decides an
            // apply's owner/execution edge. Bind each call's answer, including
            // named callbacks that also have independent invocations.
            let mut apply_sites = self.effect_apply_calls().iter().collect::<Vec<_>>();
            apply_sites.sort_unstable_by_key(|(site, _)| **site);
            hasher.update(b"effect-apply-execution\0");
            for ((path, span, argument), execution) in apply_sites {
                hasher.update(u64::try_from(path.len()).unwrap_or(u64::MAX).to_le_bytes());
                hasher.update(path.as_bytes());
                hasher.update(span.start.to_le_bytes());
                hasher.update(span.end.to_le_bytes());
                hasher.update(u64::try_from(*argument).unwrap_or(u64::MAX).to_le_bytes());
                let tag = match execution {
                    crate::effect_execution::ApplyExecution::Proven => 0_u8,
                    crate::effect_execution::ApplyExecution::Absent => 1,
                    crate::effect_execution::ApplyExecution::Unknown => 2,
                };
                hasher.update([tag]);
            }
            // ADR 0314: which prototypes another file writes decides which
            // receivers keep an exact member here.
            {
                let assigned = self.assigned_members();
                hasher.update(b"prototype-writes\0");
                hasher.update([u8::from(assigned.global)]);
                let mut constructors = assigned.constructors.iter().collect::<Vec<_>>();
                constructors.sort_unstable();
                for (path, span) in constructors {
                    hasher.update(u64::try_from(path.len()).unwrap_or(u64::MAX).to_le_bytes());
                    hasher.update(path.as_bytes());
                    hasher.update(span.start.to_le_bytes());
                    hasher.update(span.end.to_le_bytes());
                }
                let mut hosts = assigned.hosts.clone();
                hosts.sort_unstable();
                for (name, dom) in hosts {
                    hasher.update(u64::try_from(name.len()).unwrap_or(u64::MAX).to_le_bytes());
                    hasher.update(name.as_bytes());
                    hasher.update([u8::from(dom)]);
                }
            }
            let mut member_targets = self.object_member_targets().iter().collect::<Vec<_>>();
            member_targets.sort_unstable_by(|a, b| {
                (a.0.path.as_ref(), a.0.start_byte, a.0.end_byte).cmp(&(
                    b.0.path.as_ref(),
                    b.0.start_byte,
                    b.0.end_byte,
                ))
            });
            hasher.update(b"object-member-targets\0");
            for (site, (path, span)) in member_targets {
                for value in [site.path.as_ref(), path.as_str()] {
                    hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_le_bytes());
                    hasher.update(value.as_bytes());
                }
                hasher.update(site.start_byte.to_le_bytes());
                hasher.update(site.end_byte.to_le_bytes());
                hasher.update(span.start.to_le_bytes());
                hasher.update(span.end.to_le_bytes());
            }
            // Returned-source proofs inspect callee syntax and binding facts,
            // even when its public type and read summary do not change. Bind
            // the byte identity of every file that can hold such a proof (a
            // bare identifier or `new Proxy` return, ADR 0222 and 0287); a file gaining or losing one
            // enters or leaves the digest, so the set itself is bound too.
            let mut source_inputs = self
                .files()
                .iter()
                .filter(|file| returns_source_root(file))
                .collect::<Vec<_>>();
            source_inputs.sort_unstable_by(|left, right| left.path.cmp(&right.path));
            hasher.update(b"returned-source-inputs\0");
            for file in source_inputs {
                let path = file.path.as_str();
                hasher.update(u64::try_from(path.len()).unwrap_or(u64::MAX).to_le_bytes());
                hasher.update(path.as_bytes());
                hasher.update(file.source_hash.as_str().as_bytes());
            }
            // ADR 0319: an invocation-flow proof follows a function value
            // through any file of a closed program -- bindings, returns,
            // object members, callee parameters, component props -- so a
            // literal's role here can move with another file's contents.
            // Every consumer reads the role from one per-build table of
            // answers (`invocation_flow_answers`); bind that table, as ADR
            // 0302 binds each apply's answer. A closed program with no proven
            // answer still writes the marker, so closing or opening the
            // boundary always moves the digest.
            if self.program_closed {
                let mut answers = self.invocation_flow_answers().iter().collect::<Vec<_>>();
                answers.sort_unstable_by(|left, right| left.0.cmp(right.0));
                hasher.update(b"invocation-flow-answers\0");
                for ((path, span), role) in answers {
                    hasher.update(u64::try_from(path.len()).unwrap_or(u64::MAX).to_le_bytes());
                    hasher.update(path.as_bytes());
                    hasher.update(span.start.to_le_bytes());
                    hasher.update(span.end.to_le_bytes());
                    hasher.update(format!("{role:?}").as_bytes());
                    hasher.update([0]);
                }
                // The owner seeds read the same walk's owner answers.
                let mut literals = self.unowned_event_literals().iter().collect::<Vec<_>>();
                literals.sort_unstable();
                hasher.update(b"unowned-event-literals\0");
                for (path, span) in literals {
                    hasher.update(u64::try_from(path.len()).unwrap_or(u64::MAX).to_le_bytes());
                    hasher.update(path.as_bytes());
                    hasher.update(span.start.to_le_bytes());
                    hasher.update(span.end.to_le_bytes());
                }
            }
            for (path, span) in component_keys {
                hasher.update(u64::try_from(path.len()).unwrap_or(u64::MAX).to_le_bytes());
                hasher.update(path.as_bytes());
                hasher.update(span.start.to_le_bytes());
                hasher.update(span.end.to_le_bytes());
            }
            if let Some(returned) = self.returned_callback_proof_digest() {
                hasher.update(b"returned-callbacks\0");
                hasher.update(returned);
            }
            // Review round 1 of ADR 0293: an exempt store Get is withheld
            // only while no file writes a member of its key, writes through a
            // prototype, or defines a property reflectively.
            if let Some(census) = self.exempt_store_key_census() {
                hasher.update(b"exempt-store-keys\0");
                for (key, written) in census {
                    hasher.update(u64::try_from(key.len()).unwrap_or(u64::MAX).to_le_bytes());
                    hasher.update(key.as_bytes());
                    hasher.update([u8::from(written)]);
                }
            }
            // ADR 0277: whether a helper calls its parameter on every
            // completion decides the role of a callback written in its
            // caller, and moving that call into a branch changes no summary.
            let mut parameter_calls = self
                .parameter_completion_calls()
                .iter()
                .copied()
                .collect::<Vec<_>>();
            parameter_calls.sort_unstable();
            // ADR 0278: a guard's declared `never` lives in another file's
            // types, so it is bound here rather than by this file's bytes.
            let mut never_paths = self.never_returning_calls().iter().collect::<Vec<_>>();
            never_paths.sort_unstable();
            hasher.update(b"never-returning-calls\0");
            for (path, calls) in never_paths {
                hasher.update(u64::try_from(path.len()).unwrap_or(u64::MAX).to_le_bytes());
                hasher.update(path.as_bytes());
                for call in calls {
                    hasher.update(call.start.to_le_bytes());
                    hasher.update(call.end.to_le_bytes());
                }
            }
            hasher.update(b"parameter-completion-calls\0");
            for (path, span, index) in parameter_calls {
                hasher.update(u64::try_from(path.len()).unwrap_or(u64::MAX).to_le_bytes());
                hasher.update(path.as_bytes());
                hasher.update(span.start.to_le_bytes());
                hasher.update(span.end.to_le_bytes());
                hasher.update(u64::try_from(index).unwrap_or(u64::MAX).to_le_bytes());
            }
            // A wrapper in one file decides the role of a callback written in
            // another. Hashed only when present, so a project with no such
            // wrapper keeps the digest it had.
            let forwarded = self.result_access_forwarded_index();
            if !forwarded.is_empty() {
                let mut paths = forwarded.keys().copied().collect::<Vec<_>>();
                paths.sort_unstable();
                hasher.update(b"result-access-forwarded\0");
                for path in paths {
                    hasher.update(u64::try_from(path.len()).unwrap_or(u64::MAX).to_le_bytes());
                    hasher.update(path.as_bytes());
                    for entry in &forwarded[path] {
                        hasher.update(entry.argument.start.to_le_bytes());
                        hasher.update(entry.argument.end.to_le_bytes());
                    }
                }
            }
            Some(hasher.finalize().into())
        })
    }

    /// ADR 0277: whether the synchronous project function at `function` in
    /// `path` calls its single-name parameter `index`, by that parameter's
    /// symbol, at a completion site of its own body.
    pub(crate) fn parameter_called_on_every_completion(
        &self,
        path: &str,
        function: Span,
        index: usize,
    ) -> bool {
        self.parameter_completion_calls()
            .contains(&(path, function, index))
    }

    /// ADR 0278: whether a call TypeScript resolves to a `never`-returning
    /// signature is written directly in `function` before `site`. It may run
    /// first and throw, so `site` is not on every completion a caller can
    /// continue from.
    pub(crate) fn never_returning_call_before(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
        site: Span,
    ) -> bool {
        self.never_returning_calls()
            .get(file.path.as_str())
            .is_some_and(|calls| {
                // In the body or the parameter list (a default may call one),
                // never in a nested function.
                calls.iter().any(|call| {
                    call.end <= site.start
                        && function.span.contains(*call)
                        && !file.ast.functions_within(function.span).any(|nested| {
                            nested.span != function.span && nested.span.contains(*call)
                        })
                })
            })
    }

    /// ADR 0280: the `never` and `asserts` calls written in `function`'s
    /// body or parameter list, outside nested functions.
    pub(crate) fn never_returning_calls_within(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> Vec<Span> {
        self.never_returning_calls()
            .get(file.path.as_str())
            .map(|calls| {
                calls
                    .iter()
                    .copied()
                    .filter(|call| {
                        function.span.contains(*call)
                            && !file.ast.functions_within(function.span).any(|nested| {
                                nested.span != function.span && nested.span.contains(*call)
                            })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// ADR 0280: whether `function` tests one of its parameters, so that a
    /// call's arguments may decide one of its branches.
    pub(crate) fn tests_its_parameters(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> bool {
        self.parameter_testing_functions
            .get_or_init(|| {
                let facts: &'a ProjectFacts = self.facts;
                let mut testing = HashSet::new();
                for file in &facts.files {
                    let ast = &file.ast;
                    let mut owners = HashMap::<Span, Span>::new();
                    for function in &ast.functions {
                        for parameter in &function.parameters {
                            for name in &parameter.names {
                                owners.insert(name.span, function.span);
                            }
                        }
                    }
                    if owners.is_empty() {
                        continue;
                    }
                    let tests = ast
                        .conditional_tests
                        .iter()
                        .copied()
                        .chain(ast.logical_expressions.iter().map(|logical| logical.left))
                        .collect::<Vec<_>>();
                    for (reference, declaration) in &ast.reference_declarations {
                        if let Some(function) = owners.get(declaration)
                            && function.contains(*reference)
                            && tests.iter().any(|test| test.contains(*reference))
                        {
                            testing.insert((file.path.as_str(), *function));
                        }
                    }
                }
                testing
            })
            .contains(&(file.path.as_str(), function.span))
    }

    fn never_returning_calls(&self) -> &HashMap<&'a str, Vec<Span>> {
        self.never_returning_calls.get_or_init(|| {
            let facts: &'a ProjectFacts = self.facts;
            let mut calls = HashMap::<&'a str, Vec<Span>>::new();
            for file in &facts.files {
                for call in &file.ast.calls {
                    if self
                        .resolved_callee_call(file, call.callee)
                        .is_some_and(|resolved| {
                            // The compiler's own type flag, not the display
                            // text (docs/typefacts/compiler-semantic-facts.md).
                            resolved.never_returns || resolved.assertion_signature
                        })
                    {
                        calls.entry(file.path.as_str()).or_default().push(call.span);
                    }
                }
            }
            for spans in calls.values_mut() {
                spans.sort_unstable();
            }
            calls
        })
    }

    fn parameter_completion_calls(&self) -> &HashSet<(&'a str, Span, usize)> {
        self.parameter_completion_calls.get_or_init(|| {
            let facts: &'a ProjectFacts = self.facts;
            let mut calls = HashSet::new();
            // ADR 0285: calls passing a parameter on to another function.
            let mut forwards = Vec::new();
            for file in &facts.files {
                // Direct `eval` can write any parameter it sees (review
                // round 1 of ADR 0285).
                if file
                    .ast
                    .module_hazards
                    .iter()
                    .any(|hazard| hazard.kind == solid_facts::ast::ModuleHazardKind::Eval)
                    || file.ast.calls.iter().any(|call| {
                        file.source_text(file.ast.peel_ts_sugar_span(call.callee)) == Some("eval")
                    })
                {
                    continue;
                }
                let mut parameters = HashMap::<&str, (Span, usize)>::new();
                for function in &file.ast.functions {
                    if function.r#async || function.generator {
                        continue;
                    }
                    for (index, parameter) in function.parameters.iter().enumerate() {
                        if parameter.shape != solid_facts::ast::BindingShape::Identifier {
                            continue;
                        }
                        let Some(name) = parameter.names.first() else {
                            continue;
                        };
                        // A written parameter may hold a replacement by the
                        // time it is called (`cb = () => 0; cb()`).
                        if binding_written(file, name.span) {
                            continue;
                        }
                        if let Some(symbol) = self.entities().at(file.path.as_str(), name.span) {
                            parameters.insert(symbol.as_str(), (function.span, index));
                        }
                    }
                }
                if parameters.is_empty() {
                    continue;
                }
                for call in &file.ast.calls {
                    let Some(&(function, index)) = self
                        .callee_symbol(file, call.callee)
                        .and_then(|symbol| parameters.get(symbol))
                    else {
                        continue;
                    };
                    if crate::owners::containing_ast_function(&file.ast, call.span)
                        .is_some_and(|owner| owner.span == function)
                        && crate::interproc::site_on_every_completion(
                            self, file, function, call.span,
                        )
                    {
                        calls.insert((file.path.as_str(), function, index));
                    }
                }
                for call in &file.ast.calls {
                    for (position, argument) in call.arguments.iter().enumerate() {
                        let span = file.ast.peel_ts_sugar_span(argument.span);
                        if argument.spread
                            || file
                                .ast
                                .identifiers
                                .binary_search_by_key(&span, |identifier| identifier.span)
                                .is_err()
                        {
                            continue;
                        }
                        let Some(&(function, index)) = self
                            .entities()
                            .at(file.path.as_str(), span)
                            .and_then(|symbol| parameters.get(symbol.as_str()))
                        else {
                            continue;
                        };
                        if crate::owners::containing_ast_function(&file.ast, call.span)
                            .is_some_and(|owner| owner.span == function)
                            && crate::interproc::site_on_every_completion(
                                self, file, function, call.span,
                            )
                        {
                            forwards.push((file, call, position, function, index));
                        }
                    }
                }
            }
            // ADR 0285: `function f(cb) { g(cb) }` calls `cb` on every
            // completion when the call is on f's every-completion path, the
            // argument is the parameter itself, unchanged, at an exactly
            // mapped position, and `g` calls that position on every
            // completion. `g` must be a function declaration: a `const`
            // callee may be in its temporal dead zone (ADR 0218). To a
            // fixpoint, so a chain of forwards composes.
            let mut changed = true;
            while changed {
                changed = false;
                for &(file, call, position, function, index) in &forwards {
                    let key = (file.path.as_str(), function, index);
                    if calls.contains(&key) {
                        continue;
                    }
                    let forwarded = crate::execution_role::project_callee_at_parameter(
                        file, call, position, self,
                    )
                    .is_some_and(|(callee_file, callee, exact)| {
                        exact
                            && callee.kind == solid_facts::ast::FunctionKind::Declaration
                            && calls.contains(&(callee_file.path.as_str(), callee.span, position))
                    });
                    if forwarded {
                        calls.insert(key);
                        changed = true;
                    }
                }
            }
            calls
        })
    }

    /// The arguments in `path` that a project function forwards, unchanged,
    /// into a slot the dialect says runs when the call's returned object is
    /// read ([`solid_dialect::Dialect::callback_runs_on_result_access`]).
    ///
    /// `function hideBy(props, hidden) { return omit(props, hidden); }` makes
    /// the second argument of every in-project `hideBy(…)` call such a
    /// callback: its code runs on reads of the view, wherever those happen,
    /// exactly as if it were written at `omit`. One level of forwarding, by
    /// exact parameter identity (a single-name parameter binding the
    /// argument's declaration resolves to) and exact call-site resolution
    /// ([`Self::function_call_sites`]); a wrapper of a wrapper, a parameter
    /// renamed through a local, or a destructured parameter is not followed.
    pub(super) fn result_access_forwarded_arguments(&self, path: &str) -> &[ResultAccessForwarded] {
        self.result_access_forwarded_index()
            .get(path)
            .map_or(&[], Vec::as_slice)
    }

    fn result_access_forwarded_index(&self) -> &HashMap<&'a str, Vec<ResultAccessForwarded>> {
        self.result_access_forwarded.get_or_init(|| {
            let mut forwarded = HashMap::<&'a str, Vec<ResultAccessForwarded>>::new();
            // Most dialect variants answer no slot at all; skip the walk.
            let answered = self.project_primitives().iter().any(|primitive| {
                (0..PROBED_ARGUMENTS).any(|count| {
                    (0..count).any(|argument| {
                        self.dialect
                            .callback_runs_on_result_access(*primitive, argument, count)
                    })
                })
            });
            if !answered {
                return forwarded;
            }
            let facts: &'a ProjectFacts = self.facts;
            for file in &facts.files {
                for call in &file.ast.calls {
                    let Some(primitive) = self.primitive_at_call(file, call.span) else {
                        continue;
                    };
                    let count = call.arguments.len();
                    for (index, argument) in call.arguments.iter().enumerate() {
                        if argument.spread
                            || argument.value != solid_facts::ast::ArgumentValueKind::Identifier
                            || !self
                                .dialect
                                .callback_runs_on_result_access(primitive, index, count)
                        {
                            continue;
                        }
                        let Some(declaration) = argument.binding_declaration else {
                            continue;
                        };
                        for function in file.ast.functions_body_containing(call.span) {
                            let Some(parameter) = function.parameters.iter().position(|binding| {
                                binding.names.len() == 1 && binding.names[0].span == declaration
                            }) else {
                                continue;
                            };
                            for (caller, callee) in
                                self.function_call_sites(file.path.as_str(), function.span)
                            {
                                let Some(site) =
                                    caller.ast.calls.iter().find(|site| site.callee == callee)
                                else {
                                    continue;
                                };
                                // A spread up to the slot moves arguments into it.
                                if site
                                    .arguments
                                    .iter()
                                    .take(parameter.saturating_add(1))
                                    .any(|candidate| candidate.spread)
                                {
                                    continue;
                                }
                                if let Some(forwarded_argument) = site.arguments.get(parameter) {
                                    forwarded.entry(caller.path.as_str()).or_default().push(
                                        ResultAccessForwarded {
                                            argument: forwarded_argument.span,
                                            primitive,
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
            }
            for entries in forwarded.values_mut() {
                entries.sort_unstable_by_key(|entry| (entry.argument.start, entry.argument.end));
                entries.dedup_by_key(|entry| entry.argument);
            }
            forwarded
        })
    }

    /// The position of a call in `file.ast.calls`, for the primitive tables
    /// that are index-aligned with it.
    pub(super) fn call_index(&self, file: &FileFacts, span: Span) -> Option<usize> {
        self.ast_indexes
            .get(file.path.as_str())
            .and_then(|index| index.call_index_by_span(span))
    }

    /// The primitive resolved for the call occupying exactly `span`.
    pub(super) fn primitive_at_call(
        &self,
        file: &FileFacts,
        span: Span,
    ) -> Option<solid_dialect::Primitive> {
        let index = self.call_index(file, span)?;
        super::known_primitive(&self.primitives(file).calls[index])
    }

    /// Whether a member expression occupies exactly `span`, which distinguishes
    /// `factory(...).member()` from `factory(...)()`.
    pub(super) fn is_member_span(&self, file: &FileFacts, span: Span) -> bool {
        self.member_property_at(file, span).is_some()
    }

    /// The property span of the member expression occupying exactly `span`.
    pub(super) fn member_property_at(&self, file: &FileFacts, span: Span) -> Option<Span> {
        self.ast_indexes
            .get(file.path.as_str())
            .and_then(|index| index.member_property(span))
    }

    /// The position of a file in project facts, for the per-file memo tables.
    fn file_index(&self, file: &FileFacts) -> usize {
        *self
            .files_by_path
            .get_or_init(|| {
                self.facts
                    .files
                    .iter()
                    .enumerate()
                    .map(|(index, file)| (file.path.as_str(), index))
                    .collect()
            })
            .get(file.path.as_str())
            .expect("per-file lookup for a file outside project facts")
    }

    /// How this file's callback positions name its own functions, memoized.
    ///
    /// The answer depends only on the file, so a read-by-read derivation was
    /// re-scanning every call and JSX element in the file for every read the
    /// classifier was asked about.
    pub(super) fn named_callback_roles(&self, file: &FileFacts) -> &super::NamedCallbackRoles {
        let index = self.file_index(file);
        let slots = self
            .file_named_callback_roles
            .get_or_init(|| self.facts.files.iter().map(|_| OnceLock::new()).collect());
        slots[index].get_or_init(|| {
            super::named_callback_roles(
                &self.facts.files[index],
                self.entities,
                self.symbol_names,
                self,
            )
        })
    }

    /// The memoized primitive names for one project file.
    pub(super) fn primitives(&self, file: &FileFacts) -> &FilePrimitives {
        let index = self.file_index(file);
        let slots = self
            .file_primitives
            .get_or_init(|| self.facts.files.iter().map(|_| OnceLock::new()).collect());
        slots[index].get_or_init(|| {
            let file = &self.facts.files[index];
            FilePrimitives {
                calls: file
                    .ast
                    .calls
                    .iter()
                    .map(|call| {
                        super::call_primitive_name(
                            file,
                            call,
                            self.entities,
                            self.symbol_names,
                            self.dialect,
                        )
                    })
                    .collect(),
                jsx: file
                    .ast
                    .jsx_elements
                    .iter()
                    .map(|element| {
                        super::jsx_primitive_name(
                            file,
                            element,
                            self.entities,
                            self.symbol_names,
                            self.dialect,
                        )
                    })
                    .collect(),
            }
        })
    }

    pub(super) fn entities(&self) -> &'a EntitySymbols {
        self.entities
    }

    pub(super) fn object_member_targets(&self) -> &crate::object_members::Targets {
        self.object_members
            .get_or_init(|| crate::object_members::index(self.facts, self))
    }

    pub(super) fn literal_receiver_members(&self) -> &HashSet<(String, Span)> {
        self.literal_receiver_members
            .get_or_init(|| crate::object_members::literal_receiver_members(self))
    }

    pub(super) fn object_member_function(
        &self,
        file: &FileFacts,
        callee: Span,
    ) -> Option<(&'a FileFacts, &'a solid_facts::ast::FunctionFact)> {
        let (path, span) = self
            .object_member_targets()
            .get(&crate::location(file.path.shared(), callee))?;
        let file = self.file_by_path(path)?;
        Some((
            file,
            file.ast
                .functions
                .iter()
                .find(|function| function.span == *span)?,
        ))
    }

    pub(super) fn symbol_names(&self) -> &'a HashMap<SymbolId, SymbolName> {
        self.symbol_names
    }

    pub(super) fn symbol_at(
        &self,
        path: &str,
        span: Span,
    ) -> Option<solid_facts::TypeScriptSymbol<'a>> {
        let entity = self.entity_at(path, span)?;
        if entity.symbol_unresolved || entity.symbol.is_empty() {
            return None;
        }
        self.symbols_by_id().get(entity.symbol.as_ref()).copied()
    }

    pub(super) fn files(&self) -> &'a [FileFacts] {
        &self.facts.files
    }

    /// ADR 0319: the invocation-flow answer of every literal, computed once.
    pub(crate) fn invocation_flow_answers(&self) -> &HashMap<(String, Span), crate::ExecutionRole> {
        self.invocation_flow_answers
            .get_or_init(|| crate::execution_role::invocation_flow_answers(self))
    }

    /// The literals whose every invocation is a host event dispatch, for the
    /// owner seeds; see [`crate::execution_role::unowned_event_literals`].
    pub(crate) fn unowned_event_literals(&self) -> &HashSet<(String, Span)> {
        self.unowned_event_literals
            .get_or_init(|| crate::execution_role::unowned_event_literals(self))
    }

    pub(super) fn tuple_iteration_may_be_mutated(&self) -> bool {
        *self
            .tuple_iteration_veto
            .get_or_init(|| crate::interproc::accessor_provenance::tuple_iteration_write(self))
    }

    /// Facts are immutable for this lookup's lifetime. Proxy and invocation
    /// guards consume the same project census as tuple identity, without
    /// repeating it at every source-call site. A new generation has a new lock.
    pub(super) fn visible_string_code(&self) -> bool {
        *self
            .string_code_veto
            .get_or_init(|| crate::interproc::accessor_provenance::visible_string_code(self))
    }

    /// ADR 0292: whether `property` holding `function` is the `default` of a
    /// module object a dialect component loader renders, and that function's
    /// one reference ([`crate::owners::lazy_default_render_witnesses`]).
    pub(super) fn is_lazy_default_render_witness(
        &self,
        file: &FileFacts,
        function: Span,
        property: Span,
    ) -> bool {
        self.lazy_default_render_witnesses
            .get_or_init(|| crate::owners::lazy_default_render_witnesses(self))
            .contains(&(file.path.as_str(), function, property))
    }

    pub(super) fn file_by_path(&self, path: &str) -> Option<&'a FileFacts> {
        self.files_by_path
            .get_or_init(|| {
                self.facts
                    .files
                    .iter()
                    .enumerate()
                    .map(|(index, file)| (file.path.as_str(), index))
                    .collect()
            })
            .get(path)
            .map(|index| &self.facts.files[*index])
    }

    /// A missing symbol is missing reference evidence, not an empty census.
    /// The compiler's declarations of `symbol`, at their name spans; `None`
    /// when Type Facts does not know the symbol.
    pub(super) fn symbol_declarations(&self, symbol: &str) -> Option<&'a [typefacts::Declaration]> {
        self.symbols_by_id()
            .get(symbol)
            .map(|candidate| candidate.declarations())
    }

    pub(super) fn symbol_references_if_present(&self, symbol: &str) -> Option<Vec<Location>> {
        self.symbols_by_id()
            .get(symbol)
            .map(|candidate| candidate.references().cloned().collect())
    }

    /// ADR 0324: whether the router-parameters premise applies to this
    /// project.
    pub(super) fn router_params(&self) -> &crate::router_params::RouterParamsStatus {
        self.router_params.get_or_init(|| {
            crate::router_params::router_params_status(
                self.facts,
                self.dialect,
                &self.router_install,
            )
        })
    }

    /// The compiler's attested resolution of the project's import specifiers,
    /// when the analysis carries it.
    pub(super) fn resolved_imports(&self) -> Option<&'a solid_facts::AttestedImportIndex> {
        self.facts.resolved_imports.as_ref()
    }

    /// The project bundler's resolution of each module load, when the host
    /// asked for it (ADR 0220).
    pub(super) fn runtime_resolutions(
        &self,
    ) -> Option<&'a solid_facts::runtime_resolution::RuntimeResolutionIndex> {
        self.facts.runtime_resolutions.as_ref()
    }

    /// The files declaring the module a namespace binding at `span` names:
    /// its symbol's alias target's declarations. `None` when Type Facts does
    /// not resolve it.
    pub(super) fn namespace_module_paths(&self, path: &str, span: Span) -> Option<Vec<&'a str>> {
        let symbol = self.entities.at(path, span)?;
        let symbols = self.symbols_by_id();
        let target = symbols.get(symbol.as_str())?.alias_target();
        let paths = symbols
            .get(target)?
            .declarations()
            .iter()
            .map(|declaration| declaration.location.path.as_ref())
            .collect::<Vec<&'a str>>();
        (!paths.is_empty()).then_some(paths)
    }

    pub(super) fn symbol_references(&self, symbol: &str) -> Vec<Location> {
        self.symbols_by_id()
            .get(symbol)
            .map(|candidate| candidate.references().cloned().collect())
            .unwrap_or_default()
    }

    /// The binding declaration named by an exact canonical symbol reference.
    ///
    /// Returned functions can cross files and destructuring patterns before
    /// they are called. Building this reverse index once keeps that proof
    /// linear in project facts instead of rescanning every binding for every
    /// call site.
    /// ADR 0209: the project class `symbol` declares: its one declaration is a
    /// class declaration, written at the name of a class in an analyzed file.
    /// A symbol with several declarations (a class merged with an interface or
    /// a namespace) names no exact class.
    pub(super) fn class_for_symbol(
        &self,
        symbol: &str,
    ) -> Option<(&'a FileFacts, &'a solid_facts::ast::ClassFact)> {
        let [declaration] = self.symbols_by_id().get(symbol)?.declarations() else {
            return None;
        };
        if &*declaration.kind != "class" {
            return None;
        }
        let file = self
            .facts
            .files
            .iter()
            .find(|file| *declaration.location.path == *file.path.as_str())?;
        let (Ok(start), Ok(end)) = (
            u32::try_from(declaration.location.start_byte),
            u32::try_from(declaration.location.end_byte),
        ) else {
            return None;
        };
        let at = Span::new(start, end);
        let class = file.ast.classes.iter().find(|class| {
            class
                .name
                .as_ref()
                .is_some_and(|name| name.span == at || class.span == at)
        })?;
        Some((file, class))
    }

    /// ADR 0209: whether some assignment in the project writes a property
    /// named `name` on any object, or writes through any `prototype`.
    pub(super) fn member_name_may_be_reassigned(&self, name: &str) -> bool {
        let assigned = self.assigned_members();
        assigned.global
            || !assigned.constructors.is_empty()
            || !assigned.hosts.is_empty()
            || assigned.names.contains(name)
    }

    /// ADR 0314: [`Self::member_name_may_be_reassigned`] for a value whose
    /// prototype chain holds only built-in ECMAScript prototypes -- a fresh
    /// literal, a primitive, a built-in collection (the ADR 0211 `Builtin`
    /// origin). A write through a project constructor's prototype cannot
    /// reach it, and neither can one through a host prototype every
    /// declaration of which is in `lib.dom.d.ts`: no ECMAScript value inherits
    /// from a DOM interface.
    pub(super) fn member_may_be_reassigned_on_builtin(&self, name: &str) -> bool {
        let assigned = self.assigned_members();
        assigned.global
            || assigned.hosts.iter().any(|(_, dom)| !dom)
            || assigned.names.contains(name)
    }

    /// ADR 0314: [`Self::member_name_may_be_reassigned`] for an instance of
    /// the project class `class`: a write through a prototype reaches it only
    /// when its chain holds that prototype. Every ancestor must be a project
    /// class this resolves; an unresolved `extends` may be anything.
    pub(super) fn member_may_be_reassigned_on_class(
        &self,
        name: &str,
        file: &'a FileFacts,
        class: &'a solid_facts::ast::ClassFact,
    ) -> bool {
        let assigned = self.assigned_members();
        assigned.global
            || assigned.hosts.iter().any(|(_, dom)| !dom)
            || assigned.names.contains(name)
            || self.class_chain_prototype_written(file, class, 8)
    }

    fn class_chain_prototype_written(
        &self,
        file: &'a FileFacts,
        class: &'a solid_facts::ast::ClassFact,
        depth: usize,
    ) -> bool {
        let assigned = self.assigned_members();
        if depth == 0 {
            return true;
        }
        if !assigned.constructors.is_empty() {
            // The class's own name, or the `const` a class expression is bound to.
            let declared = class.name.as_ref().map(|name| name.span).into_iter().chain(
                file.ast
                    .bindings
                    .iter()
                    .filter(|binding| {
                        binding.initializer.is_some_and(|initializer| {
                            file.ast.peel_ts_sugar_span(initializer) == class.span
                        })
                    })
                    .flat_map(|binding| binding.names.iter().map(|name| name.span)),
            );
            for declaration in declared {
                if assigned
                    .constructors
                    .contains(&(file.path.to_string(), declaration))
                {
                    return true;
                }
            }
        }
        let Some(heritage) = class.heritage else {
            return false;
        };
        let heritage = file.ast.peel_ts_sugar_span(heritage);
        let base = file
            .ast
            .reference_declaration(heritage)
            .and_then(|declaration| {
                file.ast
                    .classes
                    .iter()
                    .find(|candidate| {
                        candidate
                            .name
                            .as_ref()
                            .is_some_and(|name| name.span == declaration)
                    })
                    .map(|base| (file, base))
            })
            .or_else(|| {
                self.entities
                    .at(file.path.as_str(), heritage)
                    .and_then(|symbol| self.class_for_symbol(symbol))
            });
        base.is_none_or(|(base_file, base)| {
            self.class_chain_prototype_written(base_file, base, depth - 1)
        })
    }

    fn assigned_members(&self) -> &AssignedMembers {
        self.assigned_member_names.get_or_init(|| {
            let facts: &'a ProjectFacts = self.facts;
            let mut names = HashSet::new();
            let mut prototype = false;
            let mut constructors = HashSet::new();
            let mut pending_hosts = Vec::new();
            let mut record =
                |file: &'a FileFacts, owner: Option<PrototypeOwner>, prototype: &mut bool| {
                    match owner {
                        Some(PrototypeOwner::Constructor(declaration)) => {
                            constructors.insert((file.path.to_string(), declaration));
                        }
                        Some(PrototypeOwner::Hosts(method, hosts)) => {
                            pending_hosts.push((method, hosts));
                        }
                        None => *prototype = true,
                    }
                };
            for file in &facts.files {
                let targets = file
                    .ast
                    .assignments
                    .iter()
                    .map(|assignment| assignment.target)
                    .chain(file.ast.iteration_targets.iter().copied());
                for target in targets {
                    let target = file.ast.peel_ts_sugar_span(target);
                    let text = file.source_text(target).unwrap_or_default();
                    // Every member written by the target: the target itself,
                    // or a leaf of a destructuring pattern (`[c.run] = …`) or
                    // loop head (`for (c.run of …)`).
                    // A member that is another written member's object
                    // (`C.prototype` in `C.prototype.m = f`) is read, not
                    // written.
                    let inside = file
                        .ast
                        .members
                        .iter()
                        .filter(|member| target.contains(member.span))
                        .collect::<Vec<_>>();
                    let mut written = inside
                        .iter()
                        .copied()
                        .filter(|member| {
                            !inside.iter().any(|outer| {
                                file.ast.peel_ts_sugar_span(outer.object) == member.span
                            })
                        })
                        .peekable();
                    if written.peek().is_none() && text.contains("prototype") {
                        prototype = true;
                    }
                    for member in written {
                        if file
                            .ast
                            .computed_members
                            .binary_search(&member.span)
                            .is_ok()
                        {
                            // A computed write names its key only when the key is
                            // a literal, or a `const` bound to a string literal.
                            // Any other key, on a prototype, may be any member.
                            if let Some(literal) = file
                                .ast
                                .literal_computed_members
                                .iter()
                                .find(|literal| literal.span == member.span)
                            {
                                names.insert(literal.key.to_string());
                                if literal.key.as_str() == "prototype" {
                                    prototype = true;
                                }
                            } else if let Some(key) = constant_string_key(file, member.property) {
                                if key == "prototype" {
                                    prototype = true;
                                }
                                names.insert(key);
                            } else if file
                                .source_text(member.object)
                                .unwrap_or_default()
                                .contains("prototype")
                            {
                                // `R.prototype[key] = …`: narrowed to `R`'s
                                // instances when `R` is proven (ADR 0314).
                                let object = file.ast.peel_ts_sugar_span(member.object);
                                match file.ast.members.iter().find(|prototype_member| {
                                    prototype_member.span == object
                                        && file
                                            .ast
                                            .computed_members
                                            .binary_search(&prototype_member.span)
                                            .is_err()
                                        && file.source_text(prototype_member.property)
                                            == Some("prototype")
                                }) {
                                    Some(prototype_member) => record(
                                        file,
                                        self.prototype_owner(file, prototype_member.object),
                                        &mut prototype,
                                    ),
                                    None => prototype = true,
                                }
                            }
                        } else if let Some(property) = file.source_text(member.property) {
                            names.insert(property.to_string());
                            if property == "prototype" {
                                // `R.prototype = …` (ADR 0314).
                                record(
                                    file,
                                    self.prototype_owner(file, member.object),
                                    &mut prototype,
                                );
                            }
                        }
                    }
                    // A dynamic key not spelled through a prototype (`c[key] = f`
                    // with `key: keyof C`, or through a prototype alias) is not
                    // modeled. Like `Object.defineProperty`, it is a reflective
                    // write this trust boundary leaves out (ADR 0215): vetoing
                    // every dynamic write would refuse every `xs[i] = v`.
                }
            }
            // A host proof holds only while neither the method nor the
            // global constructors it names are rewritten.
            let mut hosts = Vec::new();
            for (method, found) in pending_hosts {
                if names.contains(&method)
                    || found.iter().any(|(host, _)| {
                        names.contains(host) || global_identifier_written(facts, host)
                    })
                {
                    prototype = true;
                }
                hosts.extend(found);
            }
            AssignedMembers {
                names,
                global: prototype,
                constructors,
                hosts,
            }
        })
    }

    /// ADR 0314: whose instances a write through `receiver.prototype` can
    /// reach, when that is provable; `None` for any other receiver.
    ///
    /// - A class, a function declaration, or a `const` bound to a function or
    ///   class expression, declared in this file and never rebound: that
    ///   constructor's instances.
    /// - A parameter of a static class method nothing rebinds, when every
    ///   access to that method anywhere is a direct call through the class's
    ///   own name and passes, at that parameter, a global constructor declared
    ///   only in TypeScript's default libraries: those host prototypes.
    fn prototype_owner(&self, file: &'a FileFacts, receiver: Span) -> Option<PrototypeOwner> {
        use solid_facts::ast::BindingShape;
        let receiver = file.ast.peel_ts_sugar_span(receiver);
        if file
            .ast
            .identifiers
            .binary_search_by_key(&receiver, |identifier| identifier.span)
            .is_err()
        {
            return None;
        }
        let declaration = file.ast.reference_declaration(receiver)?;
        if let Some((function, index)) = file.ast.functions.iter().find_map(|function| {
            function
                .parameters
                .iter()
                .position(|parameter| parameter.names.iter().any(|name| name.span == declaration))
                .map(|index| (function, index))
        }) {
            return self.host_prototype_owner(file, function, index);
        }
        let declared_constructor = file.ast.classes.iter().any(|class| {
            class
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration)
        }) || file.ast.functions.iter().any(|function| {
            function.kind == solid_facts::ast::FunctionKind::Declaration
                && function
                    .name
                    .as_ref()
                    .is_some_and(|name| name.span == declaration)
        });
        if declared_constructor {
            return (!binding_written(file, declaration))
                .then_some(PrototypeOwner::Constructor(declaration));
        }
        let binding = file
            .ast
            .bindings
            .iter()
            .find(|binding| binding.names.iter().any(|name| name.span == declaration))?;
        let initializer = file.ast.peel_ts_sugar_span(binding.initializer?);
        (binding.immutable
            && binding.shape == BindingShape::Identifier
            && (file.ast.functions.iter().any(|function| {
                function.span == initializer
                    && function.kind != solid_facts::ast::FunctionKind::Arrow
            }) || file
                .ast
                .classes
                .iter()
                .any(|class| class.span == initializer)))
        .then_some(PrototypeOwner::Constructor(declaration))
    }

    fn host_prototype_owner(
        &self,
        file: &'a FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
        index: usize,
    ) -> Option<PrototypeOwner> {
        use solid_facts::ast::{BindingShape, ClassElementKind};
        let parameter = function.parameters.get(index)?;
        let [name] = parameter.names.as_slice() else {
            return None;
        };
        if parameter.shape != BindingShape::Identifier
            || parameter.initializer.is_some()
            || (function.rest_parameter && index + 1 == function.parameters.len())
            || binding_written(file, name.span)
            || file.ast.identifiers.iter().any(|identifier| {
                function.span.contains(identifier.span)
                    && file.source_text(identifier.span) == Some("arguments")
            })
        {
            return None;
        }
        let (class, element) = file.ast.classes.iter().find_map(|class| {
            class
                .elements
                .iter()
                .find(|element| {
                    element.value == Some(function.span)
                        && element.r#static
                        && element.kind == ClassElementKind::Method
                        && !element.computed
                })
                .map(|element| (class, element))
        })?;
        let method = file.source_text(element.key?)?;
        let class_name = class.name.as_ref()?;
        let class_symbol = self.entities.at(file.path.as_str(), class_name.span)?;
        let is_this_class = |reference_file: &FileFacts, span: Span| {
            self.entities
                .at(reference_file.path.as_str(), span)
                .and_then(|symbol| self.class_for_symbol(symbol))
                .is_some_and(|(found_file, found)| {
                    found_file.path == file.path && found.span == class.span
                })
        };
        // The class never escapes: past its declaration and imports, every
        // reference is the object of a member access naming its key.
        for reference in self.symbol_references_if_present(class_symbol.as_str())? {
            let reference_file = self.file_by_path(reference.path.as_ref())?;
            let (Ok(start), Ok(end)) = (
                u32::try_from(reference.start_byte),
                u32::try_from(reference.end_byte),
            ) else {
                return None;
            };
            let span = Span::new(start, end);
            if (reference_file.path == file.path && span == class_name.span)
                || reference_file
                    .ast
                    .imports
                    .iter()
                    .any(|import| import.span.contains(span))
            {
                continue;
            }
            if !reference_file.ast.members.iter().any(|member| {
                reference_file.ast.peel_ts_sugar_span(member.object) == span
                    && reference_file
                        .ast
                        .computed_members
                        .binary_search(&member.span)
                        .is_err()
            }) {
                return None;
            }
        }
        let mut hosts = Vec::new();
        for other in self.files() {
            if other
                .ast
                .literal_computed_members
                .iter()
                .any(|literal| literal.key.as_str() == method)
                || other.ast.bindings.iter().any(|binding| {
                    binding
                        .object_slots
                        .iter()
                        .any(|slot| slot.property.as_str() == method)
                })
            {
                return None;
            }
            for member in other.ast.members.iter().filter(|member| {
                other
                    .ast
                    .computed_members
                    .binary_search(&member.span)
                    .is_err()
                    && other.source_text(member.property) == Some(method)
            }) {
                if !is_this_class(other, other.ast.peel_ts_sugar_span(member.object)) {
                    return None;
                }
                let call = other.ast.calls.iter().find(|call| {
                    !call.construct && other.ast.peel_ts_sugar_span(call.callee) == member.span
                })?;
                if call
                    .arguments
                    .iter()
                    .take(index + 1)
                    .any(|argument| argument.spread)
                {
                    return None;
                }
                hosts.push(self.global_constructor(other, call.arguments.get(index)?.span)?);
            }
        }
        Some(PrototypeOwner::Hosts(method.to_owned(), hosts))
    }

    /// The name of the global constructor `value` names, and whether every
    /// `var` declaration of it is in `lib.dom.d.ts`, when it is an identifier
    /// no project binding declares, every declaration of it is ambient, and
    /// its value is declared only by TypeScript's default libraries.
    fn global_constructor(&self, file: &'a FileFacts, value: Span) -> Option<(String, bool)> {
        let value = file.ast.peel_ts_sugar_span(value);
        if file
            .ast
            .identifiers
            .binary_search_by_key(&value, |identifier| identifier.span)
            .is_err()
            || file.ast.reference_declaration(value).is_some()
        {
            return None;
        }
        let symbol = self.symbol_at(file.path.as_str(), value)?;
        if !symbol.alias_target().is_empty() || symbol.declarations().is_empty() {
            return None;
        }
        // Every declaration is ambient, in a declaration file no analyzed
        // source is: none creates a runtime binding (`@webgpu/types` merges an
        // interface into `HTMLCanvasElement`; `babylonjs` re-declares
        // `var OffscreenCanvas`). The value is the host's, which a default
        // library declares, under the standard host premise (ADR 0295, 0300).
        let mut dom = true;
        let mut library = false;
        for declaration in symbol.declarations() {
            let path = declaration.location.path.as_ref();
            let name = std::path::Path::new(path).file_name()?.to_str()?;
            if self.file_by_path(path).is_some() || !name.ends_with(".d.ts") {
                return None;
            }
            if declaration.kind.as_ref() == "variable" && name.starts_with("lib.") {
                library = true;
                dom &= name == "lib.dom.d.ts";
            }
        }
        let variable = library;
        variable
            .then(|| (file.source_text(value).map(str::to_owned), dom))
            .and_then(|(name, dom)| Some((name?, dom)))
    }

    /// Review round 1 of ADR 0293: whether any analyzed file may define a
    /// property reflectively, installing an accessor where a literal wrote
    /// data. Deliberately syntactic and wide: any member access, destructured
    /// property or callee spelled `defineProperty`, `defineProperties` or
    /// `__defineGetter__`, whatever its receiver, so an alias
    /// (`const define = Object.defineProperty`) is caught where it is taken.
    pub(super) fn property_may_be_defined_reflectively(&self) -> bool {
        const NAMES: [&str; 3] = ["defineProperty", "defineProperties", "__defineGetter__"];
        *self.reflective_property_definition.get_or_init(|| {
            self.files().iter().any(|file| {
                file.ast.members.iter().any(|member| {
                    file.source_text(member.property).is_some_and(|property| {
                        NAMES.contains(&property.trim_matches(['\'', '"', '`']))
                    })
                }) || file.ast.bindings.iter().any(|binding| {
                    binding
                        .object_slots
                        .iter()
                        .any(|slot| NAMES.contains(&slot.property.as_str()))
                }) || file.ast.calls.iter().any(|call| {
                    file.source_text(call.callee)
                        .is_some_and(|callee| NAMES.iter().any(|name| callee.ends_with(name)))
                })
            })
        })
    }

    /// ADR 0314: [`Self::property_may_be_defined_reflectively`] for one
    /// property name. Every spelling of a reflective definer must be the
    /// callee of a call whose key argument (the second) is a string literal
    /// other than `name`; any other use -- an alias, a destructuring, a
    /// computed or missing key, `defineProperties`, `__defineGetter__` with
    /// this name -- may define `name`.
    pub(super) fn property_may_be_defined_reflectively_as(&self, name: &str) -> bool {
        use solid_facts::ast::ArgumentLiteralFact;
        const NAMES: [&str; 3] = ["defineProperty", "defineProperties", "__defineGetter__"];
        if !self.property_may_be_defined_reflectively() {
            return false;
        }
        self.files().iter().any(|file| {
            let spelled = |text: &str| NAMES.contains(&text.trim_matches(['\'', '"', '`']));
            file.ast.bindings.iter().any(|binding| {
                binding
                    .object_slots
                    .iter()
                    .any(|slot| NAMES.contains(&slot.property.as_str()))
            }) || file.ast.members.iter().any(|member| {
                file.source_text(member.property).is_some_and(spelled)
                    && !file.ast.calls.iter().any(|call| {
                        file.ast.peel_ts_sugar_span(call.callee) == member.span
                            && file.source_text(member.property).is_some_and(|text| {
                                text.trim_matches(['\'', '"', '`']) != "defineProperties"
                            })
                            && matches!(
                                call.arguments.get(1).map(|key| (&key.literal_value, key.spread)),
                                Some((ArgumentLiteralFact::String(key), false)) if key.as_str() != name
                            )
                            && !call.arguments.first().is_some_and(|target| target.spread)
                    })
            }) || file.ast.calls.iter().any(|call| {
                file.source_text(call.callee).is_some_and(|callee| {
                    NAMES.iter().any(|definer| callee.ends_with(definer))
                }) && !file
                    .ast
                    .members
                    .iter()
                    .any(|member| file.ast.peel_ts_sugar_span(call.callee) == member.span)
            })
        })
    }

    /// Review round 1 of ADR 0293: the project-wide inputs of an exempt store
    /// Get's proof (`local_access::store_get_decision`), for the cross-file
    /// digest: per key the dialect exempts that some Get spells, whether a
    /// member of that name is written; then the prototype-write flag and
    /// [`Self::property_may_be_defined_reflectively`]. `None` when no Get
    /// spells an exempt key, so such a project keeps the digest it had.
    fn exempt_store_key_census(&self) -> Option<Vec<(String, bool)>> {
        let mut keys = self
            .files()
            .iter()
            .flat_map(|file| {
                file.ast
                    .members
                    .iter()
                    .filter_map(|member| file.source_text(member.property))
                    .chain(
                        file.ast
                            .literal_computed_members
                            .iter()
                            .map(|literal| literal.key.as_str()),
                    )
            })
            .filter(|key| self.dialect.store_key_may_skip_strict_read(key))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if keys.is_empty() {
            return None;
        }
        keys.sort_unstable();
        keys.dedup();
        let mut census = keys
            .into_iter()
            .map(|key| {
                let written = self.member_name_may_be_reassigned(&key);
                (key, written)
            })
            .collect::<Vec<_>>();
        census.push((
            "\0reflective-property-definition".into(),
            self.property_may_be_defined_reflectively(),
        ));
        census.push((
            "\0reflective-prototype-write".into(),
            crate::interproc::accessor_provenance::reflective_prototype_write(self),
        ));
        Some(census)
    }

    /// The expression inside any parentheses around `value`; `None` when a
    /// type assertion (`as`, `<T>`, `!`, `satisfies`) wraps it on the way,
    /// which changes the declared type and never the value.
    pub(super) fn peel_parentheses(file: &FileFacts, value: Span) -> Option<Span> {
        let mut value = value;
        while let Ok(index) = file
            .ast
            .transparent_wrappers
            .binary_search_by_key(&value, |wrapper| wrapper.span)
        {
            let wrapper = &file.ast.transparent_wrappers[index];
            if wrapper.inner == value {
                break;
            }
            let parenthesized = wrapper.inner.start > value.start
                && file.source.as_bytes().get(value.start as usize) == Some(&b'(');
            if !parenthesized {
                return None;
            }
            value = wrapper.inner;
        }
        Some(value)
    }

    /// ADR 0320: whether the expression at `value` evaluates to a primitive
    /// other than `undefined`, proven from what it is rather than from its
    /// declared type: a literal or template, an operator whose result is
    /// always primitive, a `.length` of a proven built-in value, a primitive
    /// result of an audited standard-library function, a conditional or
    /// logical expression all of whose results are, or an unexported `const`
    /// of this file initialized with one. A type assertion (`as`, `<T>`, `!`)
    /// anywhere on the way refuses: it changes the declared type, never the
    /// value. Only this file is read, so a cached answer follows its source.
    pub(super) fn proven_primitive(&self, file: &FileFacts, value: Span, depth: usize) -> bool {
        if depth == 0 {
            return false;
        }
        let Some(value) = Self::peel_parentheses(file, value) else {
            return false;
        };
        if file.ast.primitive_results.binary_search(&value).is_ok() {
            return true;
        }
        let Some(text) = file.source_text(value) else {
            return false;
        };
        if primitive_literal_text(text) {
            return true;
        }
        if let Some(conditional) = file
            .ast
            .conditional_expressions
            .iter()
            .find(|conditional| conditional.span == value)
        {
            return self.proven_primitive(file, conditional.consequent, depth - 1)
                && self.proven_primitive(file, conditional.alternate, depth - 1);
        }
        if let Some(logical) = file
            .ast
            .logical_expressions
            .iter()
            .find(|logical| logical.span == value)
        {
            return self.proven_primitive(file, logical.left, depth - 1)
                && self.proven_primitive(file, logical.right, depth - 1);
        }
        if let Some(member) = file.ast.members.iter().find(|member| member.span == value) {
            return file.source_text(member.property) == Some("length")
                && file
                    .ast
                    .computed_members
                    .binary_search(&member.span)
                    .is_err()
                && !self.member_may_be_reassigned_on_builtin("length")
                && matches!(
                    self.value_origin(
                        file,
                        member.object,
                        solid_facts::ast::RuntimeValueKind::Unknown,
                        depth - 1
                    ),
                    Some(ValueOrigin::Builtin)
                );
        }
        if let Some(call) = file.ast.call_at(value) {
            if call.construct || call.arguments.iter().any(|argument| argument.spread) {
                return false;
            }
            return self
                .standard_library_declaration(file, call, typefacts::CallKind::Call)
                .is_some_and(|declaration| {
                    PRIMITIVE_RESULT_FUNCTIONS.contains(&declaration.qualified_name.as_ref())
                });
        }
        let Some((binding_file, binding, _)) = self.binding_at_reference(file.path.as_str(), value)
        else {
            return false;
        };
        // ADR 0325: a named import of another module's exported constant.
        if binding_file.path != file.path {
            return self.imported_const_is_primitive(file, value, binding_file, binding, depth);
        }
        if !binding.immutable
            || binding.shape != solid_facts::ast::BindingShape::Identifier
            || crate::execution_role::flow_binding_exported(
                binding_file,
                binding
                    .names
                    .first()
                    .map_or(binding.declaration, |name| name.span),
            )
            || binding_file.ast.functions.iter().any(|function| {
                function.parameters.iter().any(|parameter| {
                    parameter
                        .names
                        .iter()
                        .any(|name| binding.names.iter().any(|bound| bound.span == name.span))
                })
            })
        {
            return false;
        }
        binding
            .initializer
            .is_some_and(|initializer| self.proven_primitive(file, initializer, depth - 1))
    }

    /// ADR 0325: whether the identifier at `value`, a named import of
    /// `binding` (declared in `binding_file`), holds a primitive other than
    /// `undefined`. Only the exact shape:
    ///
    /// - the identifier's declaration is the local of a named, value import
    ///   (not a namespace, a default or a type-only one) of a module that the
    ///   compiler's attested resolution lands on `binding_file` itself, with
    ///   no re-export hop;
    /// - `binding_file` is a module, and `binding` is a module-level `const`
    ///   of one plain identifier that the module exports under exactly the
    ///   imported name (`export const X`, or `export { N as X }`), not through
    ///   an `export … from`;
    /// - the initializer is proven primitive by [`Self::proven_primitive`],
    ///   in `binding_file`.
    ///
    /// A `const` cannot be reassigned and an ES module namespace is frozen, so
    /// nothing can change the value after initialization; anything that is
    /// not exactly this refuses (a barrel, an `export *`, a default import, an
    /// object, a call the audited table does not name, an assertion).
    fn imported_const_is_primitive(
        &self,
        file: &FileFacts,
        value: Span,
        binding_file: &'a FileFacts,
        binding: &solid_facts::ast::BindingFact,
        depth: usize,
    ) -> bool {
        use solid_facts::ast::{BindingShape, ExportKind, ImportKind};
        let value = file.ast.peel_ts_sugar_span(value);
        let Some(declaration) = file.ast.reference_declaration(value) else {
            return false;
        };
        let Some((import, imported)) = file.ast.imports.iter().find_map(|import| {
            import.bindings.iter().find_map(|candidate| {
                (candidate.local.span == declaration
                    && candidate.kind == ImportKind::Named
                    && !candidate.type_only
                    && !import.type_only)
                    .then_some((import, candidate.imported.as_deref()))
            })
        }) else {
            return false;
        };
        let (Some(imported), Some(imports)) = (imported, self.resolved_imports()) else {
            return false;
        };
        let solid_facts::SpecifierAttestation::Attested(row) =
            imports.specifier(file.path.as_str(), import.span, import.module.as_str())
        else {
            return false;
        };
        let landing = if row.included_path.is_empty() {
            row.resolved_path.as_ref()
        } else {
            row.included_path.as_ref()
        };
        let [name] = binding.names.as_slice() else {
            return false;
        };
        if landing != binding_file.path.as_str()
            || row.resolution == solid_facts::ImportResolution::Unresolved
            || !crate::execution_role::file_is_module(binding_file)
            || file_names_commonjs_exports(binding_file)
            || !binding.immutable
            || binding.shape != BindingShape::Identifier
            || binding_file
                .ast
                .functions
                .iter()
                .any(|function| function.span.contains(binding.declaration))
        {
            return false;
        }
        let exported = binding_file.ast.exports.iter().any(|export| {
            export.module.is_none()
                && export.kind == ExportKind::Named
                && !export.type_only
                && (export.declarations.iter().any(|specifier| {
                    specifier.local.span == name.span
                        && specifier.exported.as_str() == imported
                        && !specifier.type_only
                }) || export.specifiers.iter().any(|specifier| {
                    specifier.exported.as_str() == imported
                        && !specifier.type_only
                        && binding_file.ast.reference_declaration(specifier.local.span)
                            == Some(name.span)
                }))
        });
        exported
            && binding.initializer.is_some_and(|initializer| {
                self.proven_primitive(binding_file, initializer, depth.saturating_sub(1))
            })
    }

    /// ADR 0325: the imported-constant answers the cross-file proofs read,
    /// as `(path, declaration span)` of every exported module-level `const`
    /// of one identifier whose initializer is proven primitive: the set
    /// [`Self::imported_const_is_primitive`] can accept. Empty when no file
    /// imports a binding by name.
    fn exported_primitive_consts(&self) -> Vec<(String, Span)> {
        use solid_facts::ast::{BindingShape, ImportKind};
        if !self.files().iter().any(|file| {
            file.ast.imports.iter().any(|import| {
                import
                    .bindings
                    .iter()
                    .any(|binding| binding.kind == ImportKind::Named)
            })
        }) {
            return Vec::new();
        }
        let mut found = Vec::new();
        for file in self.files() {
            if !crate::execution_role::file_is_module(file) || file_names_commonjs_exports(file) {
                continue;
            }
            for binding in &file.ast.bindings {
                let [name] = binding.names.as_slice() else {
                    continue;
                };
                if !binding.immutable
                    || binding.shape != BindingShape::Identifier
                    || file
                        .ast
                        .functions
                        .iter()
                        .any(|function| function.span.contains(binding.declaration))
                    || !file.ast.exports.iter().any(|export| {
                        export.module.is_none()
                            && (export
                                .declarations
                                .iter()
                                .any(|specifier| specifier.local.span == name.span)
                                || export.specifiers.iter().any(|specifier| {
                                    file.ast.reference_declaration(specifier.local.span)
                                        == Some(name.span)
                                }))
                    })
                {
                    continue;
                }
                if binding
                    .initializer
                    .is_some_and(|initializer| self.proven_primitive(file, initializer, 6))
                {
                    found.push((file.path.to_string(), name.span));
                }
            }
        }
        found.sort_unstable();
        found
    }

    /// ADR 0211: what the origin of the value at `value` proves about the
    /// object it evaluates to, followed through `const` bindings.
    ///
    /// `kind` is the value's syntactic runtime kind where the fact supplies
    /// one (an argument's, a binding initializer's), `Unknown` elsewhere.
    pub(super) fn value_origin(
        &self,
        file: &FileFacts,
        value: Span,
        kind: solid_facts::ast::RuntimeValueKind,
        depth: usize,
    ) -> Option<ValueOrigin<'a>> {
        let origin = self.value_origin_assuming(file, value, kind, depth, None)?;
        // ADR 0320: a project instance's constructor may hand `this` to code
        // that copies keys into it; a built-in constructor hands its result
        // to nothing, and an unshared binding holds it.
        match origin {
            ValueOrigin::ProjectClass(..) if self.objects_may_be_assigned_into() => None,
            origin => Some(origin),
        }
    }

    /// [`Self::value_origin`], with one parameter -- a setter updater's
    /// previous value -- assumed to hold a built-in value already. The
    /// assumption is the induction step: a signal every write of which keeps a
    /// built-in value built-in, from a built-in initial value, only ever holds
    /// one.
    fn value_origin_assuming(
        &self,
        file: &FileFacts,
        value: Span,
        kind: solid_facts::ast::RuntimeValueKind,
        depth: usize,
        assumed: Option<(&str, Span)>,
    ) -> Option<ValueOrigin<'a>> {
        use solid_facts::ast::RuntimeValueKind;
        if depth == 0 {
            return None;
        }
        // A fresh array: its members are `Array.prototype`'s. A primitive's
        // are its wrapper prototype's, and a nullish value has none to run.
        if matches!(
            kind,
            RuntimeValueKind::Array | RuntimeValueKind::Primitive | RuntimeValueKind::Nullish
        ) {
            return Some(ValueOrigin::Builtin);
        }
        let value = file.ast.peel_ts_sugar_span(value);
        if file.ast.array_literals.binary_search(&value).is_ok() {
            return Some(ValueOrigin::Builtin);
        }
        // `null`, and an `undefined` no local declaration shadows: nothing.
        match file.source_text(value) {
            Some("null") => return Some(ValueOrigin::Builtin),
            Some("undefined") if file.ast.reference_declaration(value).is_none() => {
                return Some(ValueOrigin::Builtin);
            }
            _ => {}
        }
        if let Some(call) = file.ast.call_at(value) {
            return self.call_origin(file, call, depth, assumed);
        }
        if let Some(conditional) = file
            .ast
            .conditional_expressions
            .iter()
            .find(|conditional| conditional.span == value)
        {
            let arm = |span| {
                matches!(
                    self.value_origin_assuming(
                        file,
                        span,
                        RuntimeValueKind::Unknown,
                        depth - 1,
                        assumed
                    ),
                    Some(ValueOrigin::Builtin)
                )
            };
            return (arm(conditional.consequent) && arm(conditional.alternate))
                .then_some(ValueOrigin::Builtin);
        }
        // `a ?? b`, `a || b`, `a && b`: the value is one operand or the other.
        if let Some(logical) = file
            .ast
            .logical_expressions
            .iter()
            .find(|logical| logical.span == value)
        {
            let operand = |span| {
                matches!(
                    self.value_origin_assuming(
                        file,
                        span,
                        RuntimeValueKind::Unknown,
                        depth - 1,
                        assumed
                    ),
                    Some(ValueOrigin::Builtin)
                )
            };
            return (operand(logical.left) && operand(logical.right))
                .then_some(ValueOrigin::Builtin);
        }
        if let Some((path, parameter)) = assumed
            && path == file.path.as_str()
            && file.ast.reference_declaration(value) == Some(parameter)
        {
            return Some(ValueOrigin::Builtin);
        }
        let (binding_file, binding, symbol) =
            self.binding_at_reference(file.path.as_str(), value)?;
        if !binding.immutable || binding.shape != solid_facts::ast::BindingShape::Identifier {
            return None;
        }
        // ADR 0320: a parameter's default holds only when no argument is
        // passed; an object a binding holds must stay unshared.
        let [name] = binding.names.as_slice() else {
            return None;
        };
        if binding_file.ast.functions.iter().any(|function| {
            function
                .parameters
                .iter()
                .any(|parameter| parameter.names.iter().any(|n| n.span == name.span))
        }) || (!matches!(
            binding.initializer_value_kind,
            solid_facts::ast::RuntimeValueKind::Primitive
                | solid_facts::ast::RuntimeValueKind::Nullish
        ) && !{
            // Untracked consumers: only callees in the binding's own file,
            // whose source this file's cached answers already depend on.
            let mut consulted = Vec::new();
            self.binding_stays_closed(
                binding_file,
                name.span,
                &symbol,
                file,
                value,
                &mut consulted,
            ) && consulted
                .iter()
                .all(|(path, _)| path.as_str() == binding_file.path.as_str())
        }) {
            return None;
        }
        self.value_origin_assuming(
            binding_file,
            binding.initializer?,
            binding.initializer_value_kind,
            depth - 1,
            assumed,
        )
    }

    fn call_origin(
        &self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        depth: usize,
        assumed: Option<(&str, Span)>,
    ) -> Option<ValueOrigin<'a>> {
        if !call.construct
            && call.arguments.is_empty()
            && let Some(origin) = self.accessor_origin(file, call, depth)
        {
            return Some(origin);
        }
        // A plain call of an exact project function returns what its body
        // returns. A method call is not followed: its receiver selects it.
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        if !call.construct
            && !self.is_member_span(file, callee)
            && file.ast.computed_members.binary_search(&callee).is_err()
            && let Some((function_file, function)) = self
                .callee_symbol(file, call.callee)
                .and_then(|symbol| self.function_for_symbol(symbol))
                .filter(|(function_file, function)| {
                    self.function_value_is_current(function_file, function)
                })
        {
            return self
                .returns_builtin(function_file, function, None, depth)
                .then_some(ValueOrigin::Builtin);
        }
        if call.construct {
            if let Some((class_file, class)) = self
                .callee_symbol(file, call.callee)
                .and_then(|symbol| self.class_for_symbol(symbol))
            {
                return Some(ValueOrigin::ProjectClass(class_file, class));
            }
            let declaration =
                self.standard_library_declaration(file, call, typefacts::CallKind::Construct)?;
            return BUILTIN_VALUE_CONSTRUCTORS
                .contains(&declaration.qualified_name.as_ref())
                .then_some(ValueOrigin::Builtin);
        }
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        let member = file.ast.members.iter().find(|member| member.span == callee);
        // A member of a proven built-in receiver is that receiver's own: its
        // class's prototype selects it, so no declaration is needed. An
        // unproven receiver falls through to the declaration below.
        // `split` defers to its separator's `Symbol.split`, user code unless
        // the separator is a primitive (or a RegExp literal).
        let separator_is_primitive = call.arguments.first().is_none_or(|separator| {
            !separator.spread
                && separator.runtime_value_kind == solid_facts::ast::RuntimeValueKind::Primitive
        });
        if let Some(member) = member
            && let Some(method) = file.source_text(member.property)
            && (RECEIVER_RETURNING_METHODS.contains(&method) || FRESH_VALUE_METHODS.contains(&method))
            && (method != "split" || separator_is_primitive)
            // ADR 0314: the receiver below is a proven built-in value.
            && !self.member_may_be_reassigned_on_builtin(method)
            // A species-built result follows the receiver's `constructor`.
            && (RECEIVER_RETURNING_METHODS.contains(&method)
                || !self.member_may_be_reassigned_on_builtin("constructor"))
            && matches!(
                self.value_origin_assuming(
                    file,
                    member.object,
                    solid_facts::ast::RuntimeValueKind::Unknown,
                    depth - 1,
                    assumed,
                ),
                Some(ValueOrigin::Builtin)
            )
        {
            return Some(ValueOrigin::Builtin);
        }
        let declaration =
            self.standard_library_declaration(file, call, typefacts::CallKind::Call)?;
        let name = declaration.qualified_name.as_ref();
        if FRESH_ARRAY_STATICS.contains(&name) {
            // `Array.from` called through a subclass (`List.from`) builds that
            // subclass, under the same declaration.
            let owner = name.split('.').next().unwrap_or_default();
            let global = owner.strip_suffix("Constructor").unwrap_or(owner);
            // The receiver must be that global, not a local of its name.
            return member
                .is_some_and(|member| {
                    let receiver = file.ast.peel_ts_sugar_span(member.object);
                    file.source_text(receiver) == Some(global)
                        && file.ast.reference_declaration(receiver).is_none()
                })
                .then_some(ValueOrigin::Builtin);
        }
        if name == "String.split" {
            return separator_is_primitive.then_some(ValueOrigin::Builtin);
        }
        None
    }

    /// ADR 0211, phase 2: the value an accessor call `items()` returns, when
    /// every value the accessor can hold is proven.
    ///
    /// - `const [items, setItems] = createSignal(initial)` declared in a
    ///   function: the initial value, and every value written through the
    ///   setter, whose every reference in its file is the callee of a call --
    ///   a setter handed anywhere else may write anything. An updater's
    ///   returns are proven with its previous value assumed.
    /// - `const sorted = createMemo(() => …)`: every value its compute, a
    ///   synchronous function literal, returns.
    fn accessor_origin(
        &self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        depth: usize,
    ) -> Option<ValueOrigin<'a>> {
        use solid_dialect::Primitive;
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        let (binding_file, binding, _) = self.binding_at_reference(file.path.as_str(), callee)?;
        if !binding.immutable {
            return None;
        }
        let initializer_span = binding_file
            .ast
            .peel_ts_sugar_span(binding.call_initializer?);
        let index = binding_file
            .ast
            .calls
            .iter()
            .position(|candidate| candidate.span == initializer_span)?;
        let initializer = &binding_file.ast.calls[index];
        let primitive = self.primitives(binding_file).calls[index]
            .as_ref()
            .and_then(super::PrimitiveName::primitive)?;
        let builtin = |file: &FileFacts, argument: &solid_facts::ast::ArgumentFact, assumed| {
            !argument.spread
                && matches!(
                    self.value_origin_assuming(
                        file,
                        argument.span,
                        argument.runtime_value_kind,
                        depth - 1,
                        assumed,
                    ),
                    Some(ValueOrigin::Builtin)
                )
        };
        match (binding.shape, primitive) {
            (solid_facts::ast::BindingShape::Array, Primitive::CreateSignal) => {
                // The accessor is the tuple's first slot, read in its own file.
                let accessor = binding.array_slots.first()?.as_ref()?;
                if binding_file.path != file.path
                    || file.ast.reference_declaration(callee) != Some(accessor.span)
                    || !binding_file
                        .ast
                        .functions
                        .iter()
                        .any(|function| function.body.contains(binding.declaration))
                {
                    return None;
                }
                // `createSignal(fn)` is a writable memo; its value is `fn`'s.
                if let Some(initial) = initializer.arguments.first()
                    && (matches!(
                        initial.value,
                        solid_facts::ast::ArgumentValueKind::Function
                            | solid_facts::ast::ArgumentValueKind::AsyncFunction
                    ) || !builtin(binding_file, initial, None))
                {
                    return None;
                }
                if let Some(Some(setter)) = binding.array_slots.get(1)
                    && !self.setter_writes_builtin(binding_file, setter.span, depth)
                {
                    return None;
                }
                Some(ValueOrigin::Builtin)
            }
            (solid_facts::ast::BindingShape::Identifier, Primitive::CreateMemo) => {
                // Options can supply a value no compute returns
                // (`loadingValue`), so only the bare form is followed.
                let [compute] = initializer.arguments.as_slice() else {
                    return None;
                };
                let function =
                    crate::cleanup::callback_argument_literal(binding_file, compute.span)?;
                self.returns_builtin(binding_file, function, None, depth)
                    .then_some(ValueOrigin::Builtin)
            }
            _ => None,
        }
    }

    /// Whether every write through the setter declared at `setter` is proven
    /// a built-in value: each reference to it in its file is a call's callee,
    /// and each call writes nothing, a built-in value, or an updater whose
    /// returns are built-in given a built-in previous value.
    fn setter_writes_builtin(&self, file: &FileFacts, setter: Span, depth: usize) -> bool {
        file.ast
            .reference_declarations
            .iter()
            .filter(|(_, declaration)| *declaration == setter)
            .all(|(reference, _)| {
                let Some(call) = self
                    .call_by_callee(file, *reference)
                    .filter(|call| file.ast.peel_ts_sugar_span(call.callee) == *reference)
                else {
                    return false;
                };
                let Some(written) = call.arguments.first() else {
                    return true;
                };
                if written.spread {
                    return false;
                }
                if matches!(
                    written.value,
                    solid_facts::ast::ArgumentValueKind::Function
                        | solid_facts::ast::ArgumentValueKind::AsyncFunction
                ) {
                    let Some(updater) =
                        crate::cleanup::callback_argument_literal(file, written.span)
                    else {
                        return false;
                    };
                    let previous = updater
                        .parameters
                        .first()
                        // A default replaces `undefined` with a value nothing
                        // proves, so a defaulted previous value is not assumed.
                        .filter(|parameter| {
                            parameter.shape == solid_facts::ast::BindingShape::Identifier
                                && parameter.initializer.is_none()
                        })
                        .and_then(|parameter| parameter.names.first())
                        .map(|name| name.span)
                        .filter(|name| !binding_written(file, *name));
                    return self.returns_builtin(
                        file,
                        updater,
                        previous.map(|name| (file.path.as_str(), name)),
                        depth,
                    );
                }
                matches!(
                    self.value_origin(file, written.span, written.runtime_value_kind, depth - 1),
                    Some(ValueOrigin::Builtin)
                )
            })
    }

    /// Whether every value the synchronous `function` returns is proven a
    /// built-in value, with `assumed` (a parameter's name) taken as one.
    fn returns_builtin(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
        assumed: Option<(&str, Span)>,
        depth: usize,
    ) -> bool {
        if function.r#async || function.generator {
            return false;
        }
        let proven = |returned: &solid_facts::ast::ReturnFact| {
            returned.argument.is_none_or(|argument| {
                matches!(
                    self.value_origin_assuming(
                        file,
                        argument,
                        returned.runtime_value_kind,
                        depth - 1,
                        assumed,
                    ),
                    Some(ValueOrigin::Builtin)
                )
            })
        };
        if function.expression_body {
            return function.expression_return.as_ref().is_some_and(proven);
        }
        file.ast.own_returns(function).all(proven)
    }

    pub(super) fn standard_library_declaration(
        &self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        kind: typefacts::CallKind,
    ) -> Option<&'a typefacts::ResolvedDeclaration> {
        self.resolved_callee_call(file, call.callee)
            .filter(|resolved| {
                resolved.validity == ResolvedCallValidity::Valid
                    && resolved.kind == kind
                    && resolved.targets.is_none()
            })
            .and_then(|resolved| resolved.declaration.as_ref())
            .filter(|declaration| declaration.standard_library)
    }

    /// ADR 0211: the instance method `property` of exactly `class`, declared
    /// in its own body: one plain method, not static, not an accessor.
    pub(super) fn class_method_symbol(
        &self,
        class_file: &'a FileFacts,
        class: &'a solid_facts::ast::ClassFact,
        property: &str,
    ) -> Option<&'a SymbolId> {
        use solid_facts::ast::ClassElementKind;
        if !self.class_instance_is_exact(class_file, class, property, 4) {
            return None;
        }
        let mut methods = class.elements.iter().filter(|element| {
            !element.r#static
                && element.key.and_then(|key| class_file.source_text(key)) == Some(property)
        });
        let method = methods.next()?;
        if methods.next().is_some() || method.kind != ClassElementKind::Method {
            return None;
        }
        let function = class_file
            .ast
            .functions
            .iter()
            .find(|function| Some(function.span) == method.value)?;
        self.entities.at(
            class_file.path.as_str(),
            function.method_name.as_ref()?.span,
        )
    }

    /// Whether the value a project function's symbol names now is that
    /// function: a declaration whose name nothing writes, a method, or the
    /// direct initializer of a `const`. A `let` that is reassigned, or a
    /// binding initialized by a call that merely contains the function
    /// (`const f = wrap(() => …)`), proves nothing.
    pub(super) fn function_value_is_current(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> bool {
        // A class element's or an object literal method's own function. A
        // function nested inside a method inherits its `method_name`, so that
        // field alone says nothing.
        let element_function = file.ast.classes.iter().any(|class| {
            class
                .elements
                .iter()
                .any(|element| element.value == Some(function.span))
        }) || file.ast.object_properties.iter().any(|property| {
            !property.data && file.ast.peel_ts_sugar_span(property.value) == function.span
        });
        if element_function {
            return true;
        }
        if function.kind == solid_facts::ast::FunctionKind::Declaration {
            return function
                .name
                .as_ref()
                .is_some_and(|name| !binding_written(file, name.span));
        }
        file.ast.bindings.iter().any(|binding| {
            binding.immutable
                && binding.shape == solid_facts::ast::BindingShape::Identifier
                && binding.initializer.is_some_and(|initializer| {
                    file.ast.peel_ts_sugar_span(initializer) == function.span
                })
        })
    }

    /// Whether `new C(…)` of this class evaluates to an instance whose
    /// `property` is found on its prototype chain (ADR 0209, 0211 as
    /// amended): no constructor in the chain returns another object, and no
    /// class in the chain defines `property` as an own field, a parameter
    /// property, or under a computed key. Every ancestor must be a project
    /// class this resolves; an unresolved `extends` proves nothing.
    pub(super) fn class_instance_is_exact(
        &self,
        file: &'a FileFacts,
        class: &'a solid_facts::ast::ClassFact,
        property: &str,
        depth: usize,
    ) -> bool {
        use solid_facts::ast::ClassElementKind;
        if depth == 0 {
            return false;
        }
        for element in &class.elements {
            if element.r#static {
                continue;
            }
            match element.kind {
                ClassElementKind::Constructor => {
                    let Some(constructor) = file
                        .ast
                        .functions
                        .iter()
                        .find(|function| Some(function.span) == element.value)
                    else {
                        return false;
                    };
                    let returns_value = file
                        .ast
                        .own_returns(constructor)
                        .any(|returned| returned.argument.is_some());
                    let parameter_property = file.ast.parameter_properties.iter().any(|name| {
                        constructor.span.contains(*name)
                            && file.source_text(*name) == Some(property)
                    });
                    if returns_value || parameter_property {
                        return false;
                    }
                }
                ClassElementKind::Field
                    if element.computed
                        || element.key.and_then(|key| file.source_text(key)) == Some(property) =>
                {
                    return false;
                }
                ClassElementKind::Method | ClassElementKind::Getter | ClassElementKind::Setter
                    if element.computed =>
                {
                    return false;
                }
                _ => {}
            }
        }
        let Some(heritage) = class.heritage else {
            return true;
        };
        let heritage = file.ast.peel_ts_sugar_span(heritage);
        let base = file
            .ast
            .reference_declaration(heritage)
            .and_then(|declaration| {
                file.ast
                    .classes
                    .iter()
                    .find(|candidate| {
                        candidate
                            .name
                            .as_ref()
                            .is_some_and(|name| name.span == declaration)
                    })
                    .map(|base| (file, base))
            })
            .or_else(|| {
                self.entities
                    .at(file.path.as_str(), heritage)
                    .and_then(|symbol| self.class_for_symbol(symbol))
            });
        base.is_some_and(|(base_file, base)| {
            self.class_instance_is_exact(base_file, base, property, depth - 1)
        })
    }

    pub(super) fn binding_at_reference(
        &self,
        path: &str,
        span: Span,
    ) -> Option<(&'a FileFacts, &'a solid_facts::ast::BindingFact, SymbolId)> {
        let references = self.bindings_by_reference().get(path)?;
        let key = |span: Span| (u64::from(span.start), u64::from(span.end));
        // Transparent TypeScript wrappers, however nested, preserve the value
        // of the identifier they wrap. A missed wrapped span resolves as that
        // identifier, and only when it peels to exactly one identifier.
        let resolution = references.get(&key(span)).or_else(|| {
            let file = self.file_by_path(path)?;
            let peeled = file.ast.peel_ts_sugar_span(span);
            let identifier = file
                .ast
                .identifiers
                .binary_search_by_key(&peeled, |id| id.span)
                .is_ok()
                || file.ast.reference_declaration(peeled).is_some();
            (peeled != span && identifier)
                .then(|| references.get(&key(peeled)))
                .flatten()
        })?;
        let file = &self.facts.files[resolution.file];
        Some((
            file,
            &file.ast.bindings[resolution.binding],
            resolution.symbol.clone(),
        ))
    }

    fn bindings_by_reference(&self) -> &BindingsByReference {
        self.bindings_by_reference.get_or_init(|| {
            let symbols = self.symbols_by_id.get_or_init(|| {
                self.facts
                    .typescript
                    .symbols()
                    .map(|candidate| (candidate.id(), candidate))
                    .collect()
            });
            let mut by_path = HashMap::<String, HashMap<(u64, u64), BindingResolution>>::new();
            let mut by_symbol = HashMap::<SymbolId, BindingResolution>::new();
            for (file_index, file) in self.facts.files.iter().enumerate() {
                for (binding_index, binding) in file.ast.bindings.iter().enumerate() {
                    for name in &binding.names {
                        let Some(symbol) = self.entities.at(file.path.as_str(), name.span) else {
                            continue;
                        };
                        let resolution = BindingResolution {
                            file: file_index,
                            binding: binding_index,
                            symbol: symbol.clone(),
                        };
                        by_symbol
                            .entry(symbol.clone())
                            .or_insert_with(|| resolution.clone());
                        by_path
                            .entry(file.path.to_string())
                            .or_default()
                            .entry((u64::from(name.span.start), u64::from(name.span.end)))
                            .or_insert_with(|| resolution.clone());
                        if let Some(candidate) = symbols.get(symbol.as_str()) {
                            for reference in candidate.references() {
                                by_path
                                    .entry(reference.path.to_string())
                                    .or_default()
                                    .entry((reference.start_byte, reference.end_byte))
                                    .or_insert_with(|| resolution.clone());
                            }
                        }
                    }
                }
            }
            // The symbol-reference table proves aliases even when no entity
            // was demanded at a use. Conversely, exact entity facts can exist
            // at a use omitted from that reference projection. Retain both
            // compiler proofs, matching the former direct-or-reference query.
            //
            // An entity demanded at a composite expression carries the symbol
            // of the token it begins with (`ctx.a() === "x"` names `ctx`), not
            // the expression's value. Those keys are retained because several
            // proofs withhold findings through them; a consumer needing value
            // identity must first require an exact identifier span (accessor
            // provenance's `exact_identifier`).
            for (path, entities) in &self.entities.by_path {
                for ((start, end), symbol) in entities {
                    if let Some(resolution) = by_symbol.get(symbol) {
                        by_path
                            .entry(path.clone())
                            .or_default()
                            .entry((*start, *end))
                            .or_insert_with(|| resolution.clone());
                    }
                }
            }
            by_path
        })
    }

    /// Whether the last reference in this JSX member-object span resolves to
    /// a binding initialized by the dialect's `createContext` primitive
    /// anywhere in the project.
    ///
    /// Context providers are routinely declared in one module and rendered
    /// in another. Indexing canonical TypeScript symbols keeps that ordinary
    /// import/re-export boundary from erasing the runtime contract while
    /// still rejecting objects that merely expose a property named
    /// `Provider`. Matching the reference that ends with the object span also
    /// supports `<contexts.ValueContext.Provider>` without confusing it with
    /// `<ValueContext.someObject.Provider>`.
    pub(super) fn is_context_reference(&self, path: &str, span: Span) -> bool {
        // ADR 0289: under runtime redirects (ADR 0186, ADR 0284) the compiler
        // binds an import to the declaration file beside the runtime one, so
        // it lists the reference under the declaration's symbol. The entity
        // at the span carries the redirected runtime binding, which decides.
        if !self.facts.runtime_symbol_redirects.is_empty()
            && self
                .entities
                .at(path, span)
                .is_some_and(|symbol| self.context_symbols().contains(symbol.as_str()))
        {
            return true;
        }
        self.context_symbols().iter().any(|symbol| {
            self.symbols_by_id
                .get_or_init(|| {
                    self.facts
                        .typescript
                        .symbols()
                        .map(|candidate| (candidate.id(), candidate))
                        .collect()
                })
                .get(symbol)
                .is_some_and(|candidate| {
                    candidate.references().any(|reference| {
                        reference.path.as_ref() == path
                            && reference.start_byte >= u64::from(span.start)
                            && reference.end_byte == u64::from(span.end)
                    })
                })
        })
    }

    fn context_symbols(&self) -> &HashSet<&'a str> {
        self.context_symbols.get_or_init(|| {
            self.facts
                .files
                .iter()
                .flat_map(|file| {
                    let primitives = self.primitives(file);
                    file.ast.bindings.iter().filter_map(move |binding| {
                        let initializer = binding.call_initializer?;
                        let call = file
                            .ast
                            .calls
                            .iter()
                            .position(|call| call.span == initializer)?;
                        (super::known_primitive(&primitives.calls[call])
                            == Some(solid_dialect::Primitive::CreateContext))
                        .then(|| {
                            binding.names.iter().find_map(|name| {
                                self.entities
                                    .at(file.path.as_str(), name.span)
                                    .map(|symbol| symbol.as_str())
                            })
                        })
                        .flatten()
                    })
                })
                .collect()
        })
    }

    pub(super) fn call_by_callee(
        &self,
        file: &FileFacts,
        callee: Span,
    ) -> Option<&solid_facts::ast::CallFact> {
        self.ast_indexes
            .get(file.path.as_str())
            .and_then(|index| index.call_by_callee(callee))
    }

    /// The compiler-selected signature and argument mapping for this call.
    ///
    /// Resolved-call facts are demanded at the complete callee expression,
    /// unlike member declaration identity which lives on its property span.
    pub(super) fn resolved_callee_call(
        &self,
        file: &FileFacts,
        callee: Span,
    ) -> Option<&'a ResolvedCall> {
        let original_callee = callee;
        let callee = file.ast.peel_ts_sugar_span(callee);
        let call_span = self
            .call_by_callee(file, original_callee)
            .or_else(|| self.call_by_callee(file, callee))
            .map_or(original_callee, |call| call.span);
        // A wrapped callee has its own exact call demand. Prefer that fact:
        // the peeled expression may also be another invocation's location
        // (a factory result, for example), or may have only a symbol demand.
        self.entity_at(file.path.as_str(), original_callee)
            .and_then(|entity| entity.resolved_call.as_deref())
            .or_else(|| {
                self.entity_at(file.path.as_str(), callee)
                    .and_then(|entity| entity.resolved_call.as_deref())
            })
            .or_else(|| {
                self.entity_at(file.path.as_str(), call_span)
                    .and_then(|entity| entity.resolved_call.as_deref())
            })
    }

    pub(super) fn ast_file_index(&self, path: &str) -> Option<&'a CachedAstFileIndex> {
        self.ast_indexes.get(path)
    }

    pub(super) fn function_called_at(
        &self,
        path: &str,
        callee: Span,
    ) -> Option<(&'a FileFacts, &'a solid_facts::ast::FunctionFact)> {
        let symbol = self.entities.at(path, callee)?;
        self.function_for_symbol(symbol)
    }

    fn method_symbol_ref(&self, path: &str, span: Span) -> Option<&'a str> {
        self.declaration_symbols
            .get_or_init(|| declaration_symbols(&self.facts.typescript))
            .get(&(path, u64::from(span.start), u64::from(span.end)))
            .copied()
    }

    /// A static member of an actual namespace import names its exported
    /// value, not a structurally compatible method. Resolve the property
    /// symbol itself; a selected call signature need not name that value.
    pub(super) fn namespace_member_function(
        &self,
        file: &FileFacts,
        callee: Span,
    ) -> Option<(&'a FileFacts, &'a solid_facts::ast::FunctionFact)> {
        let callee = file.ast.peel_ts_sugar_span(callee);
        if file.ast.computed_members.binary_search(&callee).is_ok() {
            return None;
        }
        let member = file
            .ast
            .members
            .iter()
            .find(|member| member.span == callee)?;
        let receiver = file.ast.peel_ts_sugar_span(member.object);
        let declaration = file.ast.reference_declaration(receiver)?;
        let namespace = file.ast.imports.iter().any(|import| {
            !import.type_only
                && import.bindings.iter().any(|binding| {
                    !binding.type_only
                        && binding.kind == solid_facts::ast::ImportKind::Namespace
                        && binding.local.span == declaration
                })
        });
        if !namespace {
            return None;
        }
        let symbol = self.entities.at(file.path.as_str(), member.property)?;
        self.function_for_symbol(symbol)
            .filter(|(target_file, function)| {
                function.method_name.is_none()
                    && self.function_value_is_current(target_file, function)
            })
    }

    pub(super) fn function_for_symbol(
        &self,
        symbol: &str,
    ) -> Option<(&'a FileFacts, &'a solid_facts::ast::FunctionFact)> {
        match self.functions_by_symbol().get(symbol)? {
            SymbolFunction::Resolved { file, function } => {
                let file = &self.facts.files[*file];
                Some((file, &file.ast.functions[*function]))
            }
            SymbolFunction::Aborted => None,
        }
    }

    pub(super) fn entity_at(&self, path: &str, span: Span) -> Option<&'a EntityFact> {
        self.entities_by_location()
            .get(&(path, u64::from(span.start), u64::from(span.end)))
            .copied()
    }

    pub(super) fn typescript_file(&self, path: &str) -> Option<&'a FileFact> {
        self.facts.typescript.file(path)
    }

    /// The symbol a callee span resolves to. Transparent wrappers are peeled
    /// because they preserve the called value; arbitrary contained symbols are
    /// never substituted, since `handlers[i]()` and `wrapper.value()` must not
    /// inherit the identity of `i` or `wrapper` when the complete callee has no
    /// semantic fact.
    pub(super) fn callee_symbol(&self, file: &FileFacts, callee: Span) -> Option<&'a str> {
        let callee = file.ast.peel_ts_sugar_span(callee);
        let member_property = self
            .ast_indexes
            .get(file.path.as_str())
            .and_then(|index| index.member_property(callee));
        let computed_member = file.ast.computed_members.contains(&callee);
        // ADR 0322: a member of a namespace import that a runtime redirect
        // binds to selected source is the redirected export: the entity at
        // the property carries it, the resolved declaration names the
        // declaration file the compiler bound instead.
        if let Some(property) = member_property
            && !computed_member
            && !self.facts.runtime_symbol_redirects.is_empty()
            && self.namespace_member_function(file, callee).is_some()
            && let Some(symbol) = self.entities.at(file.path.as_str(), property)
            && self
                .facts
                .runtime_symbol_redirects
                .values()
                .any(|target| target.as_str() == symbol.as_str())
        {
            return Some(symbol.as_str());
        }
        member_property
            .and_then(|property| {
                self.resolved_declaration_symbol(file, callee).or_else(|| {
                    if computed_member {
                        None
                    } else {
                        self.entities
                            .at(file.path.as_str(), property)
                            .map(SymbolId::as_str)
                    }
                })
            })
            .or_else(|| {
                if member_property.is_none() {
                    self.entities
                        .at(file.path.as_str(), callee)
                        .map(SymbolId::as_str)
                } else {
                    None
                }
            })
    }

    /// Exact callable identities for one call site.
    ///
    /// A normal TypeScript call fact supplies one selected declaration. A
    /// composite/union call deliberately supplies none; in that case this
    /// method only expands aliases and object values whose exact property
    /// declarations are present in the semantic table. It never turns a
    /// property spelling into a project-wide method lookup. Callers must
    /// compare the returned candidates' summaries before using more than one.
    pub(super) fn callee_symbols(&self, file: &FileFacts, callee: Span) -> Vec<SymbolId> {
        let callee = file.ast.peel_ts_sugar_span(callee);
        let member_property = self
            .ast_indexes
            .get(file.path.as_str())
            .and_then(|index| index.member_property(callee));
        // For an identifier call the entity at the complete callee is the
        // value binding (including a function parameter). A resolved
        // FunctionType declaration is not the runtime function identity and
        // must not replace it.
        if member_property.is_none()
            && let Some(symbol) = self.entities.at(file.path.as_str(), callee)
        {
            if !self.direct_value_aliases().contains(symbol) {
                return vec![symbol.clone()];
            }
            let mut visited = HashSet::new();
            let aliases = self.direct_value_symbols(file, callee, &mut visited);
            if !aliases.is_empty() {
                return aliases;
            }
            return vec![symbol.clone()];
        }
        if let Some(call) = self.resolved_callee_call(file, callee)
            && call.validity == ResolvedCallValidity::Valid
            && let Some(declaration) = call.declaration.as_ref()
            && !declaration.symbol.is_empty()
        {
            let symbol = SymbolId::from(declaration.symbol.as_ref());
            // A selected declaration is sufficient when it names an
            // analyzed implementation. A TypeScript interface/signature
            // declaration without one is only a structural type, so continue
            // to exact receiver/call-site dispatch below.
            if self.function_for_symbol(symbol.as_str()).is_some() {
                return vec![symbol];
            }
        }
        let Some(member_property) = member_property else {
            return self
                .entities
                .at(file.path.as_str(), callee)
                .cloned()
                .into_iter()
                .collect();
        };
        if file.ast.computed_members.binary_search(&callee).is_ok() {
            return Vec::new();
        }
        let Some(property_name) = file.source_text(member_property) else {
            return Vec::new();
        };
        let member = file.ast.members.iter().find(|member| member.span == callee);
        let Some(member) = member else {
            return Vec::new();
        };
        let mut symbols = Vec::new();
        let mut visited_spans = HashSet::new();
        self.member_value_symbols(
            file,
            member.object,
            property_name,
            &mut visited_spans,
            &mut symbols,
        );
        if symbols.is_empty() {
            self.structural_parameter_member_symbols(
                file,
                member.object,
                property_name,
                &mut symbols,
            );
        }
        symbols.sort_unstable();
        symbols.dedup();
        symbols
    }

    /// The root symbol of a non-computed member callee, and the access path
    /// reaching it from that root.
    ///
    /// `reader.read(value)` answers `(reader, ["read"])`, and
    /// `parsed.modifiers.includes(m)` answers `(parsed, ["modifiers",
    /// "includes"])` -- **not** `(parsed, ["includes"])`. Rooting a whole
    /// chain at its last segment names a property the receiver does not have,
    /// and a consumer matches a contract's path as a *prefix* of the observed
    /// access (`type_facts::parameter_value_source_matches`), so a wrong first
    /// segment is a demand no runtime can witness.
    ///
    /// A computed member or a callee with no member fact answers `None`, the
    /// same conservative gate [`Self::callee_symbols`] applies -- `handlers[i]()`
    /// must never be read as a property named `i`.
    ///
    /// **The chain's root must be a plain identifier.** `EntitySymbols::at`
    /// answers a symbol for any span the compiler emitted an entity at, and at
    /// a conditional, sequence, logical, or call expression that symbol is some
    /// *operand's* -- not the value the chain walks through. Trusting it
    /// attaches properties of the chain's result to a binding that never has
    /// them: `(k ? options.a : options.b).c.slice(n)` would be reported as `k`
    /// reading `["c", "slice"]`. Those are refused, and so is
    /// `options().slice(n)`, whose `slice` is a property of the call's result.
    ///
    /// Segments *inside* the chain fail closed to the longest exact prefix
    /// rather than being guessed or skipped: `props[key].values()` cannot name
    /// anything and answers an empty path, and `props.of[key].values()` answers
    /// `["of"]`. A chain longer than [`MEMBER_CALLEE_PATH_LIMIT`] is cut the
    /// same way. With the root pinned to an identifier, a shorter path is
    /// always a true prefix of the real access, and so a strictly weaker claim
    /// under both the prefix matcher and the exact one
    /// (`type_facts::parameter_value_source_matches` and its `_exact` sibling
    /// compare a witness against *this* stated path, never against the access
    /// it was cut from). An empty path claims only "read through this
    /// parameter".
    pub(super) fn member_callee_receiver(
        &self,
        file: &FileFacts,
        callee: Span,
    ) -> Option<(SymbolId, Vec<String>)> {
        let callee = file.ast.peel_ts_sugar_span(callee);
        if file.ast.computed_members.binary_search(&callee).is_ok() {
            return None;
        }
        let ast_index = self.ast_indexes.get(file.path.as_str())?;
        // The callee must itself be a named member. `notify(callback)` is a
        // plain identifier: it is a call *of* a value, not a call *through* a
        // property of one, and answering it with a zero-segment path would
        // make every ordinary call read as a member access on its callee.
        let leaf = member_at(file, callee)?;
        let leaf_property = ast_index
            .member_property(callee)
            .and_then(|property| file.source_text(property))?
            .to_owned();
        // Walk leaf -> root, then reverse: the model orders a path outwards
        // from the parameter. An unnameable segment discards everything
        // collected so far -- those sit *outside* it, and only the segments
        // still to be walked form an exact prefix of the real access.
        let mut segments = vec![leaf_property];
        let mut current = file.ast.peel_ts_sugar_span(leaf.object);
        let root = loop {
            let Some(member) = member_at(file, current) else {
                break current;
            };
            if file.ast.computed_members.binary_search(&current).is_ok() {
                segments.clear();
            } else if let Some(name) = ast_index
                .member_property(current)
                .and_then(|property| file.source_text(property))
            {
                segments.push(name.to_owned());
            } else {
                segments.clear();
            }
            let next = file.ast.peel_ts_sugar_span(member.object);
            // A member's object is a strict sub-span of the member itself, so
            // every step shrinks the span and the walk terminates. The depth
            // it terminates at bounds the *path*, not the walk: cutting the
            // walk short would lose the root and force a drop, and a dropped
            // row is the negative claim this must never make. A fact table
            // that does not shrink is malformed, and refused rather than
            // walked forever.
            if next.start < current.start || next.end >= current.end {
                return None;
            }
            current = next;
        };
        // Refuse a root that is not a plain identifier: see the doc comment.
        // Everything collected above sits on the chain's *result*, so a
        // symbol answered at a compound span would be given a path of
        // properties it does not have.
        if file
            .ast
            .identifiers
            .binary_search_by_key(&root, |identifier| identifier.span)
            .is_err()
        {
            return None;
        }
        segments.reverse();
        segments.truncate(MEMBER_CALLEE_PATH_LIMIT);
        let receiver = self.entities.at(file.path.as_str(), root)?;
        Some((receiver.clone(), segments))
    }

    /// Every exact implementation `value.property` may resolve to.
    ///
    /// This retains a finite runtime union rather than silently selecting one
    /// candidate. Consumers may certify it only after proving every candidate
    /// has equivalent behavior; an empty result is missing evidence, not
    /// safety.
    ///
    /// Records in `consulted` every function whose source the answer read
    /// (factories, callees an object is handed to), so a cached consumer can
    /// depend on them.
    pub(super) fn member_value_symbols_at_tracked(
        &self,
        file: &FileFacts,
        value: Span,
        property_name: &str,
        consulted: &mut Vec<(String, Span)>,
    ) -> Vec<SymbolId> {
        // ADR 0321: an object literal written as the argument itself is held
        // only by the callee's parameter, which the caller proved is only
        // member-read: no other code can reach it to replace a member.
        let fresh_argument = file
            .ast
            .object_get_shapes
            .iter()
            .any(|shape| shape.span == file.ast.peel_ts_sugar_span(value));
        if !fresh_argument && self.own_member_may_be_replaced(property_name) {
            return Vec::new();
        }
        let mut visited = HashSet::new();
        let mut symbols = self
            .member_value_candidates(file, value, property_name, &mut visited, true, consulted)
            .unwrap_or_default();
        symbols.sort_unstable();
        symbols.dedup();
        symbols
    }

    /// Follow a nested member path through fresh, closed data literals. The
    /// argument and every held intermediate have a single consumption channel;
    /// the receiving parameter may only call this exact path. In particular,
    /// neither a const declaration nor a structural signature proves stability.
    pub(super) fn nested_member_value_symbols_at(
        &self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        parameter: usize,
        path: &[String],
    ) -> Vec<SymbolId> {
        let resolve = || {
            if path.len() < 2
                || path.len() > 8
                || call.construct
                || call.arguments.iter().any(|argument| argument.spread)
                || self.files().iter().any(|file| {
                    file.ast
                        .module_hazards
                        .iter()
                        .any(|hazard| hazard.kind == solid_facts::ast::ModuleHazardKind::Eval)
                })
            {
                return None;
            }
            let (callee_file, callee, exact) =
                crate::execution_role::project_callee_at_parameter(file, call, parameter, self)?;
            let input = callee.parameters.get(parameter)?;
            if !exact
                || input.shape != solid_facts::ast::BindingShape::Identifier
                || input.names.len() != 1
                || callee.rest_parameter
                || binding_written(callee_file, input.names[0].span)
                || callee_file.ast.identifiers.iter().any(|id| {
                    callee.span.contains(id.span)
                        && callee_file.source_text(id.span) == Some("arguments")
                })
            {
                return None;
            }
            let symbol = self
                .entities
                .at(callee_file.path.as_str(), input.names[0].span)?;
            let mut seen = false;
            for (reference, declaration) in &callee_file.ast.reference_declarations {
                if *declaration != input.names[0].span {
                    continue;
                }
                seen = true;
                // Extracting, returning, assigning, or forwarding any prefix
                // exposes the receiver. Only an immediate call of the full
                // selected path keeps the fresh object's channel closed.
                if !callee_file.ast.calls.iter().any(|inner| {
                    !inner.construct
                        // Read summaries do not instantiate the selected
                        // method's callback parameters at this indirect edge.
                        && inner.arguments.is_empty()
                        && inner.callee.contains(*reference)
                        // member_callee_receiver intentionally returns an
                        // exact prefix at computed segments. A prefix cannot
                        // stand for this whole runtime dispatch.
                        && !callee_file.ast.computed_members.iter()
                            .any(|member| inner.callee.contains(*member))
                        && self
                            .member_callee_receiver(callee_file, inner.callee)
                            .is_some_and(|(root, selected)| &root == symbol && selected == path)
                }) {
                    return None;
                }
            }
            if !seen {
                return None;
            }
            let argument = call.arguments.get(parameter)?;
            // Resolving a nonempty path proves the supplied argument is a
            // fresh object (possibly through closed aliases), never undefined.
            // An identifier parameter's default therefore cannot replace it.
            let (owner, value) = self.nested_member_value(file, argument.span, path, 16)?;
            let function = owner
                .ast
                .functions
                .iter()
                .find(|function| function.span == value);
            let symbol = if let Some(function) = function {
                let name = function_binding_name(owner, function)?;
                self.entities.at(owner.path.as_str(), name.span)?
            } else {
                if !owner.ast.identifiers.iter().any(|id| id.span == value) {
                    return None;
                }
                self.entities.at(owner.path.as_str(), value).or_else(|| {
                    let declaration = owner.ast.reference_declaration(value)?;
                    self.entities.at(owner.path.as_str(), declaration)
                })?
            };
            let (function_file, function) = self.function_for_symbol(symbol)?;
            (self.function_value_is_current(function_file, function)
                && crate::object_members::receiver_inert(function_file, function))
            .then(|| vec![symbol.clone()])
        };
        resolve().unwrap_or_default()
    }

    fn nested_member_value(
        &self,
        file: &FileFacts,
        value: Span,
        path: &[String],
        depth: usize,
    ) -> Option<(&'a FileFacts, Span)> {
        if depth == 0 {
            return None;
        }
        let file = self.file_by_path(file.path.as_str())?;
        let value = file.ast.peel_ts_sugar_span(value);
        // Resolve bindings before literals, but never chase a compound span's
        // root entity. A binding must be a const or runtime lexical let with
        // no writes, initialized before its sole reference, and consumed in
        // its own frame (no captured alias). Ambient/namespace lets supply
        // no runtime initialization proof.
        if !path.is_empty() && file.ast.identifiers.iter().any(|id| id.span == value) {
            let declaration = file.ast.reference_declaration(value)?;
            let owner = file;
            let binding = owner
                .ast
                .bindings
                .iter()
                .find(|binding| binding.names.iter().any(|name| name.span == declaration))?;
            let declaration = binding.names.first()?.span;
            let initializer = binding.initializer?;
            let frame = |file: &FileFacts, span| {
                file.ast
                    .functions
                    .iter()
                    .filter(|function| function.body.contains(span))
                    .min_by_key(|function| function.span.end - function.span.start)
                    .map(|function| function.span)
            };
            if !(binding.immutable || binding.runtime_let)
                || binding.shape != solid_facts::ast::BindingShape::Identifier
                || binding.names.len() != 1
                || binding_written(owner, declaration)
                || initializer.end > value.start
                || frame(owner, declaration).is_none()
                || frame(owner, declaration) != frame(file, value)
                || owner
                    .ast
                    .reference_declarations
                    .iter()
                    .filter(|(_, bound)| *bound == declaration)
                    .any(|(reference, _)| *reference != value)
            {
                return None;
            }
            return self.nested_member_value(owner, initializer, path, depth - 1);
        }
        let Some((property, rest)) = path.split_first() else {
            return Some((file, value));
        };
        let properties = crate::interproc::accessor_provenance::closed_properties(file, value)?;
        let selected = properties.into_iter().find(|candidate| {
            file.source_text(candidate.key)
                .is_some_and(|key| key.trim_matches(['\'', '"']) == property)
        })?;
        self.nested_member_value(file, selected.value, rest, depth - 1)
    }

    /// Resolve `parameter.member()` through the exact project call sites of
    /// the containing function. This is the structural-interface seam: the
    /// member declaration itself may be a composite TypeScript signature,
    /// while each analyzed call site supplies an exact runtime value. Every
    /// call site must resolve to a concrete property value; one missing or
    /// ambiguous site makes the whole dispatch uncertifiable.
    fn structural_parameter_member_symbols(
        &self,
        file: &FileFacts,
        object: Span,
        property_name: &str,
        symbols: &mut Vec<SymbolId>,
    ) {
        let object = file.ast.peel_ts_sugar_span(object);
        let Some(receiver_symbol) = self.entities.at(file.path.as_str(), object) else {
            return;
        };
        let Some(function) = file
            .ast
            .functions_body_containing(object)
            .min_by_key(|function| function.body.end - function.body.start)
        else {
            return;
        };
        let Some(parameter_index) = function.parameters.iter().position(|parameter| {
            parameter.names.iter().any(|name| {
                self.entities
                    .at(file.path.as_str(), name.span)
                    .is_some_and(|symbol| symbol == receiver_symbol)
            })
        }) else {
            return;
        };
        let Some(function_name) = function_binding_name(file, function) else {
            return;
        };
        if self
            .entities
            .at(file.path.as_str(), function_name.span)
            .is_none()
        {
            return;
        }
        let call_sites = self.function_call_sites(file.path.as_str(), function.span);
        if call_sites.is_empty() {
            return;
        }
        let mut unresolved_site = false;
        for (caller_file, callee) in call_sites {
            let Some(call) = self.call_by_callee(caller_file, callee) else {
                unresolved_site = true;
                continue;
            };
            let Some(argument) = call.arguments.get(parameter_index) else {
                unresolved_site = true;
                continue;
            };
            let mut site_symbols = Vec::new();
            let mut visited = HashSet::new();
            self.member_value_symbols(
                caller_file,
                argument.span,
                property_name,
                &mut visited,
                &mut site_symbols,
            );
            site_symbols.sort_unstable();
            site_symbols.dedup();
            if site_symbols.is_empty() {
                unresolved_site = true;
            } else {
                symbols.extend(site_symbols);
            }
        }
        if unresolved_site {
            symbols.clear();
        }
    }

    fn member_value_symbols(
        &self,
        file: &FileFacts,
        object: Span,
        property_name: &str,
        visited_spans: &mut HashSet<Span>,
        symbols: &mut Vec<SymbolId>,
    ) {
        self.member_value_symbols_in(file, object, property_name, visited_spans, symbols, false);
    }

    /// [`Self::member_value_symbols`]; `factories` also follows the result of
    /// a project factory call ([`Self::factory_member_value_candidates`]).
    /// Only the per-call-site parameter member resolution asks for that (ADR
    /// 0314).
    ///
    /// ADR 0320: the candidates are trusted only when every part of the value
    /// resolved. An alternative that does not resolve -- one arm of a
    /// conditional, a binding without an inspectable initializer, a spread
    /// that may supply or override the member, a value the program may
    /// rewrite -- withholds the whole value, and nothing is added. A resolved
    /// union keeps every alternative's candidates.
    fn member_value_symbols_in(
        &self,
        file: &FileFacts,
        object: Span,
        property_name: &str,
        _visited_spans: &mut HashSet<Span>,
        symbols: &mut Vec<SymbolId>,
        factories: bool,
    ) {
        if self.own_member_may_be_replaced(property_name) {
            return;
        }
        let mut visited = HashSet::new();
        let mut consulted = Vec::new();
        if let Some(found) = self.member_value_candidates(
            file,
            object,
            property_name,
            &mut visited,
            factories,
            &mut consulted,
        ) && consulted
            .iter()
            .all(|(path, _)| path.as_str() == file.path.as_str())
        {
            // An untracked consumer's cached answer depends on this file's
            // source only, so only an answer read from it is kept.
            symbols.extend(found);
        }
    }

    /// Whether some code may replace an own member `name` of an object it did
    /// not create: an assignment of that name or through an unknown computed
    /// key, or a reflective definition of it. Code copying unknown keys into
    /// an object (`Object.assign`) cannot reach one the resolution proves
    /// unshared ([`Self::binding_stays_closed`], a fresh literal or factory
    /// result whose methods never use their receiver).
    fn own_member_may_be_replaced(&self, name: &str) -> bool {
        let assigned = self.assigned_members();
        assigned.global
            || assigned.names.contains(name)
            || self.property_may_be_defined_reflectively_as(name)
    }

    /// ADR 0320: whether any code may copy keys into an existing object: a
    /// member named `assign`, `setPrototypeOf`, `defineProperty` or
    /// `defineProperties` that is not the callee of a call whose first
    /// argument is a fresh literal (read, aliased, or called on anything
    /// else); any `Reflect` other than the object of a member it does not
    /// name `set`, `defineProperty` or `setPrototypeOf` directly on a fresh
    /// literal; or a destructuring that takes one of those names.
    fn objects_may_be_assigned_into(&self) -> bool {
        const COPIERS: [&str; 4] = [
            "assign",
            "setPrototypeOf",
            "defineProperty",
            "defineProperties",
        ];
        *self.objects_assigned_into.get_or_init(|| {
            // An assignment through a key nothing names (`t[key] = v`) may
            // write any member of whatever `t` is, unless `t` is proven a
            // built-in value (`xs[i] = v` on an array), which no project
            // instance is.
            let unknown_key_write = self.files().iter().any(|file| {
                file.ast.assignments.iter().any(|assignment| {
                    let target = file.ast.peel_ts_sugar_span(assignment.target);
                    file.ast.computed_members.binary_search(&target).is_ok()
                        && file
                            .ast
                            .literal_computed_members
                            .iter()
                            .all(|literal| literal.span != target)
                        && file
                            .ast
                            .members
                            .iter()
                            .find(|member| member.span == target)
                            .is_some_and(|member| {
                                constant_string_key(file, member.property).is_none()
                                    && !matches!(
                                        self.value_origin_assuming(
                                            file,
                                            member.object,
                                            solid_facts::ast::RuntimeValueKind::Unknown,
                                            6,
                                            None,
                                        ),
                                        Some(ValueOrigin::Builtin)
                                    )
                            })
                })
            });
            unknown_key_write
                || self.files().iter().any(|file| {
                    let fresh_first = |member: Span| {
                        file.ast.calls.iter().any(|call| {
                            file.ast.peel_ts_sugar_span(call.callee) == member
                                && call.arguments.first().is_some_and(|argument| {
                                    let target = file.ast.peel_ts_sugar_span(argument.span);
                                    !argument.spread
                                        && (file
                                            .ast
                                            .object_get_shapes
                                            .iter()
                                            .any(|shape| shape.span == target)
                                            || file
                                                .ast
                                                .array_literals
                                                .binary_search(&target)
                                                .is_ok()
                                            || file
                                                .ast
                                                .functions
                                                .iter()
                                                .any(|function| function.span == target))
                                })
                        })
                    };
                    let unquoted = |span: Span| {
                        file.source_text(span)
                            .map(|text| text.trim_matches(['\'', '"', '`']).to_owned())
                    };
                    file.ast.members.iter().any(|member| {
                        let Some(name) = unquoted(member.property) else {
                            return false;
                        };
                        let reflect = file.source_text(file.ast.peel_ts_sugar_span(member.object))
                            == Some("Reflect");
                        (COPIERS.contains(&name.as_str()) || (reflect && name == "set"))
                            && !fresh_first(member.span)
                    }) || file.ast.identifiers.iter().any(|identifier| {
                        file.source_text(identifier.span) == Some("Reflect")
                            && !file.ast.members.iter().any(|member| {
                                file.ast.peel_ts_sugar_span(member.object) == identifier.span
                            })
                    }) || file.ast.bindings.iter().any(|binding| {
                        binding.object_slots.iter().any(|slot| {
                            COPIERS.contains(&slot.property.as_str()) || slot.property == "set"
                        })
                    })
                })
        })
    }

    /// The exact implementations `object.property` resolves to, or `None`
    /// when some part of the value does not resolve.
    fn member_value_candidates(
        &self,
        file: &FileFacts,
        object: Span,
        property_name: &str,
        visited: &mut HashSet<(String, Span)>,
        factories: bool,
        consulted: &mut Vec<(String, Span)>,
    ) -> Option<Vec<SymbolId>> {
        let object = file.ast.peel_ts_sugar_span(object);
        if !visited.insert((file.path.to_string(), object)) {
            return None;
        }
        if let Some(conditional) = file
            .ast
            .conditional_expressions
            .iter()
            .find(|conditional| conditional.span == object)
        {
            let mut both = self.member_value_candidates(
                file,
                conditional.consequent,
                property_name,
                visited,
                factories,
                consulted,
            )?;
            both.extend(self.member_value_candidates(
                file,
                conditional.alternate,
                property_name,
                visited,
                factories,
                consulted,
            )?);
            return Some(both);
        }

        let direct_symbol = self.entities.at(file.path.as_str(), object);
        let identifier_reference = direct_symbol.is_none()
            && file.ast.identifiers.iter().any(|identifier| {
                identifier.span == object
                    && identifier.role == solid_facts::ast::IdentifierRole::Reference
            });
        let binding = direct_symbol
            .and_then(|symbol| {
                self.binding_for_symbol(symbol)
                    .map(|(binding_file, binding)| (binding_file, binding, symbol.clone()))
            })
            .or_else(|| {
                identifier_reference
                    .then(|| self.binding_at_reference(file.path.as_str(), object))?
            });
        if let Some((binding_file, binding, symbol)) = binding {
            // ADR 0314: only a `const` naming its whole initializer holds that
            // value. A `let` may hold another by now, and a destructured name
            // holds a member of the initializer, not the initializer. A
            // parameter's default holds only when no argument is passed.
            let [name] = binding.names.as_slice() else {
                return None;
            };
            if !binding.immutable
                || binding.shape != solid_facts::ast::BindingShape::Identifier
                || binding_file.ast.functions.iter().any(|function| {
                    function
                        .parameters
                        .iter()
                        .any(|parameter| parameter.names.iter().any(|n| n.span == name.span))
                })
                || !self.binding_stays_closed(
                    binding_file,
                    name.span,
                    &symbol,
                    file,
                    object,
                    consulted,
                )
            {
                return None;
            }
            let initializer = binding.initializer?;
            return self.member_value_candidates(
                binding_file,
                initializer,
                property_name,
                visited,
                factories,
                consulted,
            );
        }

        // ADR 0314: the object is exactly an object literal, and only its own
        // properties and spreads count: not a property of a literal nested in
        // it, nor of a literal anywhere inside a call or function that is not
        // one (`createFoo({ run })` holds no `run` of its result).
        let Some(literal) = file
            .ast
            .object_get_shapes
            .iter()
            .find(|shape| shape.span == object)
        else {
            return if factories {
                self.factory_member_value_candidates(
                    file,
                    object,
                    property_name,
                    visited,
                    consulted,
                )
            } else {
                None
            };
        };
        let direct = |span: Span| {
            literal.span.contains(span)
                && !file.ast.object_get_shapes.iter().any(|inner| {
                    inner.span != literal.span
                        && literal.span.contains(inner.span)
                        && inner.span.contains(span)
                })
        };
        // A function in the literal that uses its receiver may hand the
        // object to code that replaces the member (ADR 0320).
        if file.ast.functions.iter().any(|function| {
            direct(function.span)
                && function.kind != solid_facts::ast::FunctionKind::Arrow
                && file
                    .source_text(function.span)
                    .is_none_or(crate::execution_role::mentions_receiver)
        }) {
            return None;
        }
        // A getter's value is whatever it returns when read, not itself; a
        // key without a static name (computed, or a `__proto__:` prototype
        // setter) may name this property or supply it.
        if literal
            .properties
            .iter()
            .any(|property| property.key.as_str() == property_name && property.getter.is_some())
            || file.ast.object_properties.iter().any(|property| {
                direct(property.span)
                    && (property.computed
                        || crate::execution_role::property_static_key(file, property).is_none())
            })
        {
            return None;
        }
        let spreads = file
            .ast
            .spreads
            .iter()
            .filter(|spread| direct(spread.span))
            .collect::<Vec<_>>();
        let Some(properties) = file
            .ast
            .object_properties
            .iter()
            .filter(|property| {
                direct(property.span)
                    && crate::execution_role::property_static_key(file, property).as_deref()
                        == Some(property_name)
            })
            .max_by_key(|property| property.span.start)
        else {
            // No own property: the member is whatever the one spread copies.
            let [spread] = spreads.as_slice() else {
                return None;
            };
            return self.member_value_candidates(
                file,
                spread.argument,
                property_name,
                visited,
                factories,
                consulted,
            );
        };
        // A spread after the property may replace it.
        if spreads
            .iter()
            .any(|spread| spread.span.start > properties.span.start)
        {
            return None;
        }

        // ADR 0321: a function literal that is exactly the property's value
        // is its own summary node, identified by its span.
        if let Some(function) = file.ast.functions.iter().find(|function| {
            function.span == file.ast.peel_ts_sugar_span(properties.value)
                && function.method_name.is_none()
        }) && let Some(symbol) =
            crate::interproc::literal_property_function_symbol(file, function)
        {
            return Some(vec![symbol]);
        }
        // The property key itself is a semantic identity once demanded by
        // the backend. This works for class/object methods and for function
        // properties copied through an exact object spread.
        let mut symbols = Vec::new();
        if let Some(symbol) = self.entities.at(file.path.as_str(), properties.key) {
            symbols.push(symbol.clone());
        }
        for function in file.ast.functions.iter().filter(|function| {
            properties.value.contains(function.span)
                && function.method_name.as_ref().is_some_and(|name| {
                    name.span == properties.key
                        && self.entities.at(file.path.as_str(), name.span).is_some()
                })
        }) {
            if let Some(name) = function.method_name.as_ref()
                && let Some(symbol) = self.entities.at(file.path.as_str(), name.span)
            {
                symbols.push(symbol.clone());
            }
        }
        if symbols.is_empty() {
            return self.member_value_candidates(
                file,
                properties.value,
                property_name,
                visited,
                factories,
                consulted,
            );
        }
        Some(symbols)
    }

    /// ADR 0320: whether the object a `const` binding holds stays the one its
    /// initializer created, as far as `object` (the reference being resolved,
    /// in `file`) can tell: every other reference anywhere is the object of a
    /// non-computed member access, or an argument of an exact project callee
    /// that only reads members of it, so nothing hands it to code that could
    /// replace a member. An exported binding of an open program, or a
    /// script's global, may be referenced where this build cannot see.
    fn binding_stays_closed(
        &self,
        binding_file: &FileFacts,
        name: Span,
        symbol: &SymbolId,
        file: &FileFacts,
        object: Span,
        consulted: &mut Vec<(String, Span)>,
    ) -> bool {
        let module_level = binding_file
            .ast
            .functions
            .iter()
            .all(|function| !function.body.contains(name));
        // An exported binding is referenced from other files, whose edits a
        // cached answer here would not see; a script's binding is global.
        if module_level
            && (!crate::execution_role::file_is_module(binding_file)
                || crate::execution_role::flow_binding_exported(binding_file, name))
        {
            return false;
        }
        let Some(references) = self.symbol_references_if_present(symbol.as_str()) else {
            return false;
        };
        references.iter().all(|reference| {
            let path = reference.path.as_ref();
            let span = Span::new(
                u32::try_from(reference.start_byte).unwrap_or(u32::MAX),
                u32::try_from(reference.end_byte).unwrap_or(u32::MAX),
            );
            if (path == binding_file.path.as_str() && span == name)
                || (path == file.path.as_str() && span == object)
            {
                return true;
            }
            let Some(reference_file) = self.file_by_path(path) else {
                return false;
            };
            // Handed as an argument to an exact project callee that only
            // reads members of it.
            if let Some((call, index)) =
                reference_file
                    .ast
                    .arguments_containing(span)
                    .find(|(call, index)| {
                        reference_file
                            .ast
                            .peel_ts_sugar_span(call.arguments[*index].span)
                            == span
                    })
                && !call.arguments[index].spread
                && self.parameter_only_member_accessed_consulting(
                    reference_file,
                    call,
                    index,
                    consulted,
                )
            {
                return true;
            }
            reference_file.ast.members.iter().any(|member| {
                reference_file.ast.peel_ts_sugar_span(member.object) == span
                    && reference_file
                        .ast
                        .computed_members
                        .binary_search(&member.span)
                        .is_err()
                    && !reference_file
                        .ast
                        .assignments
                        .iter()
                        .any(|assignment| assignment.target.contains(member.span))
            })
        })
    }

    /// ADR 0320: whether the callee of `call` uses its parameter `index` only
    /// as the object of non-computed member accesses it does not assign: it
    /// hands the caller's object to nothing that could replace a member
    /// before the member runs.
    ///
    /// Records every callee whose source the answer read.
    pub(super) fn parameter_only_member_accessed_consulting(
        &self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        index: usize,
        consulted: &mut Vec<(String, Span)>,
    ) -> bool {
        self.parameter_only_member_accessed_within(file, call, index, 4, consulted)
    }

    /// [`Self::parameter_only_member_accessed_consulting`], where the callee may also
    /// hand the parameter, whole, to another exact callee of which the same
    /// holds, `depth` calls deep.
    fn parameter_only_member_accessed_within(
        &self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        index: usize,
        depth: usize,
        consulted: &mut Vec<(String, Span)>,
    ) -> bool {
        if depth == 0 {
            return false;
        }
        let Some((callee_file, callee, exact)) =
            crate::execution_role::project_callee_at_parameter(file, call, index, self)
        else {
            return false;
        };
        consulted.push((callee_file.path.to_string(), callee.span));
        let Some(parameter) = callee.parameters.get(index) else {
            return false;
        };
        let [name] = parameter.names.as_slice() else {
            return false;
        };
        exact
            && parameter.shape == solid_facts::ast::BindingShape::Identifier
            && !(callee.rest_parameter && index + 1 == callee.parameters.len())
            && !binding_written(callee_file, name.span)
            && !callee_file.ast.identifiers.iter().any(|identifier| {
                callee.span.contains(identifier.span)
                    && matches!(
                        callee_file.source_text(identifier.span),
                        Some("arguments" | "eval")
                    )
            })
            && callee_file
                .ast
                .reference_declarations
                .iter()
                .filter(|(reference, declaration)| {
                    *declaration == name.span && *reference != name.span
                })
                .all(|(reference, _)| {
                    let forwarded = callee_file
                        .ast
                        .arguments_containing(*reference)
                        .find(|(inner, position)| {
                            callee_file
                                .ast
                                .peel_ts_sugar_span(inner.arguments[*position].span)
                                == *reference
                        })
                        .is_some_and(|(inner, position)| {
                            !inner.arguments[position].spread
                                && self.parameter_only_member_accessed_within(
                                    callee_file,
                                    inner,
                                    position,
                                    depth - 1,
                                    consulted,
                                )
                        });
                    forwarded
                        || callee_file.ast.members.iter().any(|member| {
                            callee_file.ast.peel_ts_sugar_span(member.object) == *reference
                                && callee_file
                                    .ast
                                    .computed_members
                                    .binary_search(&member.span)
                                    .is_err()
                                && !callee_file
                                    .ast
                                    .assignments
                                    .iter()
                                    .any(|assignment| assignment.target.contains(member.span))
                        })
                })
    }

    /// ADR 0314: the implementations `factory(…).property` may resolve to.
    ///
    /// The callee is one exact, current, synchronous project function called
    /// plainly (a method call is selected by its receiver), and every value it
    /// returns is an object literal written in the return itself -- a fresh
    /// object nothing else holds when the call returns. Each such literal must
    /// resolve the property; one that does not leaves the union incomplete,
    /// so nothing is resolved.
    fn factory_member_value_candidates(
        &self,
        file: &FileFacts,
        object: Span,
        property_name: &str,
        visited: &mut HashSet<(String, Span)>,
        consulted: &mut Vec<(String, Span)>,
    ) -> Option<Vec<SymbolId>> {
        let call = file.ast.call_at(object)?;
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        if call.construct
            || self.is_member_span(file, callee)
            || file.ast.computed_members.binary_search(&callee).is_ok()
        {
            return None;
        }
        let (function_file, function) = self
            .callee_symbol(file, call.callee)
            .and_then(|symbol| self.function_for_symbol(symbol))
            .filter(|(function_file, function)| {
                self.function_value_is_current(function_file, function)
            })?;
        if function.r#async || function.generator {
            return None;
        }
        consulted.push((function_file.path.to_string(), function.span));
        let literal = |span: Span| {
            function_file
                .ast
                .object_get_shapes
                .iter()
                .any(|shape| shape.span == span)
        };
        let body = function_file.ast.peel_ts_sugar_span(function.body);
        let returned = if literal(body) {
            vec![body]
        } else {
            let mut returned = Vec::new();
            for value in function_file.ast.own_returns(function) {
                returned.push(function_file.ast.peel_ts_sugar_span(value.argument?));
            }
            returned
        };
        if returned.is_empty() {
            return None;
        }
        let mut found = Vec::new();
        for value in returned {
            if !literal(value) {
                return None;
            }
            found.extend(self.member_value_candidates(
                function_file,
                value,
                property_name,
                visited,
                true,
                consulted,
            )?);
        }
        Some(found)
    }

    fn binding_for_symbol(
        &self,
        symbol: &str,
    ) -> Option<(&'a FileFacts, &'a solid_facts::ast::BindingFact)> {
        let resolution = self.bindings_by_symbol().get(symbol)?;
        let file = &self.facts.files[resolution.file];
        Some((file, &file.ast.bindings[resolution.binding]))
    }

    /// Bindings whose initializer can replace the binding's callable identity.
    ///
    /// Most call sites name an import, a function, a signal tuple slot, or the
    /// direct result of another call. Their entity symbol is already the exact
    /// runtime identity. Only identifier aliases, object destructures, and
    /// conditional initializers need the recursive value proof.
    fn direct_value_aliases(&self) -> &HashSet<SymbolId> {
        self.direct_value_aliases.get_or_init(|| {
            let mut aliases = HashSet::new();
            for file in &self.facts.files {
                for binding in &file.ast.bindings {
                    let conditional_initializer = binding.initializer.is_some_and(|initializer| {
                        let initializer = file.ast.peel_ts_sugar_span(initializer);
                        file.ast
                            .conditional_expressions
                            .iter()
                            .any(|conditional| conditional.span == initializer)
                    });
                    if binding.initializer_identifier.is_none()
                        && binding.shape != solid_facts::ast::BindingShape::Object
                        && !conditional_initializer
                    {
                        continue;
                    }
                    for name in &binding.names {
                        if let Some(symbol) = self.entities.at(file.path.as_str(), name.span) {
                            aliases.insert(symbol.clone());
                        }
                    }
                }
            }
            aliases
        })
    }

    fn bindings_by_symbol(&self) -> &HashMap<SymbolId, BindingResolution> {
        self.bindings_by_symbol.get_or_init(|| {
            let mut bindings = HashMap::new();
            for (file_index, file) in self.facts.files.iter().enumerate() {
                for (binding_index, binding) in file.ast.bindings.iter().enumerate() {
                    for name in &binding.names {
                        let Some(symbol) = self.entities.at(file.path.as_str(), name.span) else {
                            continue;
                        };
                        bindings.entry(symbol.clone()).or_insert(BindingResolution {
                            file: file_index,
                            binding: binding_index,
                            symbol: symbol.clone(),
                        });
                    }
                }
            }
            bindings
        })
    }

    fn direct_value_symbols(
        &self,
        file: &FileFacts,
        value: Span,
        visited: &mut HashSet<Span>,
    ) -> Vec<SymbolId> {
        let value = file.ast.peel_ts_sugar_span(value);
        if !visited.insert(value) {
            return Vec::new();
        }
        if let Some(conditional) = file
            .ast
            .conditional_expressions
            .iter()
            .find(|conditional| conditional.span == value)
        {
            let mut symbols = self.direct_value_symbols(file, conditional.consequent, visited);
            symbols.extend(self.direct_value_symbols(file, conditional.alternate, visited));
            symbols.sort_unstable();
            symbols.dedup();
            return symbols;
        }
        let direct_symbol = self.entities.at(file.path.as_str(), value);
        let identifier_reference = direct_symbol.is_none()
            && file.ast.identifiers.iter().any(|identifier| {
                identifier.span == value
                    && identifier.role == solid_facts::ast::IdentifierRole::Reference
            });
        let Some(symbol) = direct_symbol.cloned().or_else(|| {
            identifier_reference.then(|| {
                self.binding_at_reference(file.path.as_str(), value)
                    .map(|(_, _, symbol)| symbol)
            })?
        }) else {
            return Vec::new();
        };
        let Some((binding_file, binding)) = direct_symbol
            .and_then(|symbol| self.binding_for_symbol(symbol))
            .or_else(|| {
                identifier_reference.then(|| {
                    self.binding_at_reference(file.path.as_str(), value)
                        .map(|(binding_file, binding, _)| (binding_file, binding))
                })?
            })
        else {
            return vec![symbol.clone()];
        };
        if binding.initializer_function {
            return vec![symbol.clone()];
        }
        // A destructuring pattern is not an alias of its initializer. `const
        // { href } = props` binds `props.href`; `const [first] = pair` binds
        // `pair[0]`. Carrying the initializer's identity across the pattern
        // made every destructured local *be* the whole object, which is how
        // `@solidjs/router`'s `Navigate` published a callback claim saying the
        // props object itself is invoked -- a claim no adapter can verify,
        // because `href({ navigate })` invokes a member. Only an identifier
        // binding may inherit its initializer's callable identity; the object
        // slot below resolves a destructured property on its own evidence, and
        // a slot with no inspectable value stays this binding's own symbol so
        // the claim is never made.
        let inherits_initializer_identity =
            binding.shape == solid_facts::ast::BindingShape::Identifier;
        if inherits_initializer_identity && let Some(initializer) = &binding.initializer_identifier
        {
            let aliases = self.direct_value_symbols(binding_file, initializer.span, visited);
            if !aliases.is_empty() {
                return aliases;
            }
        }
        if binding.shape == solid_facts::ast::BindingShape::Object
            && let Some(initializer) = binding.initializer
            && let Some(slot) = binding.object_slots.iter().find(|slot| {
                self.entities
                    .at(binding_file.path.as_str(), slot.local.span)
                    .is_some_and(|candidate| candidate == &symbol)
            })
        {
            let mut aliases = Vec::new();
            let mut visited_objects = HashSet::new();
            self.member_value_symbols(
                binding_file,
                initializer,
                slot.property.as_str(),
                &mut visited_objects,
                &mut aliases,
            );
            aliases.sort_unstable();
            aliases.dedup();
            if !aliases.is_empty() {
                return aliases;
            }
        }
        if inherits_initializer_identity && let Some(initializer) = binding.initializer {
            let aliases = self.direct_value_symbols(binding_file, initializer, visited);
            if aliases.len() > 1 {
                return aliases;
            }
        }
        vec![symbol.clone()]
    }

    fn resolved_declaration_symbol(&self, file: &FileFacts, callee: Span) -> Option<&'a str> {
        self.resolved_callee_call(file, callee)
            .and_then(|call| call.declaration.as_ref())
            .map(|declaration| declaration.symbol.as_ref())
            .filter(|symbol| !symbol.is_empty())
    }

    /// The type descriptor at `span`, falling back to the smallest typed entity
    /// contained in it for legacy consumers that deliberately query a region
    /// rather than one complete expression. Exact facts win: a descriptor on
    /// the demanded expression can never be replaced by an inner callee or
    /// member just because that entity happens to be smaller.
    pub(super) fn smallest_contained_descriptor(
        &self,
        path: &str,
        span: Span,
    ) -> Option<&'a TypeDescriptor> {
        self.entity_at(path, span)
            .and_then(|entity| entity.type_descriptor.as_deref())
            .or_else(|| {
                self.smallest_contained(path, span, |entity| entity.type_descriptor.is_some())
                    .and_then(|entity| entity.type_descriptor.as_deref())
            })
            .or_else(|| {
                let symbol = self.entities.at(path, span)?;
                self.descriptors_by_symbol
                    .get_or_init(|| {
                        self.facts
                            .typescript
                            .entities()
                            .filter_map(|entity| {
                                (!entity.symbol.is_empty())
                                    .then_some(entity.symbol.as_ref())
                                    .zip(entity.type_descriptor.as_deref())
                            })
                            .collect()
                    })
                    .get(symbol.as_str())
                    .copied()
            })
    }

    /// Whether a function is proven to be a Solid component by a JSX call
    /// site or by an exact compiler-resolved Solid component type alias.
    pub(super) fn function_is_component(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> bool {
        self.component_functions()
            .contains(&(file.path.as_str(), function.span))
    }

    /// Three-outcome component identity. Exact JSX uses and Solid component
    /// types are proven; a dialect naming convention only preserves the
    /// ambiguity and must never become proof by itself.
    pub(super) fn function_component_status(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> ComponentStatus {
        if self.function_is_component(file, function) {
            return ComponentStatus::Proven;
        }
        let possible = crate::owners::component_binding_name(file, function).is_some_and(|name| {
            self.dialect
                .component_name_may_be_component(file.source_text(name.span).unwrap_or_default())
        });
        if possible {
            ComponentStatus::Uncertain
        } else {
            ComponentStatus::No
        }
    }

    pub(super) fn function_may_be_component(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> bool {
        self.function_component_status(file, function) != ComponentStatus::No
    }

    fn component_functions(&self) -> &HashSet<(&'a str, Span)> {
        self.component_functions.get_or_init(|| {
            self.facts
                .files
                .iter()
                .flat_map(|file| {
                    file.ast.functions.iter().filter_map(move |function| {
                        self.compute_function_is_component(file, function)
                            .then_some((file.path.as_str(), function.span))
                    })
                })
                .collect()
        })
    }

    fn compute_function_is_component(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> bool {
        if self
            .jsx_call_site_loading(file.path.as_str(), function.span)
            .any
        {
            return true;
        }
        let binding_name = crate::owners::component_binding_name(file, function);
        let directly_contains_jsx = |span: Span| {
            file.ast.jsx_within(span).any(|element| {
                crate::owners::containing_ast_function(&file.ast, element.span)
                    .is_some_and(|owner| owner.span == function.span)
            }) || file.ast.jsx_fragments.iter().any(|fragment| {
                span.contains(*fragment)
                    && crate::owners::containing_ast_function(&file.ast, *fragment)
                        .is_some_and(|owner| owner.span == function.span)
            })
        };
        let directly_returns_jsx = function
            .expression_return
            .as_ref()
            .and_then(|returned| returned.argument)
            .is_some_and(directly_contains_jsx)
            || file.ast.returns_within(function.body).any(|returned| {
                returned.argument.is_some_and(directly_contains_jsx)
                    && crate::owners::containing_ast_function(&file.ast, returned.span)
                        .is_some_and(|owner| owner.span == function.span)
            });
        // Every remaining runtime proof below only decides whether a direct
        // JSX return is used as a component value or as an ordinary callback.
        // Without such a return, only an exact compiler-resolved Component
        // type can still establish component identity. Avoid building the
        // project-wide reference and call-site indexes for ordinary helpers.
        if !directly_returns_jsx {
            if !self.project_has_component_type() {
                return false;
            }
            let Some(name) = binding_name else {
                return false;
            };
            let Some(descriptor) =
                self.smallest_contained_descriptor(file.path.as_str(), name.span)
            else {
                return false;
            };
            return descriptor.alias_declarations.iter().any(|declaration| {
                self.dialect
                    .type_role(descriptor.origin_module.as_ref(), declaration.name.as_ref())
                    == Some(solid_dialect::TypeRole::Component)
            });
        }
        // Component identity is about the function value itself, not anything
        // lexically nested below a callback expression. The largest function
        // inside a callback/argument is the direct value; nested declarations
        // remain independently classifiable.
        let direct_function_value = |candidate: &FileFacts, container: Span| {
            candidate
                .ast
                .functions_within(container)
                .max_by_key(|candidate| candidate.span.end - candidate.span.start)
                .is_some_and(|candidate| candidate.span == function.span)
        };
        let used_as_callback_value = |candidate: &FileFacts| {
            candidate
                .compiler
                .callback_roles
                .iter()
                .any(|role| direct_function_value(candidate, role.span))
                || candidate
                    .ast
                    .arguments_containing(function.span)
                    .any(|(call, index)| {
                        direct_function_value(candidate, call.arguments[index].span)
                            && self
                                .primitive_at_call(candidate, call.span)
                                .map(|primitive| {
                                    self.dialect.callback_semantics_at(
                                        primitive,
                                        index,
                                        call.arguments.len(),
                                    )
                                })
                                .is_some_and(|semantics| {
                                    semantics.execution.is_some() || semantics.stores_as_value
                                })
                    })
        };
        let used_as_any_call_argument = file
            .ast
            .arguments_containing(function.span)
            .any(|(call, index)| direct_function_value(file, call.arguments[index].span));
        let reference_span = |reference: &typefacts::Location| {
            let candidate = self.file_by_path(reference.path.as_ref())?;
            let start = u32::try_from(reference.start_byte).ok()?;
            let end = u32::try_from(reference.end_byte).ok()?;
            Some((candidate, Span::new(start, end)))
        };
        let reference_is_callback_value = |candidate: &FileFacts, span: Span| {
            candidate.compiler.callback_roles.iter().any(|role| {
                role.span.contains(span)
                    && candidate.ast.functions_within(role.span).next().is_none()
            }) || candidate
                .ast
                .arguments_containing(span)
                .any(|(call, index)| {
                    candidate
                        .ast
                        .functions_within(call.arguments[index].span)
                        .next()
                        .is_none()
                        && self
                            .primitive_at_call(candidate, call.span)
                            .map(|primitive| {
                                self.dialect.callback_semantics_at(
                                    primitive,
                                    index,
                                    call.arguments.len(),
                                )
                            })
                            .is_some_and(|semantics| {
                                semantics.execution.is_some() || semantics.stores_as_value
                            })
                })
        };
        let component_value_operation = |candidate: &FileFacts, span: Span| {
            candidate.compiler.jsx_operations.iter().any(|operation| {
                if operation.kind == "component-property" && operation.span.contains(span) {
                    return candidate.ast.jsx_elements.iter().any(|element| {
                        element.attributes.iter().any(|attribute| {
                            attribute.value.is_some_and(|value| value.contains(span))
                                && candidate.source_text(attribute.local_name) == Some("component")
                        })
                    });
                }
                operation.kind == "component-spread"
                    && operation.span.contains(span)
                    && candidate.ast.object_properties.iter().any(|property| {
                        operation.span.contains(property.span)
                            && property.value.contains(span)
                            && candidate.source_text(property.key) == Some("component")
                    })
            })
        };
        let used_as_render_value = file.compiler.jsx_operations.iter().any(|operation| {
            matches!(
                operation.kind.as_str(),
                "component-property" | "component-spread" | "component-child"
            ) && operation.span.contains(function.span)
                && direct_function_value(file, operation.span)
                && !component_value_operation(file, function.span)
        });
        let mut referenced_as_callback = false;
        let mut referenced_as_component_property = false;
        if let Some(symbol) = binding_name
            .and_then(|name| self.entities.at(file.path.as_str(), name.span))
            .and_then(|symbol| {
                self.symbols_by_id
                    .get_or_init(|| {
                        self.facts
                            .typescript
                            .symbols()
                            .map(|candidate| (candidate.id(), candidate))
                            .collect()
                    })
                    .get(symbol.as_str())
                    .copied()
            })
        {
            for reference in symbol.references() {
                let Some((candidate, span)) = reference_span(reference) else {
                    continue;
                };
                referenced_as_callback |= reference_is_callback_value(candidate, span);
                referenced_as_component_property |= component_value_operation(candidate, span);
            }
        }
        let hoc_bound = binding_name.is_some()
            && crate::owners::function_binding_name(file, function).is_none();
        let directly_called = !self
            .function_call_sites(file.path.as_str(), function.span)
            .is_empty();
        if directly_returns_jsx
            && (self.dialect.direct_jsx_return_is_component()
                || hoc_bound
                || referenced_as_component_property)
            && !directly_called
            && (!used_as_callback_value(file) || hoc_bound)
            && (!used_as_any_call_argument || hoc_bound)
            && !used_as_render_value
            && (!referenced_as_callback || referenced_as_component_property)
        {
            return true;
        }
        let Some(name) = binding_name else {
            return false;
        };
        let Some(descriptor) = self.smallest_contained_descriptor(file.path.as_str(), name.span)
        else {
            return false;
        };
        descriptor.alias_declarations.iter().any(|declaration| {
            self.dialect
                .type_role(descriptor.origin_module.as_ref(), declaration.name.as_ref())
                == Some(solid_dialect::TypeRole::Component)
        })
    }

    fn project_has_component_type(&self) -> bool {
        *self.project_has_component_type.get_or_init(|| {
            self.facts.typescript.entities().any(|entity| {
                entity.type_descriptor.as_deref().is_some_and(|descriptor| {
                    descriptor.alias_declarations.iter().any(|declaration| {
                        self.dialect
                            .type_role(descriptor.origin_module.as_ref(), declaration.name.as_ref())
                            == Some(solid_dialect::TypeRole::Component)
                    })
                })
            })
        })
    }

    /// Whether `span` executes inside a function proven to be a component.
    pub(super) fn inside_component(&self, file: &FileFacts, span: Span) -> bool {
        file.ast
            .functions_body_containing(span)
            .any(|function| self.function_is_component(file, function))
    }

    /// Whether `span` is inside a function whose component identity is
    /// possible but not proven. Consumers use this to preserve an
    /// uncertifiable outcome instead of selecting either execution model.
    pub(super) fn inside_possible_component(&self, file: &FileFacts, span: Span) -> bool {
        file.ast.functions_body_containing(span).any(|function| {
            self.function_component_status(file, function) == ComponentStatus::Uncertain
        })
    }

    /// Compiler-derived callability at `span`, falling back to the smallest
    /// contained demanded entity only for callers that intentionally query a
    /// region, then to another demanded occurrence of the same symbol.
    pub(super) fn smallest_contained_callability(
        &self,
        path: &str,
        span: Span,
    ) -> Option<Callability> {
        self.entity_at(path, span)
            .and_then(|entity| entity.callability)
            .or_else(|| {
                self.smallest_contained(path, span, |entity| entity.callability.is_some())
                    .and_then(|entity| entity.callability)
            })
            .or_else(|| {
                let symbol = self.entities.at(path, span)?;
                self.callability_by_symbol
                    .get_or_init(|| {
                        self.facts
                            .typescript
                            .entities()
                            .filter_map(|entity| {
                                (!entity.symbol.is_empty())
                                    .then_some(entity.symbol.as_ref())
                                    .zip(entity.callability)
                            })
                            .collect()
                    })
                    .get(symbol.as_str())
                    .copied()
            })
    }

    /// Whether any JSX call site renders the function at `(path, function)`,
    /// and whether one of those call sites sits under a Loading boundary.
    pub(super) fn jsx_call_site_loading(&self, path: &str, function: Span) -> CallSiteLoading {
        self.jsx_call_sites()
            .get(&(path, function))
            .copied()
            .unwrap_or_default()
    }

    /// Call sites whose Type Facts resolve to one project function: call
    /// expressions, and the JSX tags that render a component. Aliases,
    /// imports, and same-named locals follow canonical symbols. One entry per
    /// invocation — a closing tag is part of the render it closes, not a
    /// second call.
    pub(super) fn function_call_sites(
        &self,
        path: &str,
        function: Span,
    ) -> Vec<(&'a FileFacts, Span)> {
        self.all_function_call_sites()
            .get(&(path, function))
            .into_iter()
            .flatten()
            .map(|site| (&self.facts.files[site.file], site.callee))
            .collect()
    }

    /// Every reference to a project function that one of its call sites
    /// accounts for.
    ///
    /// The same sites as [`Self::function_call_sites`], plus the extra
    /// occurrence of the callee's name a single site can spell:
    /// `<Panel></Panel>` renders `Panel` once and writes its name twice, and
    /// TypeScript reports both occurrences as references to the same symbol.
    /// The escape test asks whether *every* reference to a function is
    /// accounted for, so it asks here; every other consumer counts
    /// invocations and asks above.
    ///
    /// A closing tag can never appear on its own: the extra span is stored on
    /// the edge that the *opening* tag's resolution created, so if the opening
    /// tag resolved to nothing there is no site to carry it.
    pub(super) fn function_call_site_references(
        &self,
        path: &str,
        function: Span,
    ) -> Vec<(&'a FileFacts, Span)> {
        self.all_function_call_sites()
            .get(&(path, function))
            .into_iter()
            .flatten()
            .flat_map(|site| {
                let file = &self.facts.files[site.file];
                std::iter::once((file, site.callee))
                    .chain(site.also_referenced.map(|span| (file, span)))
            })
            .collect()
    }

    /// The argument a call of `symbol` renders, when `symbol` is declared by
    /// one of the dialect's own packages under a rendering name.
    fn rendered_argument_of(&self, symbol: &str) -> Option<usize> {
        let packages = self.dialect.primitive_defining_packages();
        self.symbols_by_id()
            .get(symbol)?
            .declarations()
            .iter()
            .filter(|declaration| declared_in_package(declaration.location.path.as_ref(), packages))
            .find_map(|declaration| {
                self.dialect
                    .renders_component_argument(declaration.name.as_ref())
            })
    }

    /// The render sites a value reference reaches, when its value reaches
    /// only a dialect component's rendering prop:
    /// `createComponent(Dynamic, { get component() { return Selected(); } })`
    /// with `const Selected = createMemo(() => cond() ? Panel : Other)`
    /// invokes the value `Panel` holds as a component, from computations that
    /// render creates and nowhere else (ADR 0138). The sites are in the
    /// reference's file; `None` for any other reference.
    ///
    /// Keyed by the reference, not by a function: the call graph asks it of
    /// each compiler-resolved reference of a function's symbols, the walk its
    /// escape test already makes, so *which* function the value is comes from
    /// those references and never from a name or a demanded entity. Kept
    /// beside [`Self::function_render_call_sites`] and apart from
    /// [`Self::function_call_sites`] for the same reason: only the attribution
    /// call graph asks who can enter a function, and neither component
    /// identity nor execution-role inheritance has this answer for a value.
    ///
    /// The syntax fact ([`solid_facts::ast::component_value_flows`]) says
    /// only where the value can go. Every span it hands back must resolve
    /// here, or the reference has no entry:
    ///
    /// - each site's component is declared in one of the dialect's
    ///   primitive-defining packages under a name the dialect says renders
    ///   that prop ([`solid_dialect::Dialect::component_prop_renderers`]),
    ///   and a call form's callee renders its first argument
    ///   ([`solid_dialect::Dialect::renders_component_argument`]);
    /// - each holder call is a primitive whose accessor yields only its
    ///   compute ([`solid_dialect::Dialect::accessor_yields_only_its_compute`]).
    pub(super) fn prop_render_sites_at(&self, path: &str, reference: Span) -> Option<Vec<Span>> {
        self.prop_render_sites().get(&(path, reference)).cloned()
    }

    fn prop_render_sites(&self) -> &PropRenderSites<'a> {
        self.prop_render_sites.get_or_init(|| {
            let mut map = PropRenderSites::new();
            let renderers = self.dialect.component_prop_renderers();
            if renderers.is_empty() {
                return map;
            }
            let mut props = renderers.iter().map(|(_, prop)| *prop).collect::<Vec<_>>();
            props.sort_unstable();
            props.dedup();
            let facts: &'a ProjectFacts = self.facts;
            for file in &facts.files {
                // Only a module that spells a rendering component and its prop
                // can hold one of these flows; the rest are not parsed again.
                if !renderers.iter().any(|(component, prop)| {
                    file.source.contains(component) && file.source.contains(prop)
                }) {
                    continue;
                }
                let Some(flows) = solid_facts::ast::component_value_flows(
                    std::path::Path::new(file.path.as_str()),
                    &file.source,
                    &props,
                ) else {
                    continue;
                };
                for flow in flows {
                    if flow
                        .holder_calls
                        .iter()
                        .all(|call| self.holder_call_is_exact(file, *call))
                        && flow.sites.iter().all(|site| self.renders_prop(file, site))
                    {
                        map.insert(
                            (file.path.as_str(), flow.reference),
                            flow.sites.iter().map(|site| site.site).collect(),
                        );
                    }
                }
            }
            map
        })
    }

    /// Whether the call at `call` is a primitive whose accessor hands its
    /// compute's results to nothing but its own reads.
    fn holder_call_is_exact(&self, file: &FileFacts, call: Span) -> bool {
        self.primitive_at_call(file, call)
            .is_some_and(|primitive| self.dialect.accessor_yields_only_its_compute(primitive))
    }

    /// Whether `site` renders its component with the dialect's semantics for
    /// its prop: the component is exactly a dialect export that renders that
    /// prop, and a call form's callee is exactly a dialect renderer of its
    /// first argument.
    fn renders_prop(&self, file: &FileFacts, site: &solid_facts::ast::ComponentPropSite) -> bool {
        if let Some(renderer) = site.renderer
            && self
                .callee_symbol(file, renderer)
                .and_then(|symbol| self.rendered_argument_of(symbol))
                != Some(0)
        {
            return false;
        }
        let Some(component) = self.entities.at(file.path.as_str(), site.component) else {
            return false;
        };
        let packages = self.dialect.primitive_defining_packages();
        let renderers = self.dialect.component_prop_renderers();
        self.symbols_by_id()
            .get(component.as_str())
            .into_iter()
            .flat_map(|symbol| symbol.declarations().iter())
            .filter(|declaration| declared_in_package(declaration.location.path.as_ref(), packages))
            .any(|declaration| {
                renderers.iter().any(|(name, prop)| {
                    *name == declaration.name.as_ref() && *prop == site.prop.as_str()
                })
            })
    }

    /// ADR 0313: the prop whose string value makes the component named at
    /// `name` render an intrinsic element
    /// ([`solid_dialect::Dialect::intrinsic_tag_renderers`]). The tag's
    /// symbol must have declarations, every one of them in one of the
    /// dialect's primitive-defining packages under that export's name.
    pub(crate) fn intrinsic_tag_prop(&self, file: &FileFacts, name: Span) -> Option<&'static str> {
        let component = self.entities.at(file.path.as_str(), name)?;
        let declarations = self.symbols_by_id().get(component.as_str())?.declarations();
        let packages = self.dialect.primitive_defining_packages();
        let (_, prop) = self
            .dialect
            .intrinsic_tag_renderers()
            .iter()
            .find(|(export, _)| {
                !declarations.is_empty()
                    && declarations.iter().all(|declaration| {
                        declared_in_package(declaration.location.path.as_ref(), packages)
                            && declaration.name.as_ref() == *export
                    })
            })?;
        Some(prop)
    }

    fn symbols_by_id(&self) -> &HashMap<&'a str, solid_facts::TypeScriptSymbol<'a>> {
        self.symbols_by_id.get_or_init(|| {
            self.facts
                .typescript
                .symbols()
                .map(|candidate| (candidate.id(), candidate))
                .collect()
        })
    }

    fn smallest_contained(
        &self,
        path: &str,
        span: Span,
        predicate: impl Fn(&EntityFact) -> bool,
    ) -> Option<&'a EntityFact> {
        let start = u64::from(span.start);
        let end = u64::from(span.end);
        let entities = self.contained_entities_by_path().get(path)?;
        let first = entities.partition_point(|entity| entity.location.start_byte < start);
        let last = entities.partition_point(|entity| entity.location.start_byte <= end);
        entities[first..last]
            .iter()
            .enumerate()
            .filter_map(|(index, entity)| {
                (entity.location.end_byte <= end && predicate(entity)).then_some((index, *entity))
            })
            .min_by_key(|(index, entity)| {
                (
                    entity.location.end_byte - entity.location.start_byte,
                    *index,
                )
            })
            .map(|(_, entity)| entity)
    }

    fn contained_entities_by_path(&self) -> &HashMap<&'a str, Vec<&'a EntityFact>> {
        self.contained_entities_by_path.get_or_init(|| {
            let mut by_path = HashMap::<&str, Vec<&EntityFact>>::new();
            for entity in self.facts.typescript.entities() {
                by_path
                    .entry(entity.location.path.as_ref())
                    .or_default()
                    .push(entity);
            }
            for entities in by_path.values_mut() {
                entities.sort_by_key(|entity| entity.location.start_byte);
            }
            by_path
        })
    }

    fn functions_by_symbol(&self) -> &HashMap<&'a str, SymbolFunction> {
        self.functions_by_symbol.get_or_init(|| {
            let mut map = HashMap::new();
            for (file_index, file) in self.facts.files.iter().enumerate() {
                for (function_index, function) in file.ast.functions.iter().enumerate() {
                    let own_method = crate::owners::own_method_name(file, function);
                    let Some(name) = function.name.as_ref().or(own_method) else {
                        continue;
                    };
                    let Some(symbol) = self
                        .entities
                        .at(file.path.as_str(), name.span)
                        .map(SymbolId::as_str)
                        .or_else(|| {
                            own_method.and_then(|method| {
                                self.method_symbol_ref(file.path.as_str(), method.span)
                            })
                        })
                    else {
                        continue;
                    };
                    map.entry(symbol).or_insert(SymbolFunction::Resolved {
                        file: file_index,
                        function: function_index,
                    });
                }
                for binding in &file.ast.bindings {
                    if !binding.initializer_function && binding.call_initializer.is_none() {
                        continue;
                    }
                    if binding.call_initializer.is_some()
                        && (binding.shape != solid_facts::ast::BindingShape::Identifier
                            || binding.names.len() != 1)
                    {
                        // A call result may name a wrapped function only when
                        // one identifier owns the whole result. Tuple/object
                        // bindings name slots, not closures nested in the
                        // call's arguments (`createSignal(() => value)`).
                        continue;
                    }
                    let mut outcome = None;
                    for name in &binding.names {
                        let Some(symbol) = self.entities.at(file.path.as_str(), name.span) else {
                            continue;
                        };
                        if map.contains_key(symbol.as_str()) {
                            continue;
                        }
                        let outcome = *outcome.get_or_insert_with(|| match binding.initializer {
                            None => Some(SymbolFunction::Aborted),
                            Some(initializer) => file
                                .ast
                                .functions
                                .iter()
                                .enumerate()
                                .filter(|(_, function)| initializer.contains(function.span))
                                .max_by_key(|(_, function)| function.span.end - function.span.start)
                                .map(|(function_index, _)| SymbolFunction::Resolved {
                                    file: file_index,
                                    function: function_index,
                                }),
                        });
                        if let Some(outcome) = outcome {
                            map.insert(symbol.as_str(), outcome);
                        }
                    }
                }
            }
            map
        })
    }

    /// Every JSX element whose tag name resolves, exactly, to one project
    /// function, paired with the function it renders.
    ///
    /// The single authority on "this tag renders that function". Two indexes
    /// need that answer — [`Self::jsx_call_sites`], which decides component
    /// identity and Loading placement, and [`Self::all_function_call_sites`],
    /// which puts the render edge in the call graph — and they must agree: the
    /// render edges are invisible to component identity only because a
    /// rendered function is already `jsx_call_site_loading(..).any`. Two copies
    /// of the resolution could drift apart on the next filter added to either;
    /// one iterator cannot.
    fn jsx_rendered_functions(
        &self,
    ) -> impl Iterator<
        Item = (
            usize,
            &'a FileFacts,
            &'a solid_facts::ast::JsxElementFact,
            &'a FileFacts,
            &'a solid_facts::ast::FunctionFact,
        ),
    > + '_ {
        self.facts
            .files
            .iter()
            .enumerate()
            .flat_map(move |(file_index, caller_file)| {
                caller_file
                    .ast
                    .jsx_elements
                    .iter()
                    .filter_map(move |element| {
                        let (target_file, target) =
                            self.function_called_at(caller_file.path.as_str(), element.name.span)?;
                        Some((file_index, caller_file, element, target_file, target))
                    })
            })
    }

    fn jsx_call_sites(&self) -> &HashMap<(&'a str, Span), CallSiteLoading> {
        self.jsx_call_sites.get_or_init(|| {
            let mut map = HashMap::<(&'a str, Span), CallSiteLoading>::new();
            for (_, _, _, target_file, target) in self.jsx_rendered_functions() {
                let entry = map
                    .entry((target_file.path.as_str(), target.span))
                    .or_default();
                entry.any = true;
            }
            map
        })
    }

    /// Every entry the graph can name for each project function.
    ///
    /// Rendering `<Panel/>` invokes `Panel`: the tag is a call site, not a
    /// value escape, and the same exact resolution that proves which function a
    /// tag renders ([`Self::jsx_rendered_functions`], shared with
    /// `jsx_call_sites` so component identity and the call graph cannot
    /// disagree about which tags resolve) names the callee here. The edge's
    /// callee is the tag *name* span only — the component's own reference — so
    /// a component used as a value rather than rendered (`<Wrap
    /// child={Panel}/>`, `return Panel`, `apply(Panel)`) is still an escape;
    /// `component_value_operation` keeps that distinction. A tag whose exact
    /// declaration is unresolved — an untyped import, an ambiguous computed
    /// name — resolves to nothing and emits no edge, so consumers keep failing
    /// closed on it rather than gaining a caller the runtime does not have.
    ///
    /// A dotted tag is not that case. `<ns.Panel/>` resolves: TypeScript
    /// reports the symbol at the whole `ns.Panel` name span, so an edge is
    /// emitted whose callee *is* that whole span — a member expression, not an
    /// identifier. Consumers that expect an identifier there fail closed on
    /// their own terms: `structural_parameter_member_symbols` finds no
    /// `CallFact` and marks the site unresolved, and the escape test's
    /// byte-exact span set does not match the `Panel` property reference it
    /// walks, so a dotted render stays an escape.
    ///
    /// Call expressions come first, then render sites: a function that is both
    /// called and rendered is asked about its call sites in that order, so a
    /// consumer that takes the first answer it can use
    /// (`semantic_write_execution_role_within` takes the first non-`Unknown`
    /// execution role) resolves the tie by an argued rule rather than by which
    /// file happens to hold the call. A call expression is the more direct
    /// evidence of the two — its own syntax names the invocation — so it wins.
    fn all_function_call_sites(&self) -> &HashMap<(&'a str, Span), Vec<FunctionCallSite>> {
        self.function_call_sites.get_or_init(|| {
            let mut map = HashMap::<(&str, Span), Vec<FunctionCallSite>>::new();
            for (file_index, caller_file) in self.facts.files.iter().enumerate() {
                for call in &caller_file.ast.calls {
                    let Some(symbol) = self.callee_symbol(caller_file, call.callee) else {
                        continue;
                    };
                    let Some((target_file, target)) = self.function_for_symbol(symbol) else {
                        continue;
                    };
                    map.entry((target_file.path.as_str(), target.span))
                        .or_default()
                        .push(FunctionCallSite {
                            file: file_index,
                            callee: call.callee,
                            also_referenced: None,
                        });
                }
            }
            for (file_index, _, element, target_file, target) in self.jsx_rendered_functions() {
                map.entry((target_file.path.as_str(), target.span))
                    .or_default()
                    .push(FunctionCallSite {
                        file: file_index,
                        callee: element.name.span,
                        // One render, one edge. `<Panel></Panel>` writes the
                        // tag name twice and TypeScript reports both
                        // occurrences as references to `Panel`, so the escape
                        // test has to account for the closing one — but it is
                        // the same invocation, so it rides on this edge
                        // instead of minting a second one. Carried here rather
                        // than re-derived by the consumer so it can only ever
                        // be the closing name of the element whose opening tag
                        // resolved to this function.
                        also_referenced: element.closing_name,
                    });
            }
            map
        })
    }

    fn entities_by_location(&self) -> &HashMap<(&'a str, u64, u64), &'a EntityFact> {
        self.entities_by_location.get_or_init(|| {
            let mut map = HashMap::new();
            for entity in self.facts.typescript.entities() {
                map.entry((
                    entity.location.path.as_ref(),
                    entity.location.start_byte,
                    entity.location.end_byte,
                ))
                .or_insert(entity);
            }
            map
        })
    }
}

/// Whether a file refers to the CommonJS exports object (`module`, `exports`)
/// as a name no declaration binds. A module that does can have its exports
/// replaced under a CommonJS transform, so its `export const` is not a frozen
/// ES namespace entry (ADR 0325). Any reference counts, read or written: the
/// proof wants the file to be ES module syntax alone.
fn file_names_commonjs_exports(file: &FileFacts) -> bool {
    file.ast.identifiers.iter().any(|identifier| {
        identifier.role == solid_facts::ast::IdentifierRole::Reference
            && file
                .source
                .get(identifier.span.start as usize..identifier.span.end as usize)
                .is_some_and(|name| name == "module" || name == "exports")
            && file.ast.reference_declaration(identifier.span).is_none()
    })
}

#[cfg(test)]
mod tests;
