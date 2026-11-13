//! ADR 0972: the syntax half of "may a package the build does not read reach a Solid action". One runtime file of an installed package is
//! parsed with Oxc and answers what it loads and what it does that no import statement shows:
//!
//! - **loads**: every specifier it names in a static `import`, an `export … from`, a CommonJS `require("…")` of the unbound `require`, a
//!   literal `import("…")` and an `import x = require("…")` (type-only forms are erased and load nothing);
//! - **hazards**: what makes the closure undecidable from those loads alone: a load by a computed name (`import(name)`, `require(name)`,
//!   `require` used as a value, `module.require`, `import.meta.glob`), code generation (`eval`, the `Function` constructor, a call of a
//!   `.constructor` member), a lookup of a global the pinned TypeScript's `lib.es*` and `lib.dom*` files do not declare, and a computed or
//!   escaping use of the global object (`globalThis[k]`, `window[k]`, `const g = window`). A file Oxc does not parse is a hazard too.
//!
//! - **accessors** (ADR 0973): the names a file defines a `get` or `set` under (class and object literal accessors, `defineProperty`, `defineProperties`,
//!   `Object.create` with descriptors, `__defineGetter__`, `accessor` fields), and whether it defines one under a name the syntax does not fix (a computed key, a
//!   `Proxy`, `defineProperty` read as a value): a member read of a package's object runs the package's code only where one matches.
//!
//! The global object goes by several names, and a member of one that gives it back is the same global (`window.self.x`, `globalThis.globalThis.x`,
//! `window.document.defaultView.x`): a member read off any of them that no library declares is a lookup of an unknown global (ADR 0973).
//!
//! The backend walks the loads (`solid-facts-backend/src/package_reach.rs`); this module never reads the filesystem and never matches text.
//!
//! What a name is: an identifier reference the semantic analysis binds to nothing in the module (Oxc's root unresolved references). The
//! operand of `typeof` reads no value and is never a hazard; an assignment that only writes the name looks nothing up. CommonJS wraps a module
//! in a function whose parameters are `exports`, `module`, `require`, `__filename` and `__dirname`; `exports` and the file names hold no code,
//! `module` is read only through `module.exports`, `module.id`, `module.hot` and `module.loaded`.

use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;

use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_ast::ast::{
    Argument, Expression, ImportOrExportKind, MethodDefinitionKind, ObjectPropertyKind,
    PropertyKey, PropertyKind, Statement, TSModuleReference, UnaryOperator,
};
use oxc_parser::Parser;
use oxc_semantic::{AstNodes, NodeId, SemanticBuilder};
use oxc_span::{GetSpan, SourceType};

/// The global value names the pinned TypeScript's `lib.es*.d.ts` and `lib.dom*.d.ts` declare at top level, one per line, sorted. Generated
/// by `known_globals_are_the_pinned_libraries_names` (ignored test) from the tsrs fork's `crates/tsrs_vfs/libs` at the revision
/// `apps/solid-typefacts/Cargo.toml` pins.
const KNOWN_GLOBALS: &str = include_str!("module_reach/known_globals.txt");

/// The names the global object goes by. A static member of one is the global of that name; anything else done with it is undecidable.
const GLOBAL_OBJECTS: &[&str] = &[
    "globalThis",
    "window",
    "self",
    "top",
    "parent",
    "frames",
    "global",
];

/// What makes a module's closure undecidable from its loads alone.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ModuleHazardKind {
    /// Oxc does not parse the file, or its semantic analysis reports an error.
    Unparseable,
    /// A load by a name the syntax does not fix: `import(x)`, `require(x)`, `require` as a value, `module.require`, `import.meta.glob`.
    DynamicLoad,
    /// Code generation: `eval`, the `Function` constructor, a call or `new` of a `.constructor` member, or one stored or passed on.
    CodeGeneration,
    /// A global no `lib.es*`/`lib.dom*` file declares, read as a value (`process`, `window.solid`).
    UnknownGlobal,
    /// The global object read by a computed key or handed on as a value (`globalThis[k]`, `const g = window`).
    GlobalObjectEscape,
}

impl ModuleHazardKind {
    /// The kebab-case word a blocker's detail names the hazard by.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Unparseable => "unparseable",
            Self::DynamicLoad => "dynamic-load",
            Self::CodeGeneration => "code-generation",
            Self::UnknownGlobal => "unknown-global",
            Self::GlobalObjectEscape => "global-object-escape",
        }
    }
}

/// One hazard: its kind and what it names (the global, the construct).
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModuleHazard {
    pub kind: ModuleHazardKind,
    pub detail: String,
}

/// The ways a file defines an accessor (a `get` or `set` that runs code when a member is read or assigned), by the name it defines them under (ADR 0973).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AccessorScan {
    /// The names a getter or setter is defined under (class `get x()`, object literal `set x(v)`, `Object.defineProperty(o, "x", { get })`), sorted and distinct.
    pub names: Vec<String>,
    /// An accessor is defined under a name the syntax does not fix, or a construct that may define one is not read closely enough to say (a computed key, a `Proxy`,
    /// `Object.defineProperty` handed on as a value, a descriptor that is not a literal): it matches every name.
    pub any_name: bool,
}

impl AccessorScan {
    /// Whether the file defines no accessor at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty() && !self.any_name
    }
}

/// What one runtime file loads, and what it does that its loads do not show.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModuleScan {
    /// The specifiers it loads, sorted and distinct.
    pub loads: Vec<String>,
    /// Its hazards, sorted and distinct. Empty when the loads are all it can reach.
    pub hazards: Vec<ModuleHazard>,
    /// The accessors it defines.
    pub accessors: AccessorScan,
}

/// Whether `name` is a global value the pinned TypeScript's `lib.es*`/`lib.dom*` files declare.
#[must_use]
pub fn is_known_global(name: &str) -> bool {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        KNOWN_GLOBALS
            .lines()
            .filter(|line| !line.is_empty())
            .collect()
    })
    .contains(name)
}

/// Scans one runtime file (`path` decides the source type: `.mjs` is a module, `.cjs` CommonJS, `.js`/`.jsx` either by its syntax).
#[must_use]
pub fn scan_module(path: &Path, source: &str) -> ModuleScan {
    let mut scan = ModuleScan::default();
    let unparseable = |scan: &mut ModuleScan, detail: &str| {
        scan.hazards.push(ModuleHazard {
            kind: ModuleHazardKind::Unparseable,
            detail: detail.to_owned(),
        });
    };
    let Ok(source_type) = SourceType::from_path(path) else {
        unparseable(&mut scan, "unknown extension");
        return scan;
    };
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        unparseable(&mut scan, "parse error");
        return scan;
    }
    let program = parsed.program;
    let built = SemanticBuilder::new().build(&program);
    if !built.errors.is_empty() {
        unparseable(&mut scan, "semantic error");
        return scan;
    }
    let mut loads = Vec::<String>::new();
    let mut hazards = Vec::<ModuleHazard>::new();
    let mut hazard = |kind: ModuleHazardKind, detail: &str| {
        hazards.push(ModuleHazard {
            kind,
            detail: detail.to_owned(),
        });
    };
    for statement in &program.body {
        match statement {
            Statement::ImportDeclaration(import)
                if import.import_kind != ImportOrExportKind::Type =>
            {
                loads.push(import.source.value.to_string());
            }
            Statement::ExportAllDeclaration(export)
                if export.export_kind != ImportOrExportKind::Type =>
            {
                loads.push(export.source.value.to_string());
            }
            Statement::ExportNamedDeclaration(export)
                if export.export_kind != ImportOrExportKind::Type =>
            {
                if let Some(source) = &export.source {
                    loads.push(source.value.to_string());
                }
            }
            Statement::TSImportEqualsDeclaration(import)
                if import.import_kind != ImportOrExportKind::Type =>
            {
                if let TSModuleReference::ExternalModuleReference(reference) =
                    &import.module_reference
                {
                    loads.push(reference.expression.value.to_string());
                }
            }
            _ => {}
        }
    }
    let semantic = &built.semantic;
    let nodes = semantic.nodes();
    for node in nodes.iter() {
        match node.kind() {
            AstKind::ImportExpression(import) => match literal_specifier(&import.source) {
                Some(specifier) => loads.push(specifier),
                None => hazard(ModuleHazardKind::DynamicLoad, "import()"),
            },
            AstKind::MetaProperty(meta) if meta.meta.name == "import" => {
                let member = match nodes.parent_kind(node.id()) {
                    AstKind::StaticMemberExpression(member) => Some(member.property.name.as_str()),
                    _ => None,
                };
                // `import.meta.url`, `import.meta.env` load nothing; a glob, a computed member or an alias of `import.meta` may.
                if member.is_none_or(|name| name == "glob") {
                    hazard(ModuleHazardKind::DynamicLoad, "import.meta");
                }
            }
            _ => {}
        }
    }
    // The right operand of `0 && …` never runs: esbuild writes `0 && (module.exports = { a, b })` as a hint for CommonJS named exports, naming bindings a
    // re-export never declares in the file.
    let dead = nodes
        .iter()
        .filter_map(|node| match node.kind() {
            AstKind::LogicalExpression(logical)
                if logical.operator == oxc_syntax::operator::LogicalOperator::And
                    && matches!(&logical.left, Expression::NumericLiteral(zero) if zero.value == 0.0) =>
            {
                Some(logical.right.span())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let scoping = semantic.scoping();
    for (name, references) in scoping.root_unresolved_references() {
        let name = name.as_str();
        for reference in references {
            let reference = scoping.get_reference(*reference);
            let id = reference.node_id();
            let at = nodes.get_node(id).kind().span();
            // A reference in a type position (`callback: Function`, `session: XRSession`) is erased: no value is looked up.
            if reference.is_type()
                || under_typeof(nodes, id)
                || dead
                    .iter()
                    .any(|span| span.start <= at.start && at.end <= span.end)
            {
                continue;
            }
            let read = reference.is_read();
            match name {
                "require" => match nodes.parent_kind(id) {
                    AstKind::CallExpression(call)
                        if call.callee.get_inner_expression().span()
                            == nodes.get_node(id).kind().span() =>
                    {
                        let specifier = match call.arguments.as_slice() {
                            [argument] => argument.as_expression().and_then(literal_specifier),
                            _ => None,
                        };
                        match specifier {
                            Some(specifier) => loads.push(specifier),
                            None => hazard(ModuleHazardKind::DynamicLoad, "require()"),
                        }
                    }
                    _ => hazard(ModuleHazardKind::DynamicLoad, "require"),
                },
                "eval" => hazard(ModuleHazardKind::CodeGeneration, "eval"),
                // A timer given a string evaluates it (`setTimeout("code", 0)`): as `eval`. A function or a name is the timer's ordinary use.
                "setTimeout" | "setInterval" | "setImmediate" | "execScript"
                    if string_timer(nodes, id) =>
                {
                    hazard(ModuleHazardKind::CodeGeneration, name);
                }
                "Function" => {
                    if !function_use_is_inert(nodes, id) {
                        hazard(ModuleHazardKind::CodeGeneration, "Function");
                    }
                }
                "exports" | "__filename" | "__dirname" | "undefined" | "NaN" | "Infinity"
                | "arguments" => {}
                "module" => {
                    let member = static_member_name(nodes, id);
                    if !member
                        .is_some_and(|member| matches!(member, "exports" | "id" | "hot" | "loaded"))
                    {
                        hazard(ModuleHazardKind::DynamicLoad, "module");
                    }
                }
                // `document.defaultView` is the global object, so `document` is followed too; any other use of `document` holds no global object.
                "document" => match global_use(nodes, id, name, Holder::Document) {
                    GlobalUse::Member { name: member, path } if !is_known_global(&member) => {
                        hazard(ModuleHazardKind::UnknownGlobal, &path);
                    }
                    GlobalUse::Escape { path } => {
                        hazard(ModuleHazardKind::GlobalObjectEscape, &path);
                    }
                    GlobalUse::Member { .. } | GlobalUse::Written | GlobalUse::Inert => {}
                },
                _ if GLOBAL_OBJECTS.contains(&name) => {
                    match global_use(nodes, id, name, Holder::Global) {
                        GlobalUse::Member { name: member, .. } if is_known_global(&member) => {}
                        GlobalUse::Member { path, .. } => {
                            hazard(ModuleHazardKind::UnknownGlobal, &path);
                        }
                        GlobalUse::Written | GlobalUse::Inert => {}
                        GlobalUse::Escape { path } => {
                            hazard(ModuleHazardKind::GlobalObjectEscape, &path);
                        }
                    }
                }
                _ if is_known_global(name) => {}
                _ if !read => {}
                _ => hazard(ModuleHazardKind::UnknownGlobal, name),
            }
        }
    }
    // `fn.constructor("code")` and `x.constructor.constructor("code")` reach the `Function` constructor. A class instance's `.constructor` is its class
    // (`new this.constructor()`, `x.constructor === Foo`, `this.constructor = d`), which is no code generation; that a function value's is `Function` is
    // not seen unless the call hands it a string literal.
    for node in nodes.iter() {
        let AstKind::StaticMemberExpression(member) = node.kind() else {
            continue;
        };
        if member.property.name != "constructor" {
            continue;
        }
        if constructor_generates_code(nodes, node.id(), member) {
            hazard(ModuleHazardKind::CodeGeneration, ".constructor");
        }
    }
    loads.sort();
    loads.dedup();
    hazards.sort();
    hazards.dedup();
    scan.loads = loads;
    scan.hazards = hazards;
    scan.accessors = scan_accessors(nodes);
    scan
}

/// The static name a property key fixes: `Some(Some(name))`, `Some(None)` for a private name (no code outside the class reads it), `None` for a key the syntax does not
/// fix (a computed expression).
fn key_name(key: &PropertyKey<'_>) -> Option<Option<String>> {
    match key {
        PropertyKey::StaticIdentifier(name) => Some(Some(name.name.to_string())),
        PropertyKey::PrivateIdentifier(_) => Some(None),
        PropertyKey::StringLiteral(literal) => Some(Some(literal.value.to_string())),
        PropertyKey::NumericLiteral(literal) => Some(Some(literal.value.to_string())),
        PropertyKey::TemplateLiteral(template) if template.expressions.is_empty() => template
            .quasis
            .first()
            .and_then(|quasi| quasi.value.cooked.as_ref())
            .map(|value| Some(value.to_string())),
        _ => None,
    }
}

/// How a property descriptor reads: `Some(true)` when it names a `get` or a `set`, `Some(false)` when it is an object literal that names neither (a data property),
/// `None` when it is not a literal the syntax reads (a variable, a call, a spread, a computed key).
fn descriptor_defines_accessor(expression: &Expression<'_>) -> Option<bool> {
    let Expression::ObjectExpression(object) = expression.get_inner_expression() else {
        return None;
    };
    let mut accessor = false;
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        match key_name(&property.key) {
            Some(Some(name)) if name == "get" || name == "set" => accessor = true,
            Some(_) => {}
            None => return None,
        }
    }
    Some(accessor)
}

/// Every way the file defines an accessor: class and object literal `get`/`set`, an `accessor` field, `Object.defineProperty` and `Reflect.defineProperty` (a literal
/// name and a descriptor that names a `get` or `set`), `Object.defineProperties` and `Object.create(proto, descriptors)` (a literal map), `__defineGetter__` and
/// `__defineSetter__`, and a `Proxy`. A form the syntax does not read closely enough to say which name it defines under (a computed key, a descriptor that is a variable,
/// `Object.defineProperty` read as a value) matches every name.
fn scan_accessors(nodes: &AstNodes<'_>) -> AccessorScan {
    let mut sink = AccessorSink::default();
    // The calls whose callee member is read for what it defines, so the member itself is not also read as a value.
    let mut read_callees = HashSet::<oxc_span::Span>::new();
    for node in nodes.iter() {
        let AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        let Expression::StaticMemberExpression(callee) = call.callee.get_inner_expression() else {
            continue;
        };
        let method = callee.property.name.as_str();
        let argument = |index: usize| call.arguments.get(index).and_then(Argument::as_expression);
        match method {
            "defineProperty" => {
                read_callees.insert(callee.span);
                match (argument(1), argument(2)) {
                    (Some(key), Some(descriptor)) => {
                        // A name this call defines only when its descriptor may be an accessor; a data descriptor defines none.
                        if descriptor_defines_accessor(descriptor) != Some(false) {
                            sink.define(literal_specifier(key).map(Some));
                        }
                    }
                    _ => sink.define(None),
                }
            }
            "defineProperties" | "create" => {
                // `Object.create(proto)` defines nothing; a second argument is the descriptors, as `defineProperties`' is.
                let is_create = method == "create";
                if is_create
                    && !matches!(callee.object.get_inner_expression(), Expression::Identifier(object) if object.name == "Object")
                {
                    continue;
                }
                if is_create && call.arguments.len() < 2 {
                    read_callees.insert(callee.span);
                    continue;
                }
                read_callees.insert(callee.span);
                match argument(1).map(Expression::get_inner_expression) {
                    Some(Expression::ObjectExpression(map)) => {
                        for property in &map.properties {
                            match property {
                                ObjectPropertyKind::ObjectProperty(property) => {
                                    if descriptor_defines_accessor(&property.value) != Some(false) {
                                        sink.define(key_name(&property.key));
                                    }
                                }
                                ObjectPropertyKind::SpreadProperty(_) => sink.any_name = true,
                            }
                        }
                    }
                    _ => sink.define(None),
                }
            }
            "__defineGetter__" | "__defineSetter__" => {
                read_callees.insert(callee.span);
                sink.define(argument(0).and_then(literal_specifier).map(Some));
            }
            _ => {}
        }
    }
    for node in nodes.iter() {
        match node.kind() {
            AstKind::ObjectProperty(property)
                if matches!(property.kind, PropertyKind::Get | PropertyKind::Set) =>
            {
                sink.define(key_name(&property.key));
            }
            AstKind::MethodDefinition(method)
                if matches!(
                    method.kind,
                    MethodDefinitionKind::Get | MethodDefinitionKind::Set
                ) =>
            {
                sink.define(key_name(&method.key));
            }
            AstKind::AccessorProperty(accessor) => sink.define(key_name(&accessor.key)),
            // The accessor-defining members read as a value (`const def = Object.defineProperty`, `const { defineProperty } = Object`) or by a computed
            // key: where they are called from is not followed.
            AstKind::StaticMemberExpression(member)
                if matches!(
                    member.property.name.as_str(),
                    "defineProperty" | "defineProperties" | "__defineGetter__" | "__defineSetter__"
                ) && !read_callees.contains(&member.span) =>
            {
                sink.any_name = true;
            }
            AstKind::ComputedMemberExpression(member)
                if literal_specifier(&member.expression).is_some_and(|key| {
                    matches!(
                        key.as_str(),
                        "defineProperty"
                            | "defineProperties"
                            | "__defineGetter__"
                            | "__defineSetter__"
                    )
                }) =>
            {
                sink.any_name = true;
            }
            AstKind::BindingProperty(property)
                if key_name(&property.key).is_some_and(|key| {
                    matches!(
                        key.as_deref(),
                        Some(
                            "defineProperty"
                                | "defineProperties"
                                | "__defineGetter__"
                                | "__defineSetter__"
                        )
                    )
                }) =>
            {
                sink.any_name = true;
            }
            AstKind::IdentifierReference(reference) if reference.name == "Proxy" => {
                sink.any_name = true;
            }
            _ => {}
        }
    }
    AccessorScan {
        names: sink.names.into_iter().collect(),
        any_name: sink.any_name,
    }
}

#[derive(Default)]
struct AccessorSink {
    names: std::collections::BTreeSet<String>,
    any_name: bool,
}

impl AccessorSink {
    /// A key defines an accessor under its name; one the syntax does not fix defines under every name.
    fn define(&mut self, key: Option<Option<String>>) {
        match key {
            Some(Some(name)) => {
                self.names.insert(name);
            }
            Some(None) => {}
            None => self.any_name = true,
        }
    }
}

/// Whether the reference at `id` is the callee of a call whose first argument is a string literal or a template: the timer evaluates it.
fn string_timer(nodes: &AstNodes<'_>, id: NodeId) -> bool {
    let (child, parent) = parent_past_wrappers(nodes, id);
    let AstKind::CallExpression(call) = nodes.get_node(parent).kind() else {
        return false;
    };
    call.callee.get_inner_expression().span() == nodes.get_node(child).kind().span()
        && call
            .arguments
            .first()
            .and_then(Argument::as_expression)
            .is_some_and(|first| {
                matches!(
                    first.get_inner_expression(),
                    Expression::StringLiteral(_) | Expression::TemplateLiteral(_)
                )
            })
}

/// A specifier the expression fixes: a string literal, or a template with no substitution.
fn literal_specifier(expression: &Expression<'_>) -> Option<String> {
    match expression.get_inner_expression() {
        Expression::StringLiteral(literal) => Some(literal.value.to_string()),
        Expression::TemplateLiteral(template) if template.expressions.is_empty() => template
            .quasis
            .first()
            .and_then(|quasi| quasi.value.cooked.as_ref())
            .map(|value| value.to_string()),
        _ => None,
    }
}

/// The parent of `id`, past parentheses and TypeScript's runtime-erased wrappers.
fn parent_past_wrappers(nodes: &AstNodes<'_>, mut id: NodeId) -> (NodeId, NodeId) {
    loop {
        let parent = nodes.parent_id(id);
        match nodes.get_node(parent).kind() {
            AstKind::ParenthesizedExpression(_)
            | AstKind::TSAsExpression(_)
            | AstKind::TSSatisfiesExpression(_)
            | AstKind::TSNonNullExpression(_)
            | AstKind::TSTypeAssertion(_) => id = parent,
            _ => return (id, parent),
        }
    }
}

/// Whether the reference at `id` is the operand of `typeof` (no value is read: an unbound name answers `"undefined"`).
fn under_typeof(nodes: &AstNodes<'_>, id: NodeId) -> bool {
    let (_, parent) = parent_past_wrappers(nodes, id);
    matches!(nodes.get_node(parent).kind(), AstKind::UnaryExpression(unary) if unary.operator == UnaryOperator::Typeof)
}

/// The static member name read off the reference at `id` (`module.exports`), if it is the object of one.
fn static_member_name<'a>(nodes: &'a AstNodes<'_>, id: NodeId) -> Option<&'a str> {
    let (child, parent) = parent_past_wrappers(nodes, id);
    match nodes.get_node(parent).kind() {
        AstKind::StaticMemberExpression(member)
            if member.object.get_inner_expression().span()
                == nodes.get_node(child).kind().span() =>
        {
            Some(member.property.name.as_str())
        }
        _ => None,
    }
}

/// What a value is that a chain of members started from a global-object name (or from `document`) holds.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Holder {
    /// The global object: `window`, and the members that give it back (`window.self`, `globalThis.globalThis`, `document.defaultView`).
    Global,
    /// The `document` object: only its `defaultView` is the global object, whatever else it holds is no global.
    Document,
}

enum GlobalUse {
    /// A member of the global object read by a static name or a string literal key: its name, and the chain as it was spelled (`window.self.solid`).
    Member { name: String, path: String },
    /// A member (or the name itself, sloppy code) assigned: nothing is read.
    Written,
    /// Anything else done with the global object: a computed key, an alias, an argument. The chain as it was spelled.
    Escape { path: String },
    /// `document` used for anything but `document.defaultView`.
    Inert,
}

/// How the global-object name (or `document`) at `id` is used, following the global object through every member that returns it: the names it goes by
/// ([`GLOBAL_OBJECTS`], so `window.self.x` and `window.window.x` are `window.x`) and `document.defaultView`. A member read off the chain's end that no
/// library declares is a lookup of a global however many members led to it.
fn global_use(nodes: &AstNodes<'_>, id: NodeId, root: &str, start: Holder) -> GlobalUse {
    // `window.x = …` writes the member and reads nothing of it.
    let assigned = |member: NodeId| {
        let (member_child, member_parent) = parent_past_wrappers(nodes, member);
        matches!(nodes.get_node(member_parent).kind(), AstKind::AssignmentExpression(assignment)
            if assignment.operator == oxc_syntax::operator::AssignmentOperator::Assign
                && assignment.left.span() == nodes.get_node(member_child).kind().span())
    };
    let mut at = id;
    let mut holder = start;
    let mut path = root.to_owned();
    loop {
        let (child, parent) = parent_past_wrappers(nodes, at);
        let child_span = nodes.get_node(child).kind().span();
        // `Some(key)` for a member read off the chain (`key` is `None` for a computed key the syntax does not fix), `None` when the value itself is used.
        let mut computed = false;
        let member = match nodes.get_node(parent).kind() {
            AstKind::StaticMemberExpression(member) if member.object.span() == child_span => {
                Some(Some(member.property.name.to_string()))
            }
            AstKind::ComputedMemberExpression(member) if member.object.span() == child_span => {
                computed = true;
                Some(literal_specifier(&member.expression))
            }
            AstKind::AssignmentExpression(assignment) if assignment.left.span() == child_span => {
                return GlobalUse::Written;
            }
            _ => None,
        };
        // The value itself is used, or read by a key the syntax does not fix: the global object escapes, `document` holds no global object.
        let Some(Some(name)) = member else {
            return match holder {
                Holder::Global => GlobalUse::Escape { path },
                Holder::Document => GlobalUse::Inert,
            };
        };
        if !computed && assigned(parent) {
            return GlobalUse::Written;
        }
        path = format!("{path}.{name}");
        match holder {
            Holder::Document if name == "defaultView" => holder = Holder::Global,
            Holder::Document => return GlobalUse::Inert,
            Holder::Global if GLOBAL_OBJECTS.contains(&name.as_str()) => {}
            Holder::Global if name == "document" => holder = Holder::Document,
            Holder::Global => return GlobalUse::Member { name, path },
        }
        at = parent;
    }
}

/// Whether a reference of the global `Function` constructs nothing: `Function.prototype…`, the right of `instanceof`, a strict comparison.
fn function_use_is_inert(nodes: &AstNodes<'_>, id: NodeId) -> bool {
    let (child, parent) = parent_past_wrappers(nodes, id);
    let child_span = nodes.get_node(child).kind().span();
    match nodes.get_node(parent).kind() {
        AstKind::StaticMemberExpression(member) => {
            member.property.name == "prototype" && member.object.span() == child_span
        }
        AstKind::BinaryExpression(binary) => {
            use oxc_syntax::operator::BinaryOperator;
            matches!(
                binary.operator,
                BinaryOperator::Instanceof
                    | BinaryOperator::StrictEquality
                    | BinaryOperator::StrictInequality
                    | BinaryOperator::Equality
                    | BinaryOperator::Inequality
            )
        }
        _ => false,
    }
}

/// Whether the `.constructor` member at `id` can generate code: its object is itself a `.constructor` (`x.constructor.constructor`, the `Function`
/// constructor of any value), or the member is called (or constructed) with a string literal or a template for its first argument.
fn constructor_generates_code(
    nodes: &AstNodes<'_>,
    id: NodeId,
    member: &oxc_ast::ast::StaticMemberExpression<'_>,
) -> bool {
    if let Expression::StaticMemberExpression(inner) = member.object.get_inner_expression()
        && inner.property.name == "constructor"
    {
        return true;
    }
    let (child, parent) = parent_past_wrappers(nodes, id);
    let child_span = nodes.get_node(child).kind().span();
    let code_string = |arguments: &[Argument<'_>]| {
        arguments
            .first()
            .and_then(Argument::as_expression)
            .is_some_and(|first| {
                matches!(
                    first.get_inner_expression(),
                    Expression::StringLiteral(_) | Expression::TemplateLiteral(_)
                )
            })
    };
    match nodes.get_node(parent).kind() {
        AstKind::CallExpression(call) => {
            call.callee.get_inner_expression().span() == child_span && code_string(&call.arguments)
        }
        AstKind::NewExpression(new) => {
            new.callee.get_inner_expression().span() == child_span && code_string(&new.arguments)
        }
        _ => false,
    }
}

/// Every top-level global value name a TypeScript library file declares: `declare var`/`let`/`const`, `declare function`, `declare class`
/// and `declare namespace`. `None` when it does not parse.
#[must_use]
pub fn library_globals(source: &str) -> Option<Vec<String>> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::d_ts()).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    let mut names = Vec::new();
    for statement in &parsed.program.body {
        match statement {
            Statement::VariableDeclaration(declaration) => {
                for declarator in &declaration.declarations {
                    if let oxc_ast::ast::BindingPattern::BindingIdentifier(identifier) =
                        &declarator.id
                    {
                        names.push(identifier.name.to_string());
                    }
                }
            }
            Statement::FunctionDeclaration(function) => {
                if let Some(id) = &function.id {
                    names.push(id.name.to_string());
                }
            }
            Statement::ClassDeclaration(class) => {
                if let Some(id) = &class.id {
                    names.push(id.name.to_string());
                }
            }
            Statement::TSModuleDeclaration(module) => {
                if let oxc_ast::ast::TSModuleDeclarationName::Identifier(id) = &module.id {
                    names.push(id.name.to_string());
                }
            }
            _ => {}
        }
    }
    Some(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(source: &str) -> ModuleScan {
        scan_module(Path::new("/p/node_modules/x/index.js"), source)
    }

    fn kinds(scan: &ModuleScan) -> Vec<(ModuleHazardKind, &str)> {
        scan.hazards
            .iter()
            .map(|hazard| (hazard.kind, hazard.detail.as_str()))
            .collect()
    }

    #[test]
    fn static_loads_are_collected_and_type_only_ones_are_erased() {
        let scan = scan_module(
            Path::new("/p/node_modules/x/index.ts"),
            "import a from './a.js'; import type T from 't'; export * from 'b'; export { c } from \"c\"; \
             import('d'); const e = require('e'); export type { U } from 'u';",
        );
        assert_eq!(scan.loads, ["./a.js", "b", "c", "d", "e"]);
        assert!(scan.hazards.is_empty(), "{:?}", scan.hazards);
    }

    #[test]
    fn a_computed_load_is_a_dynamic_load() {
        for source in [
            "import(name);",
            "require(name);",
            "const r = require; r('x');",
            "module.require('x');",
            "import.meta.glob('./*.js');",
            "const m = import.meta;",
        ] {
            let scan = scan(source);
            assert!(
                scan.hazards
                    .iter()
                    .any(|hazard| hazard.kind == ModuleHazardKind::DynamicLoad),
                "{source}: {:?}",
                scan.hazards
            );
        }
        assert!(scan("export const u = import.meta.url;").hazards.is_empty());
        assert!(
            scan("module.exports = { a: 1 }; exports.b = 2;")
                .hazards
                .is_empty()
        );
    }

    #[test]
    fn code_generation_is_a_hazard() {
        for source in [
            "eval('x');",
            "const e = eval;",
            "new Function('return 1');",
            "Function('return this')();",
            "(() => {}).constructor('x')();",
            "new (function () {}).constructor('return 1');",
            "export const f = (x) => x.constructor.constructor('return this')();",
        ] {
            assert!(
                kinds(&scan(source))
                    .iter()
                    .any(|(kind, _)| *kind == ModuleHazardKind::CodeGeneration),
                "{source}"
            );
        }
        assert_eq!(
            kinds(&scan("setTimeout('import(\"solid-js\")', 0);")),
            [(ModuleHazardKind::CodeGeneration, "setTimeout")]
        );
        assert!(
            scan("const tick = () => {}; setTimeout(() => {}, 0); setInterval(tick, 10);")
                .hazards
                .is_empty()
        );
        for source in [
            "export const f = (x) => x instanceof Function;",
            "export const g = Function.prototype.toString;",
            "export const n = (x) => x.constructor === Object || x.constructor.name;",
            "export const t = (x) => typeof eval;",
            // A class instance's constructor is its class: the clone idiom, a write, a switch.
            "export class V { clone() { return new this.constructor(this.x); } }",
            "function Base() {} function Sub() { this.constructor = Base; }",
            "export const k = (a) => { switch (a.constructor) { case Array: return 1; default: return 2; } };",
            "export const F = (async () => {}).constructor;",
            // esbuild's CommonJS export hint names bindings a re-export never declares: dead code.
            "0 && (module.exports = { handle, defaultIsContentTypeBinary });",
        ] {
            assert!(
                scan(source).hazards.is_empty(),
                "{source}: {:?}",
                scan(source).hazards
            );
        }
    }

    #[test]
    fn globals_are_known_by_the_libraries_names_only() {
        assert!(
            scan("export const f = () => { setTimeout(() => {}, 1); document.body; Math.max(1); window.addEventListener('x', f); }")
                .hazards
                .is_empty()
        );
        assert_eq!(
            kinds(&scan("export const e = process.env.NODE_ENV;")),
            [(ModuleHazardKind::UnknownGlobal, "process")]
        );
        assert_eq!(
            kinds(&scan("window.solidAction();")),
            [(ModuleHazardKind::UnknownGlobal, "window.solidAction")]
        );
        assert_eq!(
            kinds(&scan("export const f = (key) => globalThis[key]();")),
            [(ModuleHazardKind::GlobalObjectEscape, "globalThis")]
        );
        assert_eq!(
            kinds(&scan("const g = window;")),
            [(ModuleHazardKind::GlobalObjectEscape, "window")]
        );
        // `typeof` reads no value; a write looks nothing up.
        assert!(
            scan("export const b = typeof window !== 'undefined' && typeof process === 'object';")
                .hazards
                .is_empty()
        );
        assert!(scan("window.__x = 1;").hazards.is_empty());
        assert!(
            scan("globalThis['setTimeout'](() => {});")
                .hazards
                .is_empty()
        );
    }

    /// ADR 0973: the global object goes by more than one name, and a member read off any of them is a lookup of a global (the review of #231: `window.self.x` was
    /// judged clear where `window.x` was an unknown global).
    #[test]
    fn the_global_object_is_followed_through_the_members_that_return_it() {
        for (source, detail) in [
            (
                "export const f = (cb) => window.self.__act(cb);",
                "window.self.__act",
            ),
            (
                "globalThis.globalThis.__act();",
                "globalThis.globalThis.__act",
            ),
            (
                "window.window.window.__act();",
                "window.window.window.__act",
            ),
            ("self.self.__act();", "self.self.__act"),
            ("window.top.parent.__act();", "window.top.parent.__act"),
            ("window.frames.__act();", "window.frames.__act"),
            ("window['self'].__act();", "window.self.__act"),
            ("window.self['__act']();", "window.self.__act"),
            ("(window as any).self.__act();", "window.self.__act"),
            (
                "document.defaultView.__act();",
                "document.defaultView.__act",
            ),
            (
                "window.document.defaultView.__act();",
                "window.document.defaultView.__act",
            ),
            (
                "document.defaultView.self.__act();",
                "document.defaultView.self.__act",
            ),
        ] {
            let scan = scan_module(Path::new("/p/node_modules/x/index.ts"), source);
            assert_eq!(
                kinds(&scan),
                [(ModuleHazardKind::UnknownGlobal, detail)],
                "{source}"
            );
        }
        // What the libraries declare is no lookup of an unknown global, however the global object was reached; the rest of `document` holds no global object.
        for source in [
            "window.self.setTimeout(() => {});",
            "window.self.window.document.body;",
            "document.defaultView.addEventListener('x', () => {});",
            "document.defaultView.document.title;",
            "export const t = document.title; document.body.__x; document.__x;",
            "const d = document; d.createElement('div');",
            "const run = (x) => x; run(document);",
            // A write reads nothing, however the global object was reached.
            "window.self.__x = 1; document.defaultView.__y = 2; globalThis.globalThis.__z = 3;",
        ] {
            let scan = scan_module(Path::new("/p/node_modules/x/index.ts"), source);
            assert!(scan.hazards.is_empty(), "{source}: {:?}", scan.hazards);
        }
        // The global object itself handed on, or read by a computed key, at any step of the chain.
        for (source, detail) in [
            ("const w = window.self;", "window.self"),
            ("const run = (x) => x; run(window.top);", "window.top"),
            ("export const f = (k) => window.self[k];", "window.self"),
            (
                "export const g = (k) => document.defaultView[k]();",
                "document.defaultView",
            ),
            (
                "export const w = document.defaultView;",
                "document.defaultView",
            ),
            (
                "export const h = (k) => window.document.defaultView.self[k];",
                "window.document.defaultView.self",
            ),
        ] {
            assert_eq!(
                kinds(&scan(source)),
                [(ModuleHazardKind::GlobalObjectEscape, detail)],
                "{source}"
            );
        }
    }

    /// ADR 0973: every way a file defines an accessor, under the name it defines it under (a getter a package declares runs its code where the project reads a
    /// member: `void clock.now`).
    #[test]
    fn accessors_are_found_by_every_way_to_define_one() {
        let accessors = |source: &str| scan(source).accessors;
        // Class and object literal accessors, instance, static and string-keyed.
        assert_eq!(
            accessors(
                "export class C { get a() { return 1; } set b(v) {} static get c() { return 1; } }"
            )
            .names,
            ["a", "b", "c"]
        );
        assert_eq!(
            accessors("export const o = { get now() { return 1; }, set 'lit'(v) {}, get 7() { return 1; } };")
                .names,
            ["7", "lit", "now"]
        );
        // The defineProperty family, with a literal name and a descriptor that names a get or a set.
        assert_eq!(
            accessors("Object.defineProperty(o, 'x', { get() { return 1; } });").names,
            ["x"]
        );
        assert_eq!(
            accessors("Reflect.defineProperty(o, 'y', { enumerable: true, set: (v) => {} });")
                .names,
            ["y"]
        );
        assert_eq!(
            accessors("Object.defineProperties(o, { a: { get() {} }, b: { value: 1 } });").names,
            ["a"]
        );
        assert_eq!(
            accessors("export const o = Object.create(p, { z: { get() {} } });").names,
            ["z"]
        );
        assert_eq!(
            accessors("o.__defineGetter__('g', f); o.__defineSetter__('s', f);").names,
            ["g", "s"]
        );
        // A descriptor the syntax does not read may be an accessor under the literal name.
        let unread = accessors("Object.defineProperty(o, 'x', descriptor);");
        assert_eq!(unread.names, ["x"]);
        assert!(!unread.any_name);
        // Under a name the syntax does not fix, or by a construct read as a value: every name.
        for source in [
            "Object.defineProperty(o, k, { get() {} });",
            "Object.defineProperty(o, k, descriptor);",
            "const o = { get [k]() { return 1; } };",
            "class C { static get [k]() { return 1; } }",
            "const view = new Proxy(target, handler);",
            "const def = Object.defineProperty;",
            "const { defineProperty } = Object;",
            "Object['defineProperties'](o, props);",
            "Object.defineProperties(o, { ...more });",
            "Object.defineProperties(o, props);",
            "Object.create(p, descriptors);",
        ] {
            assert!(accessors(source).any_name, "{source}");
        }
        // None: a data descriptor, an object with no descriptors, a private accessor no code outside the class reads, a plain file.
        for source in [
            "Object.defineProperty(exports, '__esModule', { value: true });",
            "Object.defineProperty(o, 'x', { value: 1, enumerable: false });",
            "export const o = Object.create(proto);",
            "class C { get #p() { return 1; } static m() {} }",
            "export const f = (x) => x.get(1) + x.set(2); export const o = { get: 1, set: 2 };",
        ] {
            assert!(
                accessors(source).is_empty(),
                "{source}: {:?}",
                accessors(source)
            );
        }
    }

    #[test]
    fn a_type_reference_looks_nothing_up() {
        let scan = scan_module(
            Path::new("/p/node_modules/x/index.ts"),
            "export function f(callback: Function, session: XRSession): Promise<void> { return callback as never; }",
        );
        assert!(scan.hazards.is_empty(), "{:?}", scan.hazards);
    }

    #[test]
    fn an_unparseable_file_is_a_hazard() {
        assert_eq!(
            scan("export const = ;").hazards[0].kind,
            ModuleHazardKind::Unparseable
        );
    }

    #[test]
    fn a_local_binding_shadows_a_global() {
        assert!(
            scan("const process = { env: {} }; export const e = process.env;")
                .hazards
                .is_empty()
        );
        assert!(
            scan("export function f(require) { return require('x'); }")
                .hazards
                .is_empty()
        );
        assert!(
            scan("export function f(require) { return require('x'); }")
                .loads
                .is_empty()
        );
    }

    /// Regenerates `module_reach/known_globals.txt` from the libraries of the tsrs revision the producer pins:
    /// `SOLID_FACTS_TS_LIBS=<checkout>/crates/tsrs_vfs/libs cargo test -p solid-facts known_globals -- --ignored`.
    #[test]
    #[ignore = "regenerates a checked-in file from the tsrs checkout"]
    fn known_globals_are_the_pinned_libraries_names() {
        let directory = std::env::var("SOLID_FACTS_TS_LIBS").expect("SOLID_FACTS_TS_LIBS");
        let mut names = std::collections::BTreeSet::new();
        let mut files = std::fs::read_dir(&directory)
            .expect("libs")
            .map(|entry| entry.expect("entry").path())
            .filter(|path| {
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default();
                (name.starts_with("lib.es") || name.starts_with("lib.dom"))
                    && name.ends_with(".d.ts")
            })
            .collect::<Vec<_>>();
        files.sort();
        for file in files {
            let source = std::fs::read_to_string(&file).expect("read");
            names.extend(
                library_globals(&source).unwrap_or_else(|| panic!("{} parses", file.display())),
            );
        }
        let text = names
            .into_iter()
            .map(|name| name + "\n")
            .collect::<String>();
        std::fs::write(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/module_reach/known_globals.txt"
            ),
            text,
        )
        .expect("write");
    }
}
