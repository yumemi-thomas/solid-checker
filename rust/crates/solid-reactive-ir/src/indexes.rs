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
mod class_prototype;
mod completion;
mod components;
mod compound;
mod identity_wrappers;
pub(crate) use identity_wrappers::identity_wrapped_literal_span;
mod initialization;
mod member_values;
mod primitive_proof;

/// Functions by `(path, span)`: the proven components, then the escaped ones
/// among them ([`SemanticLookup::escaped_component_functions`]).
pub(super) type ComponentSets<'a> = (HashSet<(&'a str, Span)>, HashSet<(&'a str, Span)>);

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
                && !file.ast.members.iter().any(|member| {
                    let object = if through_wrappers {
                        file.ast.peel_ts_sugar_span(member.object)
                    } else {
                        member.object
                    };
                    object == *reference
                })
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
    /// ADR 0346: the project's identity wrappers, by declaration symbol.
    identity_wrappers: OnceLock<HashMap<&'a str, usize>>,
    /// ADR 0319: every literal's invocation-flow answer.
    invocation_flow_answers: OnceLock<HashMap<(String, Span), crate::ExecutionRole>>,
    /// The literals whose every invocation is a host event dispatch.
    unowned_event_literals: OnceLock<HashSet<(String, Span)>>,
    /// ADR 0320: whether some call may copy keys into an existing object.
    objects_assigned_into: OnceLock<bool>,
    router_params: OnceLock<crate::router_params::RouterParamsStatus>,
    /// ADR 0334: the route option values the audited router renders.
    route_component_values: OnceLock<HashSet<(String, Span)>>,
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
    /// ADR 0339: references to function-valued bindings not proven to follow
    /// their declarators.
    initialization: initialization::State,
    /// ADR 0342: the member values of compound components that tags render.
    compound: compound::State<'a>,
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
    /// Proven components, then functions whose only missing proof is an
    /// escaping reference (`ComponentStatus::Uncertain`).
    component_functions: OnceLock<ComponentSets<'a>>,
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
    /// ADR 0338: whether some analysed file may patch `Function.prototype.bind`.
    function_bind_patched: OnceLock<bool>,
    /// ADR 0292: `(path, function span, property span)` for each object
    /// property that is a proven component's one route, the `default` of the
    /// module a dialect component loader renders.
    lazy_default_render_witnesses: OnceLock<HashSet<(&'a str, Span, Span)>>,
    effect_apply_regions: OnceLock<crate::effect_execution::ApplyRegions>,
    effect_apply_calls: OnceLock<crate::effect_execution::ApplyCalls<'a>>,
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
            identity_wrappers: OnceLock::new(),
            invocation_flow_answers: OnceLock::new(),
            unowned_event_literals: OnceLock::new(),
            objects_assigned_into: OnceLock::new(),
            router_params: OnceLock::new(),
            route_component_values: OnceLock::new(),
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
            initialization: initialization::State::default(),
            compound: compound::State::default(),
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
            function_bind_patched: OnceLock::new(),
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
            // The project-wide proofs below are independent and each a whole
            // pass over the program; on every incremental build they are
            // recomputed. Warm them concurrently so the digest costs the
            // longest of them, not their sum. Each fills its own `OnceLock`,
            // so the answers are the ones a sequential run computes.
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
            if crate::pipeline::available_analysis_workers() > 1 {
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
            }
            let components = self.component_functions();
            let mut component_keys = components.iter().copied().collect::<Vec<_>>();
            component_keys.sort_unstable();
            let mut escaped_keys = self
                .escaped_component_functions()
                .iter()
                .copied()
                .collect::<Vec<_>>();
            escaped_keys.sort_unstable();
            let mut hasher = Sha256::new();
            hasher.update(b"components\0");
            // ADR 0282: a mutation in another file can withdraw the tuple
            // callable edge without changing the consuming file's facts.
            // Projects with no supported tuple result cannot consume this
            // premise. Do not invalidate their unrelated object/source caches.
            if tuple_results {
                hasher.update(b"tuple-iteration-veto\0");
                hasher.update([u8::from(self.tuple_iteration_may_be_mutated())]);
            }
            // ADR 0324: whether the router premise applies is a property of
            // every file (a history in one decides what another reads).
            hasher.update(b"router-params\0");
            hasher.update([u8::from(self.router_params().applies())]);
            // ADR 0346: a wrapper in another file decides whether a call hands
            // back a callback literal, so every wrapper's identity is digested.
            hasher.update(b"identity-wrappers\0");
            let mut wrappers = self.identity_wrappers().iter().collect::<Vec<_>>();
            wrappers.sort_unstable();
            for (symbol, index) in wrappers {
                hasher.update(
                    u64::try_from(symbol.len())
                        .unwrap_or(u64::MAX)
                        .to_le_bytes(),
                );
                hasher.update(symbol.as_bytes());
                hasher.update(u64::try_from(*index).unwrap_or(u64::MAX).to_le_bytes());
            }
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
            // ADR 0339: a mention of a function-valued binding that may run
            // before its declarator supplies no call site or reachability
            // edge; another module's imports decide it.
            hasher.update(b"uninitialized-references\0");
            let mut withheld = self
                .uninitialized_references()
                .iter()
                .flat_map(|(path, spans)| spans.iter().map(move |span| (path.as_str(), *span)))
                .collect::<Vec<_>>();
            withheld.sort_unstable();
            for (path, span) in withheld {
                hasher.update(u64::try_from(path.len()).unwrap_or(u64::MAX).to_le_bytes());
                hasher.update(path.as_bytes());
                hasher.update(span.start.to_le_bytes());
                hasher.update(span.end.to_le_bytes());
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
            hasher.update(b"escaped-components\0");
            for (path, span) in escaped_keys {
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

    /// ADR 0334: whether `span` in `path` is a component value of a route's
    /// options that the audited `@tanstack/solid-router` only renders.
    pub(super) fn route_component_value(&self, path: &str, span: Span) -> bool {
        self.route_component_values
            .get_or_init(|| {
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
                    crate::object_members::namespace_exposed_modules(self.facts, self).as_ref(),
                    crate::interproc::accessor_provenance::reflective_prototype_write(self),
                )
            })
            .contains(&(path.to_owned(), span))
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
