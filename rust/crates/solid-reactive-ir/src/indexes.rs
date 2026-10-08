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
/// Whether some function in `file` returns a bare identifier: the only shape
/// whose callee syntax a returned-source proof (ADR 0222) reads beyond the
/// summaries and Type Facts the caches compare.
pub(crate) fn returns_bare_identifier(file: &FileFacts) -> bool {
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

/// Render sites by rendered function: `(file index, rendered argument span)`
/// per site ([`SemanticLookup::function_render_call_sites`]).
type RenderCallSites<'a> = HashMap<(&'a str, Span), Vec<(usize, Span)>>;

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
    facts: &'a ProjectFacts,
    /// The Solid-version vocabulary this build analyzes with. Every consumer
    /// of the lookup shares one dialect; a build never mixes two.
    pub(super) dialect: &'a dyn solid_dialect::Dialect,
    /// Whether the user asserted that the analyzed files are the whole
    /// program. Build-wide like the dialect, and false unless selected: a
    /// build that was never told stays fail-closed.
    pub(super) program_closed: bool,
    ast_indexes: &'a HashMap<solid_facts::core::SourcePath, CachedAstFileIndex>,
    entities: &'a EntitySymbols,
    symbol_names: &'a HashMap<SymbolId, SymbolName>,
    resolved_contracts: &'a crate::contracts::ResolvedContracts,
    functions_by_symbol: OnceLock<HashMap<&'a str, SymbolFunction>>,
    /// ADR 0209: the property names some assignment in the project writes
    /// (`x.name = …`), and whether any writes through a `prototype`.
    assigned_member_names: OnceLock<(HashSet<String>, bool)>,
    entities_by_location: OnceLock<HashMap<(&'a str, u64, u64), &'a EntityFact>>,
    contained_entities_by_path: OnceLock<HashMap<&'a str, Vec<&'a EntityFact>>>,
    descriptors_by_symbol: OnceLock<HashMap<&'a str, &'a TypeDescriptor>>,
    callability_by_symbol: OnceLock<HashMap<&'a str, Callability>>,
    symbols_by_id: OnceLock<HashMap<&'a str, solid_facts::TypeScriptSymbol<'a>>>,
    context_symbols: OnceLock<HashSet<&'a str>>,
    jsx_call_sites: OnceLock<HashMap<(&'a str, Span), CallSiteLoading>>,
    declaration_symbols: OnceLock<DeclarationSymbols<'a>>,
    function_call_sites: OnceLock<HashMap<(&'a str, Span), Vec<FunctionCallSite>>>,
    render_call_sites: OnceLock<RenderCallSites<'a>>,
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
    pub(super) fn new(
        facts: &'a ProjectFacts,
        ast_indexes: &'a HashMap<solid_facts::core::SourcePath, CachedAstFileIndex>,
        entities: &'a EntitySymbols,
        symbol_names: &'a HashMap<SymbolId, SymbolName>,
        dialect: &'a dyn solid_dialect::Dialect,
        resolved_contracts: &'a crate::contracts::ResolvedContracts,
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
            facts,
            dialect,
            program_closed,
            ast_indexes,
            entities,
            symbol_names,
            resolved_contracts,
            functions_by_symbol: OnceLock::new(),
            assigned_member_names: OnceLock::new(),
            entities_by_location: OnceLock::new(),
            contained_entities_by_path: OnceLock::new(),
            descriptors_by_symbol: OnceLock::new(),
            callability_by_symbol: OnceLock::new(),
            symbols_by_id: OnceLock::new(),
            context_symbols: OnceLock::new(),
            jsx_call_sites: OnceLock::new(),
            declaration_symbols: OnceLock::new(),
            function_call_sites: OnceLock::new(),
            render_call_sites: OnceLock::new(),
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
        }
    }

    /// Whether an exact whole-function graph was actually installed.
    pub(super) fn returned_callable_is_bound(&self, symbol: &str) -> bool {
        self.resolved_contracts
            .returned_callable_bindings
            .contains(symbol)
    }

    /// Whether this exact function-object member callee has a binding.
    pub(super) fn returned_member_is_bound(&self, file: &FileFacts, callee: Span) -> bool {
        self.resolved_contracts
            .callee_bindings
            .contains_key(&crate::location(
                file.path.shared(),
                file.ast.peel_ts_sugar_span(callee),
            ))
    }

    /// The known callback rows of the contract bound to `symbol` that invoke
    /// the argument as a callable. A non-call row (a property read or
    /// coercion of the argument, `ContractCallback::is_invocation`) is not an
    /// invocation any caller of this lookup models, and is left out; so is a
    /// member-path row (item B of ways-to-improve § 3.3), which invokes a
    /// member of the argument rather than the argument, and whose execution
    /// or owner is no claim about the function written at the slot. `Some`
    /// still means the enumeration is known, even when it holds only such
    /// rows.
    pub(super) fn contract_callbacks(&self, symbol: &str) -> Option<Vec<&super::ContractCallback>> {
        self.resolved_contracts
            .by_symbol
            .get(symbol)
            .and_then(|binding| binding.summary.callbacks.known())
            .map(|callbacks| {
                callbacks
                    .iter()
                    .filter(|callback| callback.invokes_argument())
                    .collect()
            })
    }

    /// ADR 0207: the `event-handler-props` items of the accepted contract
    /// bound to `symbol`.
    pub(super) fn contract_event_handler_props(
        &self,
        symbol: &str,
    ) -> Option<&[super::EventHandlerPropsClaim]> {
        self.resolved_contracts
            .by_symbol
            .get(symbol)
            .map(|binding| binding.summary.event_handler_props.as_slice())
    }

    pub(super) fn contract_inline_accessor_invocation(
        &self,
        symbol: &str,
        parameter: usize,
    ) -> Option<bool> {
        self.resolved_contracts
            .by_symbol
            .get(symbol)
            .and_then(|binding| {
                binding
                    .summary
                    .inline_accessor_invocations
                    .get(&parameter)
                    .copied()
            })
    }

    /// ADR 0152: the argument slots the value the contract export bound to
    /// `symbol` returns invokes on its own invoker's stack, or `None` when no
    /// contract is bound to it.
    pub(super) fn contract_returned_invocations(
        &self,
        symbol: &str,
    ) -> Option<&std::collections::BTreeSet<usize>> {
        self.resolved_contracts
            .by_symbol
            .get(symbol)
            .map(|binding| &binding.summary.returned_invocations)
    }

    /// The package and imported export name a symbol's contract binding
    /// carries, whatever its claim status. Consumer-side obligations name the
    /// export whose claim could not be applied, which is the same identity the
    /// unknown-claim path reports.
    pub(super) fn contract_export_identity(&self, symbol: &str) -> Option<(&str, &str)> {
        self.resolved_contracts
            .by_symbol
            .get(symbol)
            .map(|binding| {
                (
                    binding.package_name.as_str(),
                    binding.imported_name.as_str(),
                )
            })
    }

    pub(super) fn unknown_contract_callback_export(&self, symbol: &str) -> Option<(&str, &str)> {
        self.resolved_contracts
            .by_symbol
            .get(symbol)
            .filter(|binding| {
                binding.summary.callbacks.is_open()
                    || binding
                        .summary
                        .open_claims
                        .contains(&crate::contract_semantics::ClaimDomain::Callbacks)
            })
            .map(|binding| {
                (
                    binding.package_name.as_str(),
                    binding.imported_name.as_str(),
                )
            })
    }

    /// Whether the accepted contract bound to `symbol` closes `creates` with
    /// no item, or `None` when no contract is bound to it at all.
    ///
    /// `Some(false)` is the counterexample the generator's `creates` proposal
    /// walk refuses on: an open domain and a domain carrying a `create` item
    /// are both reasons not to propose that the calling export publishes none.
    /// `None` means this callee is not a contracted dependency export, which is
    /// not a counterexample this walk can name — see
    /// [`crate::CreatesProposalWalk`].
    pub(super) fn contract_creates_closed_empty(&self, symbol: &str) -> Option<bool> {
        self.resolved_contracts
            .by_symbol
            .get(symbol)
            .map(|binding| binding.summary.creates_closed_empty)
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

    /// The module specifier the compiler resolved this callee's declaration to
    /// originate from, or `None` when the build has no resolved declaration for
    /// it (or the producer recorded no origin).
    ///
    /// An exact resolved fact and nothing weaker: it is the callee's own
    /// `ResolvedDeclaration::origin_module`, never derived from the callee's
    /// spelling or from an import statement standing nearby.
    ///
    /// **Exact, and frequently absent** — measured 2026-09-17, and worth
    /// knowing before trusting it. Probed against the audited rc.3 install,
    /// every `solid-js` primitive callee answered `None` here, so the caller
    /// that names the package half of a
    /// [`crate::CreatesDeclineKind::DialectSilent`] record reached its
    /// import-statement fallback instead. That record's package is therefore
    /// usually the *written specifier's* package rather than this one, which is
    /// weaker than it reads: for `solid-js` it cannot tell a re-export of
    /// `@solidjs/signals`' declaration from `solid-js`' own re-declaration.
    ///
    /// Nothing decides an audit on it. The proposal-side denial lookup moved to
    /// [`Self::callee_declaration_source_file`] for exactly that reason, and the
    /// census that proves a claim binds an archive. What remains is that the
    /// decline record — an instrument for ranking *which primitive to audit
    /// next* — can name a package coarser than the one whose bytes the call
    /// reached.
    /// The file the callee's resolved declaration is written in.
    ///
    /// Distinct from [`Self::callee_origin_module`], and the distinction is the
    /// point: `origin_module` is the module the *specifier* resolved to, so it
    /// cannot tell a re-export from a re-declaration. This resolves through the
    /// re-export to the file that actually declares the name, which is the only
    /// thing that identifies the package whose audited bytes a call reaches.
    ///
    /// Measured against the audited rc.3 install: a `solid-js` import of
    /// `untrack` answers `@solidjs/signals/.../core/core.d.ts`, while a
    /// `solid-js` import of `createSignal` answers
    /// `solid-js/types/client/hydration.d.ts` -- 2.0 re-exports the first and
    /// re-declares the second.
    pub(super) fn callee_declaration_source_file(
        &self,
        file: &FileFacts,
        callee: Span,
    ) -> Option<&'a str> {
        let declaration = self
            .resolved_callee_call(file, callee)?
            .declaration
            .as_ref()?;
        (!declaration.source_file.is_empty()).then_some(declaration.source_file.as_ref())
    }

    pub(super) fn callee_origin_module(&self, file: &FileFacts, callee: Span) -> Option<&'a str> {
        let declaration = self
            .resolved_callee_call(file, callee)?
            .declaration
            .as_ref()?;
        (!declaration.origin_module.is_empty()).then_some(declaration.origin_module.as_ref())
    }

    pub(super) fn contract_owner_requirements(
        &self,
        symbol: &str,
    ) -> Option<&[super::ContractOwnerRequirement]> {
        self.resolved_contracts
            .by_symbol
            .get(symbol)
            .and_then(|binding| binding.summary.owner_requirements.known())
            .map(Vec::as_slice)
    }

    /// ADR 0179: see [`super::ContractExport::leaf_forbidden_operations`].
    pub(super) fn contract_leaf_forbidden_operations(
        &self,
        symbol: &str,
    ) -> Option<&[super::ContractOwnerRequirement]> {
        self.resolved_contracts
            .by_symbol
            .get(symbol)
            .map(|binding| binding.summary.leaf_forbidden_operations.as_slice())
    }

    /// Every use of this exact callback slot is a tracked invocation under
    /// a created owner. Closure proves the universal context; no lower bound
    /// or schedule is inferred. This is a read-only consumer proof.
    pub(super) fn contract_callback_reads_are_tracked(
        &self,
        symbol: &str,
        parameter: usize,
    ) -> bool {
        let Some(binding) = self.resolved_contracts.by_symbol.get(symbol) else {
            return false;
        };
        if binding
            .summary
            .open_claims
            .contains(&crate::contract_semantics::ClaimDomain::Callbacks)
        {
            return false;
        }
        // A returned argument/opaque member can expose this same function
        // to another invoker. This slice handles a closed accessor return
        // only; broader result shapes need their own escape/use proof.
        if binding
            .summary
            .open_claims
            .contains(&crate::contract_semantics::ClaimDomain::Returns)
            || !binding.summary.returns.known().is_some_and(|returned| {
                returned.as_ref().is_some_and(|returned| {
                    returned.kind == "accessor"
                        && returned.parameter.is_none()
                        && returned.elements.is_empty()
                        && returned.properties.is_empty()
                })
            })
        {
            return false;
        }
        let Some(callbacks) = binding.summary.callbacks.known() else {
            return false;
        };
        let mut rows = callbacks
            .iter()
            .filter(|row| row.parameter == parameter)
            .peekable();
        rows.peek().is_some()
            && rows.all(|row| {
                row.invokes_argument()
                    && row.execution == "tracked"
                    && row.owner.as_deref() == Some("created")
            })
    }

    /// ADR 0183: the parameters an accepted contract states are invoked, on
    /// every call and during it, as the tracked compute of an owned
    /// computation the export creates.
    pub(super) fn contract_guaranteed_callback_parameters(
        &self,
        symbol: &str,
    ) -> Option<&std::collections::BTreeSet<usize>> {
        self.resolved_contracts
            .by_symbol
            .get(symbol)
            .map(|binding| &binding.summary.guaranteed_callback_parameters)
    }

    pub(super) fn has_contract_binding(&self, symbol: &SymbolId) -> bool {
        self.resolved_contracts.by_symbol.contains_key(symbol)
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
    pub(super) fn cross_file_proof_digest(&self) -> Option<CrossFileProofDigest> {
        *self.cross_file_proof_digest.get_or_init(|| {
            let components = self.component_functions();
            let mut component_keys = components.iter().copied().collect::<Vec<_>>();
            component_keys.sort_unstable();
            let mut hasher = Sha256::new();
            hasher.update(b"components\0");
            // Returned-source proofs inspect callee syntax and binding facts,
            // even when its public type and read summary do not change. Bind
            // the byte identity of every file that can hold such a proof (a
            // bare identifier return, ADR 0222); a file gaining or losing one
            // enters or leaves the digest, so the set itself is bound too.
            let mut source_inputs = self
                .files()
                .iter()
                .filter(|file| returns_bare_identifier(file))
                .collect::<Vec<_>>();
            source_inputs.sort_unstable_by(|left, right| left.path.cmp(&right.path));
            hasher.update(b"returned-source-inputs\0");
            for file in source_inputs {
                let path = file.path.as_str();
                hasher.update(u64::try_from(path.len()).unwrap_or(u64::MAX).to_le_bytes());
                hasher.update(path.as_bytes());
                hasher.update(file.source_hash.as_str().as_bytes());
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

    pub(super) fn symbol_names(&self) -> &'a HashMap<SymbolId, SymbolName> {
        self.symbol_names
    }

    pub(super) fn files(&self) -> &'a [FileFacts] {
        &self.facts.files
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
    pub(super) fn symbol_references_if_present(&self, symbol: &str) -> Option<Vec<Location>> {
        self.symbols_by_id()
            .get(symbol)
            .map(|candidate| candidate.references().cloned().collect())
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
        let (names, prototype) = self.assigned_member_names.get_or_init(|| {
            let facts: &'a ProjectFacts = self.facts;
            let mut names = HashSet::new();
            let mut prototype = false;
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
                                prototype = true;
                            }
                        } else if let Some(property) = file.source_text(member.property) {
                            names.insert(property.to_string());
                            if property == "prototype" {
                                prototype = true;
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
            (names, prototype)
        });
        *prototype || names.contains(name)
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
        self.value_origin_assuming(file, value, kind, depth, None)
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
        let (binding_file, binding, _) = self.binding_at_reference(file.path.as_str(), value)?;
        if !binding.immutable || binding.shape != solid_facts::ast::BindingShape::Identifier {
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
            && !self.member_name_may_be_reassigned(method)
            // A species-built result follows the receiver's `constructor`.
            && (RECEIVER_RETURNING_METHODS.contains(&method)
                || !self.member_name_may_be_reassigned("constructor"))
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
        crate::returns_walk::own_returns(&file.ast, function).all(proven)
    }

    fn standard_library_declaration(
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
                    let returns_value = crate::returns_walk::own_returns(&file.ast, constructor)
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
        let resolution = self
            .bindings_by_reference()
            .get(path)?
            .get(&(u64::from(span.start), u64::from(span.end)))?;
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
        self.entity_at(file.path.as_str(), callee)
            .and_then(|entity| entity.resolved_call.as_deref())
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
        if let Some(symbol) = self
            .resolved_contracts
            .callee_bindings
            .get(&crate::location(file.path.shared(), callee))
        {
            return Some(symbol.as_str());
        }
        let member_property = self
            .ast_indexes
            .get(file.path.as_str())
            .and_then(|index| index.member_property(callee));
        let computed_member = file.ast.computed_members.contains(&callee);
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
        if let Some(symbol) = self
            .resolved_contracts
            .callee_bindings
            .get(&crate::location(file.path.shared(), callee))
        {
            return vec![symbol.clone()];
        }
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
            if aliases
                .iter()
                .any(|alias| self.returned_callable_is_bound(alias.as_str()))
            {
                return vec![symbol.clone()];
            }
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
            // analyzed implementation or an explicitly resolved package
            // contract. A TypeScript interface/signature declaration without
            // either is only a structural type, so continue to exact
            // receiver/call-site dispatch below.
            if self.function_for_symbol(symbol.as_str()).is_some()
                || self.resolved_contracts.by_symbol.contains_key(&symbol)
            {
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
    pub(super) fn member_value_symbols_at(
        &self,
        file: &FileFacts,
        value: Span,
        property_name: &str,
    ) -> Vec<SymbolId> {
        let mut symbols = Vec::new();
        let mut visited = HashSet::new();
        self.member_value_symbols(file, value, property_name, &mut visited, &mut symbols);
        symbols.sort_unstable();
        symbols.dedup();
        symbols
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
        let object = file.ast.peel_ts_sugar_span(object);
        if !visited_spans.insert(object) {
            return;
        }
        if let Some(conditional) = file
            .ast
            .conditional_expressions
            .iter()
            .find(|conditional| conditional.span == object)
        {
            self.member_value_symbols(
                file,
                conditional.consequent,
                property_name,
                visited_spans,
                symbols,
            );
            self.member_value_symbols(
                file,
                conditional.alternate,
                property_name,
                visited_spans,
                symbols,
            );
            return;
        }

        let direct_symbol = self.entities.at(file.path.as_str(), object);
        let identifier_reference = direct_symbol.is_none()
            && file.ast.identifiers.iter().any(|identifier| {
                identifier.span == object
                    && identifier.role == solid_facts::ast::IdentifierRole::Reference
            });
        let binding = direct_symbol
            .and_then(|symbol| self.binding_for_symbol(symbol))
            .or_else(|| {
                identifier_reference.then(|| {
                    self.binding_at_reference(file.path.as_str(), object)
                        .map(|(binding_file, binding, _)| (binding_file, binding))
                })?
            });
        if let Some((binding_file, binding)) = binding {
            if let Some(initializer) = binding.initializer {
                self.member_value_symbols(
                    binding_file,
                    initializer,
                    property_name,
                    visited_spans,
                    symbols,
                );
                if !symbols.is_empty() {
                    return;
                }
            }
            // A binding without an inspectable initializer is not evidence
            // for any implementation. Retaining its variable symbol would
            // turn receiver identity into a method-name guess.
            return;
        }

        let Some(properties) = file
            .ast
            .object_properties
            .iter()
            .filter(|property| {
                object.contains(property.span)
                    && !property.computed
                    && file.source_text(property.key) == Some(property_name)
            })
            .max_by_key(|property| property.span.start)
        else {
            for spread in file
                .ast
                .spreads
                .iter()
                .filter(|spread| object.contains(spread.span))
            {
                self.member_value_symbols(
                    file,
                    spread.argument,
                    property_name,
                    visited_spans,
                    symbols,
                );
            }
            return;
        };

        // The property key itself is a semantic identity once demanded by
        // the backend. This works for class/object methods and for function
        // properties copied through an exact object spread.
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
            self.member_value_symbols(
                file,
                properties.value,
                property_name,
                visited_spans,
                symbols,
            );
        }
        for spread in file.ast.spreads.iter().filter(|spread| {
            object.contains(spread.span) && spread.span.start > properties.span.start
        }) {
            self.member_value_symbols(file, spread.argument, property_name, visited_spans, symbols);
        }
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

    /// Call sites that **render** the project function at `(path, function)`
    /// through a dialect renderer: `createComponent(Panel, props)` invokes
    /// `Panel` exactly as the tag `<Panel/>` does (ADR 0136). Each site's span
    /// is the rendered argument, the function's own reference.
    ///
    /// Kept apart from [`Self::function_call_sites`] on purpose. That map
    /// feeds component identity (a function called directly is not a
    /// component) and execution-role inheritance (the role at the callee
    /// span), and neither question is the same for an argument of a Solid
    /// call; the call graph in `attribution` is the one consumer, and it asks
    /// only who can enter a function.
    ///
    /// Exact or nothing, at both ends:
    ///
    /// - the callee resolves to a symbol one of whose declarations sits in one
    ///   of the dialect's primitive-defining packages under a name the dialect
    ///   says renders an argument
    ///   ([`solid_dialect::Dialect::renders_component_argument`]); a project
    ///   function that happens to be called `createComponent` is not one;
    /// - the argument at that position is a bare identifier, no spread up to
    ///   it, whose symbol is one project function. A parameter, a computed
    ///   value, `options.Wrap || Fallback`, or an unresolved import resolves
    ///   to no function and adds no edge, so the reference stays the value
    ///   escape it was.
    pub(super) fn function_render_call_sites(
        &self,
        path: &str,
        function: Span,
    ) -> Vec<(&'a FileFacts, Span)> {
        self.render_call_sites()
            .get(&(path, function))
            .into_iter()
            .flatten()
            .map(|(file, argument)| (&self.facts.files[*file], *argument))
            .collect()
    }

    fn render_call_sites(&self) -> &RenderCallSites<'a> {
        self.render_call_sites.get_or_init(|| {
            let mut map = RenderCallSites::new();
            let facts: &'a ProjectFacts = self.facts;
            for (file_index, file) in facts.files.iter().enumerate() {
                for call in &file.ast.calls {
                    let Some(index) = self
                        .callee_symbol(file, call.callee)
                        .and_then(|symbol| self.rendered_argument_of(symbol))
                    else {
                        continue;
                    };
                    if call
                        .arguments
                        .iter()
                        .take(index.saturating_add(1))
                        .any(|argument| argument.spread)
                    {
                        continue;
                    }
                    let Some(argument) = call.arguments.get(index) else {
                        continue;
                    };
                    if argument.value != solid_facts::ast::ArgumentValueKind::Identifier {
                        continue;
                    }
                    let Some((target_file, target)) =
                        self.function_called_at(file.path.as_str(), argument.span)
                    else {
                        continue;
                    };
                    map.entry((target_file.path.as_str(), target.span))
                        .or_default()
                        .push((file_index, argument.span));
                }
            }
            map
        })
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
                    let Some(name) = function.name.as_ref().or(function.method_name.as_ref())
                    else {
                        continue;
                    };
                    let Some(symbol) = self
                        .entities
                        .at(file.path.as_str(), name.span)
                        .map(SymbolId::as_str)
                        .or_else(|| {
                            function.method_name.as_ref().and_then(|method| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use solid_facts::compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap};
    use solid_facts::core::{Generation, SourceHash, SourcePath};

    const PATH: &str = "app.tsx";

    /// The span of the `index`-th occurrence of `needle` in `source`.
    fn span_of(source: &str, needle: &str, index: usize) -> Span {
        let start = source
            .match_indices(needle)
            .nth(index)
            .expect("needle occurs in source")
            .0;
        Span::new(
            u32::try_from(start).unwrap(),
            u32::try_from(start + needle.len()).unwrap(),
        )
    }

    fn project(source: &str) -> ProjectFacts {
        let generation = Generation::new(1).unwrap();
        ProjectFacts {
            generation,
            project_id: "fixture".into(),
            files: vec![FileFacts {
                generation,
                path: SourcePath::new(PATH).unwrap(),
                source_hash: SourceHash::of(source),
                source: Arc::from(source),
                ast: Arc::new(solid_facts::ast::extract(PATH, source).unwrap()),
                compiler: Arc::new(ExecutionMap {
                    compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
                    source_hash: SourceHash::of(source),
                    semantic_model: Default::default(),
                    tracked_regions: Vec::new(),
                    untracked_regions: Vec::new(),
                    discarded_regions: Vec::new(),
                    ownership_regions: Vec::new(),
                    callback_roles: Vec::new(),
                    jsx_operations: Vec::new(),
                }),
            }],
            typescript: TypeScriptTable::from_parts(
                3,
                1,
                "fixture",
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
            typescript_changes: None,
            resolved_imports: None,
            runtime_resolutions: None,
            runtime_symbol_redirects: HashMap::new(),
        }
    }

    fn entity_symbols(spans: &[(Span, &str)]) -> EntitySymbols {
        EntitySymbols {
            by_path: HashMap::from([(
                PATH.to_string(),
                spans
                    .iter()
                    .map(|(span, symbol)| {
                        (
                            (u64::from(span.start), u64::from(span.end)),
                            SymbolId::from(*symbol),
                        )
                    })
                    .collect(),
            )]),
        }
    }

    /// Builds a lookup over one synthetic file and runs `body` against it.
    fn with_lookup<T>(
        source: &str,
        spans: &[(Span, &str)],
        body: impl FnOnce(&SemanticLookup<'_>) -> T,
    ) -> T {
        let facts = project(source);
        let entities = entity_symbols(spans);
        let ast_indexes = HashMap::new();
        let symbol_names = HashMap::new();
        let dialect = solid_dialect::Solid2;
        let contracts = crate::contracts::ResolvedContracts {
            bindings: Vec::new(),
            by_symbol: HashMap::new(),
            callee_bindings: HashMap::new(),
            returned_callable_bindings: HashSet::new(),
            missing_exports: Vec::new(),
            counts: crate::ContractBindingCounts::default(),
        };
        let lookup = SemanticLookup::new(
            &facts,
            &ast_indexes,
            &entities,
            &symbol_names,
            &dialect,
            &contracts,
            false,
        );
        body(&lookup)
    }

    #[test]
    fn returned_member_callee_bindings_are_exact_and_instance_local() {
        let source = "const first = make(); const second = make(); first.clear(); second.clear();";
        let facts = project(source);
        let entities = entity_symbols(&[]);
        let ast_indexes = facts
            .files
            .iter()
            .map(|file| (file.path.clone(), CachedAstFileIndex::new(file)))
            .collect::<HashMap<_, _>>();
        let symbol_names = HashMap::new();
        let dialect = solid_dialect::Solid2;
        let first = span_of(source, "first.clear", 0);
        let second = span_of(source, "second.clear", 0);
        let contracts = crate::contracts::ResolvedContracts {
            bindings: vec![],
            by_symbol: HashMap::new(),
            callee_bindings: HashMap::from([
                (
                    crate::location(facts.files[0].path.shared(), first),
                    SymbolId::from("first-instance"),
                ),
                (
                    crate::location(facts.files[0].path.shared(), second),
                    SymbolId::from("second-instance"),
                ),
            ]),
            returned_callable_bindings: HashSet::new(),
            missing_exports: vec![],
            counts: crate::ContractBindingCounts::default(),
        };
        let lookup = SemanticLookup::new(
            &facts,
            &ast_indexes,
            &entities,
            &symbol_names,
            &dialect,
            &contracts,
            false,
        );
        assert_eq!(
            lookup.callee_symbol(&facts.files[0], first),
            Some("first-instance")
        );
        assert_eq!(
            lookup.callee_symbols(&facts.files[0], first),
            vec![SymbolId::from("first-instance")]
        );
        assert_eq!(
            lookup.callee_symbol(&facts.files[0], second),
            Some("second-instance")
        );
        assert_eq!(
            lookup.callee_symbols(&facts.files[0], second),
            vec![SymbolId::from("second-instance")]
        );
        assert_eq!(
            lookup.callee_symbol(&facts.files[0], span_of(source, "make", 0)),
            None
        );
    }

    /// Call sites keyed by target function span, as `(callee start, end)`.
    /// One entry per invocation, which is what every consumer but the escape
    /// test reads.
    fn call_sites(source: &str, spans: &[(Span, &str)]) -> Vec<(Span, Vec<(u32, u32)>)> {
        with_lookup(source, spans, |lookup| {
            let mut sites = lookup
                .all_function_call_sites()
                .iter()
                .map(|((_, function), sites)| {
                    let mut callees = sites
                        .iter()
                        .map(|site| (site.callee.start, site.callee.end))
                        .collect::<Vec<_>>();
                    callees.sort_unstable();
                    (*function, callees)
                })
                .collect::<Vec<_>>();
            sites.sort_by_key(|(function, _)| (function.start, function.end));
            sites
        })
    }

    /// Every reference to `function` that its call sites account for — the set
    /// the escape test in `attribution` tests membership in.
    fn accounted_references(
        source: &str,
        spans: &[(Span, &str)],
        function: Span,
    ) -> Vec<(u32, u32)> {
        with_lookup(source, spans, |lookup| {
            let mut references = lookup
                .function_call_site_references(PATH, function)
                .into_iter()
                .map(|(_, span)| (span.start, span.end))
                .collect::<Vec<_>>();
            references.sort_unstable();
            references
        })
    }

    #[test]
    fn a_jsx_tag_resolving_to_a_project_function_is_a_call_site() {
        let source =
            "function Panel() {\n  return <p />;\n}\nfunction App() {\n  return <Panel />;\n}\n";
        let declaration = span_of(source, "Panel", 0);
        let tag = span_of(source, "Panel", 1);
        let panel = span_of(source, "function Panel() {\n  return <p />;\n}", 0);

        assert_eq!(
            call_sites(source, &[(declaration, "panel"), (tag, "panel")]),
            vec![(panel, vec![(tag.start, tag.end)])]
        );
    }

    #[test]
    fn a_component_used_as_a_value_is_not_a_call_site() {
        // The rendered tag is `Wrap`, and only `Wrap` gains the edge: `Panel`
        // appears as an attribute value, which renders nothing by itself.
        let source = concat!(
            "function Panel() {\n  return <p />;\n}\n",
            "function Wrap(props) {\n  return <p />;\n}\n",
            "function App() {\n  return <Wrap child={Panel} />;\n}\n",
        );
        let panel_declaration = span_of(source, "Panel", 0);
        let panel_value = span_of(source, "Panel", 1);
        let wrap_declaration = span_of(source, "Wrap", 0);
        let wrap_tag = span_of(source, "Wrap", 1);
        let wrap = span_of(source, "function Wrap(props) {\n  return <p />;\n}", 0);

        assert_eq!(
            call_sites(
                source,
                &[
                    (panel_declaration, "panel"),
                    (panel_value, "panel"),
                    (wrap_declaration, "wrap"),
                    (wrap_tag, "wrap"),
                ]
            ),
            vec![(wrap, vec![(wrap_tag.start, wrap_tag.end)])]
        );
    }

    #[test]
    fn a_resolvable_dotted_tag_is_an_edge_whose_callee_is_the_whole_name() {
        // TypeScript reports the component's symbol at both the whole
        // `ns.Panel` name and the `Panel` property inside it, so the tag does
        // resolve and the edge is emitted — with the whole name as its callee,
        // because that is the span the tag names. The property reference is a
        // *different* span, which is why the escape test in `attribution`
        // still reports the enumeration incomplete for a dotted render: that
        // test is byte-exact span membership, not a containment check.
        let source =
            "function Panel() {\n  return <p />;\n}\nfunction App() {\n  return <ns.Panel />;\n}\n";
        let declaration = span_of(source, "Panel", 0);
        let whole_name = span_of(source, "ns.Panel", 0);
        let property = span_of(source, "Panel", 1);
        let panel = span_of(source, "function Panel() {\n  return <p />;\n}", 0);

        assert_eq!(
            call_sites(
                source,
                &[
                    (declaration, "panel"),
                    (whole_name, "panel"),
                    (property, "panel"),
                ]
            ),
            vec![(panel, vec![(whole_name.start, whole_name.end)])]
        );
        assert_eq!(
            accounted_references(
                source,
                &[
                    (declaration, "panel"),
                    (whole_name, "panel"),
                    (property, "panel"),
                ],
                panel
            ),
            vec![(whole_name.start, whole_name.end)],
            "the property reference the escape test walks is not accounted for"
        );
    }

    #[test]
    fn a_dotted_tag_without_an_exact_whole_name_entity_is_not_a_call_site() {
        // The counterfactual to the test above: no entity at the whole name,
        // as for a namespace object TypeScript could not resolve. The tag
        // names no proven declaration, so the graph keeps failing closed
        // instead of inventing a caller.
        let source =
            "function Panel() {\n  return <p />;\n}\nfunction App() {\n  return <ns.Panel />;\n}\n";
        let declaration = span_of(source, "Panel", 0);
        let property = span_of(source, "Panel", 1);

        assert!(call_sites(source, &[(declaration, "panel"), (property, "panel")]).is_empty());
    }

    #[test]
    fn a_closing_tag_is_accounted_for_by_the_opening_tags_edge() {
        // `<Panel></Panel>` renders `Panel` once and writes its name twice.
        // TypeScript reports both occurrences as references to the same
        // symbol, so the escape test has to account for both — but the call
        // graph must still hold exactly one edge, because there is one
        // invocation.
        let source = "function Panel() {\n  return <p />;\n}\nfunction App() {\n  return <Panel></Panel>;\n}\n";
        let declaration = span_of(source, "Panel", 0);
        let opening = span_of(source, "Panel", 1);
        let closing = span_of(source, "Panel", 2);
        let panel = span_of(source, "function Panel() {\n  return <p />;\n}", 0);
        let entities = [
            (declaration, "panel"),
            (opening, "panel"),
            (closing, "panel"),
        ];

        assert_eq!(
            call_sites(source, &entities),
            vec![(panel, vec![(opening.start, opening.end)])],
            "one render is one edge"
        );
        assert_eq!(
            accounted_references(source, &entities, panel),
            vec![(opening.start, opening.end), (closing.start, closing.end)],
            "both occurrences of the tag name are accounted for"
        );
    }

    #[test]
    fn a_closing_tag_cannot_account_for_a_reference_on_its_own() {
        // The invariant behind the test above: the closing name rides on the
        // edge the *opening* tag's resolution created. With no entity at the
        // opening name there is no edge, so the closing name accounts for
        // nothing — a closing tag can never be the thing that makes a
        // reference look like a call.
        let source = "function Panel() {\n  return <p />;\n}\nfunction App() {\n  return <Panel></Panel>;\n}\n";
        let declaration = span_of(source, "Panel", 0);
        let closing = span_of(source, "Panel", 2);
        let panel = span_of(source, "function Panel() {\n  return <p />;\n}", 0);
        let entities = [(declaration, "panel"), (closing, "panel")];

        assert!(call_sites(source, &entities).is_empty());
        assert!(accounted_references(source, &entities, panel).is_empty());
    }

    /// Both directions of the alias gate, so neither forcing can survive.
    ///
    /// Forcing it true republishes the claim `@solidjs/router`'s `Navigate`
    /// made: the destructured `href` *is* `props`, so a call of `href` reads as
    /// a call of the props object. Forcing it false drops the identifier alias,
    /// so `alias()` stops resolving to `handler` and every claim about the
    /// aliased callable disappears.
    #[test]
    fn only_an_identifier_binding_inherits_its_initializer_identity() {
        let source = concat!(
            "const alias = handler;\n",
            "const { href } = props;\n",
            "alias();\n",
            "href();\n",
        );
        let entities = [
            (span_of(source, "alias", 0), "symbol:alias"),
            (span_of(source, "alias", 1), "symbol:alias"),
            (span_of(source, "handler", 0), "symbol:handler"),
            (span_of(source, "href", 0), "symbol:href"),
            (span_of(source, "href", 1), "symbol:href"),
            (span_of(source, "props", 0), "symbol:props"),
        ];
        let (aliased, destructured) = with_lookup(source, &entities, |lookup| {
            let file = &lookup.facts.files[0];
            let mut visited = HashSet::new();
            let aliased =
                lookup.direct_value_symbols(file, span_of(source, "alias", 1), &mut visited);
            let mut visited = HashSet::new();
            let destructured =
                lookup.direct_value_symbols(file, span_of(source, "href", 1), &mut visited);
            (aliased, destructured)
        });
        assert_eq!(
            aliased,
            vec![SymbolId::from("symbol:handler")],
            "an identifier binding is an alias of its initializer"
        );
        assert_eq!(
            destructured,
            vec![SymbolId::from("symbol:href")],
            "a destructured local is its own binding, never the object it came from"
        );
    }

    /// The receiver symbol and access path `member_callee_receiver` answers
    /// for `callee`.
    ///
    /// This builds its own lookup because `with_lookup` deliberately carries
    /// no AST indexes, and the member walk reads property spans out of one.
    fn member_callee_path(
        source: &str,
        callee: Span,
        spans: &[(Span, &str)],
    ) -> Option<(String, Vec<String>)> {
        let facts = project(source);
        let ast_indexes = facts
            .files
            .iter()
            .map(|file| (file.path.clone(), CachedAstFileIndex::new(file)))
            .collect::<HashMap<_, _>>();
        let entities = entity_symbols(spans);
        let symbol_names = HashMap::new();
        let dialect = solid_dialect::Solid2;
        let contracts = crate::contracts::ResolvedContracts {
            bindings: Vec::new(),
            by_symbol: HashMap::new(),
            callee_bindings: HashMap::new(),
            returned_callable_bindings: HashSet::new(),
            missing_exports: Vec::new(),
            counts: crate::ContractBindingCounts::default(),
        };
        let lookup = SemanticLookup::new(
            &facts,
            &ast_indexes,
            &entities,
            &symbol_names,
            &dialect,
            &contracts,
            false,
        );
        lookup
            .member_callee_receiver(&facts.files[0], callee)
            .map(|(receiver, path)| (receiver.to_string(), path))
    }

    /// `props` and the body reference to it, the entity pair every chain
    /// rooted at the first parameter needs.
    fn props_entities(source: &str) -> Vec<(Span, &'static str)> {
        vec![
            (span_of(source, "props", 0), "symbol:props"),
            (span_of(source, "props", 1), "symbol:props"),
        ]
    }

    /// The whole chain, not its last segment.
    ///
    /// Reverting the walk to the pre-fix rooting — resolve the immediate
    /// object, report one property — answers `["includes"]` here, a property
    /// of `props.modifiers` attributed to `props`. That claim can never be
    /// witnessed, because a consumer matches the stated path as a *prefix* of
    /// the observed access.
    #[test]
    fn a_member_chain_reads_the_whole_path_from_its_root() {
        let source = "function f(props, m) {\n  return props.modifiers.includes(m);\n}\n";
        let callee = span_of(source, "props.modifiers.includes", 0);

        assert_eq!(
            member_callee_path(source, callee, &props_entities(source)),
            Some((
                "symbol:props".to_owned(),
                vec!["modifiers".to_owned(), "includes".to_owned()]
            ))
        );
    }

    #[test]
    fn a_one_segment_chain_reads_that_one_property() {
        let source = "function f(props) {\n  return props.of();\n}\n";
        let callee = span_of(source, "props.of", 0);

        assert_eq!(
            member_callee_path(source, callee, &props_entities(source)),
            Some(("symbol:props".to_owned(), vec!["of".to_owned()]))
        );
    }

    /// A segment that cannot be named exactly cuts the path back to the
    /// longest exact prefix *from the root*, and never invents the segments
    /// outside it.
    #[test]
    fn an_unnameable_segment_truncates_to_the_prefix_from_the_root() {
        let inside = "function f(props, key) {\n  return props.of[key].values();\n}\n";
        let at_root = "function f(props, key) {\n  return props[key].values();\n}\n";

        assert_eq!(
            member_callee_path(
                inside,
                span_of(inside, "props.of[key].values", 0),
                &props_entities(inside)
            ),
            Some(("symbol:props".to_owned(), vec!["of".to_owned()])),
            "the segment outside the computed one is not a property of `props`"
        );
        assert_eq!(
            member_callee_path(
                at_root,
                span_of(at_root, "props[key].values", 0),
                &props_entities(at_root)
            ),
            Some(("symbol:props".to_owned(), Vec::new())),
            "nothing can be named, so the row claims only a read through `props`"
        );
    }

    /// A chain deeper than the path limit keeps its row, cut to the limit.
    ///
    /// Dropping it would publish the negative claim "this export performs no
    /// parameter read" into a `Complete` reads set. The pair pins both sides
    /// of the boundary so neither a smaller limit nor a reinstated refusal
    /// survives.
    #[test]
    fn an_over_long_chain_is_truncated_and_never_dropped() {
        let chain = |segments: usize| {
            let names = (0..segments)
                .map(|index| format!("s{index}"))
                .collect::<Vec<_>>();
            let source = format!(
                "function f(props) {{\n  return props.{}();\n}}\n",
                names.join(".")
            );
            let callee_text = format!("props.{}", names.join("."));
            let entities = props_entities(&source);
            let callee = span_of(&source, &callee_text, 0);
            let answer = member_callee_path(&source, callee, &entities);
            (names, answer)
        };

        let (exact_names, exact) = chain(MEMBER_CALLEE_PATH_LIMIT);
        assert_eq!(
            exact,
            Some(("symbol:props".to_owned(), exact_names)),
            "a chain at the limit is reported whole"
        );

        let (over_names, over) = chain(MEMBER_CALLEE_PATH_LIMIT + 1);
        assert_eq!(
            over,
            Some((
                "symbol:props".to_owned(),
                over_names[..MEMBER_CALLEE_PATH_LIMIT].to_vec()
            )),
            "one segment past the limit keeps the row at the prefix from the root"
        );
    }

    /// A chain whose root is a compound expression publishes nothing.
    ///
    /// The compiler's entity table answers a symbol at a conditional,
    /// sequence, or logical span — the *leftmost operand's*. Trusting it makes
    /// `k`, a boolean condition, read `["c", "slice"]`: every segment is a
    /// property of the chain's result, and none is a property of `k`. Each
    /// case registers that entity, so removing the identifier gate makes this
    /// test report the fabricated row rather than `None`.
    #[test]
    fn a_compound_chain_root_is_refused_rather_than_attributed() {
        let conditional =
            "function f(props, k, n) {\n  return (k ? props.a : props.b).c.slice(n);\n}\n";
        let sequence = "function f(props, k, n) {\n  return (k, props.a).c.slice(n);\n}\n";
        let logical =
            "function f(props, fallback, n) {\n  return (props.a || fallback).c.slice(n);\n}\n";

        assert_eq!(
            member_callee_path(
                conditional,
                span_of(conditional, "(k ? props.a : props.b).c.slice", 0),
                &[
                    (span_of(conditional, "k ? props.a : props.b", 0), "symbol:k"),
                    (span_of(conditional, "k", 1), "symbol:k"),
                ]
            ),
            None,
            "a ternary's condition does not own the ternary result's properties"
        );
        assert_eq!(
            member_callee_path(
                sequence,
                span_of(sequence, "(k, props.a).c.slice", 0),
                &[
                    (span_of(sequence, "k, props.a", 0), "symbol:k"),
                    (span_of(sequence, "k", 1), "symbol:k"),
                ]
            ),
            None,
            "a discarded sequence operand does not own the result's properties"
        );
        assert_eq!(
            member_callee_path(
                logical,
                span_of(logical, "(props.a || fallback).c.slice", 0),
                &[
                    (span_of(logical, "props.a || fallback", 0), "symbol:props"),
                    (span_of(logical, "props", 1), "symbol:props"),
                ]
            ),
            None,
            "either branch of a logical may supply the value the chain walks"
        );
    }

    /// `props().slice(n)` names a property of the *call's result*.
    ///
    /// The call is not the parameter, so the row that would be published here
    /// says `props` has a `slice` property when nothing establishes that.
    #[test]
    fn a_call_at_the_chain_root_is_refused() {
        let source = "function f(props, n) {\n  return props().slice(n);\n}\n";

        assert_eq!(
            member_callee_path(
                source,
                span_of(source, "props().slice", 0),
                &[
                    (span_of(source, "props()", 0), "symbol:props"),
                    (span_of(source, "props", 1), "symbol:props"),
                ]
            ),
            None
        );
    }

    /// A callee that is not a member at all, and one whose own property is
    /// computed, are both refused — the two gates that keep an ordinary call
    /// from reading as a member access.
    #[test]
    fn a_non_member_or_computed_callee_is_refused() {
        let plain = "function f(notify, cb) {\n  return notify(cb);\n}\n";
        let computed = "function f(props, key) {\n  return props.of[key]();\n}\n";

        assert_eq!(
            member_callee_path(
                plain,
                span_of(plain, "notify", 1),
                &[
                    (span_of(plain, "notify", 0), "symbol:notify"),
                    (span_of(plain, "notify", 1), "symbol:notify"),
                ]
            ),
            None,
            "a call of a value is not a call through a property of one"
        );
        assert_eq!(
            member_callee_path(
                computed,
                span_of(computed, "props.of[key]", 0),
                &props_entities(computed)
            ),
            None,
            "`handlers[i]()` must never be read as a property named `i`"
        );
    }

    #[test]
    fn a_renderer_declaration_must_sit_in_a_dialect_package() {
        let packages = ["solid-js", "@solidjs/signals", "@solidjs/web"];
        for inside in [
            "/p/node_modules/solid-js/types/client/component.d.ts",
            "/p/node_modules/@solidjs/web/types/client.d.ts",
            r"C:\p\node_modules\@solidjs\web\types\client.d.ts",
        ] {
            assert!(declared_in_package(inside, &packages), "{inside}");
        }
        for outside in [
            "/p/my-solid-js-tools/component.d.ts",
            "/p/node_modules/@solidjs/router/dist/index.d.ts",
            "/p/node_modules/web/solid.d.ts",
            "/p/node_modules/not-@solidjs/web/index.d.ts",
        ] {
            assert!(!declared_in_package(outside, &packages), "{outside}");
        }
    }
}
