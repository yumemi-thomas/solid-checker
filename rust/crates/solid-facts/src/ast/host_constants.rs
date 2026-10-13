//! Host constants (ADR 0166): the syntax half of folding an imported boolean
//! whose value the host's resolution of its package fixes, such as
//! `@solidjs/web`'s `isServer`.
//!
//! Three questions are answered here, each over one module's source text and
//! never over a name:
//!
//! - which named value imports a module makes from a bare specifier
//!   ([`named_value_imports`]), the candidates a resolver may look up;
//! - whether a resolved runtime module exports a name as a `const` bound to a
//!   `true` or `false` literal ([`exported_boolean_constant`]);
//! - given the folds a resolver proved, how a condition reads
//!   ([`HostConstantScope::truthiness`]) and which `if` arms that leaves dead
//!   ([`fold_host_constant_branches`]).
//!
//! A fold applies to a reference only when semantic resolution binds it to the
//! import specifier itself (or to a never-written `const` whose initializer
//! does). A shadowing binding, a namespace member, a re-export, a written
//! alias, or a module containing a direct `eval` states nothing.

use std::collections::HashMap;
use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_ast::ast::{
    BindingPattern, Declaration, Expression, IdentifierReference, ImportDeclarationSpecifier,
    ImportOrExportKind, ModuleExportName, Program, Statement, UnaryOperator,
    VariableDeclarationKind,
};
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_semantic::{Scoping, SemanticBuilder, SymbolId};
use oxc_span::{GetSpan, SourceType};

const MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;
const MAX_DEPTH: usize = 32;

/// One named value import from a bare specifier: `import { imported as local }
/// from "specifier"`.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct HostConstantImport {
    pub specifier: String,
    pub imported: String,
}

/// A value a resolver proved for one import: the name `imported` of
/// `specifier`, as this module imports it, is `value` under the declared host.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct HostConstantFold {
    pub specifier: String,
    pub imported: String,
    pub value: bool,
}

fn parse<'a>(allocator: &'a Allocator, path: &Path, source: &'a str) -> Option<Program<'a>> {
    if source.len() > MAX_SOURCE_BYTES {
        return None;
    }
    let source_type = SourceType::from_path(path).ok()?.with_module(true);
    let parsed = Parser::new(allocator, source, source_type).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    Some(parsed.program)
}

fn is_bare(specifier: &str) -> bool {
    !(specifier.starts_with('.')
        || specifier.starts_with('/')
        || specifier.contains(':')
        || specifier.is_empty())
}

/// The named value imports `source` makes from bare specifiers. `None` when it
/// does not parse.
#[must_use]
pub fn named_value_imports(path: &Path, source: &str) -> Option<Vec<HostConstantImport>> {
    let allocator = Allocator::default();
    let program = parse(&allocator, path, source)?;
    let mut imports = Vec::new();
    for statement in &program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        if import.import_kind == ImportOrExportKind::Type || !is_bare(&import.source.value) {
            continue;
        }
        for specifier in import.specifiers.iter().flatten() {
            let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                continue;
            };
            if specifier.import_kind == ImportOrExportKind::Type {
                continue;
            }
            imports.push(HostConstantImport {
                specifier: import.source.value.to_string(),
                imported: specifier.imported.name().to_string(),
            });
        }
    }
    imports.sort();
    imports.dedup();
    Some(imports)
}

/// Whether the module `source` exports `exported` as a module-level `const`
/// initialized with a `true` or `false` literal, and which. Exported directly
/// (`export const exported = false`) or by a local export clause (`const local
/// = false; export { local as exported }`); a re-export, a star export, a
/// default export, a `let` or `var`, any other initializer, a written binding,
/// or a direct `eval` in the module answers `None`.
#[must_use]
pub fn exported_boolean_constant(path: &Path, source: &str, exported: &str) -> Option<bool> {
    let allocator = Allocator::default();
    let program = parse(&allocator, path, source)?;
    let built = SemanticBuilder::new().build(&program);
    if !built.errors.is_empty() || contains_direct_eval(&program) {
        return None;
    }
    let scoping = built.semantic.scoping();
    // Module-level boolean consts by symbol.
    let mut constants = HashMap::<SymbolId, bool>::new();
    let mut direct = Vec::<(String, bool)>::new();
    for statement in &program.body {
        let (declaration, exported_directly) = match statement {
            Statement::VariableDeclaration(declaration) => (declaration, false),
            Statement::ExportNamedDeclaration(export) => match &export.declaration {
                Some(Declaration::VariableDeclaration(declaration)) => (declaration, true),
                _ => continue,
            },
            _ => continue,
        };
        if declaration.kind != VariableDeclarationKind::Const {
            continue;
        }
        for declarator in &declaration.declarations {
            let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                continue;
            };
            let Some(Expression::BooleanLiteral(literal)) = &declarator.init else {
                continue;
            };
            let Some(symbol) = identifier.symbol_id.get() else {
                continue;
            };
            if !scoping.symbol_redeclarations(symbol).is_empty()
                || scoping
                    .get_resolved_references(symbol)
                    .any(|reference| reference.is_write())
            {
                continue;
            }
            constants.insert(symbol, literal.value);
            if exported_directly {
                direct.push((identifier.name.to_string(), literal.value));
            }
        }
    }
    let mut answers = direct
        .into_iter()
        .filter(|(name, _)| name == exported)
        .map(|(_, value)| value)
        .collect::<Vec<_>>();
    let mut publications = 0_usize;
    for statement in &program.body {
        match statement {
            Statement::ExportNamedDeclaration(export) => {
                if export.declaration.is_some() {
                    // Counted above when it is a boolean const; any other
                    // declaration of this name makes the export not ours.
                    if let Some(declaration) = &export.declaration
                        && declares_name(declaration, exported)
                        && answers.is_empty()
                    {
                        return None;
                    }
                    continue;
                }
                for specifier in &export.specifiers {
                    if specifier.exported.name() != exported {
                        continue;
                    }
                    publications += 1;
                    if export.source.is_some()
                        || export.export_kind == ImportOrExportKind::Type
                        || specifier.export_kind == ImportOrExportKind::Type
                    {
                        return None;
                    }
                    let ModuleExportName::IdentifierReference(local) = &specifier.local else {
                        return None;
                    };
                    let symbol = reference_symbol(scoping, local)?;
                    answers.push(*constants.get(&symbol)?);
                }
            }
            Statement::ExportDefaultDeclaration(_) if exported == "default" => return None,
            _ => {}
        }
    }
    let _ = publications;
    match answers.as_slice() {
        [value] => Some(*value),
        _ => None,
    }
}

fn declares_name(declaration: &Declaration<'_>, name: &str) -> bool {
    match declaration {
        Declaration::VariableDeclaration(declaration) => {
            declaration.declarations.iter().any(|declarator| {
                matches!(&declarator.id, BindingPattern::BindingIdentifier(identifier) if identifier.name == name)
            })
        }
        Declaration::FunctionDeclaration(function) => {
            function.id.as_ref().is_some_and(|id| id.name == name)
        }
        Declaration::ClassDeclaration(class) => class.id.as_ref().is_some_and(|id| id.name == name),
        _ => true,
    }
}

fn reference_symbol(scoping: &Scoping, reference: &IdentifierReference<'_>) -> Option<SymbolId> {
    scoping
        .get_reference(reference.reference_id.get()?)
        .symbol_id()
}

fn contains_direct_eval(program: &Program<'_>) -> bool {
    struct DirectEval(bool);
    impl<'a> Visit<'a> for DirectEval {
        fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
            if identifier.name == "eval" {
                self.0 = true;
            }
        }
    }
    let mut visitor = DirectEval(false);
    visitor.visit_program(program);
    visitor.0
}

/// The folds of one parsed module, bound to the symbols its import specifiers
/// declare. Built once per module and asked per condition.
pub struct HostConstantScope<'s, 'a> {
    scoping: &'s Scoping,
    imports: HashMap<SymbolId, bool>,
    consts: HashMap<SymbolId, &'a Expression<'a>>,
}

impl<'s, 'a> HostConstantScope<'s, 'a> {
    /// `None` when the module holds a direct `eval`, which may write any
    /// binding in scope.
    #[must_use]
    pub fn new(
        program: &Program<'a>,
        semantic: &'s oxc_semantic::Semantic<'a>,
        folds: &[HostConstantFold],
    ) -> Option<Self> {
        let scoping = semantic.scoping();
        if contains_direct_eval(program) {
            return None;
        }
        let mut imports = HashMap::new();
        for statement in &program.body {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            if import.import_kind == ImportOrExportKind::Type {
                continue;
            }
            for specifier in import.specifiers.iter().flatten() {
                let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                    continue;
                };
                if specifier.import_kind == ImportOrExportKind::Type {
                    continue;
                }
                let imported = specifier.imported.name();
                let Some(fold) = folds.iter().find(|fold| {
                    fold.specifier == import.source.value.as_str()
                        && fold.imported == imported.as_str()
                }) else {
                    continue;
                };
                let Some(symbol) = specifier.local.symbol_id.get() else {
                    continue;
                };
                if scoping
                    .get_resolved_references(symbol)
                    .any(|reference| reference.is_write())
                {
                    continue;
                }
                imports.insert(symbol, fold.value);
            }
        }
        let mut consts = HashMap::new();
        if !imports.is_empty() {
            for node in semantic.nodes().iter() {
                let AstKind::VariableDeclarator(declarator) = node.kind() else {
                    continue;
                };
                if declarator.kind != VariableDeclarationKind::Const {
                    continue;
                }
                let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                    continue;
                };
                let (Some(symbol), Some(init)) = (identifier.symbol_id.get(), &declarator.init)
                else {
                    continue;
                };
                if scoping.symbol_redeclarations(symbol).is_empty()
                    && !scoping
                        .get_resolved_references(symbol)
                        .any(|reference| reference.is_write())
                {
                    consts.insert(symbol, init);
                }
            }
        }
        Some(Self {
            scoping,
            imports,
            consts,
        })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.imports.is_empty()
    }

    /// How `expression` reads as a condition: `Some((value, folded))` when it
    /// is decided, where `folded` says a host constant took part. Mirrors the
    /// producer's `literalTruthinessLocked` for the forms both read: boolean,
    /// `null`, string and decimal numeric literals, `!`, parentheses and type
    /// wrappers, a host-constant import, and a never-written `const`.
    #[must_use]
    pub fn truthiness(&self, expression: &Expression<'_>) -> Option<(bool, bool)> {
        self.truthiness_at(expression, 0)
    }

    fn truthiness_at(&self, expression: &Expression<'_>, depth: usize) -> Option<(bool, bool)> {
        if depth > MAX_DEPTH {
            return None;
        }
        match expression {
            Expression::BooleanLiteral(literal) => Some((literal.value, false)),
            Expression::NullLiteral(_) => Some((false, false)),
            Expression::StringLiteral(literal) => Some((!literal.value.is_empty(), false)),
            Expression::NumericLiteral(literal) => {
                let raw = literal.raw.as_ref()?;
                let value = raw.parse::<f64>().ok()?;
                Some((value != 0.0, false))
            }
            Expression::ParenthesizedExpression(inner) => {
                self.truthiness_at(&inner.expression, depth + 1)
            }
            Expression::TSAsExpression(inner) => self.truthiness_at(&inner.expression, depth + 1),
            Expression::TSSatisfiesExpression(inner) => {
                self.truthiness_at(&inner.expression, depth + 1)
            }
            Expression::TSNonNullExpression(inner) => {
                self.truthiness_at(&inner.expression, depth + 1)
            }
            Expression::TSTypeAssertion(inner) => self.truthiness_at(&inner.expression, depth + 1),
            Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::LogicalNot => {
                self.truthiness_at(&unary.argument, depth + 1)
                    .map(|(value, folded)| (!value, folded))
            }
            Expression::Identifier(identifier) => {
                let symbol = reference_symbol(self.scoping, identifier)?;
                if let Some(value) = self.imports.get(&symbol) {
                    return Some((*value, true));
                }
                let init = self.consts.get(&symbol)?;
                self.truthiness_at(init, depth + 1)
            }
            _ => None,
        }
    }
}

/// `source` with every `if` arm a host-constant fold leaves dead blanked, and
/// every statement after an `if` whose live arm always leaves the function;
/// the decided condition is rewritten to the literal it reads as (`!0` or
/// `!1`).
/// Byte offsets and line breaks are preserved, so every span of the result is
/// the span of the same text in `source`. `None` when the module does not
/// parse, holds a direct `eval`, or no fold decides anything.
///
/// Only a condition a host constant takes part in is folded, so a module the
/// resolver proved nothing for is analysed exactly as before. A dead arm
/// becomes the empty statement `;`; a dead `else` is removed with its keyword;
/// a dead statement after the `if` is removed, except that a function
/// declaration keeps its text (it is hoisted, and live code may call it) and a
/// variable declaration keeps its bindings with each initializer reduced to
/// `0`, so no reference changes the binding it resolves to.
#[must_use]
pub fn fold_host_constant_branches(
    path: &Path,
    source: &str,
    folds: &[HostConstantFold],
) -> Option<String> {
    if folds.is_empty() {
        return None;
    }
    let allocator = Allocator::default();
    let program = parse(&allocator, path, source)?;
    let built = SemanticBuilder::new().build(&program);
    if !built.errors.is_empty() {
        return None;
    }
    let scope = HostConstantScope::new(&program, &built.semantic, folds)?;
    if scope.is_empty() {
        return None;
    }
    let nodes = built.semantic.nodes();
    let mut edits = Vec::<Edit>::new();
    for node in nodes.iter() {
        let AstKind::IfStatement(statement) = node.kind() else {
            continue;
        };
        let Some((value, true)) = scope.truthiness(&statement.test) else {
            continue;
        };
        // The condition itself becomes the literal it reads as (`!0`, `!1`),
        // so every later syntax walk -- `unconditional_calls` among them --
        // decides it without knowing about host constants.
        let test = statement.test.span();
        if test.end - test.start >= 2 {
            edits.push(Edit::literal(test.start, test.end, value));
        }
        let (live, dead_arm) = if value {
            (Some(&statement.consequent), statement.alternate.as_ref())
        } else {
            (statement.alternate.as_ref(), Some(&statement.consequent))
        };
        if let Some(dead) = dead_arm {
            if value {
                // `else <alternate>`: from the end of the consequent.
                edits.push(Edit::blank(
                    statement.consequent.span().end,
                    dead.span().end,
                ));
            } else {
                edits.push(Edit::empty_statement(dead.span().start, dead.span().end));
            }
        }
        if live.is_some_and(always_leaves_function) {
            let parent = nodes.parent_kind(node.id());
            let siblings = match parent {
                AstKind::FunctionBody(body) => Some(&body.statements),
                AstKind::BlockStatement(block) => Some(&block.body),
                _ => None,
            };
            if let Some(siblings) = siblings {
                let mut after = false;
                for sibling in siblings {
                    if after {
                        dead_statement(sibling, &mut edits);
                    } else if sibling.span() == statement.span {
                        after = true;
                    }
                }
            }
        }
    }
    if edits.is_empty() {
        return None;
    }
    Some(apply(source, edits))
}

fn always_leaves_function(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::ReturnStatement(_) | Statement::ThrowStatement(_) => true,
        Statement::BlockStatement(block) => {
            for inner in &block.body {
                if matches!(
                    inner,
                    Statement::ReturnStatement(_) | Statement::ThrowStatement(_)
                ) {
                    return true;
                }
                let mut jumps = Jumps::default();
                jumps.visit_statement(inner);
                if jumps.found {
                    return false;
                }
            }
            false
        }
        _ => false,
    }
}

/// A `break` or `continue` outside every nested function: a path out of an arm
/// that is not out of the function.
#[derive(Default)]
struct Jumps {
    found: bool,
}

impl<'a> Visit<'a> for Jumps {
    fn visit_break_statement(&mut self, _: &oxc_ast::ast::BreakStatement<'a>) {
        self.found = true;
    }
    fn visit_continue_statement(&mut self, _: &oxc_ast::ast::ContinueStatement<'a>) {
        self.found = true;
    }
    fn visit_function(&mut self, _: &oxc_ast::ast::Function<'a>, _: oxc_syntax::scope::ScopeFlags) {
    }
    fn visit_arrow_function_expression(&mut self, _: &oxc_ast::ast::ArrowFunctionExpression<'a>) {}
    fn visit_class(&mut self, _: &oxc_ast::ast::Class<'a>) {}
}

fn dead_statement(statement: &Statement<'_>, edits: &mut Vec<Edit>) {
    match statement {
        Statement::FunctionDeclaration(_) => {}
        Statement::VariableDeclaration(declaration) => {
            for declarator in &declaration.declarations {
                if let Some(init) = &declarator.init {
                    edits.push(Edit::zero(init.span().start, init.span().end));
                }
            }
        }
        _ => edits.push(Edit::blank(statement.span().start, statement.span().end)),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EditKind {
    Blank,
    EmptyStatement,
    Zero,
    Literal(bool),
}

#[derive(Clone, Copy, Debug)]
struct Edit {
    start: u32,
    end: u32,
    kind: EditKind,
}

impl Edit {
    fn blank(start: u32, end: u32) -> Self {
        Self {
            start,
            end,
            kind: EditKind::Blank,
        }
    }
    fn empty_statement(start: u32, end: u32) -> Self {
        Self {
            start,
            end,
            kind: EditKind::EmptyStatement,
        }
    }
    fn zero(start: u32, end: u32) -> Self {
        Self {
            start,
            end,
            kind: EditKind::Zero,
        }
    }
    fn literal(start: u32, end: u32, value: bool) -> Self {
        Self {
            start,
            end,
            kind: EditKind::Literal(value),
        }
    }
}

fn apply(source: &str, mut edits: Vec<Edit>) -> String {
    // Outermost first: an edit inside one already applied is subsumed.
    edits.sort_by_key(|edit| (edit.start, std::cmp::Reverse(edit.end)));
    let mut bytes = source.as_bytes().to_vec();
    let mut covered_until = 0_u32;
    for edit in edits {
        if edit.end <= covered_until || edit.start >= edit.end {
            continue;
        }
        if edit.start < covered_until {
            // Partial overlap cannot happen between statement spans; refuse it
            // rather than guess.
            continue;
        }
        let range = edit.start as usize..edit.end as usize;
        for byte in &mut bytes[range.clone()] {
            if *byte != b'\n' && *byte != b'\r' {
                *byte = b' ';
            }
        }
        match edit.kind {
            EditKind::Blank => {}
            EditKind::EmptyStatement => bytes[range.start] = b';',
            EditKind::Zero => bytes[range.start] = b'0',
            EditKind::Literal(value) => {
                bytes[range.start] = b'!';
                bytes[range.start + 1] = if value { b'0' } else { b'1' };
            }
        }
        covered_until = edit.end;
    }
    // Only ASCII bytes were written, over whole UTF-8 sequences inside the
    // spans (span boundaries are character boundaries), so this is valid UTF-8.
    String::from_utf8(bytes).unwrap_or_else(|_| source.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fold(value: bool) -> Vec<HostConstantFold> {
        vec![HostConstantFold {
            specifier: "@solidjs/web".into(),
            imported: "isServer".into(),
            value,
        }]
    }

    #[test]
    fn a_boolean_const_export_is_read_from_the_module() {
        let web = "const isServer = false;\nconst isDev = false;\nexport { isDev, isServer };\n";
        assert_eq!(
            exported_boolean_constant(Path::new("web.js"), web, "isServer"),
            Some(false)
        );
        let server = "export const isServer = true;\n";
        assert_eq!(
            exported_boolean_constant(Path::new("server.js"), server, "isServer"),
            Some(true)
        );
        // @solid-primitives/utils: not a literal.
        let utils = "const isServer = typeof window === \"undefined\";\nexport { isServer };\n";
        assert_eq!(
            exported_boolean_constant(Path::new("utils.js"), utils, "isServer"),
            None
        );
        let written = "let isServer = false;\nisServer = true;\nexport { isServer };\n";
        assert_eq!(
            exported_boolean_constant(Path::new("w.js"), written, "isServer"),
            None
        );
        let forwarded = "export { isServer } from \"./x.js\";\n";
        assert_eq!(
            exported_boolean_constant(Path::new("f.js"), forwarded, "isServer"),
            None
        );
        assert_eq!(
            exported_boolean_constant(
                Path::new("a.js"),
                "const a = true;\nexport { a as b };\n",
                "b"
            ),
            Some(true)
        );
    }

    #[test]
    fn only_a_condition_bound_to_the_import_is_folded() {
        let source = "import { isServer } from \"@solidjs/web\";\n\
function guarded(fn) {\n  if (isServer) return;\n  onCleanup(fn);\n}\n\
function shadowed(fn) {\n  const isServer = fn();\n  if (isServer) return;\n  onCleanup(fn);\n}\n\
const isSupported = !isServer;\n\
function viaConst(fn) {\n  if (isSupported) {\n    onCleanup(fn);\n  }\n}\n";
        let browser = fold_host_constant_branches(Path::new("m.js"), source, &fold(false))
            .expect("browser folds");
        assert_eq!(browser.len(), source.len());
        assert!(browser.contains("if (!1      ) ;"), "{browser}");
        assert!(browser.contains("const isServer = fn();\n  if (isServer) return;"));
        assert!(
            browser.contains("if (!0         ) {\n    onCleanup(fn);"),
            "{browser}"
        );
        let node = fold_host_constant_branches(Path::new("m.js"), source, &fold(true))
            .expect("node folds");
        let guarded = &node[..node.find("function shadowed").unwrap()];
        assert!(guarded.contains("if (!0      ) return;"), "{guarded}");
        assert!(!guarded.contains("onCleanup"), "{guarded}");
        assert!(node.contains("const isServer = fn();\n  if (isServer) return;\n  onCleanup(fn);"));
        assert!(!node[node.find("function viaConst").unwrap()..].contains("onCleanup"));
    }

    #[test]
    fn nothing_folds_without_a_bound_import() {
        let aliased = "import * as web from \"@solidjs/web\";\nfunction f() { if (web.isServer) return; g(); }\n";
        assert!(fold_host_constant_branches(Path::new("m.js"), aliased, &fold(true)).is_none());
        let other = "import { isServer } from \"@solid-primitives/utils\";\nfunction f() { if (isServer) return; g(); }\n";
        assert!(fold_host_constant_branches(Path::new("m.js"), other, &fold(true)).is_none());
        let plain = "function f() { if (false) return; g(); }\n";
        assert!(fold_host_constant_branches(Path::new("m.js"), plain, &fold(true)).is_none());
    }
}
