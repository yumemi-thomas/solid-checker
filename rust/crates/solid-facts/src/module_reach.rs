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

/// The members of the global object a file writes at run time (ADR 0973): `window.x = …`, `globalThis["x"] = …`, `Object.defineProperty(globalThis, "x", …)`,
/// `Object.assign(window, { x })`, `Reflect.set`, an unbound `x = …`. A call of a global some installed package writes runs that package's code, whatever type
/// the library declares the global with.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GlobalWrites {
    /// The names written, sorted and distinct.
    pub names: Vec<String>,
    /// A global is written under a name the syntax does not fix (`window[key] = v`, `Object.assign(globalThis, source)`, `Object.defineProperty(window, key, d)`): any may be.
    /// A key the file's own constants decide is a name (`const KEY = "x"`, `for (const k of ["a", "b"])`, `Object.assign(globalThis, table)` over a `const` object literal),
    /// and a key that is a symbol writes no name.
    pub any_name: bool,
}

impl GlobalWrites {
    /// Whether the file writes no global at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty() && !self.any_name
    }

    fn name(&mut self, name: &str) {
        // `globalThis.__proto__ = p` makes the members of `p` globals (an unbound name is looked up through the global object's prototype chain): any name.
        if name == "__proto__" {
            self.any_name = true;
            return;
        }
        self.names.push(name.to_owned());
    }

    /// The writes a key decides: its names, nothing for a symbol, any for a key that is not known.
    fn keys(&mut self, keys: Keys) {
        match keys {
            Keys::Names(names) => {
                for name in names {
                    self.name(&name);
                }
            }
            Keys::Symbol => {}
            Keys::Unknown => self.any_name = true,
        }
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
                "document" => match global_use(scoping, nodes, id, name, Holder::Document) {
                    GlobalUse::Member { name: member, path } if !is_known_global(&member) => {
                        hazard(ModuleHazardKind::UnknownGlobal, &path);
                    }
                    GlobalUse::Escape { path, at } => {
                        hazard(ModuleHazardKind::GlobalObjectEscape, &path);
                        call_writes(scoping, nodes, at, &mut writes);
                    }
                    // `document.defaultView.x = …` writes the global `x`.
                    GlobalUse::Written { keys, path } => {
                        if matches!(keys, Keys::Unknown) {
                            hazard(ModuleHazardKind::GlobalObjectEscape, &path);
                        }
                        writes.keys(keys);
                    }
                    GlobalUse::Reassigned | GlobalUse::Member { .. } | GlobalUse::Inert => {}
                },
                _ if GLOBAL_OBJECTS.contains(&name) => {
                    match global_use(scoping, nodes, id, name, Holder::Global) {
                        GlobalUse::Member { name: member, .. } if is_known_global(&member) => {}
                        GlobalUse::Member { path, .. } => {
                            hazard(ModuleHazardKind::UnknownGlobal, &path);
                        }
                        // A write reads nothing, but it installs a global: the name, the names the key's constants decide, none for a symbol, or any where the key is
                        // not known (which also hands the object on).
                        GlobalUse::Written { keys, path } => {
                            if matches!(keys, Keys::Unknown) {
                                hazard(ModuleHazardKind::GlobalObjectEscape, &path);
                            }
                            writes.keys(keys);
                        }
                        GlobalUse::Reassigned | GlobalUse::Inert => {}
                        GlobalUse::Escape { path, at } => {
                            hazard(ModuleHazardKind::GlobalObjectEscape, &path);
                            call_writes(scoping, nodes, at, &mut writes);
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
    writes.names.sort();
    writes.names.dedup();
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
    /// A member of the global object read by a static name or a string literal key: its name, and the chain as it was spelled (`window.self.solid`).
    Member { name: String, path: String },
    /// A member of the global object assigned (`window.x = …`, `window["x"] = …`): nothing is read, and the global `x` is installed; `keys` is what the key is where it is not a
    /// name the syntax fixes (`window[k] = …`: the names the file's constants say, a symbol, or nothing known, where any global may be). The chain as it was spelled.
    Written { keys: Keys, path: String },
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
    let assigned = |member: NodeId| {
        let (member_child, member_parent) = parent_past_wrappers(nodes, member);
        let span = nodes.get_node(member_child).kind().span();
        match nodes.get_node(member_parent).kind() {
            AstKind::AssignmentExpression(assignment) => {
                assignment.operator == oxc_syntax::operator::AssignmentOperator::Assign
                    && assignment.left.span() == span
            }
            AstKind::ArrayAssignmentTarget(_)
            | AstKind::AssignmentTargetWithDefault(_)
            | AstKind::AssignmentTargetPropertyProperty(_)
            | AstKind::AssignmentTargetRest(_) => true,
            AstKind::ForInStatement(each) => each.left.span() == span,
            AstKind::ForOfStatement(each) => each.left.span() == span,
            _ => false,
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
                Some(literal_specifier(&member.expression))
            }
            AstKind::AssignmentExpression(assignment) if assignment.left.span() == child_span => {
                return GlobalUse::Reassigned;
            }
            _ => None,
        };
        // The value itself is used, or read by a key the syntax does not fix: the global object escapes, `document` holds no global object. A key the syntax does not
        // fix that is assigned writes what the key is: the names the file's constants decide, a symbol, or any global.
        let Some(Some(name)) = member else {
            if computed && assigned(parent) && holder == Holder::Global {
                let keys = match nodes.get_node(parent).kind() {
                    AstKind::ComputedMemberExpression(member) => {
                        key_of(scoping, nodes, &member.expression, KEY_DEPTH)
                    }
                    _ => Keys::Unknown,
                };
                return GlobalUse::Written {
                    keys,
                    path: format!("{path}[…]"),
                };
            }
            return match holder {
                Holder::Global => GlobalUse::Escape { path, at },
                Holder::Document => GlobalUse::Inert,
            };
        };
        if assigned(parent) {
            // `window.document.title = …` writes `document`'s member, no global.
            if holder == Holder::Document {
                return GlobalUse::Inert;
            }
            return GlobalUse::Written {
                keys: Keys::Names(vec![name.clone()]),
                path: format!("{path}.{name}"),
            };
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

/// The globals a call writes when the global object (the value of the node `at`) is its target: `Object.defineProperty(globalThis, "x", d)`, `Reflect.set(window, "x", v)`,
/// `Object.defineProperties(window, { x: d })`, `Object.assign(globalThis, { x })`, `Object.setPrototypeOf(window, p)`. A key or a source the syntax does not fix writes any.
fn call_writes(scoping: &Scoping, nodes: &AstNodes<'_>, at: NodeId, writes: &mut GlobalWrites) {
    let (child, parent) = parent_past_wrappers(nodes, at);
    let AstKind::CallExpression(call) = nodes.get_node(parent).kind() else {
        return;
    };
    if call
        .arguments
        .first()
        .and_then(Argument::as_expression)
        .is_none_or(|first| first.span() != nodes.get_node(child).kind().span())
    {
        return;
    }
    let Expression::StaticMemberExpression(callee) = call.callee.get_inner_expression() else {
        return;
    };
    if !matches!(
        callee.object.get_inner_expression(),
        Expression::Identifier(object) if object.name == "Object" || object.name == "Reflect"
    ) {
        return;
    }
    let argument = |index: usize| call.arguments.get(index).and_then(Argument::as_expression);
    match callee.property.name.as_str() {
        "defineProperty" | "set" => writes
            .keys(argument(1).map_or(Keys::Unknown, |key| key_of(scoping, nodes, key, KEY_DEPTH))),
        "defineProperties" => source_writes(scoping, nodes, argument(1), writes),
        "assign" => {
            for source in call.arguments.iter().skip(1) {
                source_writes(scoping, nodes, source.as_expression(), writes);
            }
        }
        "setPrototypeOf" => writes.any_name = true,
        _ => {}
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

    /// ADR 0973: the globals a file installs, by every way to write one (a call of a global whose type the library declares runs the package's code where a package writes it).
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

    /// ADR 0973: a key the file's own constants decide is a name, a symbol writes none, and only a key nothing in the file decides writes any global.
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
