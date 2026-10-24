//! Read-optimized project indexes used by every analysis stage.
//!
//! This module hides AST and TypeScript table layout from rule discovery. The
//! builder asks semantic questions here instead of repeatedly scanning facts.

use crate::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};

use sha2::{Digest, Sha256};
use solid_facts::core::Span;
use solid_facts::{FileFacts, ProjectFacts, TypeScriptSymbol, TypeScriptTable};
use typefacts::{
    Callability, EntityFact, FileFact, Location, ResolvedCall, ResolvedCallValidity, TypeDescriptor,
};

use super::{SymbolId, SymbolName};
use crate::owners::function_binding_name;

mod callees;
mod census;
pub(crate) use callees::{
    function_value_is_current_in, plain_callee_symbol, resolved_callee_call_in,
    standard_library_declaration_of,
};
pub(crate) use census::{
    CENSUS_TABLES_FROM, CallPrimitives, CallSite, CallSitesOf, CensusTables, ComponentIdentity,
    FileComponentStatuses, LazyWitnessesIn, PropRenderSitesIn, ResultAccessForwardedAll,
    TagRendered,
};
mod class_prototype;
mod closed_members;
mod completion;
mod components;
pub(crate) use components::unproven_component_status;
mod compound;
mod event_delivery;
mod identity_wrappers;
mod memo_probes;
mod returned_invocations;
mod returned_uses;
pub(crate) use identity_wrappers::{
    IdentityWrapper, WrapperAt, identity_wrapped_literal_span, identity_wrapper_candidates,
};
pub(crate) use memo_probes::{BindingResolutionRows, probing};
pub(crate) use returned_invocations::{INVOCATION_TABLES_FROM, InvocationTables};
pub(crate) use returned_uses::{RETURNED_TABLES_FROM, ReturnedTables, UseKind};
mod initialization;
mod intrinsics;
mod member_values;
mod primitive_proof;
mod project_memos;
pub(crate) use intrinsics::mentions_word;
pub(crate) use project_memos::{
    EffectApplyCalls, EffectApplyRegions, PROJECT_TABLES_FROM, ProjectTables, RouterParams,
    StringCodeVeto, TupleIterationVeto,
};

/// What every reflective definer spelling in the project says about the key
/// it defines: `any` when some spelling may define any name, otherwise only
/// the names in `sole_keys`.
#[derive(Default)]
pub(super) struct ReflectiveDefinitionNames {
    any: bool,
    sole_keys: HashSet<String>,
}

#[derive(Default)]
pub(super) struct BindingDeclarations<'a> {
    initialized_by_symbol: HashMap<(&'a str, &'a SymbolId), &'a solid_facts::ast::BindingFact>,
    by_name_span: HashMap<(&'a str, Span), &'a solid_facts::ast::BindingFact>,
}

/// Per-build cache of `returned_callback_invocation_sites` per factory call:
/// the uses are a memo ([`ReturnedUses`](returned_uses::ReturnedUses)), what
/// each inherits is assembled from lookup tables that are not memos yet.
pub(super) type ReturnedInvocationSites = std::sync::Mutex<
    HashMap<
        (String, Span),
        (
            std::sync::Arc<Vec<crate::owners::ReturnedCallbackInvocationSite>>,
            crate::base::reads::Reads,
        ),
    >,
>;

/// Functions by `(path, span)`: the proven components, the escaped ones among
/// them ([`SemanticLookup::escaped_component_functions`]), and the called
/// functions that may also be rendered
/// ([`SemanticLookup::possible_component_functions`]).
pub(super) type ComponentSets<'a> = (
    HashSet<(&'a str, Span)>,
    HashSet<(&'a str, Span)>,
    HashSet<(&'a str, Span)>,
);

/// Whether the reference at `span` is the declaration that introduces a
/// binding or the import/export surface that forwards it. Neither is a runtime
/// entry: emission resolves the export surface itself, by name.
///
/// Only the export *specifier* counts: `export { Panel }`, or the name a
/// declaration export introduces. An `ExportNamedDeclaration`'s own span
/// covers the whole declaration, body included (solid-facts
/// `visit_export_named_declaration`), so testing containment accepted every
/// reference written inside an exported function: `apply(Panel)` and
/// `return Panel` read as "export surface", and the escape they prove was
/// lost. That is the unsound direction: callers read a true verdict as "the
/// enumerated entries are all of them".
///
/// A dotted tag stays unaccounted for. `<ns.Panel/>` resolves at the whole
/// `ns.Panel` name span, but the reference walked is the `Panel` property
/// inside it, so it matches no surface here. That is the conservative
/// direction; closing it is a resolution question rather than a widening one.
pub(crate) fn reference_is_declaration_or_module_surface(file: &FileFacts, span: Span) -> bool {
    file.ast
        .functions
        .iter()
        .any(|function| function.name.as_ref().is_some_and(|name| name.span == span))
        || file
            .ast
            .bindings
            .iter()
            .any(|binding| binding.names.iter().any(|name| name.span == span))
        || file.ast.exports.iter().any(|export| {
            export
                .specifiers
                .iter()
                .chain(export.declarations.iter())
                .any(|specifier| specifier.local.span == span)
        })
        || file
            .ast
            .imports
            .iter()
            .any(|import| import.span.contains(span))
}

/// The string a computed key names when it is a `const` bound to a string
/// literal in the same file (`const key = "run"; c[key] = f`).
pub(crate) fn constant_string_key(file: &FileFacts, key: Span) -> Option<String> {
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
/// nothing unless its file also holds one of these. The interprocedural
/// result cache counts such a file as call-role syntax
/// (`interproc::call_role_syntax`); a chunk records the proof's answers it
/// reads instead (ADR 0392, [`crate::base::reads::Section::ReturnedSources`]).
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
    binding_written_by(file, declaration, false)
}

/// [`binding_written`], except that a member write whose object is wrapped in
/// TypeScript (`(c as X).p = v`, `c!.p = v`) writes a member of the value, not
/// the binding, as the plain `c.p = v` does. Identity wrappers use this reading;
/// the other binding analyses keep the conservative one above.
pub(super) fn binding_written_through_wrappers(file: &FileFacts, declaration: Span) -> bool {
    binding_written_by(file, declaration, true)
}

fn binding_written_by(file: &FileFacts, declaration: Span, through_wrappers: bool) -> bool {
    // Asked again and again for the same declaration, and each answer reads
    // the whole file, so it is remembered with the file's span index.
    file.ast
        .span_index()
        .flag(u8::from(through_wrappers), declaration, || {
            compute_binding_written_by(file, declaration, through_wrappers)
        })
}

fn compute_binding_written_by(file: &FileFacts, declaration: Span, through_wrappers: bool) -> bool {
    // A direct `eval` reaches every binding visible where it is written and
    // can rebind it (`eval("go = () => {}")`) with no static reference.
    if crate::interproc::property_gets::direct_eval_in(file, Span::new(0, u32::MAX)) {
        return true;
    }
    // A redeclaration (`var xs = …` over a parameter, a second `function f`)
    // writes the same binding without any reference. Any other declaration
    // of the same name in the scope counts, a shadowing one included.
    let name = file.source_text(declaration);
    let scope = file
        .ast
        .innermost_function_containing(declaration)
        .map_or(Span::new(0, u32::MAX), |function| function.span);
    // `bindings` is sorted by declaration span, so those inside `scope` start
    // at the first declaration that does not start before it.
    let first_in_scope = file
        .ast
        .bindings
        .partition_point(|binding| binding.declaration.start < scope.start);
    let redeclared = file.ast.bindings[first_in_scope..]
        .iter()
        .take_while(|binding| binding.declaration.start <= scope.end)
        .any(|binding| {
            scope.contains(binding.declaration)
                && binding
                    .names
                    .iter()
                    .any(|other| other.span != declaration && file.source_text(other.span) == name)
        })
        || file.ast.functions_within(scope).any(|function| {
            function.kind == solid_facts::ast::FunctionKind::Declaration
                && function.name.as_ref().is_some_and(|other| {
                    other.span != declaration && file.source_text(other.span) == name
                })
        });
    if redeclared {
        return true;
    }
    file.ast.references_to(declaration).iter().any(|reference| {
        file.ast.any_write_target_containing(*reference)
            && !if through_wrappers {
                file.ast.member_with_peeled_object(*reference).is_some()
            } else {
                file.ast.any_member_with_object(*reference)
            }
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
    /// The entries of `by_path` whose symbol names the whole span
    /// (`EntityFact::symbol_covers_span`).
    pub(super) exact_by_path: HashMap<String, HashMap<(u64, u64), SymbolId>>,
}

impl EntitySymbols {
    /// The symbol of the entity at `location`, whatever its span. At a call
    /// span this is the callee's; at a member span, the object's. Use it for a
    /// callee or a declaration position only.
    pub(super) fn get(&self, location: &Location) -> Option<&SymbolId> {
        self.by_path
            .get(location.path.as_ref())
            .and_then(|entities| entities.get(&(location.start_byte, location.end_byte)))
    }

    /// The symbol that is the value at exactly `location`: an entity whose
    /// symbol names the whole span. A callback's or a value's identity is read
    /// here, so a call or member span never answers with a token inside it.
    pub(super) fn exact(&self, location: &Location) -> Option<&SymbolId> {
        self.exact_by_path
            .get(location.path.as_ref())
            .and_then(|entities| entities.get(&(location.start_byte, location.end_byte)))
    }

    pub(super) fn at(&self, path: &str, span: Span) -> Option<&SymbolId> {
        self.by_path
            .get(path)
            .and_then(|entities| entities.get(&(u64::from(span.start), u64::from(span.end))))
    }

    /// The symbol of the value `span` denotes, when that value is one identifier
    /// (ADR 0346). TypeScript wrappers keep the value (`h as X`, `h!`,
    /// `h satisfies X`), so the identity is the identifier's symbol:
    ///
    /// - the covering symbol at the peeled span, when the entity exists there;
    /// - else, when the peeled span is a single identifier that starts at the
    ///   span's first byte, the symbol at the span itself. An entity names the
    ///   symbol at its first byte, and that token is the identifier.
    ///
    /// A call, a member or any other expression answers with nothing: the
    /// token at its first byte is a callee or an object, not the value.
    pub(crate) fn value_symbol(
        &self,
        file: &solid_facts::FileFacts,
        span: Span,
    ) -> Option<&SymbolId> {
        let path = file.path.as_str();
        let peeled = file.ast.peel_ts_sugar_span(span);
        if let Some(symbol) = self
            .exact_by_path
            .get(path)
            .and_then(|entities| entities.get(&(u64::from(peeled.start), u64::from(peeled.end))))
        {
            return Some(symbol);
        }
        if peeled.start != span.start || !crate::owners::is_identifier_span(file, peeled) {
            return None;
        }
        self.at(path, span)
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
    let mut by_location = DeclarationSymbols::default();
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
        let mut calls_by_span = HashMap::default();
        let mut calls_by_callee = HashMap::<Span, Vec<_>>::default();
        let mut direct_calls_by_callee = HashMap::default();
        for (index, call) in file.ast.calls.iter().enumerate() {
            calls_by_span.entry(call.span).or_insert(index);
            calls_by_callee.entry(call.callee).or_default().push(index);
            if call.direct_callee {
                direct_calls_by_callee.entry(call.callee).or_insert(index);
            }
        }
        let mut functions_by_span = HashMap::default();
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

type BindingsByReference = HashMap<String, HashMap<(u64, u64), BindingResolution>>;

/// One argument a project wrapper forwards into a result-access slot
/// ([`SemanticLookup::result_access_forwarded_arguments`]).
#[derive(Clone, Copy, Debug)]
pub(super) struct ResultAccessForwarded {
    /// The argument at the wrapper's call site.
    pub(super) argument: Span,
    /// The primitive whose slot the wrapper forwards it into.
    pub(super) primitive: solid_dialect::Primitive,
}

/// The entity at each exact `(path, start_byte, end_byte)`: the first in
/// `entities`' order when several share one location.
pub(crate) fn first_entity_by_location<'a>(
    entities: impl Iterator<Item = &'a EntityFact>,
) -> HashMap<(&'a str, u64, u64), &'a EntityFact> {
    let mut map = HashMap::default();
    for entity in entities {
        map.entry((
            entity.location.path.as_ref(),
            entity.location.start_byte,
            entity.location.end_byte,
        ))
        .or_insert(entity);
    }
    map
}

/// Lazy project-wide lookups that replace repeated whole-project scans.
///
/// Every map is built at most once per build, on first use, in the exact
/// file/declaration order the scans it replaces used, so first-match and
/// first-writer results are unchanged. Warm builds that never ask a question
/// never pay for an index.
pub(super) struct SemanticLookup<'a> {
    object_members: OnceLock<crate::object_members::Targets>,
    /// ADR 0358: the handlers each intrinsic element's spread supplies.
    spread_handlers: OnceLock<crate::object_members::SpreadTargets>,
    /// Every member whose receiver is bound to an exact object literal, and
    /// the ones whose literal is declared in another file.
    literal_receiver_members: OnceLock<crate::object_members::LiteralReceiverCensus>,
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
    functions_by_symbol: crate::base::reads::Recorded<HashMap<&'a str, SymbolFunction>>,
    /// L2: the memo tables ([`crate::base::memo`]); ADR 0346's identity
    /// wrappers are the first.
    memos: crate::base::memo::build::LookupMemos<'a>,
    /// ADR 0319: every literal's invocation-flow answer.
    invocation_flow_answers: OnceLock<HashMap<(String, Span), crate::ExecutionRole>>,
    /// The literals whose every invocation is a host event dispatch.
    event_invoked_literals: OnceLock<HashSet<(String, Span)>>,
    /// ADR 0292: whether some file holds a reflective write through a
    /// `prototype`.
    reflective_prototype_write: crate::base::reads::Recorded<bool>,
    /// ADR 0320: whether some call may copy keys into an existing object.
    objects_assigned_into: crate::base::reads::Recorded<bool>,
    router_params: crate::base::reads::Recorded<crate::router_params::RouterParamsStatus>,
    /// ADR 0334: the route option values the audited router renders.
    route_component_values: crate::base::reads::Recorded<HashSet<(String, Span)>>,
    /// ADR 0209: the property names some assignment in the project writes
    /// (`x.name = …`), and which prototypes it writes through (ADR 0314).
    assigned_member_names: crate::base::reads::Recorded<AssignedMembers>,
    /// [`Self::property_may_be_defined_reflectively_as`], summarised once
    /// over the project so each per-name query is a set lookup.
    reflective_definition_names: crate::base::reads::Recorded<ReflectiveDefinitionNames>,
    /// Per file path: the first binding with an initializer that declares each
    /// symbol, and the first binding declaring a name at each span.
    binding_declarations: crate::base::reads::Recorded<BindingDeclarations<'a>>,
    /// The invocation sites of each factory call's returned function, by
    /// file path and call span. Several passes ask the same factory call.
    returned_invocation_sites: ReturnedInvocationSites,
    /// This build's views of the returned-invocation and returned-source memos.
    invocation_answers: returned_invocations::State,
    /// ADR 0349: the Promise and array-iteration premises, by their tripwires.
    intrinsics: crate::base::reads::Recorded<intrinsics::Intrinsics>,
    /// ADR 0351: each member read that names a closed object literal's function.
    closed_members: crate::base::reads::Recorded<HashMap<(&'a str, Span), Span>>,
    entities_by_location:
        crate::base::reads::Recorded<HashMap<(&'a str, u64, u64), &'a EntityFact>>,
    contained_entities_by_path: crate::base::reads::Recorded<HashMap<&'a str, Vec<&'a EntityFact>>>,
    descriptors_by_symbol: crate::base::reads::Recorded<HashMap<&'a str, &'a TypeDescriptor>>,
    callability_by_symbol: crate::base::reads::Recorded<HashMap<&'a str, Callability>>,
    symbols_by_id:
        crate::base::reads::Recorded<HashMap<&'a str, solid_facts::TypeScriptSymbol<'a>>>,
    context_symbols: crate::base::reads::Recorded<HashSet<&'a str>>,
    declaration_symbols: crate::base::reads::Recorded<DeclarationSymbols<'a>>,
    /// ADR 0339: references to function-valued bindings not proven to follow
    /// their declarators.
    initialization: initialization::State,
    /// [`crate::interproc::accessor_provenance::MemberStability::index`] of the lookup's facts.
    member_stability:
        crate::base::reads::Recorded<crate::interproc::accessor_provenance::MemberStability>,
    /// Study S5c: the component census's side of the lookup.
    census: census::State<'a>,
    direct_value_aliases: crate::base::reads::Recorded<HashSet<SymbolId>>,
    /// The closed program's namespace census (ADR 0347), set once per build.
    namespace_hand_outs: crate::base::reads::Recorded<crate::attribution::NamespaceHandOuts>,
    bindings_by_symbol: crate::base::reads::Recorded<HashMap<SymbolId, BindingResolution>>,
    bindings_by_reference: crate::base::reads::Recorded<BindingsByReference>,
    /// L0: the analyzed files by interned id ([`crate::base`]).
    file_table: crate::base::FileTable,
    /// Each path's TypeFacts references, read by the references index.
    reference_rows: OnceLock<crate::base::file_table::ReferenceRows>,
    /// L1: the base indexes, each folded on first read ([`crate::base`]).
    base: crate::base::BaseIndexes,
    file_primitives: OnceLock<Vec<crate::base::reads::Recorded<FilePrimitives>>>,
    file_named_callback_roles:
        OnceLock<Vec<crate::base::reads::Recorded<super::NamedCallbackRoles>>>,
    project_primitives: crate::base::reads::Recorded<HashSet<solid_dialect::Primitive>>,
    callback_capabilities: crate::base::reads::Recorded<DialectCallbackCapabilities>,
    returned_callback_proof_digest: OnceLock<Option<CrossFileProofDigest>>,
    project_has_component_type: crate::base::reads::Recorded<bool>,
    /// Proven components, then functions whose only missing proof is an
    /// escaping reference (`ComponentStatus::Uncertain`).
    component_functions: OnceLock<ComponentSets<'a>>,
    result_access_forwarded:
        crate::base::reads::Recorded<HashMap<&'a str, Vec<ResultAccessForwarded>>>,
    /// [`Self::section_fingerprint`], once per build.
    section_fingerprints: [OnceLock<[u8; 32]>; crate::base::reads::Section::ALL.len()],
    tuple_iteration_veto: crate::base::reads::Recorded<bool>,
    string_code_veto: crate::base::reads::Recorded<bool>,
    /// ADR 0277: `(path, function span, parameter index)` for each parameter
    /// a synchronous project function calls on its every-completion path.
    parameter_completion_calls: crate::base::reads::Recorded<HashSet<(&'a str, Span, usize)>>,
    /// ADR 0278: the calls TypeScript resolves to a signature returning
    /// `never`, or (ADR 0279) to an `asserts` signature, per path. Such a call never completes normally, so a caller
    /// that catches its throw carries on with the rest of the body skipped.
    never_returning_calls: OnceLock<HashMap<&'a str, Vec<Span>>>,
    /// ADR 0280: `(path, function span)` for each function that tests one
    /// of its own parameters in a condition or a logical operand, the only
    /// functions a call's arguments can decide anything in.
    parameter_testing_functions: crate::base::reads::Recorded<HashSet<(&'a str, Span)>>,
    /// ADR 0338: whether some analysed file may patch `Function.prototype.bind`.
    function_bind_patched: crate::base::reads::Recorded<bool>,
    effect_apply_regions: crate::base::reads::Recorded<crate::effect_execution::ApplyRegions>,
    effect_apply_calls: crate::base::reads::Recorded<crate::effect_execution::ApplyCalls<'a>>,
}

/// Which parts of the returned-adapter interface this build's dialect can
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
    /// ADR 0302: a view of the [`EffectApplyCalls`] memo, built once per build.
    pub(crate) fn effect_apply_calls(&self) -> &crate::effect_execution::ApplyCalls<'a> {
        self.effect_apply_calls.get_or_init(|| {
            self.whole_program_answer::<EffectApplyCalls>()
                .into_iter()
                .filter_map(|(path, span, argument, execution)| {
                    Some((
                        (self.file_by_path(&path)?.path.as_str(), span, argument),
                        execution,
                    ))
                })
                .collect()
        })
    }
    /// A view of the [`EffectApplyRegions`] memo, built once per build.
    pub(crate) fn effect_apply_regions(&self) -> &crate::effect_execution::ApplyRegions {
        self.effect_apply_regions.get_or_init(|| {
            self.whole_program_answer::<EffectApplyRegions>()
                .into_iter()
                .collect()
        })
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

    pub(crate) fn memo_store(&self) -> &crate::base::memo::build::MemoStore {
        self.memos.store()
    }

    /// Reads and records memos through a retained session's store instead of
    /// one of the lookup's own.
    #[must_use]
    pub(super) fn with_memo_store(
        mut self,
        store: &'a crate::base::memo::build::MemoStore,
    ) -> Self {
        self.memos = crate::base::memo::build::LookupMemos::shared(store, &self.file_table);
        self
    }

    /// The build as memo computations read it. The first call begins the
    /// build: the store's invalidation runs before any read.
    pub(crate) fn memo_world(&self) -> crate::base::memo::build::BuildWorld<'_> {
        debug_assert!(
            !probing(),
            "a memo probe read a memo: the build would begin from inside its own invalidation"
        );
        let redirects = !self.facts.runtime_symbol_redirects.is_empty();
        let world = crate::base::memo::build::BuildWorld::new(
            self.base_views_over(self.memos.table()),
            self.memos.store(),
            self.program_closed,
            self,
        )
        .with_project(self)
        .with_runtime_redirects(redirects);
        self.memos.begin_once(|| {
            self.memos.store().begin_with(
                world.views(),
                self.program_closed,
                self,
                crate::cache::typescript_unchanged(self.facts),
                redirects,
            )
        });
        world
    }

    /// A whole-program memo's answer. Inside a memo probe it is computed
    /// straight from this lookup instead ([`project_memos::WholeProgram`]):
    /// a probe is re-asked while a build begins, so it may not read a memo,
    /// and the direct answer is the memo's own function of the lookup.
    pub(crate) fn whole_program_answer<M: project_memos::WholeProgram>(&self) -> M::Value {
        if memo_probes::probing() {
            M::direct(self)
        } else {
            self.memo_world().answer::<M>(&())
        }
    }

    /// Ends the build's memo use ([`crate::base::memo::build::LookupMemos::end`]).
    pub(crate) fn end_memos(&self) -> Option<crate::base::memo::build::MemoReport> {
        self.memos.end()
    }

    /// Records the closed program's namespace census (ADR 0347). The pipeline
    /// sets it once per build, before any owner pass reads it.
    /// `reads` are what computing it read ([`crate::base::reads`]): a reader
    /// records them.
    pub(crate) fn set_namespace_hand_outs(
        &self,
        hand_outs: crate::attribution::NamespaceHandOuts,
        reads: crate::base::reads::Reads,
    ) {
        let _ = self.namespace_hand_outs.set(hand_outs, reads);
    }

    /// Whether the program names the function at `function` in the module at
    /// `path` as a value it hands out (ADR 0347, amendment): a static member read or
    /// a named dynamic-import read. Until the census is set, nothing is.
    pub(crate) fn hands_out_by_name(&self, path: &str, function: Span) -> bool {
        self.namespace_hand_outs
            .get()
            .is_some_and(|hand_outs| hand_outs.named(path, function))
    }

    /// Whether the module at `path`'s namespace escapes with no name the
    /// analysis can read (ADR 0347, amendment). Until the census is set, nothing does.
    pub(crate) fn escapes_unnamed(&self, path: &str) -> bool {
        self.namespace_hand_outs
            .get()
            .is_some_and(|hand_outs| hand_outs.escapes_unnamed(path))
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
        let file_table = crate::base::FileTable::new(&facts.files);
        Self {
            object_members: OnceLock::new(),
            spread_handlers: OnceLock::new(),
            literal_receiver_members: OnceLock::new(),
            facts,
            dialect,
            program_closed,
            router_install: crate::router_params::RouterInstallEvidence::default(),
            ast_indexes,
            entities,
            symbol_names,
            functions_by_symbol: crate::base::reads::Recorded::new(),
            memos: crate::base::memo::build::LookupMemos::owned(&file_table),
            invocation_flow_answers: OnceLock::new(),
            event_invoked_literals: OnceLock::new(),
            reflective_prototype_write: crate::base::reads::Recorded::new(),
            objects_assigned_into: crate::base::reads::Recorded::new(),
            router_params: crate::base::reads::Recorded::new(),
            route_component_values: crate::base::reads::Recorded::new(),
            assigned_member_names: crate::base::reads::Recorded::new(),
            reflective_definition_names: crate::base::reads::Recorded::new(),
            returned_invocation_sites: ReturnedInvocationSites::default(),
            invocation_answers: returned_invocations::State::default(),
            binding_declarations: crate::base::reads::Recorded::new(),
            intrinsics: crate::base::reads::Recorded::new(),
            closed_members: crate::base::reads::Recorded::new(),
            entities_by_location: crate::base::reads::Recorded::new(),
            contained_entities_by_path: crate::base::reads::Recorded::new(),
            descriptors_by_symbol: crate::base::reads::Recorded::new(),
            callability_by_symbol: crate::base::reads::Recorded::new(),
            symbols_by_id: crate::base::reads::Recorded::new(),
            context_symbols: crate::base::reads::Recorded::new(),
            declaration_symbols: crate::base::reads::Recorded::new(),
            initialization: initialization::State::default(),
            member_stability: crate::base::reads::Recorded::new(),
            census: census::State::default(),
            direct_value_aliases: crate::base::reads::Recorded::new(),
            namespace_hand_outs: crate::base::reads::Recorded::new(),
            bindings_by_symbol: crate::base::reads::Recorded::new(),
            bindings_by_reference: crate::base::reads::Recorded::new(),
            file_table,
            reference_rows: OnceLock::new(),
            base: crate::base::BaseIndexes::default(),
            file_primitives: OnceLock::new(),
            file_named_callback_roles: OnceLock::new(),
            project_primitives: crate::base::reads::Recorded::new(),
            callback_capabilities: crate::base::reads::Recorded::new(),
            returned_callback_proof_digest: OnceLock::new(),
            project_has_component_type: crate::base::reads::Recorded::new(),
            component_functions: OnceLock::new(),
            result_access_forwarded: crate::base::reads::Recorded::new(),
            section_fingerprints: std::array::from_fn(|_| OnceLock::new()),
            tuple_iteration_veto: crate::base::reads::Recorded::new(),
            string_code_veto: crate::base::reads::Recorded::new(),
            parameter_completion_calls: crate::base::reads::Recorded::new(),
            never_returning_calls: OnceLock::new(),
            parameter_testing_functions: crate::base::reads::Recorded::new(),
            function_bind_patched: crate::base::reads::Recorded::new(),
            effect_apply_regions: crate::base::reads::Recorded::new(),
            effect_apply_calls: crate::base::reads::Recorded::new(),
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

    /// Whether the binding-chain machine or the use-site machine can read
    /// anything at all from this file: every read either makes of a file other
    /// than the one it was asked about is drawn from four AST tables, `calls`,
    /// `jsx_elements`, `members` and `bindings`.
    fn participates_in_returned_callback_proofs(file: &FileFacts) -> bool {
        !file.ast.calls.is_empty()
            || !file.ast.jsx_elements.is_empty()
            || !file.ast.members.is_empty()
            || !file.ast.bindings.is_empty()
    }

    /// The analyzed files, one per path (its first slot), sorted by path.
    fn files_by_path(&self) -> Vec<&'a FileFacts> {
        let all_slots = self.files();
        let mut ordered = self
            .file_table
            .ids()
            .iter()
            .filter_map(|id| all_slots.get(*self.file_table.slots_of(*id).first()?))
            .collect::<Vec<_>>();
        ordered.sort_unstable_by(|left, right| left.path.cmp(&right.path));
        ordered
    }

    /// Whether some consumer of a returned-invocation answer can tell it from
    /// no answer: the dialect gives some argument of that call (`argument <
    /// arguments`, the call's count) an execution or an owner. Every consumer
    /// reads the answer only through those two questions, at the call's own
    /// count (callee roles, owner edges, reachability, named callbacks).
    fn returned_invocation_is_read(
        &self,
        answer: returned_invocations::ReturnedInvocation,
    ) -> bool {
        let count = answer.arguments as usize;
        let slot = answer.slot.map(|slot| slot as usize);
        (0..count).any(|argument| {
            let semantics = self.dialect.returned_callback_semantics_at(
                answer.primitive,
                slot,
                argument,
                count,
            );
            semantics.execution.is_some() || semantics.owner.is_some()
        })
    }

    /// Identity of the whole-project answers that the cross-file
    /// returned-callback proofs give the per-file caches, or `None` when no
    /// such proof can exist (ADR 0392).
    ///
    /// Those proofs answer "is the function `createReaction` returned in file A
    /// ever invoked, and where?" and "is this call an invocation of one?", yet
    /// their answers land in *per-file* cache fragments keyed on that one
    /// file's source hash. Folding this digest into those fragments' reuse
    /// identity closes that hole. It binds the answers themselves:
    ///
    /// - every positive returned-invocation answer a consumer can read
    ///   ([`Self::returned_invocation_is_read`]): the binding-chain walk is a
    ///   memo per file ([`returned_invocations::ReturnedPrimitiveCalls`]);
    /// - per factory call (a primitive with a callback that runs only when
    ///   its returned function does), its proven invocation sites with what
    ///   each inherits, as every consumer reads them
    ///   (`owners::returned_callback_invocation_sites`).
    ///
    /// One read is not an answer: an inline callback of a factory takes its
    /// role from each invocation site, classified in the site's file
    /// (`callback_standing::role_at_returned_invocations`), which folds in
    /// that file's whole execution context. A factory whose inline callback
    /// has such a site in another file keeps the former contribution: the
    /// source hash of every participating file. Coarse, but never stale.
    ///
    /// The caches that rely on it: every per-file cache whose answer is
    /// computed through `returned_primitive_invocation` or through the
    /// returned-callback sites (source discovery, local accesses,
    /// interprocedural graph and results, owners, reachability).
    pub(super) fn returned_callback_proof_digest(&self) -> Option<CrossFileProofDigest> {
        *self.returned_callback_proof_digest.get_or_init(|| {
            if !self.models_returned_callbacks() {
                return None;
            }
            let ordered_files = self.files_by_path();
            let mut hasher = Sha256::new();
            let text = |hasher: &mut Sha256, value: &str| {
                hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_le_bytes());
                hasher.update(value.as_bytes());
            };
            let span = |hasher: &mut Sha256, span: Span| {
                hasher.update(span.start.to_le_bytes());
                hasher.update(span.end.to_le_bytes());
            };
            hasher.update(b"returned-invocations\0");
            for file in &ordered_files {
                let answers = self.returned_invocations(file);
                let mut read = answers
                    .iter()
                    .filter(|(_, answer)| self.returned_invocation_is_read(*answer))
                    .peekable();
                if read.peek().is_none() {
                    continue;
                }
                text(&mut hasher, file.path.as_str());
                for ((call, callee), answer) in read {
                    span(&mut hasher, *call);
                    span(&mut hasher, *callee);
                    text(&mut hasher, &format!("{:?}", answer.primitive));
                    hasher.update(answer.slot.map_or(u64::MAX, u64::from).to_le_bytes());
                    hasher.update(answer.arguments.to_le_bytes());
                }
                hasher.update([0xff]);
            }
            hasher.update(b"returned-sites\0");
            let mut classified_elsewhere = false;
            for file in &ordered_files {
                for call in &file.ast.calls {
                    let Some(primitive) = self.primitive_at_call(file, call.span) else {
                        continue;
                    };
                    let count = call.arguments.len();
                    let semantics = (0..count)
                        .map(|argument| {
                            self.dialect
                                .callback_semantics_at(primitive, argument, count)
                        })
                        .filter(|semantics| semantics.requires_return_invocation)
                        .collect::<Vec<_>>();
                    if semantics.is_empty() {
                        continue;
                    }
                    let sites = crate::owners::returned_callback_invocation_sites(file, call, self);
                    text(&mut hasher, file.path.as_str());
                    span(&mut hasher, call.span);
                    for site in &sites {
                        text(&mut hasher, &site.path);
                        span(&mut hasher, site.span);
                        text(
                            &mut hasher,
                            &format!("{:?}/{:?}", site.inherited_execution, site.inherited_owner),
                        );
                    }
                    hasher.update([0xff]);
                    classified_elsewhere |= semantics.iter().any(|semantics| {
                        semantics.execution == Some(solid_dialect::Execution::Inline)
                    }) && sites.iter().any(|site| {
                        matches!(
                            site.inherited_execution,
                            Some(solid_dialect::Execution::Inline) | None
                        ) && site.path != file.path.as_str()
                    });
                }
            }
            if classified_elsewhere {
                hasher.update(b"returned-site-files\0");
                for file in ordered_files
                    .iter()
                    .filter(|file| Self::participates_in_returned_callback_proofs(file))
                {
                    text(&mut hasher, file.path.as_str());
                    hasher.update(file.source_hash.as_str().as_bytes());
                }
            }
            Some(hasher.finalize().into())
        })
    }

    /// Identity of the returned-source answers (ADR 0222, 0287) the
    /// source-discovery fragments bake in: per binding-initializer call that
    /// returns a fresh source ([`returned_invocations::ReturnedSourceCalls`]),
    /// the creating file and call, its primitive, and everything a fragment
    /// reads of that call ([`crate::source_discovery::returned_source_facts`]).
    /// ADR 0392: the answers, not the bytes of every file returning a bare
    /// identifier.
    fn returned_source_digest(&self, hasher: &mut Sha256) {
        let text = |hasher: &mut Sha256, value: &str| {
            hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_le_bytes());
            hasher.update(value.as_bytes());
        };
        let ordered_files = self.files_by_path();
        for file in ordered_files {
            let answers = self.returned_sources(file);
            if answers.is_empty() {
                continue;
            }
            text(hasher, file.path.as_str());
            for answer in answers.iter() {
                hasher.update(answer.call.start.to_le_bytes());
                hasher.update(answer.call.end.to_le_bytes());
                let Some((origin, created, primitive)) = self.returned_source_at(file, answer.call)
                else {
                    hasher.update([0]);
                    continue;
                };
                text(hasher, origin.path.as_str());
                hasher.update(created.span.start.to_le_bytes());
                hasher.update(created.span.end.to_le_bytes());
                let facts = crate::source_discovery::returned_source_facts(
                    self, origin, created, primitive,
                );
                text(hasher, &format!("{primitive:?}/{facts:?}"));
            }
            hasher.update([0xff]);
        }
    }

    /// The fingerprint of `section`'s value in this build (L3, ADR 0397):
    /// what a stage chunk that read the section compares to decide reuse
    /// ([`crate::base::reads::ReadProof`]). Computed once per build, on the
    /// first check that needs it, without recording.
    pub(crate) fn section_fingerprint(&self, section: crate::base::reads::Section) -> [u8; 32] {
        *self.section_fingerprints[section as usize].get_or_init(|| {
            crate::base::reads::suppress(|| self.section_fingerprint_uncached(section))
        })
    }

    fn section_fingerprint_uncached(&self, section: crate::base::reads::Section) -> [u8; 32] {
        use crate::base::reads::Section;
        fn text(hasher: &mut Sha256, value: &str) {
            hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_le_bytes());
            hasher.update(value.as_bytes());
        }
        fn spans<'s>(hasher: &mut Sha256, keys: impl Iterator<Item = (&'s str, Span)>) {
            let mut keys = keys.collect::<Vec<_>>();
            keys.sort_unstable();
            for (path, span) in keys {
                text(hasher, path);
                hasher.update(span.start.to_le_bytes());
                hasher.update(span.end.to_le_bytes());
            }
            hasher.update([0xff]);
        }
        let mut hasher = Sha256::new();
        hasher.update([section as u8]);
        match section {
            Section::Components => {
                spans(&mut hasher, self.component_functions().iter().copied());
                spans(
                    &mut hasher,
                    self.escaped_component_functions().iter().copied(),
                );
                spans(
                    &mut hasher,
                    self.possible_component_functions().iter().copied(),
                );
            }
            Section::MemberTargets => {
                let mut targets = self.object_member_targets().iter().collect::<Vec<_>>();
                targets.sort_unstable_by(|a, b| {
                    (a.0.path.as_ref(), a.0.start_byte, a.0.end_byte).cmp(&(
                        b.0.path.as_ref(),
                        b.0.start_byte,
                        b.0.end_byte,
                    ))
                });
                for (site, (path, span)) in targets {
                    text(&mut hasher, site.path.as_ref());
                    text(&mut hasher, path.as_str());
                    hasher.update(site.start_byte.to_le_bytes());
                    hasher.update(site.end_byte.to_le_bytes());
                    hasher.update(span.start.to_le_bytes());
                    hasher.update(span.end.to_le_bytes());
                }
            }
            Section::SpreadTargets => {
                let mut spreads = self.spread_handler_targets().iter().collect::<Vec<_>>();
                spreads.sort_unstable_by_key(|(site, _)| {
                    (site.path.as_ref(), site.start_byte, site.end_byte)
                });
                for (site, targets) in spreads {
                    text(&mut hasher, site.path.as_ref());
                    hasher.update(site.start_byte.to_le_bytes());
                    hasher.update(site.end_byte.to_le_bytes());
                    hasher.update([u8::from(targets.is_some())]);
                    if let Some(targets) = targets {
                        let mut targets = targets.iter().collect::<Vec<_>>();
                        targets.sort_unstable();
                        for (path, span) in targets {
                            text(&mut hasher, path.as_str());
                            hasher.update(span.start.to_le_bytes());
                            hasher.update(span.end.to_le_bytes());
                        }
                    }
                    hasher.update([0xfe]);
                }
            }
            Section::LiteralReceivers => {
                // The whole census: a reader may ask about a member in any
                // file, the literal declared in that file or another.
                spans(
                    &mut hasher,
                    self.literal_receiver_members()
                        .iter()
                        .map(|(path, span)| (path.as_str(), *span)),
                );
                spans(
                    &mut hasher,
                    self.literal_receivers_across_files()
                        .iter()
                        .map(|(path, span)| (path.as_str(), *span)),
                );
            }
            Section::FlowAnswers => {
                hasher.update([u8::from(self.program_closed)]);
                let mut answers = self.invocation_flow_answers().iter().collect::<Vec<_>>();
                answers.sort_unstable_by(|left, right| left.0.cmp(right.0));
                for ((path, span), role) in answers {
                    text(&mut hasher, path);
                    hasher.update(span.start.to_le_bytes());
                    hasher.update(span.end.to_le_bytes());
                    text(&mut hasher, &format!("{role:?}"));
                }
            }
            Section::EventLiterals => {
                hasher.update([u8::from(self.program_closed)]);
                spans(
                    &mut hasher,
                    self.event_invoked_literals()
                        .iter()
                        .map(|(path, span)| (path.as_str(), *span)),
                );
            }
            Section::NeverReturning => {
                let mut paths = self.never_returning_calls().iter().collect::<Vec<_>>();
                paths.sort_unstable();
                for (path, calls) in paths {
                    text(&mut hasher, path);
                    for call in calls {
                        hasher.update(call.start.to_le_bytes());
                        hasher.update(call.end.to_le_bytes());
                    }
                    hasher.update([0xfe]);
                }
            }
            Section::ReturnedSources => self.returned_source_digest(&mut hasher),
            Section::ReturnedCallbacks => match self.returned_callback_proof_digest() {
                Some(digest) => {
                    hasher.update([1]);
                    hasher.update(digest);
                }
                None => hasher.update([0]),
            },
            Section::SynchronousDispatch => {
                hasher.update([u8::from(
                    self.hazard_present(crate::base::Hazard::SynchronousDispatch),
                )]);
            }
            Section::ExportedPrimitiveConsts => {
                for (path, span) in self.exported_primitive_consts() {
                    text(&mut hasher, &path);
                    hasher.update(span.start.to_le_bytes());
                    hasher.update(span.end.to_le_bytes());
                }
            }
            Section::ReflectivePrototypeWrite => {
                hasher.update([u8::from(self.reflective_prototype_write_present())]);
            }
            Section::ReflectiveDefinition => {
                hasher.update([u8::from(
                    self.hazard_present(crate::base::Hazard::ReflectiveDefinitionSpelling),
                )]);
                let mut keys = self
                    .base
                    .definers(|| self.base_views())
                    .keys()
                    .map(|key| format!("{key:?}"))
                    .collect::<Vec<_>>();
                keys.sort_unstable();
                for key in keys {
                    text(&mut hasher, &key);
                }
            }
        }
        hasher.finalize().into()
    }

    /// Begins the build's memos, outside every recording: the first read
    /// would otherwise begin them from inside whichever stage asks first
    /// (ADR 0392's deadlock: a lane that begins them re-asks the recorded
    /// probes while holding its own table's lock).
    ///
    /// Then warms the project-wide answers most stages read, concurrently,
    /// so the build pays the longest of them rather than their sum. Each
    /// fills its own once-per-build cache, so the answers are the ones a
    /// sequential run computes; nothing records them here.
    // Threads only with a second analysis worker; otherwise the lanes run inline (wasm32-wasip1).
    #[allow(clippy::disallowed_methods)]
    pub(crate) fn begin_memos(&self) {
        if memo_probes::probing() {
            return;
        }
        crate::base::reads::suppress(|| {
            let _ = self.memo_world();
            if crate::pipeline::available_analysis_workers() <= 1 {
                return;
            }
            let tuple_results = self.files().iter().any(|file| {
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
            });
            let limit = crate::pipeline::analysis_worker_limit_for_lanes(4);
            std::thread::scope(|scope| {
                let warm = |work: &(dyn Fn() + Sync)| {
                    let _limit = crate::pipeline::AnalysisWorkerLimit::enter(limit);
                    work();
                };
                if tuple_results {
                    scope.spawn(move || {
                        warm(&|| {
                            let _ = self.tuple_iteration_may_be_mutated();
                        })
                    });
                }
                scope.spawn(move || {
                    warm(&|| {
                        let _ = self.effect_apply_calls();
                    })
                });
                scope.spawn(move || {
                    warm(&|| {
                        let _ = self.object_member_targets();
                    })
                });
                warm(&|| {
                    let _ = self.component_functions();
                });
            });
        });
    }
}

impl crate::base::reads::ReadAnswers for SemanticLookup<'_> {
    fn section_fingerprint(&self, section: crate::base::reads::Section) -> [u8; 32] {
        SemanticLookup::section_fingerprint(self, section)
    }

    fn memo_changed_at(
        &self,
        store: u64,
        read: crate::base::reads::MemoRead,
    ) -> Option<crate::base::memo::Revision> {
        (store == self.memos.store().id())
            .then(|| self.memo_world().changed_at(read.table, read.query))
            .flatten()
    }

    fn memo_store_id(&self) -> u64 {
        self.memos.store().id()
    }

    fn describe_memo(&self, read: crate::base::reads::MemoRead) -> String {
        self.memos.store().describe(read.table, read.query)
    }
}

impl<'a> SemanticLookup<'a> {
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
        self.file_table
            .last_slot(file.path.as_str())
            .expect("per-file lookup for a file outside project facts")
    }

    /// How this file's callback positions name its own functions, memoized.
    ///
    /// The answer depends only on the file, so a read-by-read derivation was
    /// re-scanning every call and JSX element in the file for every read the
    /// classifier was asked about.
    pub(super) fn named_callback_roles(&self, file: &FileFacts) -> &super::NamedCallbackRoles {
        let index = self.file_index(file);
        let slots = self.file_named_callback_roles.get_or_init(|| {
            self.facts
                .files
                .iter()
                .map(|_| crate::base::reads::Recorded::new())
                .collect()
        });
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
        let slots = self.file_primitives.get_or_init(|| {
            self.facts
                .files
                .iter()
                .map(|_| crate::base::reads::Recorded::new())
                .collect()
        });
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
        crate::base::reads::note(crate::base::reads::Section::MemberTargets);
        self.object_members
            .get_or_init(|| crate::base::reads::suppress(|| crate::object_members::index(self)))
    }

    /// ADR 0358: what each spread argument of an intrinsic element supplies.
    pub(crate) fn spread_handler_targets(&self) -> &crate::object_members::SpreadTargets {
        crate::base::reads::note(crate::base::reads::Section::SpreadTargets);
        self.spread_handlers.get_or_init(|| {
            crate::base::reads::suppress(|| crate::object_members::spread_index(self))
        })
    }

    pub(super) fn literal_receiver_members(&self) -> &HashSet<(String, Span)> {
        &self.literal_receiver_census().all
    }

    /// The literal-receiver members whose literal another file declares,
    /// sorted: the part of the census that moves while the member's own file
    /// does not.
    pub(super) fn literal_receivers_across_files(&self) -> &[(String, Span)] {
        &self.literal_receiver_census().across_files
    }

    fn literal_receiver_census(&self) -> &crate::object_members::LiteralReceiverCensus {
        crate::base::reads::note(crate::base::reads::Section::LiteralReceivers);
        self.literal_receiver_members.get_or_init(|| {
            crate::base::reads::suppress(|| crate::object_members::literal_receiver_members(self))
        })
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
        Some((file, file.ast.function_at(*span)?))
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
        if entity.symbol_unresolved() || entity.symbol.is_empty() {
            return None;
        }
        self.symbols_by_id().get(entity.symbol.as_ref()).copied()
    }

    pub(super) fn files(&self) -> &'a [FileFacts] {
        &self.facts.files
    }

    /// ADR 0319: the invocation-flow answer of every literal, computed once.
    pub(crate) fn invocation_flow_answers(&self) -> &HashMap<(String, Span), crate::ExecutionRole> {
        crate::base::reads::note(crate::base::reads::Section::FlowAnswers);
        self.invocation_flow_answers.get_or_init(|| {
            crate::base::reads::suppress(|| crate::execution_role::invocation_flow_answers(self))
        })
    }

    /// Whether [`Self::invocation_flow_answers`] is already computed.
    pub(crate) fn invocation_flow_answers_ready(&self) -> bool {
        self.invocation_flow_answers.get().is_some()
    }

    /// The literals whose every invocation comes through event delivery, for the
    /// owner seeds; see [`crate::execution_role::event_invoked_literals`].
    pub(crate) fn event_invoked_literals(&self) -> &HashSet<(String, Span)> {
        crate::base::reads::note(crate::base::reads::Section::EventLiterals);
        self.event_invoked_literals.get_or_init(|| {
            crate::base::reads::suppress(|| crate::execution_role::event_invoked_literals(self))
        })
    }

    pub(super) fn tuple_iteration_may_be_mutated(&self) -> bool {
        *self
            .tuple_iteration_veto
            .get_or_init(|| self.whole_program_answer::<TupleIterationVeto>())
    }

    /// Facts are immutable for this lookup's lifetime. Proxy and invocation
    /// guards consume the same project census as tuple identity, without
    /// repeating it at every source-call site. A new generation has a new lock.
    pub(super) fn visible_string_code(&self) -> bool {
        *self
            .string_code_veto
            .get_or_init(|| self.whole_program_answer::<StringCodeVeto>())
    }

    /// ADR 0292: whether `property` holding `function` is the `default` of a
    /// module object a dialect component loader renders, and that function's
    /// one reference ([`LazyWitnessesIn`]).
    pub(super) fn is_lazy_default_render_witness(
        &self,
        file: &FileFacts,
        function: Span,
        property: Span,
    ) -> bool {
        self.memos
            .table()
            .id_of(file.path.as_str())
            .is_some_and(|id| {
                self.memo_world()
                    .answer::<LazyWitnessesIn>(&id)
                    .binary_search(&(function, property))
                    .is_ok()
            })
    }

    /// The analyzed file path a plain relative import specifier resolves to,
    /// exactly as [`solid_facts::resolve_relative_module_path`] over every
    /// analyzed path, without scanning them.
    pub(super) fn relative_module_path(&self, from: &str, module: &str) -> Option<&'a str> {
        let facts: &'a ProjectFacts = self.facts;
        solid_facts::resolve_relative_module_path_indexed(from, module, |probe| {
            let slots = self.file_table.slots_of(self.file_table.id_of(probe)?);
            Some((facts.files[*slots.first()?].path.as_str(), slots.len()))
        })
    }

    /// The index of the first file in `facts.files` whose path is `path`:
    /// what `facts.files.iter().position(..)` answers, read from an index
    /// built once.
    pub(super) fn first_file_index(&self, path: &str) -> Option<usize> {
        self.file_table.first_slot(path)
    }

    /// L0: this build's analyzed files by interned id.
    pub(crate) fn file_table(&self) -> &crate::base::FileTable {
        &self.file_table
    }

    /// The per-file views of this build's files over `table`: what the base
    /// indexes fold.
    pub(crate) fn base_views_over<'s>(
        &'s self,
        table: &'s crate::base::FileTable,
    ) -> crate::base::Views<'s> {
        crate::base::Views {
            table,
            files: self.files(),
            sources: crate::base::ViewSources {
                ast_indexes: self.ast_indexes,
                symbols: self.entities,
                entities: self.entities_by_location(),
                typescript: &self.facts.typescript,
                references: &self.reference_rows,
            },
            constants: crate::base::BuildConstants {
                dialect: self.dialect,
            },
        }
    }

    fn base_views(&self) -> crate::base::Views<'_> {
        self.base_views_over(&self.file_table)
    }

    /// L1: module hazards and module-level flags.
    /// A reader of the whole index may read the two hazards a section
    /// binds ([`crate::base::reads`]).
    pub(crate) fn base_hazards(&self) -> &crate::base::BaseIndex<crate::base::Hazard, u32> {
        crate::base::reads::note(crate::base::reads::Section::SynchronousDispatch);
        crate::base::reads::note(crate::base::reads::Section::ReflectiveDefinition);
        self.base.hazards(|| self.base_views())
    }

    /// Whether some analyzed file has `hazard`.
    pub(crate) fn hazard_present(&self, hazard: crate::base::Hazard) -> bool {
        self.base
            .hazards(|| self.base_views())
            .contains_key(&hazard)
    }

    /// L1: module edges by specifier.
    pub(crate) fn base_modules(
        &self,
    ) -> &crate::base::BaseIndex<crate::base::ModuleKey, crate::base::ModuleEdge> {
        self.base.modules(|| self.base_views())
    }

    /// L1: member uses by name.
    pub(crate) fn base_members(
        &self,
    ) -> &crate::base::BaseIndex<crate::base::MemberName, crate::base::MemberUse> {
        self.base.members(|| self.base_views())
    }

    /// L1: reflective-definer keys.
    pub(crate) fn base_definers(&self) -> &crate::base::BaseIndex<crate::base::DefinerKey, ()> {
        crate::base::reads::note(crate::base::reads::Section::ReflectiveDefinition);
        self.base.definers(|| self.base_views())
    }

    /// Every slot of the analyzed files in file order, with the entries
    /// `index` holds for it: how a census that reads files in order replays
    /// their contributions.
    pub(crate) fn slot_entries<'s, K, C>(
        &'s self,
        index: &'s crate::base::BaseIndex<K, C>,
    ) -> impl Iterator<Item = (&'a FileFacts, &'s [(K, C)])> + 's
    where
        K: Eq + std::hash::Hash + Clone,
        C: PartialEq,
        'a: 's,
    {
        let files: &'a [FileFacts] = self.files();
        files.iter().enumerate().map(move |(slot, file)| {
            (
                file,
                index.slot(
                    self.file_table.slot_id(slot),
                    self.file_table.slot_rank(slot),
                ),
            )
        })
    }

    pub(super) fn file_by_path(&self, path: &str) -> Option<&'a FileFacts> {
        let facts: &'a ProjectFacts = self.facts;
        self.file_table
            .last_slot(path)
            .map(|slot| &facts.files[slot])
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
        self.router_params
            .get_or_init(|| self.whole_program_answer::<RouterParams>())
    }

    /// ADR 0334: whether `span` in `path` is a component value of a route's
    /// options that the audited `@tanstack/solid-router` only renders.
    pub(super) fn route_component_value(&self, path: &str, span: Span) -> bool {
        self.route_component_values()
            .contains(&(path.to_string(), span))
    }

    /// Every route component value (ADR 0334), by path and span.
    pub(super) fn route_component_values(&self) -> &HashSet<(String, Span)> {
        self.route_component_values.get_or_init(|| {
            crate::route_components::route_component_values(
                self.facts,
                self.dialect,
                &self.router_install,
                &|file, name| {
                    let symbol = self.entities.at(file.path.as_str(), name)?;
                    let references = self.symbol_references_if_present(symbol.as_str())?;
                    references
                        .iter()
                        .map(|reference| {
                            Some((
                                reference.path.to_string(),
                                Span::new(
                                    u32::try_from(reference.start_byte).ok()?,
                                    u32::try_from(reference.end_byte).ok()?,
                                ),
                            ))
                        })
                        .collect()
                },
                crate::object_members::namespace_exposed_modules(self).as_ref(),
                crate::interproc::accessor_provenance::reflective_prototype_write(self),
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

    fn binding_declarations(&self) -> &BindingDeclarations<'a> {
        self.binding_declarations.get_or_init(|| {
            let entities = self.entities();
            let mut declarations = BindingDeclarations::default();
            for file in self.files() {
                let path = file.path.as_str();
                for binding in &file.ast.bindings {
                    for name in &binding.names {
                        declarations
                            .by_name_span
                            .entry((path, name.span))
                            .or_insert(binding);
                        if binding.initializer.is_some()
                            && let Some(symbol) = entities.at(path, name.span)
                        {
                            declarations
                                .initialized_by_symbol
                                .entry((path, symbol))
                                .or_insert(binding);
                        }
                    }
                }
            }
            declarations
        })
    }

    /// The first binding of `path`, in file order, that has an initializer and
    /// a name whose entity is `symbol`.
    pub(crate) fn initialized_binding_of_symbol(
        &self,
        path: &str,
        symbol: &SymbolId,
    ) -> Option<&'a solid_facts::ast::BindingFact> {
        self.binding_declarations()
            .initialized_by_symbol
            .get(&(path, symbol))
            .copied()
    }

    /// The first binding of `path`, in file order, with a name written at
    /// exactly `span`.
    pub(crate) fn binding_named_at(
        &self,
        path: &str,
        span: Span,
    ) -> Option<&'a solid_facts::ast::BindingFact> {
        self.binding_declarations()
            .by_name_span
            .get(&(path, span))
            .copied()
    }

    /// The memoised result for the factory call at `span` in `path`.
    pub(crate) fn returned_invocation_sites_memo(
        &self,
        path: &str,
        span: Span,
        compute: impl FnOnce() -> Vec<crate::owners::ReturnedCallbackInvocationSite>,
    ) -> std::sync::Arc<Vec<crate::owners::ReturnedCallbackInvocationSite>> {
        // The sites are classified in their own files' contexts: the section
        // binds them ([`Self::returned_callback_proof_digest`]).
        crate::base::reads::note(crate::base::reads::Section::ReturnedCallbacks);
        let key = (path.to_owned(), span);
        let found = self
            .returned_invocation_sites
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&key)
            .cloned();
        if let Some((sites, reads)) = found {
            crate::base::reads::replay(&reads);
            return sites;
        }
        // Computed outside the lock: the computation re-enters the lookup.
        // Its reads are kept for every later reader ([`crate::base::reads`]).
        let (computed, reads) = crate::base::reads::record(|| std::sync::Arc::new(compute()));
        self.returned_invocation_sites
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(key)
            .or_insert((computed, reads))
            .0
            .clone()
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
        let file = &self.facts.files[self.first_file_index(&declaration.location.path)?];
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
