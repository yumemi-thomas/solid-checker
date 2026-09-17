//! Whether a module's exact bytes emit any JavaScript.
//!
//! Two premises live here, and they answer the same question for different
//! evidence.
//!
//! [`ModuleFlavor::Module`] is decided by the bytes alone: parsed as a
//! TypeScript module, is every module-level statement erasable? It is
//! deliberately blind to the filename — a member called `index.js` whose
//! content is `export declare function f(): void;` gets the same answer as the
//! identical bytes called `index.d.ts`, because a suffix is a publisher's claim
//! about a file while the statement list is evidence about it.
//!
//! [`ModuleFlavor::DeclarationFile`] is the narrower premise that *does* admit
//! the suffix, and only ever conjoined with an ambient-only parse: TypeScript
//! itself decides declaration-file semantics by suffix, and it emits no
//! JavaScript for such a file at all — including for the re-export forms a
//! plain module would emit. The caller owes the suffix check and, crucially,
//! must have authenticated the member first; what this module owes is the
//! proof that the bytes really are ambient. That is why the gate below rejects
//! an implementation body, an initializer, an expression statement and a
//! side-effect import anywhere in the tree: those are exactly the shapes
//! (TS1183, TS1039) that make a publisher's `.d.ts` claim false, and without
//! them refused the suffix would be doing the work on its own.
//!
//! Every judgment is fail-closed: anything not provably non-emitting is
//! reported as the exact statement that emits, and bytes that do not parse are
//! an error rather than an answer. Neither premise is a proof that the module
//! is *useful* — see the deliberate trade recorded in `docs/precision-backlog.md`.

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Class, Declaration, ExportAllDeclaration, ExportDefaultDeclaration,
    ExportDefaultDeclarationKind, ExportNamedDeclaration, Function, ImportDeclaration, Program,
    Statement, TSEnumDeclaration, TSModuleDeclaration, TSModuleDeclarationBody,
    TSModuleDeclarationName, VariableDeclaration,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::{ParseOptions, Parser};
use oxc_span::{GetSpan, SourceType, Span as OxcSpan};
use oxc_syntax::scope::ScopeFlags;
use std::collections::BTreeSet;
use thiserror::Error;

/// Which premise to decide.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModuleFlavor {
    /// Bytes only: every module-level statement must be erasable.
    Module,
    /// The caller has authenticated a member whose suffix is `.d.ts`,
    /// `.d.mts` or `.d.cts`. Re-export forms are permitted, and the ambient
    /// gate must hold.
    DeclarationFile,
}

/// The first module-level statement that emits JavaScript, named by kind and by
/// its exact byte range in the analyzed bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmittingStatement {
    pub kind: &'static str,
    pub start: u32,
    pub end: u32,
}

impl std::fmt::Display for EmittingStatement {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} at bytes {}..{}",
            self.kind, self.start, self.end
        )
    }
}

/// What a module's bytes emit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModuleEmission {
    /// Every module-level statement is non-emitting, and at least one of them
    /// declares a name (or, under [`ModuleFlavor::DeclarationFile`], re-exports
    /// one).
    NonEmitting,
    /// There are no module-level statements at all.
    Empty,
    /// Every statement is erasable but none declares anything: `export {}` and
    /// nothing else. Deliberately not [`ModuleEmission::NonEmitting`], and for
    /// the same reason as [`ModuleEmission::Empty`] — a module that declares at
    /// least one type is a deliberate type module, while a module that declares
    /// nothing is indistinguishable from a broken build, and `export {}` beside
    /// a zero-byte sibling is exactly how one publisher spells both. It is a
    /// separate variant from `Empty` only so the refusal can say which of the
    /// two shapes it saw.
    NonDeclaring,
    /// The first statement proving the module emits JavaScript.
    Emitting(EmittingStatement),
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ModuleEmissionError {
    #[error("module bytes do not parse as a TypeScript module: {0}")]
    Parse(String),
}

/// The parse configurations tried for [`ModuleFlavor::Module`], in order.
///
/// The order is **not** load-bearing and must not become so: every
/// configuration that parses cleanly is answered from the same statement table,
/// so the ladder only ever widens which bytes get an answer at all. The
/// `every_parse_order_agrees_on_every_case` test pins that over the whole shared
/// corpus, which is why the list is a constant rather than an argument.
const MODULE_LADDER: [ParseFlavor; 3] = [
    ParseFlavor::Ts,
    ParseFlavor::DeclarationTs,
    ParseFlavor::Tsx,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ParseFlavor {
    Ts,
    DeclarationTs,
    Tsx,
}

impl ParseFlavor {
    fn source_type(self) -> SourceType {
        match self {
            Self::Ts => SourceType::ts(),
            Self::DeclarationTs => SourceType::d_ts(),
            Self::Tsx => SourceType::tsx(),
        }
    }
}

/// Answers [`ModuleEmission`] for exact module bytes under one premise.
pub fn module_emission(
    source: &str,
    flavor: ModuleFlavor,
) -> Result<ModuleEmission, ModuleEmissionError> {
    match flavor {
        ModuleFlavor::Module => module_emission_with_ladder(source, &MODULE_LADDER),
        // A declaration file has exactly one grammar, and admitting the suffix
        // as evidence is only sound while the parse is the ambient one.
        ModuleFlavor::DeclarationFile => with_parse(
            source,
            ParseFlavor::DeclarationTs,
            declaration_file_emission,
        ),
    }
}

fn module_emission_with_ladder(
    source: &str,
    ladder: &[ParseFlavor],
) -> Result<ModuleEmission, ModuleEmissionError> {
    let mut first_error = None;
    for flavor in ladder {
        match with_parse(source, *flavor, program_emission) {
            Ok(emission) => return Ok(emission),
            Err(ModuleEmissionError::Parse(error)) => {
                first_error.get_or_insert(error);
            }
        }
    }
    Err(ModuleEmissionError::Parse(first_error.unwrap_or_else(
        || "no parse configuration accepted the bytes".into(),
    )))
}

/// Parses once into a stack-scoped arena and answers from the program while it
/// is still alive, so no Oxc node ever escapes this module.
fn with_parse<T>(
    source: &str,
    flavor: ParseFlavor,
    answer: impl FnOnce(&Program<'_>) -> T,
) -> Result<T, ModuleEmissionError> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, flavor.source_type())
        .with_options(ParseOptions {
            preserve_parens: false,
            ..ParseOptions::default()
        })
        .parse();
    if let Some(error) = parsed.errors.first() {
        return Err(ModuleEmissionError::Parse(error.to_string()));
    }
    Ok(answer(&parsed.program))
}

fn program_emission(program: &Program<'_>) -> ModuleEmission {
    if let Some(emitting) = directive_emission(program) {
        return emitting;
    }
    let mut declares = false;
    for statement in &program.body {
        if let Some(kind) = statement_emission(statement) {
            return emitting_at(kind, statement.span());
        }
        declares = declares || statement_declares(statement);
    }
    if let Some(emitting) = ambient_signature_emission(program) {
        return emitting;
    }
    if program.body.is_empty() {
        return ModuleEmission::Empty;
    }
    if !declares {
        return ModuleEmission::NonDeclaring;
    }
    ModuleEmission::NonEmitting
}

/// [`ModuleFlavor::DeclarationFile`]: the erasable table, widened by the
/// re-export forms a declaration file also erases, and narrowed by the ambient
/// gate that keeps the suffix honest.
fn declaration_file_emission(program: &Program<'_>) -> ModuleEmission {
    if let Some(emitting) = directive_emission(program) {
        return emitting;
    }
    let ambient = ambient_bindings(program);
    let mut declares = false;
    for statement in &program.body {
        match declaration_file_statement(statement, &ambient) {
            DeclarationStatement::Emitting(kind) => return emitting_at(kind, statement.span()),
            DeclarationStatement::Declares => declares = true,
            DeclarationStatement::Inert => {}
        }
    }
    if let Some(emitting) = ambient_signature_emission(program) {
        return emitting;
    }
    if program.body.is_empty() {
        return ModuleEmission::Empty;
    }
    if !declares {
        return ModuleEmission::NonDeclaring;
    }
    // Only now: an implementation body, an initializer, a nested expression
    // statement or a side-effect import anywhere in the tree means these bytes
    // are not a declaration file at all, so the suffix cannot speak for them.
    let mut gate = AmbientGate { violation: None };
    gate.visit_program(program);
    if let Some((kind, span)) = gate.violation {
        return emitting_at(kind, span);
    }
    ModuleEmission::NonEmitting
}

/// A parameter default inside a signature with no body. TS1039 forbids an
/// initializer in an ambient context, and a bodyless signature is either ambient
/// or an overload — so this is a shape TypeScript rejects and nothing emits
/// from, and **both** premises refuse it. A default in a real implementation is
/// not this shape: that function has a body, so it is already an emitting
/// statement (or gated as an implementation body).
///
/// This runs under both premises deliberately, unlike [`AmbientGate`]: the
/// generator's TypeScript parser accepts these bytes and would otherwise answer
/// non-emitting where this side answers something else, and a disagreement
/// refuses a whole proposal.
struct AmbientSignatureGate {
    violation: Option<OxcSpan>,
}

impl<'a> Visit<'a> for AmbientSignatureGate {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if function.body.is_none()
            && let Some(parameter) = function
                .params
                .items
                .iter()
                .find(|parameter| parameter.initializer.is_some())
            && self.violation.is_none()
        {
            self.violation = Some(parameter.span);
        }
        walk::walk_function(self, function, flags);
    }
}

fn ambient_signature_emission(program: &Program<'_>) -> Option<ModuleEmission> {
    let mut gate = AmbientSignatureGate { violation: None };
    gate.visit_program(program);
    gate.violation
        .map(|span| emitting_at("ambient parameter initializer", span))
}

fn emitting_at(kind: &'static str, span: OxcSpan) -> ModuleEmission {
    ModuleEmission::Emitting(EmittingStatement {
        kind,
        start: span.start,
        end: span.end,
    })
}

/// A directive is a string-literal expression statement (`"use strict";`) that
/// Oxc lifts out of the statement list. It emits, and it is the whole prelude of
/// a bundled CommonJS module, so it counts as both a statement and an emitting
/// one.
fn directive_emission(program: &Program<'_>) -> Option<ModuleEmission> {
    program
        .directives
        .first()
        .map(|directive| emitting_at("directive", directive.span))
}

enum DeclarationStatement {
    Declares,
    Inert,
    Emitting(&'static str),
}

fn declaration_file_statement(
    statement: &Statement<'_>,
    ambient: &BTreeSet<&str>,
) -> DeclarationStatement {
    // A declaration file erases every re-export form, including one naming a
    // value and one naming `default`, because it emits no module at all. The
    // plain-module premise cannot: there the same bytes are a working barrel.
    match statement {
        Statement::ExportAllDeclaration(_) => return DeclarationStatement::Declares,
        Statement::ExportNamedDeclaration(declaration)
            if declaration.declaration.is_none() && declaration.source.is_some() =>
        {
            // `export { name } from "m"` and `export { default } from "m"` name
            // something; `export {} from "m"` names nothing, and a declaration
            // file emits no evaluation of `"m"` either, so it is vacuous rather
            // than emitting.
            return if declaration.specifiers.is_empty() {
                DeclarationStatement::Inert
            } else {
                DeclarationStatement::Declares
            };
        }
        Statement::ExportDefaultDeclaration(declaration) => {
            if let ExportDefaultDeclarationKind::Identifier(identifier) = &declaration.declaration {
                // `export default _default;` where `_default` is a `declare`d
                // binding in these same bytes: the whole file is ambient, so
                // there is no expression to evaluate. An identifier nothing
                // here declares, or any other expression, is not that shape.
                return if ambient.contains(identifier.name.as_str()) {
                    DeclarationStatement::Declares
                } else {
                    DeclarationStatement::Emitting("default export of an undeclared binding")
                };
            }
        }
        _ => {}
    }
    match statement_emission(statement) {
        Some(kind) => DeclarationStatement::Emitting(kind),
        None if statement_declares(statement) => DeclarationStatement::Declares,
        None => DeclarationStatement::Inert,
    }
}

/// The names a file's own top-level `declare`d (or bodyless) declarations bind.
/// Only these can back an `export default <Identifier>` in a declaration file:
/// the identifier must name something the same bytes declare ambiently, so
/// there is nothing to evaluate.
fn ambient_bindings<'a>(program: &Program<'a>) -> BTreeSet<&'a str> {
    let mut names = BTreeSet::new();
    for statement in &program.body {
        match statement {
            Statement::VariableDeclaration(declaration) => {
                collect_variable(declaration, &mut names);
            }
            Statement::FunctionDeclaration(function) => collect_function(function, &mut names),
            Statement::ClassDeclaration(class) => collect_class(class, &mut names),
            Statement::TSEnumDeclaration(declaration) => collect_enum(declaration, &mut names),
            Statement::TSModuleDeclaration(declaration) => collect_module(declaration, &mut names),
            Statement::ExportNamedDeclaration(export) => match &export.declaration {
                Some(Declaration::VariableDeclaration(declaration)) => {
                    collect_variable(declaration, &mut names);
                }
                Some(Declaration::FunctionDeclaration(function)) => {
                    collect_function(function, &mut names);
                }
                Some(Declaration::ClassDeclaration(class)) => collect_class(class, &mut names),
                Some(Declaration::TSEnumDeclaration(declaration)) => {
                    collect_enum(declaration, &mut names);
                }
                Some(Declaration::TSModuleDeclaration(declaration)) => {
                    collect_module(declaration, &mut names);
                }
                _ => {}
            },
            _ => {}
        }
    }
    names
}

fn collect_variable<'a>(declaration: &VariableDeclaration<'a>, names: &mut BTreeSet<&'a str>) {
    if !declaration.declare {
        return;
    }
    for declarator in &declaration.declarations {
        if let Some(identifier) = declarator.id.get_binding_identifier() {
            names.insert(identifier.name.as_str());
        }
    }
}

fn collect_function<'a>(function: &Function<'a>, names: &mut BTreeSet<&'a str>) {
    if (function.declare || function.body.is_none())
        && let Some(identifier) = &function.id
    {
        names.insert(identifier.name.as_str());
    }
}

fn collect_class<'a>(class: &Class<'a>, names: &mut BTreeSet<&'a str>) {
    if class.declare
        && let Some(identifier) = &class.id
    {
        names.insert(identifier.name.as_str());
    }
}

fn collect_enum<'a>(declaration: &TSEnumDeclaration<'a>, names: &mut BTreeSet<&'a str>) {
    if declaration.declare {
        names.insert(declaration.id.name.as_str());
    }
}

fn collect_module<'a>(declaration: &TSModuleDeclaration<'a>, names: &mut BTreeSet<&'a str>) {
    if let TSModuleDeclarationName::Identifier(identifier) = &declaration.id {
        names.insert(identifier.name.as_str());
    }
}

/// Rejects the shapes that make a `.d.ts` claim false. Every one of them is
/// something TypeScript refuses to accept in an ambient context, so their
/// presence means the member is not the declaration file its suffix claims.
struct AmbientGate {
    violation: Option<(&'static str, OxcSpan)>,
}

impl AmbientGate {
    fn record(&mut self, kind: &'static str, span: OxcSpan) {
        if self.violation.is_none() {
            self.violation = Some((kind, span));
        }
    }
}

impl<'a> Visit<'a> for AmbientGate {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some(body) = &function.body {
            self.record("implementation body", body.span);
        }
        walk::walk_function(self, function, flags);
    }

    fn visit_variable_declarator(&mut self, declarator: &oxc_ast::ast::VariableDeclarator<'a>) {
        if let Some(init) = &declarator.init {
            self.record("variable initializer", init.span());
        }
        walk::walk_variable_declarator(self, declarator);
    }

    fn visit_property_definition(&mut self, property: &oxc_ast::ast::PropertyDefinition<'a>) {
        if let Some(value) = &property.value {
            self.record("property initializer", value.span());
        }
        walk::walk_property_definition(self, property);
    }

    fn visit_accessor_property(&mut self, property: &oxc_ast::ast::AccessorProperty<'a>) {
        if let Some(value) = &property.value {
            self.record("property initializer", value.span());
        }
        walk::walk_accessor_property(self, property);
    }

    fn visit_static_block(&mut self, block: &oxc_ast::ast::StaticBlock<'a>) {
        self.record("class static block", block.span);
        walk::walk_static_block(self, block);
    }

    fn visit_expression_statement(&mut self, statement: &oxc_ast::ast::ExpressionStatement<'a>) {
        self.record("expression statement", statement.span);
        walk::walk_expression_statement(self, statement);
    }

    fn visit_import_declaration(&mut self, declaration: &ImportDeclaration<'a>) {
        if !declaration.import_kind.is_type() && declaration.specifiers.is_none() {
            self.record("side-effect import", declaration.span);
        }
        walk::walk_import_declaration(self, declaration);
    }
}

/// Names the reason a module-level statement emits JavaScript, or `None` when
/// it provably emits none.
fn statement_emission(statement: &Statement<'_>) -> Option<&'static str> {
    match statement {
        Statement::TSTypeAliasDeclaration(_) | Statement::TSInterfaceDeclaration(_) => None,
        // `declare global { ... }` and, in declaration-file syntax, a bare
        // `global { ... }`: an augmentation block whose body can only declare.
        Statement::TSGlobalDeclaration(_) => None,
        // `export as namespace React;` names a UMD global for type consumers.
        Statement::TSNamespaceExportDeclaration(_) => None,
        Statement::TSEnumDeclaration(declaration) => enum_emission(declaration),
        Statement::TSModuleDeclaration(declaration) => module_declaration_emission(declaration),
        Statement::FunctionDeclaration(function) => function_emission(function),
        Statement::ClassDeclaration(class) => class_emission(class),
        Statement::VariableDeclaration(declaration) => variable_emission(declaration),
        // `import A = require("m")` and `import A = B.C` both bind a value.
        Statement::TSImportEqualsDeclaration(_) => Some("import-equals declaration"),
        Statement::ImportDeclaration(declaration) => import_emission(declaration),
        Statement::ExportNamedDeclaration(declaration) => export_named_emission(declaration),
        Statement::ExportAllDeclaration(declaration) => export_all_emission(declaration),
        Statement::ExportDefaultDeclaration(declaration) => export_default_emission(declaration),
        // `export = value` is a CommonJS export assignment.
        Statement::TSExportAssignment(_) => Some("export assignment"),
        Statement::ExpressionStatement(_) => Some("expression statement"),
        Statement::BlockStatement(_) => Some("block statement"),
        Statement::EmptyStatement(_) => Some("empty statement"),
        Statement::IfStatement(_)
        | Statement::SwitchStatement(_)
        | Statement::TryStatement(_)
        | Statement::WithStatement(_)
        | Statement::LabeledStatement(_) => Some("control-flow statement"),
        Statement::DoWhileStatement(_)
        | Statement::ForInStatement(_)
        | Statement::ForOfStatement(_)
        | Statement::ForStatement(_)
        | Statement::WhileStatement(_) => Some("loop statement"),
        Statement::BreakStatement(_)
        | Statement::ContinueStatement(_)
        | Statement::ReturnStatement(_)
        | Statement::ThrowStatement(_)
        | Statement::DebuggerStatement(_) => Some("executable statement"),
    }
}

/// The same judgment for the declaration under an `export`.
fn declaration_emission(declaration: &Declaration<'_>) -> Option<&'static str> {
    match declaration {
        Declaration::TSTypeAliasDeclaration(_) | Declaration::TSInterfaceDeclaration(_) => None,
        Declaration::TSGlobalDeclaration(_) => None,
        Declaration::TSEnumDeclaration(declaration) => enum_emission(declaration),
        Declaration::TSModuleDeclaration(declaration) => module_declaration_emission(declaration),
        Declaration::FunctionDeclaration(function) => function_emission(function),
        Declaration::ClassDeclaration(class) => class_emission(class),
        Declaration::VariableDeclaration(declaration) => variable_emission(declaration),
        Declaration::TSImportEqualsDeclaration(_) => Some("import-equals declaration"),
    }
}

/// A `declare enum` is erased; every other enum emits an object, and that
/// includes `const enum`, whose inlining is a compiler option rather than a
/// property of these bytes.
fn enum_emission(declaration: &TSEnumDeclaration<'_>) -> Option<&'static str> {
    (!declaration.declare).then_some("enum declaration")
}

/// A `declare namespace`/`declare module` is erased, and so is an ambient
/// module declaration named by a string literal (`declare module "image:*"`),
/// whose name is only meaningful to a type consumer. An instantiated
/// `namespace N { ... }` emits an object. A namespace with no body at all
/// (`declare namespace N;`) declares nothing to instantiate either.
fn module_declaration_emission(declaration: &TSModuleDeclaration<'_>) -> Option<&'static str> {
    let ambient = declaration.declare
        || matches!(declaration.id, TSModuleDeclarationName::StringLiteral(_))
        || !matches!(
            declaration.body,
            Some(TSModuleDeclarationBody::TSModuleBlock(_))
        );
    (!ambient).then_some("namespace declaration")
}

/// A `declare function` is erased, and so is a bodyless declaration — an
/// ambient signature or an overload signature. An overload's *implementation*
/// is a separate statement with a body, so a real overloaded function still
/// emits.
fn function_emission(function: &Function<'_>) -> Option<&'static str> {
    (!function.declare && function.body.is_some()).then_some("function declaration")
}

fn class_emission(class: &Class<'_>) -> Option<&'static str> {
    (!class.declare).then_some("class declaration")
}

fn variable_emission(declaration: &VariableDeclaration<'_>) -> Option<&'static str> {
    (!declaration.declare).then_some("variable declaration")
}

/// Only `import type` is erased. A bare `import "./effects.js"` has no clause
/// at all and is exactly the side-effect import this rule must never clear, and
/// an all-type-specifier clause is left emitting because whether it survives is
/// a compiler-option question rather than a property of these bytes.
fn import_emission(declaration: &ImportDeclaration<'_>) -> Option<&'static str> {
    (!declaration.import_kind.is_type()).then_some("value import")
}

fn export_named_emission(declaration: &ExportNamedDeclaration<'_>) -> Option<&'static str> {
    if let Some(inner) = &declaration.declaration {
        return declaration_emission(inner);
    }
    if declaration.export_kind.is_type() {
        return None;
    }
    if declaration.specifiers.is_empty() {
        // `export {}` marks a module and emits nothing; `export {} from "m"`
        // still evaluates `m`.
        return declaration
            .source
            .is_some()
            .then_some("re-export of no names");
    }
    declaration
        .specifiers
        .iter()
        .any(|specifier| !specifier.export_kind.is_type())
        .then_some("value export specifier")
}

fn export_all_emission(declaration: &ExportAllDeclaration<'_>) -> Option<&'static str> {
    (!declaration.export_kind.is_type()).then_some("re-export of all names")
}

fn export_default_emission(declaration: &ExportDefaultDeclaration<'_>) -> Option<&'static str> {
    match &declaration.declaration {
        ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => None,
        ExportDefaultDeclarationKind::FunctionDeclaration(function) => function_emission(function),
        ExportDefaultDeclarationKind::ClassDeclaration(class) => class_emission(class),
        // Every remaining kind is an expression, including an identifier whose
        // only declaration is ambient: the emitted `export default name` still
        // binds the module's default export to an evaluated expression.
        _ => Some("default export"),
    }
}

/// Whether an erasable statement introduces at least one name — a type, an
/// ambient value, a namespace, or a global augmentation. A statement that only
/// re-exports locals (`export {}`, `export { type Local }`) or only imports for
/// local use (`import type`) introduces none of its own, so a module made
/// entirely of those is vacuous rather than a type module. Whether the name is
/// *exported* is deliberately not the question: `interface Marker {}` with no
/// export is a deliberate declaration, and this rule is about telling a written
/// module from a broken build.
fn statement_declares(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::TSTypeAliasDeclaration(_)
        | Statement::TSInterfaceDeclaration(_)
        | Statement::TSEnumDeclaration(_)
        | Statement::TSModuleDeclaration(_)
        | Statement::TSGlobalDeclaration(_)
        | Statement::FunctionDeclaration(_)
        | Statement::ClassDeclaration(_)
        | Statement::VariableDeclaration(_) => true,
        // `export type { Signal } from "solid-js"` adds `Signal` to this
        // module's names; `export { type Local }` only re-exports one.
        Statement::ExportNamedDeclaration(declaration) => {
            declaration.declaration.is_some()
                || (declaration.source.is_some() && !declaration.specifiers.is_empty())
        }
        Statement::ExportAllDeclaration(_) => true,
        // `export default interface Options {}` and `export default abstract
        // class`: the declaration is the statement, so the erasable table
        // answers it and this must agree, or the two premises disagree about
        // one file and the whole proposal refuses.
        Statement::ExportDefaultDeclaration(declaration) => matches!(
            declaration.declaration,
            ExportDefaultDeclarationKind::TSInterfaceDeclaration(_)
                | ExportDefaultDeclarationKind::FunctionDeclaration(_)
                | ExportDefaultDeclarationKind::ClassDeclaration(_)
        ),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EmittingStatement, MODULE_LADDER, ModuleEmission, ModuleEmissionError, ModuleFlavor,
        ParseFlavor, module_emission, module_emission_with_ladder,
    };
    use serde::Deserialize;

    /// The shared corpus. Both this module's tests and
    /// `packages/cli/test/artifact-resolution.test.mjs` read these exact bytes
    /// and these exact verdicts, so a divergence between the TypeScript and Oxc
    /// statement tables is a test failure here rather than a whole-proposal
    /// refusal in the field.
    const CASES: &str = include_str!("../../../../../fixtures/module-emission/cases.json");

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Corpus {
        format: String,
        cases_version: u16,
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Case {
        name: String,
        source: String,
        module: Verdict,
        declaration_file: Verdict,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "kebab-case")]
    enum Verdict {
        NonEmitting,
        Empty,
        NonDeclaring,
        Unparsable,
        Emitting(String),
        /// Not non-emitting, with the exact variant deliberately unpinned. Used
        /// only where Oxc's and TypeScript's *grammars* disagree about whether
        /// the bytes parse at all — a `.d.ts` carrying an implementation body is
        /// a parse error to one and a gated statement to the other. Both refuse,
        /// which is the load-bearing half; a case may only use this verdict when
        /// no answer either side can give is `non-emitting`.
        Refused,
    }

    fn corpus() -> Corpus {
        let corpus: Corpus = serde_json::from_str(CASES).expect("shared corpus parses");
        assert_eq!(corpus.format, "solid-checker-module-emission-cases");
        assert_eq!(corpus.cases_version, 1);
        corpus
    }

    fn rendered(answer: &Result<ModuleEmission, ModuleEmissionError>) -> String {
        match answer {
            Ok(ModuleEmission::NonEmitting) => "non-emitting".into(),
            Ok(ModuleEmission::Empty) => "empty".into(),
            Ok(ModuleEmission::NonDeclaring) => "non-declaring".into(),
            Ok(ModuleEmission::Emitting(statement)) => format!("emitting:{}", statement.kind),
            Err(_) => "unparsable".into(),
        }
    }

    fn matches(verdict: &Verdict, answer: &str) -> bool {
        match verdict {
            Verdict::Refused => answer != "non-emitting",
            other => expected(other) == answer,
        }
    }

    fn expected(verdict: &Verdict) -> String {
        match verdict {
            Verdict::NonEmitting => "non-emitting".into(),
            Verdict::Empty => "empty".into(),
            Verdict::NonDeclaring => "non-declaring".into(),
            Verdict::Unparsable => "unparsable".into(),
            Verdict::Emitting(kind) => format!("emitting:{kind}"),
            Verdict::Refused => "refused".into(),
        }
    }

    #[test]
    fn the_shared_corpus_answers_both_premises_case_for_case() {
        let corpus = corpus();
        assert!(corpus.cases.len() >= 100, "the corpus is the attack list");
        let mut mismatches = Vec::new();
        for case in &corpus.cases {
            for (flavor, verdict) in [
                (ModuleFlavor::Module, &case.module),
                (ModuleFlavor::DeclarationFile, &case.declaration_file),
            ] {
                let answer = rendered(&module_emission(&case.source, flavor));
                if !matches(verdict, &answer) {
                    mismatches.push(format!(
                        "{} under {flavor:?}: answered {answer}, corpus says {}",
                        case.name,
                        expected(verdict)
                    ));
                }
            }
        }
        assert!(
            mismatches.is_empty(),
            "{} corpus mismatch(es):\n{}",
            mismatches.len(),
            mismatches.join("\n")
        );
    }

    /// The parse ladder's order must never decide an answer. Any mutation that
    /// makes it matter — a configuration that parses the same bytes into a
    /// different statement list, or a table that reads the source type — fails
    /// here.
    #[test]
    fn every_parse_order_agrees_on_every_case() {
        let corpus = corpus();
        let orders = [
            [
                ParseFlavor::Ts,
                ParseFlavor::DeclarationTs,
                ParseFlavor::Tsx,
            ],
            [
                ParseFlavor::Ts,
                ParseFlavor::Tsx,
                ParseFlavor::DeclarationTs,
            ],
            [
                ParseFlavor::DeclarationTs,
                ParseFlavor::Ts,
                ParseFlavor::Tsx,
            ],
            [
                ParseFlavor::DeclarationTs,
                ParseFlavor::Tsx,
                ParseFlavor::Ts,
            ],
            [
                ParseFlavor::Tsx,
                ParseFlavor::Ts,
                ParseFlavor::DeclarationTs,
            ],
            [
                ParseFlavor::Tsx,
                ParseFlavor::DeclarationTs,
                ParseFlavor::Ts,
            ],
        ];
        assert_eq!(
            orders[0], MODULE_LADDER,
            "the default order is one of these"
        );
        for case in &corpus.cases {
            let baseline = rendered(&module_emission(&case.source, ModuleFlavor::Module));
            for order in &orders {
                assert_eq!(
                    rendered(&module_emission_with_ladder(&case.source, order)),
                    baseline,
                    "{} under {order:?}",
                    case.name
                );
            }
        }
    }

    #[test]
    fn the_first_emitting_statement_is_reported_with_its_byte_range() {
        let source = "export type T = 1;\nexport const value = 2;\nstart();";
        assert_eq!(
            module_emission(source, ModuleFlavor::Module),
            Ok(ModuleEmission::Emitting(EmittingStatement {
                kind: "variable declaration",
                start: 19,
                end: 42,
            }))
        );
    }

    #[test]
    fn identical_bytes_get_one_answer_whatever_the_publisher_calls_the_file() {
        // The plain-module premise is blind to the filename by construction: it
        // is handed bytes and nothing else. This is the assertion that would
        // have to be deleted for a suffix guess to creep back into it.
        let ambient = "export declare function createRenderer(options: unknown): unknown;";
        assert_eq!(
            module_emission(ambient, ModuleFlavor::Module),
            Ok(ModuleEmission::NonEmitting)
        );
        assert_eq!(
            module_emission(&format!("{ambient}\n"), ModuleFlavor::Module),
            Ok(ModuleEmission::NonEmitting)
        );
    }

    /// The gate that keeps the declaration-file premise from being a bare
    /// suffix guess: bytes TypeScript would refuse in an ambient context are
    /// not a declaration file, whatever the member is called.
    ///
    /// Oxc's grammar already refuses a *top-level* ambient implementation body
    /// or initializer outright (`declare const value = 1;` does not parse at
    /// all), so those shapes never reach the gate here — the shared corpus
    /// records them as refused-without-a-pinned-variant, because TypeScript
    /// accepts them and reaches the mirror gate instead. What reaches this gate
    /// is everything nested inside an ambient block, where both grammars are
    /// permissive.
    #[test]
    fn the_ambient_gate_names_the_shape_that_breaks_the_suffix_claim() {
        for (source, kind) in [
            (
                "declare module \"m\" { export const value: number; start(); }",
                "expression statement",
            ),
            (
                "declare module \"m\" { import \"./effects.js\"; export const value: number; }",
                "side-effect import",
            ),
            (
                "declare module \"m\" { export const value = 1; }",
                "variable initializer",
            ),
            (
                "declare namespace N { export function f() { return 1; } }",
                "implementation body",
            ),
            (
                "declare global { function f(): void { return; } }",
                "implementation body",
            ),
            ("declare class C { static { } }", "class static block"),
            (
                "declare class C { accessor value = 1; }",
                "property initializer",
            ),
        ] {
            let answer = module_emission(source, ModuleFlavor::DeclarationFile);
            assert_eq!(rendered(&answer), format!("emitting:{kind}"), "{source:?}");
        }
    }

    /// Every gated shape is refused under the premise that admits the suffix.
    /// This is the assertion the mirror in the generator must preserve: a claim
    /// it makes that this premise would refuse costs the whole proposal.
    ///
    /// Under the bytes-only premise several of these *are* non-emitting, and
    /// deliberately so — `declare const value = 1;` is a TS1039 error that still
    /// emits nothing. Which premise a member gets is decided by its suffix by
    /// the caller, and exactly one premise runs, so the two answers never race:
    /// see `artifactCaseDisposition` and
    /// `ArtifactSnapshot::prove_non_emitting_module_target`.
    #[test]
    fn no_gated_shape_is_ever_non_emitting_as_a_declaration_file() {
        for source in [
            "declare const value = 1;",
            "declare class C { m() { return 1; } }",
            "declare class C { value = 1; }",
            "export declare function f(): void { return; }",
            "declare module \"m\" { export const value = 1; }",
            "declare namespace N { export function f() { return 1; } }",
            "declare class C { static { } }",
        ] {
            assert_ne!(
                rendered(&module_emission(source, ModuleFlavor::DeclarationFile)),
                "non-emitting",
                "{source:?}"
            );
        }
    }
}
