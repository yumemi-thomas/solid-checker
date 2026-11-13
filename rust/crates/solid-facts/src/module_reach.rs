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
    Argument, Expression, ImportOrExportKind, Statement, TSModuleReference, UnaryOperator,
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

/// What one runtime file loads, and what it does that its loads do not show.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModuleScan {
    /// The specifiers it loads, sorted and distinct.
    pub loads: Vec<String>,
    /// Its hazards, sorted and distinct. Empty when the loads are all it can reach.
    pub hazards: Vec<ModuleHazard>,
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
    let scoping = semantic.scoping();
    for (name, references) in scoping.root_unresolved_references() {
        let name = name.as_str();
        for reference in references {
            let reference = scoping.get_reference(*reference);
            let id = reference.node_id();
            if under_typeof(nodes, id) {
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
                _ if GLOBAL_OBJECTS.contains(&name) => match global_member(nodes, id) {
                    GlobalUse::Member(member) if is_known_global(&member) => {}
                    GlobalUse::Member(member) => {
                        hazard(ModuleHazardKind::UnknownGlobal, &format!("{name}.{member}"));
                    }
                    GlobalUse::Written => {}
                    GlobalUse::Other => hazard(ModuleHazardKind::GlobalObjectEscape, name),
                },
                _ if is_known_global(name) => {}
                _ if !read => {}
                _ => hazard(ModuleHazardKind::UnknownGlobal, name),
            }
        }
    }
    // A call or `new` of a `.constructor` member, or one handed on, reaches the `Function` constructor from any function value.
    for node in nodes.iter() {
        let AstKind::StaticMemberExpression(member) = node.kind() else {
            continue;
        };
        if member.property.name != "constructor" {
            continue;
        }
        if !constructor_use_is_inert(nodes, node.id()) {
            hazard(ModuleHazardKind::CodeGeneration, ".constructor");
        }
    }
    loads.sort();
    loads.dedup();
    hazards.sort();
    hazards.dedup();
    scan.loads = loads;
    scan.hazards = hazards;
    scan
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

enum GlobalUse {
    /// A member read by a static name or a string literal key.
    Member(String),
    /// The name itself assigned (`window = …`, sloppy code): nothing is read.
    Written,
    /// Anything else: a computed key, an alias, an argument.
    Other,
}

/// How the global-object name at `id` is used.
fn global_member(nodes: &AstNodes<'_>, id: NodeId) -> GlobalUse {
    let (child, parent) = parent_past_wrappers(nodes, id);
    let child_span = nodes.get_node(child).kind().span();
    // `window.x = …` writes the member and reads nothing of it.
    let assigned = |member: NodeId| {
        let (member_child, member_parent) = parent_past_wrappers(nodes, member);
        matches!(nodes.get_node(member_parent).kind(), AstKind::AssignmentExpression(assignment)
            if assignment.operator == oxc_syntax::operator::AssignmentOperator::Assign
                && assignment.left.span() == nodes.get_node(member_child).kind().span())
    };
    match nodes.get_node(parent).kind() {
        AstKind::StaticMemberExpression(member) if member.object.span() == child_span => {
            if assigned(parent) {
                return GlobalUse::Written;
            }
            GlobalUse::Member(member.property.name.to_string())
        }
        AstKind::ComputedMemberExpression(member) if member.object.span() == child_span => {
            match literal_specifier(&member.expression) {
                Some(key) => GlobalUse::Member(key),
                None => GlobalUse::Other,
            }
        }
        AstKind::AssignmentExpression(assignment) if assignment.left.span() == child_span => {
            GlobalUse::Written
        }
        _ => GlobalUse::Other,
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

/// Whether the `.constructor` member at `id` is only compared or read through (`x.constructor === Object`, `x.constructor.name`), never
/// called, constructed, stored or handed on.
fn constructor_use_is_inert(nodes: &AstNodes<'_>, id: NodeId) -> bool {
    let (child, parent) = parent_past_wrappers(nodes, id);
    let child_span = nodes.get_node(child).kind().span();
    match nodes.get_node(parent).kind() {
        AstKind::StaticMemberExpression(member) => {
            member.object.span() == child_span && member.property.name != "constructor"
        }
        AstKind::BinaryExpression(_) => true,
        AstKind::UnaryExpression(unary) => unary.operator == UnaryOperator::Typeof,
        AstKind::IfStatement(_)
        | AstKind::ConditionalExpression(_)
        | AstKind::LogicalExpression(_) => {
            // A test reads its truthiness; an arm or operand passes the value on.
            match nodes.get_node(parent).kind() {
                AstKind::IfStatement(statement) => statement.test.span() == child_span,
                AstKind::ConditionalExpression(conditional) => {
                    conditional.test.span() == child_span
                }
                _ => false,
            }
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
            "const F = (async () => {}).constructor;",
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
            scan("setTimeout(() => {}, 0); setInterval(tick, 10);")
                .hazards
                .is_empty()
        );
        for source in [
            "export const f = (x) => x instanceof Function;",
            "export const g = Function.prototype.toString;",
            "export const n = (x) => x.constructor === Object || x.constructor.name;",
            "export const t = (x) => typeof eval;",
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
