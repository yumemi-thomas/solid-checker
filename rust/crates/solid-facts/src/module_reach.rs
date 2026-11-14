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
//! - **accessors** (ADR 0974): the names a file defines a `get` or `set` under (class and object literal accessors, `defineProperty`, `defineProperties`,
//!   `Object.create` with descriptors, `__defineGetter__`, `accessor` fields), and whether it defines one under a name the syntax does not fix (a computed key, a
//!   `Proxy`, `defineProperty` read as a value): a member read of a package's object runs the package's code only where one matches.
//!
//! The global object goes by several names, and a member of one that gives it back is the same global (`window.self.x`, `globalThis.globalThis.x`,
//! `window.document.defaultView.x`): a member read off any of them that no library declares is a lookup of an unknown global (ADR 0974).
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
    Argument, BindingPattern, Expression, ForStatementLeft, ImportOrExportKind,
    MethodDefinitionKind, ObjectPropertyKind, PropertyKey, PropertyKind, Statement,
    TSModuleReference, UnaryOperator, VariableDeclarationKind,
};
use oxc_parser::Parser;
use oxc_semantic::{AstNodes, NodeId, Scoping, SemanticBuilder, SymbolId};
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

/// The ways a file defines an accessor (a `get` or `set` that runs code when a member is read or assigned), by the name it defines them under (ADR 0974).
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

/// The members of the global object a file writes at run time (ADR 0974): `window.x = …` (any assignment operator), `globalThis["x"] = …`, `Object.defineProperty(globalThis, "x", …)`,
/// `Object.assign(window, { x })`, `Reflect.set`, an unbound `x = …`, and a writer function (`Object.assign`, `Reflect.defineProperty`, …) used where the target is not seen.
/// A call of a global some installed package writes runs that package's code, whatever type the library declares it with.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GlobalWrites {
    /// The names written, sorted and distinct.
    pub names: Vec<String>,
    /// A global is written under a name the syntax does not fix (`window[key] = v`, `Object.assign(globalThis, source)`, `Object.defineProperty(window, key, d)`): any may be.
    /// A key the file's own constants decide is a name (`const KEY = "x"`, `for (const k of ["a", "b"])`, `Object.assign(globalThis, table)` over a `const` object literal),
    /// and a key that is a symbol writes no name.
    pub any_name: bool,
    /// The names the file defines an accessor (a getter or a setter) under on the global object, or may: a descriptor the syntax does not read is one that may be. A read of
    /// such a global runs the file's code, which a data write does not (sorted and distinct).
    pub accessor_names: Vec<String>,
    /// An accessor may be defined on the global object under a name nothing fixes: a key the syntax does not decide, a change of its prototype (`Object.setPrototypeOf`,
    /// `globalThis.__proto__ = p`: the members of `p` become globals), a writer that is not seen to be given a fresh object.
    pub accessors_any_name: bool,
}

impl GlobalWrites {
    /// Whether the file writes no global at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
            && !self.any_name
            && self.accessor_names.is_empty()
            && !self.accessors_any_name
    }

    fn name(&mut self, name: &str) {
        // `globalThis.__proto__ = p` makes the members of `p` globals (an unbound name is looked up through the global object's prototype chain): any name, and any of them
        // may be an accessor.
        if name == "__proto__" {
            self.any_name = true;
            self.accessors_any_name = true;
            return;
        }
        self.names.push(name.to_owned());
    }

    /// The writes a key decides: its names, nothing for a symbol, any for a key that is not known.
    fn keys(&mut self, keys: Keys) {
        self.define(keys, false);
    }

    /// The writes a key decides; `accessor` where the write may define a getter or a setter, which a read of the global then runs.
    fn define(&mut self, keys: Keys, accessor: bool) {
        match keys {
            Keys::Names(names) => {
                for name in names {
                    self.name(&name);
                    if accessor && name != "__proto__" {
                        self.accessor_names.push(name);
                    }
                }
            }
            Keys::Symbol => {}
            Keys::Unknown => {
                self.any_name = true;
                self.accessors_any_name |= accessor;
            }
        }
    }

    /// A write whose target or key nothing decides: any global, an accessor under any name where the writer defines accessors.
    fn undecided(&mut self, accessor: bool) {
        self.any_name = true;
        self.accessors_any_name |= accessor;
    }
}

/// What the key of a write is, where its syntax and the file's own constants say.
#[derive(Debug)]
enum Keys {
    /// The names it may be.
    Names(Vec<String>),
    /// A symbol (`Symbol("x")`, `Symbol.for("x")`): no global has a symbol for a name.
    Symbol,
    /// Not known.
    Unknown,
}

impl Keys {
    fn unite(self, other: Keys) -> Keys {
        match (self, other) {
            (Keys::Unknown, _) | (_, Keys::Unknown) => Keys::Unknown,
            (Keys::Names(mut names), Keys::Names(more)) => {
                names.extend(more);
                Keys::Names(names)
            }
            (Keys::Names(names), Keys::Symbol) | (Keys::Symbol, Keys::Names(names)) => {
                Keys::Names(names)
            }
            (Keys::Symbol, Keys::Symbol) => Keys::Symbol,
        }
    }
}

/// How far a key is followed through the constants of the file.
const KEY_DEPTH: u8 = 6;

/// The key `expression` is: a literal, a call of `Symbol`, a `const` (or a `let` nothing reassigns) of one, a loop variable over a known array or object, or either arm of
/// a conditional or logical expression.
fn key_of(scoping: &Scoping, nodes: &AstNodes<'_>, expression: &Expression<'_>, depth: u8) -> Keys {
    if depth == 0 {
        return Keys::Unknown;
    }
    match expression.get_inner_expression() {
        Expression::StringLiteral(literal) => Keys::Names(vec![literal.value.to_string()]),
        Expression::NumericLiteral(literal) => Keys::Names(vec![literal.value.to_string()]),
        Expression::TemplateLiteral(template) if template.expressions.is_empty() => template
            .quasis
            .first()
            .and_then(|quasi| quasi.value.cooked.as_ref())
            .map_or(Keys::Unknown, |value| Keys::Names(vec![value.to_string()])),
        Expression::CallExpression(call) if is_symbol_call(scoping, call) => Keys::Symbol,
        Expression::ConditionalExpression(conditional) => key_of(
            scoping,
            nodes,
            &conditional.consequent,
            depth - 1,
        )
        .unite(key_of(scoping, nodes, &conditional.alternate, depth - 1)),
        Expression::LogicalExpression(logical) => key_of(scoping, nodes, &logical.left, depth - 1)
            .unite(key_of(scoping, nodes, &logical.right, depth - 1)),
        Expression::Identifier(identifier) => {
            let Some((declarator, _)) = constant_declarator(scoping, nodes, identifier) else {
                return Keys::Unknown;
            };
            if let Some(keys) = loop_keys(scoping, nodes, declarator, depth - 1) {
                return keys;
            }
            match &declarator_of(nodes, declarator).and_then(|declarator| declarator.init.as_ref())
            {
                Some(init) => key_of(scoping, nodes, init, depth - 1),
                None => Keys::Unknown,
            }
        }
        _ => Keys::Unknown,
    }
}

/// Whether `call` is `Symbol(…)` or `Symbol.for(…)`, a call of the global `Symbol` (a name the file binds is its own).
fn is_symbol_call(scoping: &Scoping, call: &oxc_ast::ast::CallExpression<'_>) -> bool {
    let unbound = |identifier: &oxc_ast::ast::IdentifierReference<'_>| {
        identifier.name == "Symbol"
            && identifier
                .reference_id
                .get()
                .is_some_and(|id| scoping.get_reference(id).symbol_id().is_none())
    };
    match call.callee.get_inner_expression() {
        Expression::Identifier(callee) => unbound(callee),
        Expression::StaticMemberExpression(callee) => {
            callee.property.name == "for"
                && matches!(callee.object.get_inner_expression(), Expression::Identifier(object) if unbound(object))
        }
        _ => false,
    }
}

/// The declarator of the binding `identifier` names, with its symbol, when it is a plain `const`, `let` or `var` that nothing reassigns and nothing declares again. A `var` read
/// before its initializer ran holds `undefined` (a key that names no global a call can run), so it is no less known than a `const`; bundled code declares its constants so.
fn constant_declarator(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    identifier: &oxc_ast::ast::IdentifierReference<'_>,
) -> Option<(NodeId, SymbolId)> {
    let symbol = scoping
        .get_reference(identifier.reference_id.get()?)
        .symbol_id()?;
    if scoping.symbol_is_mutated(symbol) {
        return None;
    }
    let declaration = scoping.symbol_declaration(symbol);
    // A binding of a destructuring pattern (`const { length } = "abc"`) is not its declarator's initializer.
    let plain = matches!(nodes.kind(declaration), AstKind::VariableDeclarator(declarator)
        if matches!(declarator.id, BindingPattern::BindingIdentifier(_)));
    let declared_once = scoping.symbol_redeclarations(symbol).is_empty();
    let kind_known = matches!(nodes.parent_kind(declaration), AstKind::VariableDeclaration(held)
        if matches!(held.kind, VariableDeclarationKind::Const | VariableDeclarationKind::Let | VariableDeclarationKind::Var));
    (plain && declared_once && kind_known).then_some((declaration, symbol))
}

/// Whether the array or object a binding holds is only read after its initializer: every reference iterates it, spreads it, hands it to `Object.keys`, `Object.entries`,
/// `Object.values` or `Object.freeze`, gives it as a source to `Object.assign`, or reads a member of it. A call of a member (`table.push(k)`), an assignment to or
/// deletion of one, and any other use (an argument, a return, an alias) may change the keys it holds.
fn table_is_unmodified(scoping: &Scoping, nodes: &AstNodes<'_>, symbol: SymbolId) -> bool {
    scoping.get_resolved_references(symbol).all(|reference| {
        if reference.is_write() {
            return false;
        }
        let (child, parent) = parent_past_wrappers(nodes, reference.node_id());
        let span = nodes.get_node(child).kind().span();
        match nodes.get_node(parent).kind() {
            AstKind::ForOfStatement(each) => each.right.span() == span,
            AstKind::ForInStatement(each) => each.right.span() == span,
            AstKind::SpreadElement(_) => true,
            AstKind::StaticMemberExpression(member) => {
                member.object.span() == span && !is_changing_use(nodes, parent)
            }
            AstKind::ComputedMemberExpression(member) => {
                member.object.span() == span && !is_changing_use(nodes, parent)
            }
            AstKind::CallExpression(call) => {
                let Expression::StaticMemberExpression(callee) = call.callee.get_inner_expression()
                else {
                    return false;
                };
                let Expression::Identifier(object) = callee.object.get_inner_expression() else {
                    return false;
                };
                let first = call
                    .arguments
                    .first()
                    .is_some_and(|argument| argument.span() == span);
                object.name == "Object"
                    && match callee.property.name.as_str() {
                        "keys" | "entries" | "values" | "freeze" => true,
                        "assign" => !first,
                        _ => false,
                    }
            }
            _ => false,
        }
    })
}

/// Whether the node at `node` (past parentheses and type sugar) is used in a way that changes it or what it holds: assigned, updated, deleted, a destructuring target, the head of
/// a loop, or the callee of a call or a tag (a method may change its receiver).
fn is_changing_use(nodes: &AstNodes<'_>, node: NodeId) -> bool {
    let (child, parent) = parent_past_wrappers(nodes, node);
    let span = nodes.get_node(child).kind().span();
    match nodes.get_node(parent).kind() {
        AstKind::AssignmentExpression(assignment) => assignment.left.span() == span,
        AstKind::UpdateExpression(_)
        | AstKind::ArrayAssignmentTarget(_)
        | AstKind::ObjectAssignmentTarget(_)
        | AstKind::AssignmentTargetWithDefault(_)
        | AstKind::AssignmentTargetPropertyProperty(_)
        | AstKind::AssignmentTargetRest(_) => true,
        AstKind::UnaryExpression(unary) => unary.operator == UnaryOperator::Delete,
        AstKind::CallExpression(call) => call.callee.span() == span,
        AstKind::NewExpression(call) => call.callee.span() == span,
        AstKind::TaggedTemplateExpression(tag) => tag.tag.span() == span,
        AstKind::ForInStatement(each) => each.left.span() == span,
        AstKind::ForOfStatement(each) => each.left.span() == span,
        _ => false,
    }
}

fn declarator_of<'a>(
    nodes: &'a AstNodes<'_>,
    node: NodeId,
) -> Option<&'a oxc_ast::ast::VariableDeclarator<'a>> {
    match nodes.kind(node) {
        AstKind::VariableDeclarator(declarator) => Some(declarator),
        _ => None,
    }
}

/// The keys a loop variable takes (`for (const key of ["a", "b"])`, `for (const key in table)`, `for (const key of Object.keys(table))`) where the collection is known.
fn loop_keys(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    declarator: NodeId,
    depth: u8,
) -> Option<Keys> {
    let declaration = nodes.parent_id(declarator);
    let statement = nodes.parent_id(declaration);
    let declaration_span = nodes.get_node(declaration).kind().span();
    match nodes.kind(statement) {
        AstKind::ForInStatement(each) if matches!(&each.left, ForStatementLeft::VariableDeclaration(left) if left.span == declaration_span) => {
            Some(match literal_names(scoping, nodes, &each.right, depth) {
                Some(keys) => Keys::Names(keys),
                None => Keys::Unknown,
            })
        }
        AstKind::ForOfStatement(each) if matches!(&each.left, ForStatementLeft::VariableDeclaration(left) if left.span == declaration_span) => {
            Some(collection_keys(scoping, nodes, &each.right, depth))
        }
        _ => None,
    }
}

/// The keys an iterated collection yields: the elements of an array literal (or of a `const` bound to one) that are keys, or `Object.keys(object)` of a known object.
fn collection_keys(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    expression: &Expression<'_>,
    depth: u8,
) -> Keys {
    if depth == 0 {
        return Keys::Unknown;
    }
    match expression.get_inner_expression() {
        Expression::ArrayExpression(array) => {
            let mut keys = Keys::Names(Vec::new());
            for element in &array.elements {
                let Some(element) = element.as_expression() else {
                    return Keys::Unknown;
                };
                keys = keys.unite(key_of(scoping, nodes, element, depth - 1));
            }
            keys
        }
        Expression::CallExpression(call) => {
            let Expression::StaticMemberExpression(callee) = call.callee.get_inner_expression()
            else {
                return Keys::Unknown;
            };
            if callee.property.name != "keys"
                || !matches!(callee.object.get_inner_expression(), Expression::Identifier(object) if object.name == "Object")
            {
                return Keys::Unknown;
            }
            match call.arguments.first().and_then(Argument::as_expression) {
                Some(object) => match literal_names(scoping, nodes, object, depth - 1) {
                    Some(keys) => Keys::Names(keys),
                    None => Keys::Unknown,
                },
                None => Keys::Unknown,
            }
        }
        Expression::Identifier(identifier) => {
            let Some((declarator, symbol)) = constant_declarator(scoping, nodes, identifier) else {
                return Keys::Unknown;
            };
            match declarator_of(nodes, declarator).and_then(|declarator| declarator.init.as_ref()) {
                Some(init) if table_is_unmodified(scoping, nodes, symbol) => {
                    collection_keys(scoping, nodes, init, depth - 1)
                }
                _ => Keys::Unknown,
            }
        }
        _ => Keys::Unknown,
    }
}

/// The names a `const` object literal holds (its own keys, a spread of another known object's): `None` where any key or spread is not known.
fn literal_names(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    expression: &Expression<'_>,
    depth: u8,
) -> Option<Vec<String>> {
    if depth == 0 {
        return None;
    }
    match expression.get_inner_expression() {
        Expression::ObjectExpression(object) => {
            let mut names = Vec::new();
            for property in &object.properties {
                match property {
                    ObjectPropertyKind::ObjectProperty(property) => {
                        let key = match key_name(&property.key) {
                            Some(Some(name)) => Keys::Names(vec![name]),
                            Some(None) => Keys::Symbol,
                            None => property
                                .key
                                .as_expression()
                                .map_or(Keys::Unknown, |expression| {
                                    key_of(scoping, nodes, expression, depth - 1)
                                }),
                        };
                        match key {
                            Keys::Names(more) => names.extend(more),
                            Keys::Symbol => {}
                            Keys::Unknown => return None,
                        }
                    }
                    ObjectPropertyKind::SpreadProperty(spread) => {
                        names.extend(literal_names(scoping, nodes, &spread.argument, depth - 1)?);
                    }
                }
            }
            Some(names)
        }
        Expression::Identifier(identifier) => {
            let (declarator, symbol) = constant_declarator(scoping, nodes, identifier)?;
            let init = declarator_of(nodes, declarator)?.init.as_ref()?;
            table_is_unmodified(scoping, nodes, symbol)
                .then(|| literal_names(scoping, nodes, init, depth - 1))
                .flatten()
        }
        Expression::CallExpression(call) => {
            // `Object.freeze(table)` is the table.
            let Expression::StaticMemberExpression(callee) = call.callee.get_inner_expression()
            else {
                return None;
            };
            if callee.property.name != "freeze"
                || !matches!(callee.object.get_inner_expression(), Expression::Identifier(object) if object.name == "Object")
            {
                return None;
            }
            literal_names(
                scoping,
                nodes,
                call.arguments.first()?.as_expression()?,
                depth - 1,
            )
        }
        _ => None,
    }
}

/// Whether `name` is one of the names the global object goes by (`window`, `self`, `globalThis`, …).
#[must_use]
pub fn is_global_object_name(name: &str) -> bool {
    GLOBAL_OBJECTS.contains(&name)
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
    /// The globals it writes.
    pub global_writes: GlobalWrites,
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
    let mut writes = GlobalWrites::default();
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
            // An unbound name assigned (`setTimeout = fn`, sloppy code) installs a global of that name.
            if reference.is_write()
                && !GLOBAL_OBJECTS.contains(&name)
                && !matches!(
                    name,
                    "exports"
                        | "module"
                        | "require"
                        | "__filename"
                        | "__dirname"
                        | "undefined"
                        | "NaN"
                        | "Infinity"
                        | "arguments"
                        | "document"
                )
            {
                writes.name(name);
            }
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
                "eval" => {
                    hazard(ModuleHazardKind::CodeGeneration, "eval");
                    code_generated(&mut writes);
                }
                // A timer given a string evaluates it (`setTimeout("code", 0)`): as `eval`. A function or a name is the timer's ordinary use.
                "setTimeout" | "setInterval" | "setImmediate" | "execScript"
                    if string_timer(nodes, id) =>
                {
                    hazard(ModuleHazardKind::CodeGeneration, name);
                    code_generated(&mut writes);
                }
                "Function" => {
                    if !function_use_is_inert(nodes, id) {
                        hazard(ModuleHazardKind::CodeGeneration, "Function");
                        code_generated(&mut writes);
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
                "document" => match global_use(scoping, nodes, id, name, Holder::Document) {
                    GlobalUse::Member {
                        name: member,
                        path,
                        at,
                    } => {
                        defined_by_receiver(scoping, nodes, &member, at, &mut writes);
                        if generates_code_by_member(nodes, &member, at) {
                            hazard(ModuleHazardKind::CodeGeneration, &path);
                            code_generated(&mut writes);
                        }
                        if !is_known_global(&member) {
                            hazard(ModuleHazardKind::UnknownGlobal, &path);
                        }
                    }
                    GlobalUse::Escape { path, at } => {
                        hazard(ModuleHazardKind::GlobalObjectEscape, &path);
                        if !call_writes(scoping, nodes, at, &mut writes) {
                            escaped(&mut writes);
                        }
                    }
                    // `document.defaultView.x = …` writes the global `x`.
                    GlobalUse::Written { keys, path, read } => {
                        written(&mut hazard, &mut writes, keys, &path, read);
                    }
                    GlobalUse::Reassigned | GlobalUse::Inert => {}
                },
                _ if GLOBAL_OBJECTS.contains(&name) => {
                    match global_use(scoping, nodes, id, name, Holder::Global) {
                        GlobalUse::Member {
                            name: member,
                            path,
                            at,
                        } => {
                            defined_by_receiver(scoping, nodes, &member, at, &mut writes);
                            if generates_code_by_member(nodes, &member, at) {
                                hazard(ModuleHazardKind::CodeGeneration, &path);
                                code_generated(&mut writes);
                            }
                            if member == "valueOf" {
                                hazard(ModuleHazardKind::GlobalObjectEscape, &path);
                                escaped(&mut writes);
                            }
                            if !is_known_global(&member) {
                                hazard(ModuleHazardKind::UnknownGlobal, &path);
                            }
                        }
                        // A write reads nothing (a compound operator reads the member first), but it installs a global: the name, the names the key's constants decide,
                        // none for a symbol, or any where the key is not known (which also hands the object on).
                        GlobalUse::Written { keys, path, read } => {
                            written(&mut hazard, &mut writes, keys, &path, read);
                        }
                        GlobalUse::Reassigned | GlobalUse::Inert => {}
                        GlobalUse::Escape { path, at } => {
                            hazard(ModuleHazardKind::GlobalObjectEscape, &path);
                            if !call_writes(scoping, nodes, at, &mut writes) {
                                escaped(&mut writes);
                            }
                        }
                    }
                }
                _ if is_known_global(name) => {}
                _ if !read => {}
                _ => hazard(ModuleHazardKind::UnknownGlobal, name),
            }
        }
    }
    // `this` at the top of a script is the global object, and `defaultView` read off any object is (ADR 0974).
    if !has_module_syntax(&program) {
        for node in nodes.iter() {
            if matches!(node.kind(), AstKind::ThisExpression(_))
                && is_script_top_this(nodes, node.id())
                && !this_never_flows(nodes, node.id())
            {
                route_use(
                    scoping,
                    nodes,
                    node.id(),
                    "this",
                    true,
                    &mut hazard,
                    &mut writes,
                );
            }
        }
    }
    {
        for node in nodes.iter() {
            if let AstKind::StaticMemberExpression(member) = node.kind()
                && member.property.name == "defaultView"
                && !rooted_in_global(scoping, &member.object)
            {
                route_use(
                    scoping,
                    nodes,
                    node.id(),
                    "defaultView",
                    false,
                    &mut hazard,
                    &mut writes,
                );
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
            code_generated(&mut writes);
        }
    }
    loads.sort();
    loads.dedup();
    hazards.sort();
    hazards.dedup();
    scan.loads = loads;
    scan.hazards = hazards;
    scan.accessors = scan_accessors(nodes);
    // The writer functions used where the global object they write is not named (ADR 0974).
    indirect_writers(scoping, nodes, &mut writes);
    writes.names.sort();
    writes.names.dedup();
    writes.accessor_names.sort();
    writes.accessor_names.dedup();
    scan.global_writes = writes;
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
    /// A member of the global object read by a static name or a string literal key: its name, the chain as it was spelled (`window.self.solid`), and the node of the member
    /// at the chain's end.
    Member {
        name: String,
        path: String,
        at: NodeId,
    },
    /// A member of the global object assigned (`window.x = …`, `window["x"] = …`, `window.x ||= …`, any assignment operator): the global `x` is installed; `keys` is what the
    /// key is where it is not a name the syntax fixes (`window[k] = …`: the names the file's constants say, a symbol, or nothing known, where any global may be). The chain as
    /// it was spelled. A plain assignment reads nothing; `read` is whether the operator reads the member first (`+=`, `&&=`, `||=`, `??=`).
    Written {
        keys: Keys,
        path: String,
        read: bool,
    },
    /// The global object's own name assigned (sloppy code): nothing is read.
    Reassigned,
    /// Anything else done with the global object: a computed key read, an alias, an argument. The chain as it was spelled, and the node whose value is used.
    Escape { path: String, at: NodeId },
    /// `document` used for anything but `document.defaultView`.
    Inert,
}

/// How the global-object name (or `document`) at `id` is used, following the global object through every member that returns it: the names it goes by
/// ([`GLOBAL_OBJECTS`], so `window.self.x` and `window.window.x` are `window.x`) and `document.defaultView`. A member read off the chain's end that no
/// library declares is a lookup of a global however many members led to it.
fn global_use(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    id: NodeId,
    root: &str,
    start: Holder,
) -> GlobalUse {
    // `window.x = …` writes the member and reads nothing of it; so does a destructuring target (`[window.x] = list`, `({ k: self.y } = o)`, `[globalThis.z = 1] = []`) and a
    // loop head (`for (window.k of keys)`).
    // `Some(reads)` when the member is the target of an assignment of any operator: `reads` where the operator reads the member before it writes it (`+=`, `&&=`, `||=`, `??=`).
    let assigned = |member: NodeId| {
        let (member_child, member_parent) = parent_past_wrappers(nodes, member);
        let span = nodes.get_node(member_child).kind().span();
        match nodes.get_node(member_parent).kind() {
            AstKind::AssignmentExpression(assignment) if assignment.left.span() == span => {
                Some(assignment.operator != oxc_syntax::operator::AssignmentOperator::Assign)
            }
            AstKind::ArrayAssignmentTarget(_)
            | AstKind::AssignmentTargetWithDefault(_)
            | AstKind::AssignmentTargetPropertyProperty(_)
            | AstKind::AssignmentTargetRest(_) => Some(false),
            AstKind::ForInStatement(each) if each.left.span() == span => Some(false),
            AstKind::ForOfStatement(each) if each.left.span() == span => Some(false),
            _ => None,
        }
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
                // A key the file's own constants decide to one name is that name, as a write by it is (`const KEY = "x"; window[KEY]`).
                Some(literal_specifier(&member.expression).or_else(|| {
                    match key_of(scoping, nodes, &member.expression, KEY_DEPTH) {
                        Keys::Names(mut names) if names.len() == 1 => names.pop(),
                        _ => None,
                    }
                }))
            }
            AstKind::AssignmentExpression(assignment) if assignment.left.span() == child_span => {
                return GlobalUse::Reassigned;
            }
            _ => None,
        };
        // The value itself is used, or read by a key the syntax does not fix: the global object escapes, `document` holds no global object. A key the syntax does not
        // fix that is assigned writes what the key is: the names the file's constants decide, a symbol, or any global.
        let Some(Some(name)) = member else {
            if computed
                && holder == Holder::Global
                && let Some(read) = assigned(parent)
            {
                let keys = match nodes.get_node(parent).kind() {
                    AstKind::ComputedMemberExpression(member) => {
                        key_of(scoping, nodes, &member.expression, KEY_DEPTH)
                    }
                    _ => Keys::Unknown,
                };
                return GlobalUse::Written {
                    keys,
                    path: format!("{path}[…]"),
                    read,
                };
            }
            return match holder {
                Holder::Global => GlobalUse::Escape { path, at },
                Holder::Document => GlobalUse::Inert,
            };
        };
        if let Some(read) = assigned(parent) {
            // `window.document.title = …` writes `document`'s member, no global.
            if holder == Holder::Document {
                return GlobalUse::Inert;
            }
            return GlobalUse::Written {
                keys: Keys::Names(vec![name.clone()]),
                path: format!("{path}.{name}"),
                read,
            };
        }
        path = format!("{path}.{name}");
        match holder {
            Holder::Document if name == "defaultView" => holder = Holder::Global,
            Holder::Document => return GlobalUse::Inert,
            Holder::Global if GLOBAL_OBJECTS.contains(&name.as_str()) => {}
            Holder::Global if name == "document" => holder = Holder::Document,
            Holder::Global => {
                return GlobalUse::Member {
                    name,
                    path,
                    at: parent,
                };
            }
        }
        at = parent;
    }
}

/// A member of the global object written under `keys`: the global is installed, a key nothing decides hands the object on, and an operator that reads the member first
/// (`window.x += 1`, `window.x ||= f`) looks the name up too.
fn written(
    hazard: &mut impl FnMut(ModuleHazardKind, &str),
    writes: &mut GlobalWrites,
    keys: Keys,
    path: &str,
    read: bool,
) {
    if matches!(keys, Keys::Unknown) {
        hazard(ModuleHazardKind::GlobalObjectEscape, path);
    }
    if read
        && let Keys::Names(names) = &keys
        && names.iter().any(|name| !is_known_global(name))
    {
        hazard(ModuleHazardKind::UnknownGlobal, path);
    }
    writes.keys(keys);
}

/// A call of the global object's own `__defineGetter__` or `__defineSetter__` (`window.__defineGetter__("x", get)`, the member at `at`) defines an accessor global under
/// its first argument; the member used any other way (`.call`, an alias) defines one under a name nothing fixes.
fn defined_by_receiver(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    member: &str,
    at: NodeId,
    writes: &mut GlobalWrites,
) {
    if !matches!(member, "__defineGetter__" | "__defineSetter__") {
        return;
    }
    let (child, parent) = parent_past_wrappers(nodes, at);
    match nodes.get_node(parent).kind() {
        AstKind::CallExpression(call)
            if call.callee.get_inner_expression().span() == nodes.get_node(child).kind().span() =>
        {
            let keys = call
                .arguments
                .first()
                .and_then(Argument::as_expression)
                .map_or(Keys::Unknown, |key| key_of(scoping, nodes, key, KEY_DEPTH));
            writes.define(keys, true);
        }
        _ => writes.undecided(true),
    }
}

/// The globals a call writes when the global object (the value of the node `at`) is its target: `Object.defineProperty(globalThis, "x", d)`, `Reflect.set(window, "x", v)`,
/// `Object.defineProperties(window, { x: d })`, `Object.assign(globalThis, { x })`, `Object.setPrototypeOf(window, p)`. A key or a source the syntax does not fix writes any.
///
/// Answers whether the call was judged to the end: one of those writers, run as a statement, so that the object it returns goes nowhere (each of them returns its target, which
/// `const w = Object.defineProperty(window, "k", d)` would keep). Where it is not, the global object was handed on and [`escaped`] says what that costs.
fn call_writes(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    at: NodeId,
    writes: &mut GlobalWrites,
) -> bool {
    let (child, parent) = parent_past_wrappers(nodes, at);
    let AstKind::CallExpression(call) = nodes.get_node(parent).kind() else {
        return false;
    };
    if call
        .arguments
        .first()
        .and_then(Argument::as_expression)
        .is_none_or(|first| first.span() != nodes.get_node(child).kind().span())
    {
        return false;
    }
    let Expression::StaticMemberExpression(callee) = call.callee.get_inner_expression() else {
        return false;
    };
    if !matches!(
        callee.object.get_inner_expression(),
        Expression::Identifier(object) if object.name == "Object" || object.name == "Reflect"
    ) {
        return false;
    }
    let argument = |index: usize| call.arguments.get(index).and_then(Argument::as_expression);
    let key = || argument(1).map_or(Keys::Unknown, |key| key_of(scoping, nodes, key, KEY_DEPTH));
    match callee.property.name.as_str() {
        // A descriptor that names a get or a set, or that the syntax does not read, may define an accessor, which a read of the global runs.
        "defineProperty" => writes.define(
            key(),
            argument(2)
                .is_none_or(|descriptor| descriptor_defines_accessor(descriptor) != Some(false)),
        ),
        "set" => writes.keys(key()),
        "defineProperties" => descriptor_writes(scoping, nodes, argument(1), writes),
        "assign" => {
            for source in call.arguments.iter().skip(1) {
                source_writes(scoping, nodes, source.as_expression(), writes);
            }
        }
        // The members of the new prototype become globals, any of them an accessor.
        "setPrototypeOf" => writes.undecided(true),
        _ => return false,
    }
    let (_, statement) = parent_past_wrappers(nodes, parent);
    matches!(
        nodes.get_node(statement).kind(),
        AstKind::ExpressionStatement(_)
    )
}

/// The globals `Object.defineProperties(globalThis, descriptors)` defines: the keys of an object literal, each an accessor where its descriptor names a get or a set or is not
/// a literal; a source the syntax does not read to its descriptors defines each name its constants decide, possibly an accessor, and anything else any.
fn descriptor_writes(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    expression: Option<&Expression<'_>>,
    writes: &mut GlobalWrites,
) {
    if let Some(Expression::ObjectExpression(object)) =
        expression.map(Expression::get_inner_expression)
    {
        for property in &object.properties {
            match property {
                ObjectPropertyKind::ObjectProperty(property) => {
                    let keys = match key_name(&property.key) {
                        Some(Some(name)) => Keys::Names(vec![name]),
                        Some(None) => Keys::Symbol,
                        None => property
                            .key
                            .as_expression()
                            .map_or(Keys::Unknown, |key| key_of(scoping, nodes, key, KEY_DEPTH)),
                    };
                    writes.define(
                        keys,
                        descriptor_defines_accessor(&property.value) != Some(false),
                    );
                }
                ObjectPropertyKind::SpreadProperty(_) => writes.undecided(true),
            }
        }
        return;
    }
    match expression.and_then(|source| literal_names(scoping, nodes, source, KEY_DEPTH)) {
        Some(names) => {
            for name in names {
                writes.define(Keys::Names(vec![name]), true);
            }
        }
        None => writes.undecided(true),
    }
}

/// The globals a source object writes (`Object.assign(globalThis, source)`, `Object.defineProperties(window, descriptors)`): the keys of an object literal, of a `const` bound to
/// one, of a spread of either; anything else (a variable the file's constants do not decide, a call, an import) writes any.
fn source_writes(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    expression: Option<&Expression<'_>>,
    writes: &mut GlobalWrites,
) {
    match expression.and_then(|source| literal_names(scoping, nodes, source, KEY_DEPTH)) {
        Some(names) => {
            for name in names {
                writes.name(&name);
            }
        }
        None => writes.any_name = true,
    }
}

/// What a function of the writer family does to the object it is given.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Writer {
    /// `Object.assign`, `Reflect.set`: data written to the target, the first argument.
    Data,
    /// `Object.defineProperty`, `Object.defineProperties`, `Object.setPrototypeOf`, `Reflect.defineProperty`, `Reflect.setPrototypeOf`: an accessor may be defined on the
    /// target, the first argument.
    Define,
    /// `Object.prototype.__defineGetter__` and `__defineSetter__`: an accessor defined on the receiver (`this`).
    Receiver,
}

/// Whether `identifier` names the global `name`: no binding of the file declares it.
fn names_global(
    scoping: &Scoping,
    identifier: &oxc_ast::ast::IdentifierReference<'_>,
    name: &str,
) -> bool {
    identifier.name == name
        && identifier
            .reference_id
            .get()
            .is_some_and(|id| scoping.get_reference(id).symbol_id().is_none())
}

/// The object and the property a member expression names, the property by a static name or a string-literal key.
fn member_parts<'a>(expression: &'a Expression<'a>) -> Option<(&'a Expression<'a>, String)> {
    match expression.get_inner_expression() {
        Expression::StaticMemberExpression(member) => {
            Some((&member.object, member.property.name.to_string()))
        }
        Expression::ComputedMemberExpression(member) => {
            Some((&member.object, literal_specifier(&member.expression)?))
        }
        _ => None,
    }
}

/// How the global object is used at a node the scan reaches by a route that is no unbound name (`this` at the top of a script, `defaultView` of any object), as a name is.
#[allow(clippy::too_many_arguments)]
fn route_use(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    id: NodeId,
    root: &str,
    lookup: bool,
    hazard: &mut impl FnMut(ModuleHazardKind, &str),
    writes: &mut GlobalWrites,
) {
    match global_use(scoping, nodes, id, root, Holder::Global) {
        GlobalUse::Member { name, path, at } => {
            defined_by_receiver(scoping, nodes, &name, at, writes);
            if generates_code_by_member(nodes, &name, at) {
                hazard(ModuleHazardKind::CodeGeneration, &path);
                code_generated(writes);
            }
            if lookup && !is_known_global(&name) {
                hazard(ModuleHazardKind::UnknownGlobal, &path);
            }
        }
        GlobalUse::Written { keys, path, read } => written(hazard, writes, keys, &path, read),
        GlobalUse::Escape { path, at } => {
            hazard(ModuleHazardKind::GlobalObjectEscape, &path);
            if !call_writes(scoping, nodes, at, writes) {
                escaped(writes);
            }
        }
        GlobalUse::Reassigned | GlobalUse::Inert => {}
    }
}

/// Whether the member `name` of the global object at `at` generates code where it is used: `window.eval(…)`, `globalThis.Function(…)`, a timer handed a string.
fn generates_code_by_member(nodes: &AstNodes<'_>, name: &str, at: NodeId) -> bool {
    match name {
        "eval" => true,
        "Function" => !function_use_is_inert(nodes, at),
        "setTimeout" | "setInterval" | "setImmediate" | "execScript" => string_timer(nodes, at),
        _ => false,
    }
}

/// Whether `expression` is the global object's name, or a member chain that starts from one or from `document` (those chains are followed from their name).
fn rooted_in_global(scoping: &Scoping, expression: &Expression<'_>) -> bool {
    match expression.get_inner_expression() {
        Expression::Identifier(identifier) => {
            names_global(scoping, identifier, "document")
                || GLOBAL_OBJECTS
                    .iter()
                    .any(|name| names_global(scoping, identifier, name))
        }
        Expression::StaticMemberExpression(member) => rooted_in_global(scoping, &member.object),
        Expression::ComputedMemberExpression(member) => rooted_in_global(scoping, &member.object),
        _ => false,
    }
}

/// The global object was handed on (an argument, an assignment, a return, an element, a branch of a conditional): the code that has it may write any global, defining accessors
/// under any name (ADR 0974).
fn escaped(writes: &mut GlobalWrites) {
    writes.undecided(true);
}

/// Code is generated at run time (`eval`, `Function(…)`, a string timer, `.constructor("…")`): it can reach the global object (`Function("return this")()`) and write it.
fn code_generated(writes: &mut GlobalWrites) {
    writes.undecided(true);
}

/// Whether the file has module syntax (`import`, `export`): its top-level `this` is `undefined`. A file without is a script or CommonJS, where it may be the global object.
fn has_module_syntax(program: &oxc_ast::ast::Program<'_>) -> bool {
    program.body.iter().any(|statement| {
        matches!(
            statement,
            Statement::ImportDeclaration(_)
                | Statement::ExportAllDeclaration(_)
                | Statement::ExportDefaultDeclaration(_)
                | Statement::ExportNamedDeclaration(_)
        )
    })
}

/// Whether the `this` at `id` is the script's own: no function (an arrow function takes its enclosing `this`), class member or static block holds it.
fn is_script_top_this(nodes: &AstNodes<'_>, id: NodeId) -> bool {
    for kind in nodes.ancestor_kinds(id) {
        match kind {
            AstKind::Function(_)
            | AstKind::Class(_)
            | AstKind::StaticBlock(_)
            | AstKind::PropertyDefinition(_)
            | AstKind::AccessorProperty(_) => return false,
            AstKind::Program(_) => return true,
            _ => {}
        }
    }
    true
}

/// Whether the value of the `this` at `id` goes nowhere: the test of a condition, the left of `&&` (a truthy object is dropped for the right), `!this`, a comparison with `null`
/// or `undefined`, `typeof this`, `this instanceof C`, `"x" in this`. `(this && this.__assign) || function …` is TypeScript's own helper idiom.
fn this_never_flows(nodes: &AstNodes<'_>, id: NodeId) -> bool {
    let (child, parent) = parent_past_wrappers(nodes, id);
    let span = nodes.get_node(child).kind().span();
    match nodes.get_node(parent).kind() {
        AstKind::LogicalExpression(logical) => {
            logical.operator == oxc_syntax::operator::LogicalOperator::And
                && logical.left.get_inner_expression().span() == span
        }
        AstKind::ConditionalExpression(conditional) => {
            conditional.test.get_inner_expression().span() == span
        }
        AstKind::IfStatement(statement) => statement.test.get_inner_expression().span() == span,
        AstKind::WhileStatement(statement) => statement.test.get_inner_expression().span() == span,
        AstKind::UnaryExpression(unary) => matches!(
            unary.operator,
            UnaryOperator::LogicalNot | UnaryOperator::Typeof | UnaryOperator::Void
        ),
        AstKind::BinaryExpression(binary) => {
            use oxc_syntax::operator::BinaryOperator;
            match binary.operator {
                BinaryOperator::Equality
                | BinaryOperator::Inequality
                | BinaryOperator::StrictEquality
                | BinaryOperator::StrictInequality => true,
                BinaryOperator::Instanceof => binary.left.get_inner_expression().span() == span,
                BinaryOperator::In => binary.right.get_inner_expression().span() == span,
                _ => false,
            }
        }
        _ => false,
    }
}

/// Whether the value of `expression` may be the global object by anything the syntax shows: a name it goes by (unbound), a member that gives it back (`window.self`,
/// `document.defaultView`), `this`, an arm of a conditional or logical expression, the value of an assignment or a sequence. Anything else (an identifier of the file, another
/// member, a call's result) is not it by name: a global object that reached such a value was handed on at a place the scan records as an escape.
fn may_be_global_object(scoping: &Scoping, expression: &Expression<'_>, depth: u8) -> bool {
    if depth == 0 {
        return true;
    }
    match expression.get_inner_expression() {
        Expression::Identifier(identifier) => GLOBAL_OBJECTS
            .iter()
            .any(|name| names_global(scoping, identifier, name)),
        Expression::ThisExpression(_) => true,
        Expression::StaticMemberExpression(member) => {
            let gives = GLOBAL_OBJECTS.contains(&member.property.name.as_str())
                || member.property.name == "defaultView";
            gives && may_be_global_object(scoping, &member.object, depth - 1)
                || matches!(member.object.get_inner_expression(), Expression::Identifier(object) if names_global(scoping, object, "document"))
                    && member.property.name == "defaultView"
        }
        Expression::ComputedMemberExpression(member) => match literal_specifier(&member.expression)
        {
            Some(key) => {
                (GLOBAL_OBJECTS.contains(&key.as_str()) || key == "defaultView")
                    && may_be_global_object(scoping, &member.object, depth - 1)
            }
            None => may_be_global_object(scoping, &member.object, depth - 1),
        },
        Expression::ConditionalExpression(conditional) => {
            may_be_global_object(scoping, &conditional.consequent, depth - 1)
                || may_be_global_object(scoping, &conditional.alternate, depth - 1)
        }
        Expression::LogicalExpression(logical) => {
            may_be_global_object(scoping, &logical.left, depth - 1)
                || may_be_global_object(scoping, &logical.right, depth - 1)
        }
        Expression::SequenceExpression(sequence) => sequence
            .expressions
            .last()
            .is_some_and(|last| may_be_global_object(scoping, last, depth - 1)),
        Expression::AssignmentExpression(assignment) => {
            may_be_global_object(scoping, &assignment.right, depth - 1)
        }
        _ => false,
    }
}

/// The writer `object.property` is, spelled as the global `Object` and `Reflect` and `Object.prototype` are: `None` for any other member.
fn writer_of(scoping: &Scoping, object: &Expression<'_>, property: &str) -> Option<Writer> {
    if let Expression::Identifier(root) = object.get_inner_expression() {
        return match property {
            "assign" if names_global(scoping, root, "Object") => Some(Writer::Data),
            "set" if names_global(scoping, root, "Reflect") => Some(Writer::Data),
            "defineProperty" | "setPrototypeOf"
                if names_global(scoping, root, "Object")
                    || names_global(scoping, root, "Reflect") =>
            {
                Some(Writer::Define)
            }
            "defineProperties" if names_global(scoping, root, "Object") => Some(Writer::Define),
            _ => None,
        };
    }
    // `Object.prototype.__defineGetter__`
    let (root, middle) = member_parts(object)?;
    (middle == "prototype"
        && matches!(property, "__defineGetter__" | "__defineSetter__")
        && matches!(root.get_inner_expression(), Expression::Identifier(root) if names_global(scoping, root, "Object")))
    .then_some(Writer::Receiver)
}

/// Whether the expression is an object a write to cannot reach the global object through: a fresh object, array, function or class, a primitive, the CommonJS
/// `exports` and `module.exports`, a `const` bound to one of those, `Object.create(…)`.
fn is_fresh(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    expression: &Expression<'_>,
    depth: u8,
) -> bool {
    if depth == 0 {
        return false;
    }
    match expression.get_inner_expression() {
        Expression::ObjectExpression(_)
        | Expression::ArrayExpression(_)
        | Expression::NewExpression(_)
        | Expression::FunctionExpression(_)
        | Expression::ArrowFunctionExpression(_)
        | Expression::ClassExpression(_)
        | Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::RegExpLiteral(_)
        | Expression::TemplateLiteral(_) => true,
        Expression::Identifier(identifier) => {
            if names_global(scoping, identifier, "exports") {
                return true;
            }
            let Some((declarator, _)) = constant_declarator(scoping, nodes, identifier) else {
                return false;
            };
            declarator_of(nodes, declarator)
                .and_then(|declarator| declarator.init.as_ref())
                .is_some_and(|init| is_fresh(scoping, nodes, init, depth - 1))
        }
        Expression::StaticMemberExpression(member) => {
            member.property.name == "exports"
                && matches!(member.object.get_inner_expression(), Expression::Identifier(object) if names_global(scoping, object, "module"))
        }
        Expression::CallExpression(call) => match call.callee.get_inner_expression() {
            Expression::StaticMemberExpression(callee) => {
                callee.property.name == "create"
                    && matches!(callee.object.get_inner_expression(), Expression::Identifier(object) if names_global(scoping, object, "Object"))
            }
            _ => false,
        },
        _ => false,
    }
}

/// How the writer functions are used where the file does not call them on the global object by name: `Reflect.apply(Object.assign, null, [target, source])`,
/// `Object.assign.call(null, target, source)`, `.apply` and `.bind` of one, a name it is bound to (`const assign = Object.assign`, `const { defineProperty } = Object`,
/// `Object.assign || fallback`). Each call that is found is judged: a target that is a fresh object (or `exports`) is no write to a global, any other target may be the global
/// object under any name, and a writer that escapes (exported, passed on, stored where this walk does not follow it, read as a value) may be called with it anywhere.
fn indirect_writers(scoping: &Scoping, nodes: &AstNodes<'_>, writes: &mut GlobalWrites) {
    let mut flow = WriterFlow {
        scoping,
        nodes,
        writes,
        followed: HashSet::new(),
    };
    for node in nodes.iter() {
        match node.kind() {
            AstKind::StaticMemberExpression(member) => {
                if let Some(writer) =
                    writer_of(scoping, &member.object, member.property.name.as_str())
                {
                    flow.follow(node.id(), writer, true, WRITER_DEPTH);
                }
            }
            AstKind::ComputedMemberExpression(member) => {
                if let Some(property) = literal_specifier(&member.expression)
                    && let Some(writer) = writer_of(scoping, &member.object, &property)
                {
                    flow.follow(node.id(), writer, true, WRITER_DEPTH);
                }
            }
            // `const { assign, defineProperty: define } = Object`: each name is a writer.
            AstKind::VariableDeclarator(declarator) => {
                let (BindingPattern::ObjectPattern(pattern), Some(init)) =
                    (&declarator.id, declarator.init.as_ref())
                else {
                    continue;
                };
                let Expression::Identifier(root) = init.get_inner_expression() else {
                    continue;
                };
                if !names_global(scoping, root, "Object") && !names_global(scoping, root, "Reflect")
                {
                    continue;
                }
                for property in &pattern.properties {
                    let Some(Some(name)) = key_name(&property.key) else {
                        continue;
                    };
                    let Some(writer) = writer_of(scoping, init, &name) else {
                        continue;
                    };
                    let binding = match &property.value {
                        BindingPattern::BindingIdentifier(binding) => Some(binding),
                        BindingPattern::AssignmentPattern(with_default) => match &with_default.left
                        {
                            BindingPattern::BindingIdentifier(binding) => Some(binding),
                            _ => None,
                        },
                        _ => None,
                    };
                    match binding.and_then(|binding| binding.symbol_id.get()) {
                        Some(symbol) => flow.alias(symbol, writer, WRITER_DEPTH),
                        None => flow.undecided(writer),
                    }
                }
                if pattern.rest.is_some() {
                    flow.undecided(Writer::Define);
                }
            }
            _ => {}
        }
    }
}

/// How far a writer function is followed through names it is bound to and members it is read through.
const WRITER_DEPTH: u8 = 8;

struct WriterFlow<'a, 'b> {
    scoping: &'a Scoping,
    nodes: &'a AstNodes<'b>,
    writes: &'a mut GlobalWrites,
    /// The names a writer is bound to that have been followed.
    followed: HashSet<SymbolId>,
}

impl WriterFlow<'_, '_> {
    /// A use this walk does not follow: the writer may be called with the global object, under any name.
    fn undecided(&mut self, writer: Writer) {
        self.writes.undecided(writer != Writer::Data);
    }

    /// A target (or, for a receiver writer, a receiver) that is not a fresh object may be the global object.
    fn judge(&mut self, target: Option<&Argument<'_>>, writer: Writer) {
        let Some(target) = target else {
            return;
        };
        match target.as_expression() {
            Some(expression) if is_fresh(self.scoping, self.nodes, expression, KEY_DEPTH) => {}
            Some(expression) if !may_be_global_object(self.scoping, expression, KEY_DEPTH) => {}
            _ => self.undecided(writer),
        }
    }

    /// The targets of a call written as `Reflect.apply(writer, receiver, [target, …])` or `writer.apply(receiver, [target, …])`.
    fn judge_apply(
        &mut self,
        receiver: Option<&Argument<'_>>,
        arguments: Option<&Argument<'_>>,
        writer: Writer,
    ) {
        if writer == Writer::Receiver {
            return self.judge(receiver, writer);
        }
        match arguments
            .and_then(Argument::as_expression)
            .map(Expression::get_inner_expression)
        {
            Some(Expression::ArrayExpression(array)) => {
                if let Some(element) = array.elements.first() {
                    match element.as_expression() {
                        Some(expression)
                            if is_fresh(self.scoping, self.nodes, expression, KEY_DEPTH) => {}
                        Some(expression)
                            if !may_be_global_object(self.scoping, expression, KEY_DEPTH) => {}
                        _ => self.undecided(writer),
                    }
                }
            }
            Some(_) => self.undecided(writer),
            None => {}
        }
    }

    /// The writer bound to a name: every read of the name is a use of it.
    fn alias(&mut self, symbol: SymbolId, writer: Writer, depth: u8) {
        if !self.followed.insert(symbol) {
            return;
        }
        // A name that is assigned again may hold something else, or the writer from elsewhere.
        if self.scoping.symbol_is_mutated(symbol) {
            return self.undecided(writer);
        }
        let reads = self
            .scoping
            .get_resolved_references(symbol)
            .map(oxc_semantic::Reference::node_id)
            .collect::<Vec<_>>();
        for read in reads {
            self.follow(read, writer, false, depth);
        }
    }

    /// `id` is a node whose value is the writer function. `named` is the member that spells it, where a direct call is the call [`call_writes`] judges by the global
    /// object it is given.
    fn follow(&mut self, id: NodeId, writer: Writer, named: bool, depth: u8) {
        if depth == 0 {
            return self.undecided(writer);
        }
        let (child, parent) = parent_past_wrappers(self.nodes, id);
        let child_span = self.nodes.get_node(child).kind().span();
        match self.nodes.get_node(parent).kind() {
            AstKind::CallExpression(call)
                if call.callee.get_inner_expression().span() == child_span =>
            {
                if !named && writer != Writer::Receiver {
                    self.judge(call.arguments.first(), writer);
                }
            }
            AstKind::CallExpression(call)
                if call
                    .arguments
                    .first()
                    .is_some_and(|first| first.span() == child_span)
                    && matches!(member_parts(&call.callee), Some((object, name)) if name == "apply"
                        && matches!(object.get_inner_expression(), Expression::Identifier(reflect) if names_global(self.scoping, reflect, "Reflect"))) =>
            {
                self.judge_apply(call.arguments.get(1), call.arguments.get(2), writer);
            }
            AstKind::StaticMemberExpression(member)
                if member.object.get_inner_expression().span() == child_span =>
            {
                self.member_of(parent, member.property.name.as_str(), writer, depth);
            }
            AstKind::ComputedMemberExpression(member)
                if member.object.get_inner_expression().span() == child_span =>
            {
                match literal_specifier(&member.expression) {
                    Some(property) => self.member_of(parent, &property, writer, depth),
                    None => self.undecided(writer),
                }
            }
            AstKind::VariableDeclarator(declarator)
                if declarator
                    .init
                    .as_ref()
                    .is_some_and(|init| init.get_inner_expression().span() == child_span) =>
            {
                let exported = matches!(
                    self.nodes.parent_kind(self.nodes.parent_id(parent)),
                    AstKind::ExportNamedDeclaration(_)
                );
                match &declarator.id {
                    BindingPattern::BindingIdentifier(binding) if !exported => {
                        match binding.symbol_id.get() {
                            Some(symbol) => self.alias(symbol, writer, depth - 1),
                            None => self.undecided(writer),
                        }
                    }
                    _ => self.undecided(writer),
                }
            }
            AstKind::AssignmentExpression(assignment)
                if assignment.right.get_inner_expression().span() == child_span =>
            {
                match &assignment.left {
                    oxc_ast::ast::AssignmentTarget::AssignmentTargetIdentifier(target) => {
                        match target
                            .reference_id
                            .get()
                            .and_then(|id| self.scoping.get_reference(id).symbol_id())
                        {
                            Some(symbol) => self.alias(symbol, writer, depth - 1),
                            None => self.undecided(writer),
                        }
                    }
                    _ => self.undecided(writer),
                }
            }
            // `Object.assign = polyfill`: the member is written, the function is not used.
            AstKind::AssignmentExpression(assignment) if assignment.left.span() == child_span => {}
            // `Object.assign || fallback`, `flag ? Object.assign : fallback`, `(0, Object.assign)`: the operand is the value of the whole.
            AstKind::LogicalExpression(_) | AstKind::SequenceExpression(_) => {
                self.follow(parent, writer, false, depth - 1);
            }
            AstKind::ConditionalExpression(conditional) => {
                // The test is only looked at (`Object.assign ? … : …`).
                if conditional.test.get_inner_expression().span() != child_span {
                    self.follow(parent, writer, false, depth - 1);
                }
            }
            // The value is dropped or compared: no call.
            AstKind::ExpressionStatement(_) => {}
            AstKind::IfStatement(statement)
                if statement.test.get_inner_expression().span() == child_span => {}
            AstKind::UnaryExpression(unary)
                if matches!(
                    unary.operator,
                    UnaryOperator::Typeof | UnaryOperator::Void | UnaryOperator::LogicalNot
                ) => {}
            AstKind::BinaryExpression(binary)
                if matches!(
                    binary.operator,
                    oxc_syntax::operator::BinaryOperator::Equality
                        | oxc_syntax::operator::BinaryOperator::Inequality
                        | oxc_syntax::operator::BinaryOperator::StrictEquality
                        | oxc_syntax::operator::BinaryOperator::StrictInequality
                        | oxc_syntax::operator::BinaryOperator::Instanceof
                ) => {}
            _ => self.undecided(writer),
        }
    }

    /// The writer read through a member (`writer.call`, `writer.apply`, `writer.bind`, or a property of the function that runs nothing: `name`, `length`).
    fn member_of(&mut self, member: NodeId, property: &str, writer: Writer, depth: u8) {
        if !matches!(property, "call" | "apply" | "bind") {
            return;
        }
        let (child, parent) = parent_past_wrappers(self.nodes, member);
        let child_span = self.nodes.get_node(child).kind().span();
        let AstKind::CallExpression(call) = self.nodes.get_node(parent).kind() else {
            // `writer.call` read as a value, handed on to run later.
            return self.undecided(writer);
        };
        if call.callee.get_inner_expression().span() != child_span {
            return self.undecided(writer);
        }
        // The receiver is the first argument, the writer's own arguments follow it.
        let target = if writer == Writer::Receiver {
            call.arguments.first()
        } else {
            call.arguments.get(1)
        };
        match property {
            "call" => self.judge(target, writer),
            "apply" => self.judge_apply(call.arguments.first(), call.arguments.get(1), writer),
            // `writer.bind(receiver, target, …)` fixes the target; a bound function that takes it at the call is followed to its calls.
            _ => match target {
                Some(target) => self.judge(Some(target), writer),
                None => self.follow(parent, writer, false, depth - 1),
            },
        }
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

    /// ADR 0974: the global object goes by more than one name, and a member read off any of them is a lookup of a global (the review of #231: `window.self.x` was
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

    /// ADR 0974: every way a file defines an accessor, under the name it defines it under (a getter a package declares runs its code where the project reads a
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

    /// ADR 0974: the globals a file installs, by every way to write one (a call of a global whose type the library declares runs the package's code where a package writes it).
    #[test]
    fn globals_a_file_writes_are_found_by_every_way_to_write_one() {
        let writes = |source: &str| scan(source).global_writes;
        let named = |source: &str| {
            let found = writes(source);
            assert!(!found.any_name, "{source}: {found:?}");
            found.names
        };
        assert_eq!(
            named("globalThis.setTimeout = (cb) => cb();"),
            ["setTimeout"]
        );
        assert_eq!(named("window.fetch = f; self.btoa = g;"), ["btoa", "fetch"]);
        assert_eq!(
            named("window.self.x = 1; document.defaultView.y = 2;"),
            ["x", "y"]
        );
        assert_eq!(
            named("globalThis['queueMicrotask'] = f;"),
            ["queueMicrotask"]
        );
        assert_eq!(
            named("Object.defineProperty(globalThis, 'now', { value: f });"),
            ["now"]
        );
        assert_eq!(named("Reflect.set(window, 'z', 1);"), ["z"]);
        assert_eq!(
            named("Object.defineProperties(window, { a: { value: 1 }, b: { get() {} } });"),
            ["a", "b"]
        );
        assert_eq!(named("Object.assign(globalThis, { c: 1, d });"), ["c", "d"]);
        assert_eq!(named("structuredClone = f;"), ["structuredClone"]);
        // A destructuring target and a loop head write the member as an assignment does.
        assert_eq!(named("[window.x] = [1]; ({ k: self.y } = o);"), ["x", "y"]);
        assert_eq!(
            named("[globalThis.z = 1] = []; [...window.rest] = list;"),
            ["rest", "z"]
        );
        assert_eq!(
            named("for (window.k of keys) {} for (self.j in obj) {}"),
            ["j", "k"]
        );
        // A key the syntax does not fix writes any global.
        for source in [
            "window[key] = value;",
            "Object.defineProperty(globalThis, key, d);",
            "Object.assign(window, source);",
            "Object.assign(globalThis, { ...more });",
            "Object.defineProperties(window, props);",
            "Reflect.set(globalThis, k, v);",
            "Object.setPrototypeOf(window, p);",
            "globalThis.__proto__ = p;",
            "Object.defineProperty(window, '__proto__', { value: p });",
        ] {
            assert!(writes(source).any_name, "{source}");
        }
        // No write: a read, a call, a write to `document`'s own members, to a module's exports, to a local.
        for source in [
            "const t = window.setTimeout; window.addEventListener('x', f);",
            "document.title = 'x'; window.document.body.innerHTML = '';",
            "module.exports = f; exports.x = 1; let local = 1; local = 2;",
            "const o = {}; o.x = 1; Object.assign(o, { y: 1 }); Object.defineProperty(o, 'z', { value: 1 });",
        ] {
            assert!(writes(source).is_empty(), "{source}: {:?}", writes(source));
        }
    }

    /// ADR 0974: a key the file's own constants decide is a name, a symbol writes none, and only a key nothing in the file decides writes any global.
    #[test]
    fn a_key_the_files_constants_decide_is_a_name() {
        let writes = |source: &str| scan(source).global_writes;
        let named = |source: &str| {
            let found = writes(source);
            assert!(!found.any_name, "{source}: {found:?}");
            found.names
        };
        assert_eq!(named("const KEY = 'solid'; window[KEY] = 1;"), ["solid"]);
        // Bundled code declares its constants with `var`, a comma list and a pure annotation.
        assert_eq!(
            named(
                "var A = 1, KEY = '__ADDONS__'; function f() { return globalThis[KEY] || (globalThis[KEY] = {}); }"
            ),
            ["__ADDONS__"]
        );
        assert!(
            writes("var M = /* @__PURE__ */ Symbol.for('matchers'); Object.defineProperty(globalThis, M, { get: () => 1 });").is_empty()
        );
        assert_eq!(
            named("const a = 'p'; const b = a; globalThis[b] = 1;"),
            ["p"]
        );
        assert_eq!(
            named("const k = flag ? 'x' : 'y'; globalThis[k] = 1;"),
            ["x", "y"]
        );
        assert_eq!(
            named("for (const api of ['it', 'test']) globalThis[api] = f;"),
            ["it", "test"]
        );
        assert_eq!(
            named("const apis = ['x', 'y']; for (const k of apis) window[k] = f;"),
            ["x", "y"]
        );
        assert_eq!(
            named("const table = { a: 1, b: 2 }; for (const k in table) globalThis[k] = table[k];"),
            ["a", "b"]
        );
        assert_eq!(
            named("const table = { a: 1 }; for (const k of Object.keys(table)) window[k] = 1;"),
            ["a"]
        );
        assert_eq!(
            named("const table = { a: 1, b() {} }; Object.assign(globalThis, table);"),
            ["a", "b"]
        );
        assert_eq!(
            named("const base = { a: 1 }; Object.assign(window, { ...base, c: 2 });"),
            ["a", "c"]
        );
        assert_eq!(
            named("const K = 'q'; Object.defineProperty(globalThis, K, { value: 1 });"),
            ["q"]
        );
        assert_eq!(
            named("Object.assign(window, { ['computed']: 1, [`tpl`]: 2 });"),
            ["computed", "tpl"]
        );
        // A symbol is no global's name.
        for source in [
            "const sym = Symbol('s'); globalThis[sym] = 1;",
            "Object.defineProperty(window, Symbol.for('t'), { value: 1 });",
            "const kA = Symbol(); Object.assign(globalThis, { [kA]: 1 });",
        ] {
            assert!(writes(source).is_empty(), "{source}: {:?}", writes(source));
        }
        // A key nothing in the file decides writes any global: a call, a parameter, a variable that changes, a table that changes, a destructured binding, a `var`, a shadowed
        // `Symbol`, a source the file does not hold.
        for source in [
            "const k = f(); window[k] = 1;",
            "for (const k in config) globalThis[k] = 1;",
            "let k = 'a'; k = 'b'; window[k] = 1;",
            "function g(k) { window[k] = 1; } g('a');",
            "const [k] = ['a']; window[k] = 1;",
            "const { length } = 'abc'; window[length] = 1;",
            "var k = 'a'; var k = 'b'; window[k] = 1;",
            "const Symbol = f; globalThis[Symbol('x')] = 1;",
            "Object.assign(globalThis, { [k]: 1 });",
            "import * as patches from './patches'; Object.assign(globalThis, patches);",
            "const apis = ['x']; apis.push(more); for (const k of apis) window[k] = 1;",
            "const table = { a: 1 }; table.b = 2; for (const k in table) globalThis[k] = 1;",
            "const table = { a: 1 }; extend(table); Object.assign(window, table);",
            "const table = { a: 1 }; Object.assign(table, more); Object.assign(window, table);",
            "const apis = ['x', other]; for (const k of apis) window[k] = 1;",
            "const table = { a: 1, ...rest }; Object.assign(window, table);",
        ] {
            assert!(writes(source).any_name, "{source}");
        }
    }

    /// ADR 0974 (the reviews of #231): every assignment operator on a route to the global object writes the member, and a compound one reads it first.
    #[test]
    fn every_assignment_operator_writes_a_global() {
        let writes = |source: &str| scan(source).global_writes;
        for source in [
            "globalThis.btoa &&= wrapped;",
            "globalThis.btoa ||= wrapped;",
            "globalThis.btoa ??= wrapped;",
            "window.btoa += 1;",
            "self['btoa'] **= 2;",
            "window.self.btoa |= 1;",
            "document.defaultView.btoa >>>= 1;",
        ] {
            let found = writes(source);
            assert_eq!(found.names, ["btoa"], "{source}");
            assert!(!found.any_name, "{source}");
        }
        // A key nothing decides: any global, whatever the operator.
        for source in [
            "window[key] ??= value;",
            "globalThis[key] += 1;",
            "self[key] ||= f;",
        ] {
            assert!(writes(source).any_name, "{source}");
        }
        // The compound operator looks the member up: a global no library declares is a hazard, a known one is not.
        assert!(
            kinds(&scan("window.fzFlag ||= 1;"))
                .contains(&(ModuleHazardKind::UnknownGlobal, "window.fzFlag"))
        );
        assert!(kinds(&scan("const f = 1; window.btoa &&= f;")).is_empty());
        // A plain assignment reads nothing.
        assert!(kinds(&scan("window.fzFlag = 1;")).is_empty());
    }

    /// ADR 0974: a writer function (`Object.assign` and its kin) used where its target is not seen is a write to any global unless its target is a fresh object; one that
    /// escapes may be called with the global object anywhere.
    #[test]
    fn a_writer_function_used_elsewhere_is_judged_at_its_calls() {
        let writes = |source: &str| scan(source).global_writes;
        for source in [
            "Reflect.apply(Object.assign, null, [globalThis, { btoa }]);",
            "Object.assign.call(null, globalThis, { btoa });",
            "Object.assign.apply(null, [window, src]);",
            "Object.assign.apply(null, args);",
            "Reflect.apply(Object.assign, null, args);",
            "Reflect.apply(Reflect.set, null, [window, 'x', 1]);",
            "const bound = Object.assign.bind(); bound.apply(null, args);",
            "export const assign = Object.assign;",
            "module.exports = Object.assign;",
            "use(Object.assign);",
            "const assign = Object.assign; use(assign);",
            "let assign = Object.assign; assign = other; assign(target);",
            "const { assign } = Object; export { assign };",
            "const { assign, ...rest } = Object;",
            "const call = Object.assign.call;",
            "Object.defineProperty.call(null, globalThis, 'x', { get() {} });",
            "Reflect.defineProperty.apply(null, [window, 'x', d]);",
            "Object.setPrototypeOf.call(null, window, proto);",
            "Object.prototype.__defineGetter__.call(window, 'x', get);",
            "Reflect.apply(Object.prototype.__defineSetter__, window, ['x', set]);",
        ] {
            assert!(writes(source).any_name, "{source}");
        }
        // Only a writer that escapes says so below.
        for source in ["export const set = Reflect.defineProperty;"] {
            let found = writes(source);
            assert!(
                found.any_name && found.accessors_any_name,
                "{source}: {found:?}"
            );
        }
        for source in ["export const assign = Object.assign;"] {
            let found = writes(source);
            assert!(
                found.any_name && !found.accessors_any_name,
                "{source}: {found:?}"
            );
        }
        // No write: the target is a fresh object, `exports`, a `const` bound to one, a target the file reads that names something else (a name, a parameter: no global, whichever form
        // the writer is called by, as for a direct call), or the writer is only looked at.
        for source in [
            "Reflect.apply(Object.assign, null, [{}, src]);",
            "Object.assign.call(null, {}, src);",
            "Object.assign.call(null, [], src);",
            "Object.assign.apply(null, [new Foo(), src]);",
            "const assign = Object.assign; assign({}, a, b); assign(exports, x); assign(module.exports, y);",
            "const target = {}; const assign = Object.assign; assign(target, x);",
            "const assign = Object.assign || fallback; assign(Object.create(null), x);",
            "const bound = Object.assign.bind(null, {}); bound(src);",
            "const define = Object.defineProperty; define(exports, '__esModule', { value: true });",
            "const { assign } = Object; assign(function () {}, { a: 1 });",
            "var a = typeof Object.assign === 'function' ? Object.assign : null; if (!Object.assign) { Object.assign = polyfill; } if (Object.assign) {} Object.assign.length; Object.assign.name;",
            "Object.assign(target, source); Object.defineProperty(target, 'x', { get() {} });",
            "Object.prototype.__defineGetter__.call({}, 'x', get);",
            "const assign = Object.assign; assign(target, source);",
            "var assign = Object.assign || fallback; assign(target, source);",
            "const assign = flag ? Object.assign : fallback; assign(target, source);",
            "const { assign } = Object; assign(target, source);",
            "const { assign: merge = fallback } = Object; merge(target, source);",
            "const bound = Object.assign.bind(null, target); bound(src);",
            "const bound = Object.assign.bind(null); bound(target, src);",
            "const bound = Object.assign.bind(); bound.call(null, target, src);",
            "const assign = Object.assign; assign.call(null, target, src);",
            "Object['assign'].call(null, target, src);",
            "const define = Object.prototype.__defineGetter__; define.call(target, 'x', get);",
            "const define = Object.prototype.__defineGetter__.bind(target); define('x', get);",
            "Object.defineProperty.call(null, target, 'x', d);",
            "const define = Object.defineProperties; define(target, props);",
            "Object.prototype.__defineGetter__.call(target, 'x', get);",
            "Reflect.apply(Reflect.setPrototypeOf, null, [target, p]);",
            "Object.assign.call(null, target, src);",
            "Reflect.apply(Reflect.set, null, [target, 'x', 1]);",
        ] {
            assert!(writes(source).is_empty(), "{source}: {:?}", writes(source));
        }
    }

    /// ADR 0974: a read of a global runs a package's code only through an accessor the package defines on the global object: the names a file defines one under, or may.
    #[test]
    fn accessors_a_file_defines_on_the_global_object_are_found() {
        let writes = |source: &str| scan(source).global_writes;
        let found = writes(
            "Object.defineProperty(window, 'devicePixelRatio', { configurable: true, get() { return 1; } });",
        );
        assert_eq!(found.names, ["devicePixelRatio"]);
        assert_eq!(found.accessor_names, ["devicePixelRatio"]);
        assert!(!found.any_name && !found.accessors_any_name);
        // A setter is one, so is a descriptor the syntax does not read, and `Reflect.defineProperty` is `Object.defineProperty`.
        assert_eq!(
            writes("Reflect.defineProperty(globalThis, 'a', { set(v) {} });").accessor_names,
            ["a"]
        );
        assert_eq!(
            writes("Object.defineProperty(self, 'b', descriptor);").accessor_names,
            ["b"]
        );
        assert_eq!(
            writes("const KEY = 'c'; Object.defineProperty(window, KEY, { get: f });")
                .accessor_names,
            ["c"]
        );
        // A data descriptor and every data write define none.
        for source in [
            "Object.defineProperty(window, 'a', { value: 1, writable: true });",
            "Reflect.defineProperty(window, 'a', { value: 1 });",
            "window.a = 1; globalThis['b'] ||= 2; Object.assign(window, { c: 1 }); Reflect.set(self, 'd', 1);",
            "Object.defineProperties(window, { a: { value: 1 }, b: { value: 2 } });",
        ] {
            let found = writes(source);
            assert!(!found.names.is_empty(), "{source}");
            assert!(
                found.accessor_names.is_empty() && !found.accessors_any_name,
                "{source}: {found:?}"
            );
        }
        // `defineProperties` reads each descriptor.
        let found = writes(
            "Object.defineProperties(window, { a: { value: 1 }, b: { get() {} }, c: descriptor });",
        );
        assert_eq!(found.names, ["a", "b", "c"]);
        assert_eq!(found.accessor_names, ["b", "c"]);
        // A key nothing decides, a change of the prototype, a `__proto__` write and an unread map: an accessor under any name.
        for source in [
            "Object.defineProperty(window, key, { get() {} });",
            "Object.defineProperty(window, key, { value: 1 });",
            "Reflect.defineProperty(globalThis, key, d);",
            "Object.defineProperties(window, props);",
            "Object.defineProperties(window, { ...more });",
            "Object.setPrototypeOf(window, p);",
            "Reflect.setPrototypeOf(globalThis, p);",
            "globalThis.__proto__ = p;",
            "window['__proto__'] ||= p;",
            "Object.defineProperty(window, '__proto__', { value: p });",
        ] {
            let found = writes(source);
            assert!(found.any_name, "{source}");
            // A data descriptor under a key nothing decides defines no accessor.
            if source != "Object.defineProperty(window, key, { value: 1 });" {
                assert!(found.accessors_any_name, "{source}: {found:?}");
            }
        }
        // `__defineGetter__` and `__defineSetter__` on the global object define an accessor under their first argument.
        let found =
            writes("window.__defineGetter__('d', get); globalThis['__defineSetter__']('e', set);");
        assert_eq!(found.names, ["d", "e"]);
        assert_eq!(found.accessor_names, ["d", "e"]);
        assert!(writes("window.__defineGetter__(key, get);").accessors_any_name);
        assert!(writes("const define = window.__defineGetter__;").accessors_any_name);
    }

    /// ADR 0974 (the coordinator's check of the premise of judging a writer like a direct call): every use of the global object other than a spelled member access
    /// is recorded as an escape, whatever name the object goes by and whatever the use is.
    #[test]
    fn every_use_of_the_global_object_but_a_spelled_member_is_an_escape() {
        let names = [
            "window",
            "self",
            "globalThis",
            "top",
            "parent",
            "frames",
            "global",
            "window.self",
            "globalThis.window",
            "document.defaultView",
            "window.document.defaultView",
        ];
        let escapes = [
            "use(G);",
            "const a = G; use(a);",
            "let a; a = G; use(a);",
            "function r() { return G; } use(r);",
            "use(() => G);",
            "use([G]);",
            "use({ k: G });",
            "use({ [G]: 1 });",
            "use(x ? G : 1);",
            "use(x ? 1 : G);",
            "use(x || G);",
            "use(G ?? x);",
            "use(G && x);",
            "use(...[G]);",
            "use(`${G}`);",
            "G();",
            "new G();",
            "async function a() { await G; } use(a);",
            "use(G === x);",
            "use('x' in G);",
            "const { btoa } = G; use(btoa);",
            "export default G;",
            "for (const k in G) use(k);",
            "use((0, G).btoa);",
            "use(G[key]);",
            "use(G['a' + b]);",
            "use(void G);",
            "class C { static field = G; } use(C);",
            "function t() { throw G; } use(t);",
            "if (G) use(1);",
        ];
        for name in names {
            for template in escapes {
                // `document.defaultView` as a bare name's use is the same value; the placeholder is the whole expression.
                let source = template.replace('G', &format!("({name})"));
                let found = scan(&source);
                assert!(
                    kinds(&found)
                        .iter()
                        .any(|(kind, _)| *kind == ModuleHazardKind::GlobalObjectEscape),
                    "{source}: {:?}",
                    kinds(&found)
                );
            }
        }
        // A spelled member access (static, a string-literal key, through a cast, optional) hands the object nowhere.
        for name in names {
            for template in [
                "use(G.btoa);",
                "use(G['btoa']);",
                "use((G as any).btoa);",
                "use(G?.btoa);",
                "use(G!.btoa);",
                "G.fzFlag = 1;",
                "G['fzFlag'] = 1;",
            ] {
                let source = template.replace('G', name);
                let found = scan(&source);
                assert!(
                    !kinds(&found)
                        .iter()
                        .any(|(kind, _)| *kind == ModuleHazardKind::GlobalObjectEscape),
                    "{source}: {:?}",
                    kinds(&found)
                );
            }
        }
        // `typeof` reads no value.
        assert!(
            !kinds(&scan("use(typeof window === 'undefined');"))
                .iter()
                .any(|(kind, _)| *kind == ModuleHazardKind::GlobalObjectEscape)
        );
    }

    /// ADR 0974: the object handed on, and code that can reach it, may write any global and define an accessor under any name; a spelled member never does.
    #[test]
    fn an_escape_of_the_global_object_is_a_writer_under_any_name() {
        for source in [
            "use(window);",
            "const g = globalThis; use(g);",
            "use([self]);",
            "function r() { return window; } use(r);",
            "use(cond ? window : 1);",
            "Function('return this')();",
            "eval('x');",
            "setTimeout('code', 0);",
            "new Function('return this')().btoa = f;",
            "x.constructor.constructor('return this')();",
            // A writer returns its target: kept, it is the global object handed on.
            "const w = Object.defineProperty(window, 'k', { value: 1 }); use(w);",
            "use(Object.assign(globalThis, { k: 1 }));",
        ] {
            let found = scan(source).global_writes;
            assert!(found.any_name && found.accessors_any_name, "{source}");
        }
        // A writer judged to the end, its result unused, writes what it names and nothing else.
        let found = scan("Object.defineProperty(globalThis, 'now', { value: f });").global_writes;
        assert_eq!(found.names, ["now"]);
        assert!(!found.any_name && !found.accessors_any_name);
        let found =
            scan("Reflect.set(window, 'k', 1); Object.assign(self, { a: 1 });").global_writes;
        assert!(!found.any_name && !found.accessors_any_name, "{found:?}");
        for source in [
            "use(window.btoa);",
            "use(globalThis['btoa']);",
            "use(typeof window);",
            "use(document.title);",
        ] {
            let found = scan(source).global_writes;
            assert!(!found.any_name && !found.accessors_any_name, "{source}");
        }
    }

    /// ADR 0974: the members of the global object that generate code (`window.eval`, `globalThis.Function`, a string timer) or hand the object back (`window.valueOf()`) are
    /// routes as the unbound names are.
    #[test]
    fn members_of_the_global_object_generate_code_and_hand_it_back() {
        let has = |source: &str, kind: ModuleHazardKind| {
            kinds(&scan(source)).iter().any(|(found, _)| *found == kind)
        };
        for source in [
            "window.eval('x');",
            "globalThis['eval']('x');",
            "self.Function('return this')();",
            "window.setTimeout('code', 0);",
            "globalThis.setInterval(`code`, 0);",
        ] {
            assert!(has(source, ModuleHazardKind::CodeGeneration), "{source}");
            assert!(scan(source).global_writes.any_name, "{source}");
        }
        assert!(has(
            "use(window.valueOf());",
            ModuleHazardKind::GlobalObjectEscape
        ));
        assert!(scan("use(window.valueOf());").global_writes.any_name);
        // A function handed to a timer, and a comparison with `Function`, run no code.
        for source in [
            "window.setTimeout(() => {}, 0);",
            "use(x instanceof window.Function);",
            "use(window.Function.prototype);",
        ] {
            assert!(!has(source, ModuleHazardKind::CodeGeneration), "{source}");
        }
    }

    /// ADR 0974: `defaultView` read off any object is the global object, as `document.defaultView` is.
    #[test]
    fn default_view_of_any_object_is_the_global_object() {
        for source in [
            "use(el.ownerDocument.defaultView);",
            "const win = el.ownerDocument.defaultView || window;",
            "function w(doc) { return doc.defaultView; } use(w);",
        ] {
            let found = scan(source);
            assert!(
                kinds(&found)
                    .iter()
                    .any(|(kind, _)| *kind == ModuleHazardKind::GlobalObjectEscape),
                "{source}"
            );
            assert!(found.global_writes.any_name, "{source}");
        }
        // A member read of it, or one already followed from a name, is not a second route.
        let found = scan("use(el.ownerDocument.defaultView.btoa);");
        assert!(!found.global_writes.any_name);
        assert!(
            !kinds(&found)
                .iter()
                .any(|(kind, _)| *kind == ModuleHazardKind::GlobalObjectEscape)
        );
        let found = scan("use(document.defaultView.btoa);");
        assert!(!found.global_writes.any_name);
    }

    /// ADR 0974: `this` at the top of a file with no module syntax is the global object; handed on it escapes, as a member read it is the file's own.
    #[test]
    fn this_at_the_top_of_a_script_is_a_route_to_the_global_object() {
        let escapes = |source: &str| {
            let found = scan(source);
            kinds(&found)
                .iter()
                .any(|(kind, _)| *kind == ModuleHazardKind::GlobalObjectEscape)
        };
        // UMD: the wrapper is handed the global object.
        for source in [
            "(function (root, factory) { root.Lib = factory(); })(this, function () { return 1; });",
            "use(this);",
            "const g = this; use(g);",
            "use([this]);",
            "use(this || {});",
            "use(cond ? this : null);",
            "const run = () => use(this); run();",
            "var root = typeof exports === 'object' ? exports : this; use(root);",
        ] {
            assert!(escapes(source), "{source}");
            assert!(scan(source).global_writes.any_name, "{source}");
        }
        // Where the value goes nowhere (TypeScript's helper idiom), or `this` is not the script's.
        for source in [
            "var __assign = (this && this.__assign) || function () { return 1; }; use(__assign);",
            "if (this) { use(1); }",
            "use(!this);",
            "use(this === undefined);",
            "use(typeof this);",
            "function f() { use(this); } use(f);",
            "class C { m() { use(this); } static s = this; } use(C);",
            "const o = { m() { use(this); } }; use(o);",
        ] {
            assert!(!escapes(source), "{source}");
        }
        // A file with module syntax has `undefined` for `this`.
        assert!(!escapes("use(this); export {};"));
        assert!(!escapes("import x from 'y'; use(this, x);"));
        // A member of it: a write installs the global; a read looks a global up, as `window.x` does (a premise that a bundler wraps the file is no evidence).
        let written = scan("this.fzFlag = 1;").global_writes;
        assert_eq!(written.names, ["fzFlag"]);
        assert!(
            kinds(&scan("const f = 1; use(this.fzFlag, f);"))
                .contains(&(ModuleHazardKind::UnknownGlobal, "this.fzFlag"))
        );
        // A known global is no lookup of an unknown one.
        assert!(
            !kinds(&scan("use(this.btoa);"))
                .contains(&(ModuleHazardKind::UnknownGlobal, "this.btoa"))
        );
    }

    /// ADR 0974: a writer called through a name, `.bind`, `.call`, `.apply` or `Reflect.apply` is judged as a direct call is: a write where the target names the global
    /// object, none where it is readable and names something else, any global where nothing reads it.
    #[test]
    fn a_writer_is_judged_as_a_direct_call_where_the_target_is_read() {
        let any = |source: &str| scan(source).global_writes.any_name;
        // A parameter, a member, a call's result: named something else.
        for source in [
            "const define = Object.defineProperty; function f(to) { define(to, 'k', { get() {} }); } f({});",
            "const assign = Object.assign; function f(target, src) { assign(target, src); } f({}, {});",
            "const define = Object.defineProperty; define(holder.target, 'k', { value: 1 });",
            "const assign = Object.assign; assign(makeTarget(), { a: 1 });",
            "Reflect.apply(Object.assign, null, [target, source]);",
            "Object.assign.call(null, target, source);",
            "Object.assign.apply(null, [target, source]);",
            "const bound = Object.assign.bind(null, target); bound(source);",
        ] {
            assert!(!any(source), "{source}");
        }
        // The global object, by a name or a route: a write whatever the form.
        for source in [
            "const assign = Object.assign; assign(window, { btoa: f });",
            "const assign = Object.assign; assign(globalThis.self, { btoa: f });",
            "const assign = Object.assign; assign(cond ? window : other, { btoa: f });",
            "Reflect.apply(Object.assign, null, [globalThis, { btoa: f }]);",
            "Object.assign.call(null, self, { btoa: f });",
            "Object.assign.apply(null, [window, { btoa: f }]);",
            "const bound = Object.assign.bind(null, window); bound({ btoa: f });",
            "const assign = Object.assign; assign(document.defaultView, { btoa: f });",
        ] {
            assert!(any(source), "global: {source}");
        }
        // Nothing reads the target: any global.
        for source in [
            "const assign = Object.assign; assign(...args);",
            "Reflect.apply(Object.assign, null, args);",
            "Object.assign.apply(null, args);",
            "Object.assign.call(null, ...args);",
            "const bound = Object.assign.bind(null); use(bound);",
            "use(Object.assign);",
            "const assign = Object.assign; use(assign);",
        ] {
            assert!(any(source), "unread: {source}");
        }
        // A fresh target stays no write in either.
        for source in [
            "const assign = Object.assign; assign({}, a);",
            "Reflect.apply(Object.assign, null, [{}, a]);",
        ] {
            assert!(!any(source), "{source}");
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
