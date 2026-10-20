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
//!
//! ADR 0295 adds one premise a caller may declare for a module: it runs in an
//! HTML Window realm ([`WindowRealm`]). There, a strict comparison of
//! `typeof window` or `typeof document` with `"undefined"` is decided, when
//! the identifier is the unbound global; and a module-level `var` holding
//! exactly that comparison carries it, when every read of it is proved to run
//! after its initializer.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_ast::ast::{
    Argument, BinaryOperator, BindingPattern, Class, ClassElement, Declaration,
    ExportDefaultDeclarationKind, Expression, IdentifierReference, ImportDeclarationSpecifier,
    ImportOrExportKind, ModuleExportName, Program, Statement, UnaryOperator,
    VariableDeclarationKind,
};
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_semantic::{
    AstNodes, NodeId, ReferenceId, Scoping, SemanticBuilder, SymbolFlags, SymbolId,
};
use oxc_span::{GetSpan, SourceType, Span};

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

/// ADR 0295: every specifier whose module `source` loads before its own body
/// runs -- each static import, side-effect import, and `export … from`, except
/// a type-only one, which is erased. `None` when the module does not parse,
/// reads an unbound `require` (a CommonJS load this does not follow), or uses
/// `import.meta.glob` (a bundler-generated import). A dynamic `import()`
/// settles only after the importing module's evaluation, so it is no edge.
#[must_use]
pub fn module_load_specifiers(path: &Path, source: &str) -> Option<Vec<String>> {
    let allocator = Allocator::default();
    let program = parse(&allocator, path, source)?;
    let built = SemanticBuilder::new().build(&program);
    if !built.errors.is_empty() {
        return None;
    }
    let scoping = built.semantic.scoping();
    if scoping.root_unresolved_references().contains_key("require") {
        return None;
    }
    // Review round 1: a `require` this module binds itself -- `createRequire`
    // from `module`, a parameter, an alias -- is a CommonJS load too.
    let nodes = built.semantic.nodes();
    let required = nodes.iter().any(|node| {
        matches!(node.kind(), AstKind::IdentifierReference(identifier)
            if matches!(identifier.name.as_str(), "require" | "createRequire"))
    });
    if required {
        return None;
    }
    // `import.meta` is read only as a static member other than `glob`
    // (`import.meta.url`, `import.meta.env`), which loads nothing. An alias of
    // it, a computed member, or `import.meta.glob` may be a bundler-generated
    // import (review round 1).
    let meta = nodes.iter().any(|node| {
        let AstKind::MetaProperty(meta) = node.kind() else {
            return false;
        };
        meta.meta.name == "import"
            && !matches!(nodes.parent_kind(node.id()), AstKind::StaticMemberExpression(member)
                if matches!(member.object, Expression::MetaProperty(_))
                    && member.property.name != "glob")
    });
    if meta {
        return None;
    }
    let mut specifiers = Vec::new();
    for statement in &program.body {
        let specifier = match statement {
            Statement::ImportDeclaration(import)
                if import.import_kind != ImportOrExportKind::Type =>
            {
                &import.source.value
            }
            Statement::ExportAllDeclaration(export)
                if export.export_kind != ImportOrExportKind::Type =>
            {
                &export.source.value
            }
            Statement::ExportNamedDeclaration(export)
                if export.export_kind != ImportOrExportKind::Type =>
            {
                let Some(source) = &export.source else {
                    continue;
                };
                &source.value
            }
            // `import x = require("…")`: a CommonJS load.
            Statement::TSImportEqualsDeclaration(_) => return None,
            _ => continue,
        };
        // Node's `module` builtin exports `createRequire`, under any alias.
        if matches!(specifier.as_str(), "module" | "node:module") {
            return None;
        }
        specifiers.push(specifier.to_string());
    }
    specifiers.sort();
    specifiers.dedup();
    Some(specifiers)
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
    /// ADR 0295: the module runs in a declared Window realm.
    window_realm: bool,
    /// ADR 0295: module-level `var` aliases of a Window-realm comparison,
    /// each proved read only after its initializer ran.
    vars: HashMap<SymbolId, &'a Expression<'a>>,
    /// ADR 0295, review round 1: the reads of a `const` alias proved to run
    /// after its initializer. Only these carry the realm through the alias; a
    /// read in the temporal dead zone throws, and deciding it would erase the
    /// `ReferenceError`.
    initialized_const_reads: HashSet<ReferenceId>,
}

/// ADR 0295: the declared premise that a module runs in a standards-conforming
/// HTML Window realm, whose global `window` and `document` exist and are not
/// replaced.
pub struct WindowRealm<'f> {
    /// Whether the module is part of no static import cycle. Asked only when
    /// a module-level `var` alias needs it; a caller that cannot tell answers
    /// `false`.
    pub acyclic: &'f dyn Fn() -> bool,
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
        Self::build(program, semantic, folds, None)
    }

    /// [`Self::new`] for a module the caller declares runs in a Window realm
    /// (ADR 0295). `None` also when the module holds a `with` statement.
    #[must_use]
    pub fn in_window_realm(
        program: &Program<'a>,
        semantic: &'s oxc_semantic::Semantic<'a>,
        folds: &[HostConstantFold],
        realm: &WindowRealm<'_>,
    ) -> Option<Self> {
        Self::build(program, semantic, folds, Some(realm))
    }

    fn build(
        program: &Program<'a>,
        semantic: &'s oxc_semantic::Semantic<'a>,
        folds: &[HostConstantFold],
        realm: Option<&WindowRealm<'_>>,
    ) -> Option<Self> {
        let scoping = semantic.scoping();
        if contains_direct_eval(program) {
            return None;
        }
        if realm.is_some() && contains_with(program) {
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
        let mut const_declarators = Vec::<(SymbolId, NodeId)>::new();
        if !imports.is_empty() || realm.is_some() {
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
                    const_declarators.push((symbol, node.id()));
                }
            }
        }
        let mut scope = Self {
            scoping,
            imports,
            consts,
            window_realm: realm.is_some(),
            vars: HashMap::new(),
            initialized_const_reads: HashSet::new(),
        };
        if let Some(realm) = realm {
            // Asked at most once, and only when an alias needs it.
            let acyclic = std::cell::OnceCell::<bool>::new();
            let acyclic = || *acyclic.get_or_init(|| (realm.acyclic)());
            scope.vars = scope.initialized_var_aliases(program, semantic, &acyclic);
            scope.initialized_const_reads =
                scope.initialized_const_reads(program, semantic, &const_declarators, &acyclic);
        }
        Some(scope)
    }

    /// ADR 0295, review round 1: the reads of each `const` alias proved to
    /// run after its initializer, so that the alias holds its value there. A
    /// read in the temporal dead zone throws a `ReferenceError`, which a
    /// decided condition would erase. A read is proved when it is
    ///
    /// - in the declaration's own function (or module body), not inside a
    ///   nested function or class, and textually after the declarator: within
    ///   one activation that code runs after it, except across `switch`
    ///   cases, so a declaration directly in a `case` proves nothing;
    /// - inside a nested function or class whose outermost such boundary
    ///   below the declaration's function is an expression (an arrow, a
    ///   function expression, a method, a class) starting after the
    ///   declarator: it exists only once the declaration has run;
    /// - inside a hoisted function declaration only when the alias is
    ///   module-level and the module proof of [`Self::initialized_var_aliases`]
    ///   holds: nothing before the declaration runs this module's code, and
    ///   no import cycle reaches it.
    fn initialized_const_reads(
        &self,
        program: &Program<'a>,
        semantic: &oxc_semantic::Semantic<'a>,
        declarators: &[(SymbolId, NodeId)],
        acyclic: &dyn Fn() -> bool,
    ) -> HashSet<ReferenceId> {
        let nodes = semantic.nodes();
        let mut proved = HashSet::new();
        for &(symbol, declarator) in declarators {
            let declared = nodes.kind(declarator).span().end;
            let declaration = nodes.parent_id(declarator);
            let holder = nodes.parent_id(declaration);
            if matches!(nodes.kind(holder), AstKind::SwitchCase(_)) {
                continue;
            }
            let owner = nodes
                .ancestor_ids(declarator)
                .find(|&id| function_like(nodes.kind(id)));
            // A module-level statement of its own, so the statements before
            // it are the module code that may run first.
            let module_level = match nodes.kind(holder) {
                AstKind::Program(_) => true,
                AstKind::ExportNamedDeclaration(_) => {
                    matches!(nodes.parent_kind(holder), AstKind::Program(_))
                }
                _ => false,
            };
            let statement = program
                .body
                .iter()
                .find(|statement| {
                    let span = statement.span();
                    span.start <= declared && declared <= span.end
                })
                .filter(|_| module_level);
            let module_proof = || {
                statement.is_some_and(|statement| {
                    let start = statement.span().start;
                    program
                        .body
                        .iter()
                        .take_while(|before| before.span().end <= start)
                        .all(|before| self.runs_no_module_code(before))
                        && acyclic()
                })
            };
            for &reference in self.scoping.get_resolved_reference_ids(symbol) {
                let node = self.scoping.get_reference(reference).node_id();
                let mut boundary = None;
                for id in nodes.ancestor_ids(node) {
                    if Some(id) == owner {
                        break;
                    }
                    if function_like(nodes.kind(id)) || matches!(nodes.kind(id), AstKind::Class(_))
                    {
                        boundary = Some(id);
                    }
                }
                let initialized = match boundary {
                    None => nodes.kind(node).span().start >= declared,
                    Some(boundary) => match nodes.kind(boundary) {
                        AstKind::Function(function) if function.is_declaration() => module_proof(),
                        kind => kind.span().start >= declared,
                    },
                };
                if initialized {
                    proved.insert(reference);
                }
            }
        }
        proved
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.imports.is_empty() && !self.window_realm
    }

    /// ADR 0295: the module-level `var` aliases of a Window-realm comparison
    /// whose every read is proved to run after the initializer. A `var` is
    /// `undefined` until its declaration runs, so a read before it would take
    /// the other arm; only this layout is admitted:
    ///
    /// - one plain `var` statement at module level with one identifier
    ///   declarator, initialized with exactly the comparison, never written
    ///   or redeclared;
    /// - every reference a read, inside a function or textually after the
    ///   declaration at module level;
    /// - every module-level statement before it runs no code of this module
    ///   ([`Self::runs_no_module_code`]);
    /// - the module in no static import cycle, which the caller proves: a
    ///   module importing it back runs first and may call its hoisted
    ///   functions.
    fn initialized_var_aliases(
        &self,
        program: &Program<'a>,
        semantic: &oxc_semantic::Semantic<'a>,
        acyclic: &dyn Fn() -> bool,
    ) -> HashMap<SymbolId, &'a Expression<'a>> {
        let mut vars = HashMap::new();
        let nodes = semantic.nodes();
        for node in nodes.iter() {
            let AstKind::VariableDeclaration(declaration) = node.kind() else {
                continue;
            };
            if !matches!(nodes.parent_kind(node.id()), AstKind::Program(_))
                || declaration.kind != VariableDeclarationKind::Var
                || declaration.declare
                || declaration.declarations.len() != 1
            {
                continue;
            }
            let declarator = &declaration.declarations[0];
            let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                continue;
            };
            let (Some(symbol), Some(init)) = (identifier.symbol_id.get(), &declarator.init) else {
                continue;
            };
            if self.window_comparison(init).is_none()
                || !self.scoping.symbol_redeclarations(symbol).is_empty()
            {
                continue;
            }
            let after = declaration.span.end;
            let reads_after_init = self
                .scoping
                .get_resolved_references(symbol)
                .all(|reference| {
                    if reference.is_write() {
                        return false;
                    }
                    let node = reference.node_id();
                    nodes.ancestor_kinds(node).any(|kind| {
                        matches!(
                            kind,
                            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
                        )
                    }) || nodes.kind(node).span().start >= after
                });
            if !reads_after_init
                || !program
                    .body
                    .iter()
                    .take_while(|statement| statement.span().end <= declaration.span.start)
                    .all(|statement| self.runs_no_module_code(statement))
            {
                continue;
            }
            if !acyclic() {
                continue;
            }
            vars.insert(symbol, init);
        }
        vars
    }

    /// Whether evaluating a module-level `statement` provably calls no
    /// function of this module: an import or re-export, a function
    /// declaration, an inert class ([`inert_class`]), or a declaration whose
    /// initializers are inert ([`Self::inert_initializer`]).
    fn runs_no_module_code(&self, statement: &Statement<'_>) -> bool {
        match statement {
            Statement::ImportDeclaration(_)
            | Statement::ExportAllDeclaration(_)
            | Statement::FunctionDeclaration(_)
            | Statement::EmptyStatement(_)
            | Statement::TSTypeAliasDeclaration(_)
            | Statement::TSInterfaceDeclaration(_) => true,
            Statement::ClassDeclaration(class) => inert_class(class),
            Statement::VariableDeclaration(declaration) => {
                self.inert_declarators(&declaration.declarations)
            }
            Statement::ExportNamedDeclaration(export) => match &export.declaration {
                None | Some(Declaration::FunctionDeclaration(_)) => true,
                Some(Declaration::ClassDeclaration(class)) => inert_class(class),
                Some(Declaration::VariableDeclaration(declaration)) => {
                    self.inert_declarators(&declaration.declarations)
                }
                Some(_) => false,
            },
            Statement::ExportDefaultDeclaration(export) => matches!(
                export.declaration,
                ExportDefaultDeclarationKind::FunctionDeclaration(_)
            ),
            _ => false,
        }
    }

    fn inert_declarators(&self, declarators: &[oxc_ast::ast::VariableDeclarator<'_>]) -> bool {
        declarators.iter().all(|declarator| {
            matches!(declarator.id, BindingPattern::BindingIdentifier(_))
                && declarator
                    .init
                    .as_ref()
                    .is_none_or(|init| self.inert_initializer(init))
        })
    }

    /// An initializer whose evaluation calls no function of this module: a
    /// literal, `void` of one, a function or arrow expression (created, not
    /// run), a Window-realm comparison, or a call of an imported binding
    /// whose every argument is a literal or a parameterless arrow returning
    /// one (`createContext(null)`, `createContext(() => false)`). An imported
    /// callee reaches this module's functions only through an import cycle,
    /// which the caller excludes, or through an argument, which is closed.
    fn inert_initializer(&self, init: &Expression<'_>) -> bool {
        let init = strip(init);
        if inert_literal(init)
            || matches!(
                init,
                Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
            )
            || self.window_comparison(init).is_some()
        {
            return true;
        }
        let Expression::CallExpression(call) = init else {
            return false;
        };
        let Expression::Identifier(callee) = &call.callee else {
            return false;
        };
        if call.optional {
            return false;
        }
        let Some(symbol) = reference_symbol(self.scoping, callee) else {
            return false;
        };
        self.scoping
            .symbol_flags(symbol)
            .contains(SymbolFlags::Import)
            && call.arguments.iter().all(|argument| match argument {
                Argument::SpreadElement(_) => false,
                argument => argument.as_expression().is_some_and(closed_argument),
            })
    }

    /// ADR 0295: how a strict comparison of `typeof window` or `typeof
    /// document` with `"undefined"` reads in a Window realm: `===` is false
    /// and `!==` true. `None` for every other form, outside a declared
    /// realm, and when the identifier resolves to any binding of this module.
    fn window_comparison(&self, expression: &Expression<'_>) -> Option<bool> {
        if !self.window_realm {
            return None;
        }
        let Expression::BinaryExpression(binary) = strip(expression) else {
            return None;
        };
        let equal = match binary.operator {
            BinaryOperator::StrictEquality => true,
            BinaryOperator::StrictInequality => false,
            _ => return None,
        };
        let (left, right) = (strip(&binary.left), strip(&binary.right));
        let operand = if undefined_string(right) {
            left
        } else if undefined_string(left) {
            right
        } else {
            return None;
        };
        let Expression::UnaryExpression(unary) = operand else {
            return None;
        };
        if unary.operator != UnaryOperator::Typeof {
            return None;
        }
        let Expression::Identifier(identifier) = strip(&unary.argument) else {
            return None;
        };
        if !matches!(identifier.name.as_str(), "window" | "document") {
            return None;
        }
        let reference = self.scoping.get_reference(identifier.reference_id.get()?);
        if reference.symbol_id().is_some()
            || self
                .scoping
                .get_root_binding(identifier.name.as_str().into())
                .is_some()
        {
            return None;
        }
        Some(!equal)
    }

    /// How `expression` reads as a condition: `Some((value, folded))` when it
    /// is decided, where `folded` says a host constant took part. Mirrors the
    /// producer's `literalTruthinessLocked` for the forms both read: boolean,
    /// `null`, string and decimal numeric literals, `!`, parentheses and type
    /// wrappers, a host-constant import, and a never-written `const`.
    #[must_use]
    pub fn truthiness(&self, expression: &Expression<'_>) -> Option<(bool, bool)> {
        self.truthiness_at(expression, 0, self.window_realm)
    }

    /// `realm` is whether the Window realm may take part: it is dropped for
    /// the initializer of a `const` alias read where it is not proved
    /// initialized, which is then read as ADR 0166 reads it.
    fn truthiness_at(
        &self,
        expression: &Expression<'_>,
        depth: usize,
        realm: bool,
    ) -> Option<(bool, bool)> {
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
                self.truthiness_at(&inner.expression, depth + 1, realm)
            }
            Expression::TSAsExpression(inner) => {
                self.truthiness_at(&inner.expression, depth + 1, realm)
            }
            Expression::TSSatisfiesExpression(inner) => {
                self.truthiness_at(&inner.expression, depth + 1, realm)
            }
            Expression::TSNonNullExpression(inner) => {
                self.truthiness_at(&inner.expression, depth + 1, realm)
            }
            Expression::TSTypeAssertion(inner) => {
                self.truthiness_at(&inner.expression, depth + 1, realm)
            }
            Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::LogicalNot => {
                self.truthiness_at(&unary.argument, depth + 1, realm)
                    .map(|(value, folded)| (!value, folded))
            }
            Expression::Identifier(identifier) => {
                let symbol = reference_symbol(self.scoping, identifier)?;
                if let Some(value) = self.imports.get(&symbol) {
                    return Some((*value, true));
                }
                if let Some(init) = self.consts.get(&symbol) {
                    let initialized = identifier
                        .reference_id
                        .get()
                        .is_some_and(|reference| self.initialized_const_reads.contains(&reference));
                    return self.truthiness_at(init, depth + 1, realm && initialized);
                }
                if !realm {
                    return None;
                }
                self.truthiness_at(self.vars.get(&symbol)?, depth + 1, realm)
            }
            Expression::BinaryExpression(_) if realm => self
                .window_comparison(expression)
                .map(|value| (value, true)),
            _ => None,
        }
    }
}

/// `expression` without parentheses and identity-preserving type wrappers.
fn strip<'e, 'a>(mut expression: &'e Expression<'a>) -> &'e Expression<'a> {
    for _ in 0..MAX_DEPTH {
        expression = match expression {
            Expression::ParenthesizedExpression(inner) => &inner.expression,
            Expression::TSAsExpression(inner) => &inner.expression,
            Expression::TSSatisfiesExpression(inner) => &inner.expression,
            Expression::TSNonNullExpression(inner) => &inner.expression,
            Expression::TSTypeAssertion(inner) => &inner.expression,
            _ => return expression,
        };
    }
    expression
}

fn undefined_string(expression: &Expression<'_>) -> bool {
    matches!(expression, Expression::StringLiteral(literal) if literal.value == "undefined")
}

fn inert_literal(expression: &Expression<'_>) -> bool {
    match strip(expression) {
        Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::BigIntLiteral(_) => true,
        Expression::UnaryExpression(unary) => {
            unary.operator == UnaryOperator::Void && inert_literal(&unary.argument)
        }
        _ => false,
    }
}

/// A call argument a callee can run without reaching this module: a literal,
/// or a parameterless arrow whose expression body is one.
fn closed_argument(argument: &Expression<'_>) -> bool {
    match strip(argument) {
        Expression::ArrowFunctionExpression(arrow) => {
            arrow.expression
                && arrow.params.items.is_empty()
                && arrow.params.rest.is_none()
                && matches!(
                    arrow.body.statements.as_slice(),
                    [Statement::ExpressionStatement(statement)]
                        if inert_literal(&statement.expression)
                )
        }
        argument => inert_literal(argument),
    }
}

/// A class whose declaration runs no code: no heritage or decorators, no
/// computed key, no static block, and no static member with an initializer.
fn inert_class(class: &Class<'_>) -> bool {
    class.super_class.is_none()
        && class.decorators.is_empty()
        && class.body.body.iter().all(|element| match element {
            ClassElement::MethodDefinition(method) => {
                !method.computed && method.decorators.is_empty()
            }
            ClassElement::PropertyDefinition(property) => {
                !property.computed
                    && property.decorators.is_empty()
                    && (!property.r#static || property.value.is_none())
            }
            ClassElement::TSIndexSignature(_) => true,
            ClassElement::StaticBlock(_) | ClassElement::AccessorProperty(_) => false,
        })
}

fn contains_with(program: &Program<'_>) -> bool {
    struct With(bool);
    impl<'a> Visit<'a> for With {
        fn visit_with_statement(&mut self, _: &oxc_ast::ast::WithStatement<'a>) {
            self.0 = true;
        }
    }
    let mut visitor = With(false);
    visitor.visit_program(program);
    visitor.0
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
/// `0`, so no reference changes the binding it resolves to. A dead arm or
/// statement that declares a binding outside itself (a nested `var`, a
/// function, a class beside the `if`) keeps its text (review round 1 of ADR
/// 0295).
#[must_use]
pub fn fold_host_constant_branches(
    path: &Path,
    source: &str,
    folds: &[HostConstantFold],
) -> Option<String> {
    if folds.is_empty() {
        return None;
    }
    fold_branches(path, source, folds, None)
}

/// [`fold_host_constant_branches`] for a module the caller declares runs in a
/// Window realm (ADR 0295): a condition a Window-realm comparison decides is
/// folded too.
#[must_use]
pub fn fold_host_constant_branches_in_window_realm(
    path: &Path,
    source: &str,
    folds: &[HostConstantFold],
    realm: &WindowRealm<'_>,
) -> Option<String> {
    fold_branches(path, source, folds, Some(realm))
}

fn fold_branches(
    path: &Path,
    source: &str,
    folds: &[HostConstantFold],
    realm: Option<&WindowRealm<'_>>,
) -> Option<String> {
    let allocator = Allocator::default();
    let program = parse(&allocator, path, source)?;
    let built = SemanticBuilder::new().build(&program);
    if !built.errors.is_empty() {
        return None;
    }
    let scope = match realm {
        Some(realm) => HostConstantScope::in_window_realm(&program, &built.semantic, folds, realm)?,
        None => HostConstantScope::new(&program, &built.semantic, folds)?,
    };
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
        let removable = |span: Span| removable(built.semantic.scoping(), nodes, span);
        if let Some(dead) = dead_arm.filter(|dead| removable(dead.span())) {
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
                        dead_statement(sibling, &mut edits, &removable);
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

fn dead_statement(
    statement: &Statement<'_>,
    edits: &mut Vec<Edit>,
    removable: &dyn Fn(Span) -> bool,
) {
    match statement {
        Statement::FunctionDeclaration(_) => {}
        Statement::VariableDeclaration(declaration) => {
            for declarator in &declaration.declarations {
                if let Some(init) = &declarator.init {
                    edits.push(Edit::zero(init.span().start, init.span().end));
                }
            }
        }
        statement if removable(statement.span()) => {
            edits.push(Edit::blank(statement.span().start, statement.span().end));
        }
        // It declares a binding outside itself: its text stays.
        _ => {}
    }
}

/// Review round 1 of ADR 0295 (and of ADR 0166's fold, which shares this
/// rewrite): whether blanking `span` leaves every surviving reference bound
/// as before. No symbol declared inside it may belong to a scope outside it
/// -- a `var` is hoisted to its function, and a lexical declaration directly
/// in a function body or block belongs to that body -- and none may be a
/// function, which a script hoists out of a block (Annex B). A `var Reflect`
/// in a dead arm still shadows the global `Reflect` for the code after it.
fn removable(scoping: &Scoping, nodes: &AstNodes<'_>, span: Span) -> bool {
    let inside = |inner: Span| span.start <= inner.start && inner.end <= span.end;
    scoping.symbol_ids().all(|symbol| {
        let declared_inside = inside(scoping.symbol_span(symbol))
            || scoping
                .symbol_redeclarations(symbol)
                .iter()
                .any(|redeclaration| inside(redeclaration.span));
        if !declared_inside {
            return true;
        }
        if scoping.symbol_flags(symbol).contains(SymbolFlags::Function) {
            return false;
        }
        let scope = scoping.get_node_id(scoping.symbol_scope_id(symbol));
        inside(nodes.kind(scope).span())
    })
}

/// A function or arrow: a body that runs when it is called, not where it is
/// written.
fn function_like(kind: AstKind<'_>) -> bool {
    matches!(
        kind,
        AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
    )
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
mod tests;
