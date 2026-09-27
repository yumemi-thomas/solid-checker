//! Where an obligation inside, or at the base of, a module-level class runs,
//! without package or receipt authority (ADR 0134).

use crate::core::Span;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    ArrowFunctionExpression, BindingPattern, Class, ClassElement, ComputedMemberExpression,
    Declaration, Expression, Function, IdentifierReference, ImportDeclarationSpecifier,
    ImportOrExportKind, MethodDefinitionKind, ModuleExportName, NewExpression,
    PrivateFieldExpression, Statement, StaticMemberExpression, ThisExpression,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};
use oxc_syntax::reference::ReferenceId;
use oxc_syntax::symbol::SymbolId;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

/// When the code at an obligation's location runs, relative to the class it
/// belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassObligationKind {
    /// It runs when an instance is constructed: a base-class binding used
    /// only as class heritage, the constructor's own body (its `super(…)`
    /// included), or an instance field's initializer.
    Construction,
    /// It runs when a member of an instance is invoked later: a method or
    /// accessor body, or a function created in the constructor or in a field
    /// initializer (`this.notFound = (opts) => notFound(…)`).
    InstanceMember,
}

/// The module-local facts an attribution of a class obligation needs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassObligation {
    pub kind: ClassObligationKind,
    /// This module's export names that publish one of the affected classes.
    /// The affected classes are the class the obligation belongs to and,
    /// transitively, every module-level class that extends one of them.
    pub published: Vec<String>,
    /// The span of every `new C(…)` in this module whose callee is an
    /// affected class.
    pub construction_sites: Vec<Span>,
}

/// Classifies an obligation at `location` against the module-level classes of
/// one JavaScript module, or answers `None`.
///
/// The answer is exact or absent. It is absent unless all of these hold:
///
/// - the module parses with no error, has no semantic error, and references no
///   `eval`;
/// - `location` is either (a) exactly a value import binding whose every
///   resolved reference is the bare-identifier heritage of a module-level
///   class, or (b) inside a module-level class (`class C {}` or a
///   `var`/`let`/`const C = class {}` declarator), in its bare-identifier
///   heritage, its constructor, an instance field initializer, or an instance
///   method or accessor -- never a static member, a static block, a computed
///   key, or a nested class;
/// - every resolved reference of every affected class binding is one of: the
///   local of a value specifier in a module-level `export { … }` with no
///   `from`; the bare callee of a `new` expression; the bare heritage of
///   another module-level class, which then joins the affected set. `export
///   class C` publishes `C` directly. Any other reference -- the class passed
///   as a value, read, returned, `instanceof` -- refuses;
/// - for [`ClassObligationKind::InstanceMember`], no affected class lets its
///   instance escape inside its own body: every `this` there (outside nested
///   non-arrow functions and nested classes, which rebind it) is the object of
///   a member access.
#[must_use]
pub fn class_obligation(path: &Path, source: &str, location: Span) -> Option<ClassObligation> {
    if source.len() > 4 * 1024 * 1024 {
        return None;
    }
    let source_type = SourceType::from_path(path).ok()?.with_module(true);
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    let built = SemanticBuilder::new().build(&parsed.program);
    if !built.errors.is_empty() {
        return None;
    }
    let mut census = Census::default();
    census.visit_program(&parsed.program);
    if census.eval {
        return None;
    }
    let scoping = built.semantic.scoping();

    // Module-level classes, their binding symbols, and publications.
    let mut classes = Vec::<ModuleClass<'_, '_>>::new();
    let mut publications = HashMap::<ReferenceId, String>::new();
    let mut declared_publications = HashMap::<SymbolId, Vec<String>>::new();
    let mut import_bindings = HashMap::<(u32, u32), SymbolId>::new();
    for statement in &parsed.program.body {
        match statement {
            Statement::ClassDeclaration(class) => classes.extend(ModuleClass::declared(class)),
            Statement::VariableDeclaration(declaration) => {
                for declarator in &declaration.declarations {
                    classes.extend(ModuleClass::declarator(
                        &declarator.id,
                        declarator.init.as_ref(),
                    ));
                }
            }
            Statement::ExportNamedDeclaration(export) => {
                if export.export_kind == ImportOrExportKind::Type {
                    continue;
                }
                match &export.declaration {
                    Some(Declaration::ClassDeclaration(class)) => {
                        if let Some(module_class) = ModuleClass::declared(class) {
                            declared_publications
                                .entry(module_class.symbol)
                                .or_default()
                                .push(module_class.name.clone());
                            classes.push(module_class);
                        }
                    }
                    Some(Declaration::VariableDeclaration(declaration)) => {
                        for declarator in &declaration.declarations {
                            if let Some(module_class) =
                                ModuleClass::declarator(&declarator.id, declarator.init.as_ref())
                            {
                                declared_publications
                                    .entry(module_class.symbol)
                                    .or_default()
                                    .push(module_class.name.clone());
                                classes.push(module_class);
                            }
                        }
                    }
                    Some(_) => {}
                    None if export.source.is_none() => {
                        for specifier in &export.specifiers {
                            if specifier.export_kind == ImportOrExportKind::Type {
                                continue;
                            }
                            if let ModuleExportName::IdentifierReference(local) = &specifier.local
                                && let Some(reference) = local.reference_id.get()
                            {
                                publications
                                    .insert(reference, specifier.exported.name().to_string());
                            }
                        }
                    }
                    None => {}
                }
            }
            Statement::ExportDefaultDeclaration(_) => {
                // A default-exported class is not modeled; any reference to a
                // module-level class from here is simply not recognized below.
            }
            Statement::ImportDeclaration(import)
                if import.import_kind != ImportOrExportKind::Type =>
            {
                for specifier in import.specifiers.iter().flatten() {
                    let local = match specifier {
                        ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                            if specifier.import_kind == ImportOrExportKind::Type {
                                continue;
                            }
                            &specifier.local
                        }
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                            &specifier.local
                        }
                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                            &specifier.local
                        }
                    };
                    if let Some(symbol) = local.symbol_id.get() {
                        import_bindings.insert((local.span.start, local.span.end), symbol);
                    }
                }
            }
            _ => {}
        }
    }
    // Heritage references: reference id -> index of the class it extends.
    let mut heritage = HashMap::<ReferenceId, usize>::new();
    for (index, class) in classes.iter().enumerate() {
        if let Some(reference) = class.heritage_reference() {
            heritage.insert(reference, index);
        }
    }

    // Step 1: which class(es) the obligation belongs to, and when it runs.
    let (kind, seeds) = if let Some(symbol) = import_bindings.get(&(location.start, location.end)) {
        let references = scoping.get_resolved_reference_ids(*symbol);
        if references.is_empty() {
            return None;
        }
        // Every use of the binding must be one this classification answers:
        // a bare heritage, or a position inside a module-level class. The
        // binding's obligation then runs exactly when one of those does.
        let mut seeds = BTreeSet::new();
        let mut kind = ClassObligationKind::InstanceMember;
        for reference in references {
            if let Some(index) = heritage.get(reference) {
                seeds.insert(*index);
                kind = ClassObligationKind::Construction;
                continue;
            }
            let node = scoping.get_reference(*reference).node_id();
            let at = built.semantic.nodes().get_node(node).kind().span();
            let at = Span {
                start: at.start,
                end: at.end,
            };
            let (index, class) = classes
                .iter()
                .enumerate()
                .find(|(_, class)| contains(class.class.span, at))?;
            // A construction-time use makes the whole obligation one: its
            // domains are a superset of the instance-member `returns`.
            if class.classify(at, &census)? == ClassObligationKind::Construction {
                kind = ClassObligationKind::Construction;
            }
            seeds.insert(index);
        }
        (kind, seeds)
    } else {
        let (index, class) = classes
            .iter()
            .enumerate()
            .find(|(_, class)| contains(class.class.span, location))?;
        (class.classify(location, &census)?, BTreeSet::from([index]))
    };

    // Step 2: the affected set, closed over subclasses, with every reference
    // of every member accounted for.
    let mut affected = BTreeSet::new();
    let mut pending = seeds.into_iter().collect::<Vec<_>>();
    let mut published = BTreeSet::new();
    let mut construction_sites = BTreeSet::new();
    while let Some(index) = pending.pop() {
        if !affected.insert(index) {
            continue;
        }
        let class = &classes[index];
        if !scoping.symbol_redeclarations(class.symbol).is_empty() {
            return None;
        }
        for name in declared_publications
            .get(&class.symbol)
            .into_iter()
            .flatten()
        {
            published.insert(name.clone());
        }
        for reference in scoping.get_resolved_reference_ids(class.symbol) {
            if let Some(name) = publications.get(reference) {
                published.insert(name.clone());
            } else if let Some(site) = census.new_callees.get(reference) {
                construction_sites.insert((site.start, site.end));
            } else {
                // A subclass joins the set; anything else refuses.
                pending.push(*heritage.get(reference)?);
            }
        }
        if kind == ClassObligationKind::InstanceMember && census.instance_escapes(class.class.span)
        {
            return None;
        }
    }
    Some(ClassObligation {
        kind,
        published: published.into_iter().collect(),
        construction_sites: construction_sites
            .into_iter()
            .map(|(start, end)| Span { start, end })
            .collect(),
    })
}

fn contains(outer: oxc_span::Span, location: Span) -> bool {
    outer.start <= location.start && location.end <= outer.end
}

fn contains_span(outer: oxc_span::Span, inner: oxc_span::Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end && outer != inner
}

struct ModuleClass<'c, 'a> {
    class: &'c Class<'a>,
    symbol: SymbolId,
    name: String,
}

impl<'c, 'a> ModuleClass<'c, 'a> {
    fn declared(class: &'c Class<'a>) -> Option<Self> {
        let id = class.id.as_ref()?;
        Some(Self {
            class,
            symbol: id.symbol_id.get()?,
            name: id.name.to_string(),
        })
    }

    fn declarator(id: &BindingPattern<'a>, init: Option<&'c Expression<'a>>) -> Option<Self> {
        let BindingPattern::BindingIdentifier(identifier) = id else {
            return None;
        };
        let Expression::ClassExpression(class) = init?.get_inner_expression() else {
            return None;
        };
        Some(Self {
            class,
            symbol: identifier.symbol_id.get()?,
            name: identifier.name.to_string(),
        })
    }

    fn heritage_reference(&self) -> Option<ReferenceId> {
        let Expression::Identifier(identifier) =
            self.class.super_class.as_ref()?.get_inner_expression()
        else {
            return None;
        };
        identifier.reference_id.get()
    }

    /// When the code at `location`, inside this class's span, runs.
    fn classify(&self, location: Span, census: &Census) -> Option<ClassObligationKind> {
        // A nested class rebinds everything this classification reads.
        if census
            .classes
            .iter()
            .any(|span| contains_span(self.class.span, *span) && contains(*span, location))
        {
            return None;
        }
        if let Some(heritage) = &self.class.super_class
            && contains(heritage.span(), location)
        {
            // Only a bare identifier: evaluating it reads a binding and
            // calls nothing, so what the base does runs at construction.
            return self
                .heritage_reference()
                .map(|_| ClassObligationKind::Construction);
        }
        for element in &self.class.body.body {
            if !contains(element.span(), location) {
                continue;
            }
            return match element {
                ClassElement::MethodDefinition(method) => {
                    if method.r#static || method.computed || contains(method.key.span(), location) {
                        return None;
                    }
                    match method.kind {
                        MethodDefinitionKind::Constructor => {
                            let nested = census.functions.iter().any(|span| {
                                contains_span(method.value.span, *span) && contains(*span, location)
                            });
                            Some(if nested {
                                ClassObligationKind::InstanceMember
                            } else {
                                ClassObligationKind::Construction
                            })
                        }
                        MethodDefinitionKind::Method
                        | MethodDefinitionKind::Get
                        | MethodDefinitionKind::Set => Some(ClassObligationKind::InstanceMember),
                    }
                }
                ClassElement::PropertyDefinition(property) => {
                    if property.r#static
                        || property.computed
                        || contains(property.key.span(), location)
                    {
                        return None;
                    }
                    let value = property.value.as_ref()?;
                    let nested = census.functions.iter().any(|span| {
                        contains(
                            value.span(),
                            Span {
                                start: span.start,
                                end: span.end,
                            },
                        ) && contains(*span, location)
                    });
                    Some(if nested {
                        ClassObligationKind::InstanceMember
                    } else {
                        ClassObligationKind::Construction
                    })
                }
                ClassElement::StaticBlock(_)
                | ClassElement::AccessorProperty(_)
                | ClassElement::TSIndexSignature(_) => None,
            };
        }
        None
    }
}

/// One pass over the module for everything the classification reads.
#[derive(Default)]
struct Census {
    eval: bool,
    /// `new C(…)` with a bare identifier callee: reference id -> expression span.
    new_callees: HashMap<ReferenceId, Span>,
    /// Every function and arrow span.
    functions: Vec<oxc_span::Span>,
    /// Every class span.
    classes: Vec<oxc_span::Span>,
    /// Every `this` that is the object of a member access, by span start.
    member_this: HashSet<u32>,
    /// Every `this`, with the spans of the non-arrow functions and classes
    /// that enclose it.
    this_sites: Vec<(u32, Vec<oxc_span::Span>)>,
    /// Non-arrow functions and classes currently open.
    rebinding: Vec<oxc_span::Span>,
}

impl Census {
    /// Whether a `this` in `class`'s own body (not rebound by a nested
    /// non-arrow function or class) is used as anything but a member access.
    fn instance_escapes(&self, class: oxc_span::Span) -> bool {
        self.this_sites.iter().any(|(start, rebinding)| {
            class.start <= *start
                && *start < class.end
                // The innermost rebinding scope must be one of the class's own
                // methods (a function directly in the class), not deeper.
                && rebinding
                    .iter()
                    .filter(|span| contains_span(class, **span))
                    .count()
                    <= 1
                && !self.member_this.contains(start)
        })
    }
}

impl<'a> Visit<'a> for Census {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.eval |= identifier.name == "eval";
        walk::walk_identifier_reference(self, identifier);
    }

    fn visit_new_expression(&mut self, expression: &NewExpression<'a>) {
        if let Expression::Identifier(identifier) = expression.callee.get_inner_expression()
            && let Some(reference) = identifier.reference_id.get()
        {
            self.new_callees.insert(
                reference,
                Span {
                    start: expression.span.start,
                    end: expression.span.end,
                },
            );
        }
        walk::walk_new_expression(self, expression);
    }

    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        self.functions.push(function.span);
        self.rebinding.push(function.span);
        walk::walk_function(self, function, flags);
        self.rebinding.pop();
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.functions.push(arrow.span);
        walk::walk_arrow_function_expression(self, arrow);
    }

    fn visit_class(&mut self, class: &Class<'a>) {
        self.classes.push(class.span);
        self.rebinding.push(class.span);
        walk::walk_class(self, class);
        self.rebinding.pop();
    }

    fn visit_static_member_expression(&mut self, expression: &StaticMemberExpression<'a>) {
        if let Expression::ThisExpression(this) = &expression.object {
            self.member_this.insert(this.span.start);
        }
        walk::walk_static_member_expression(self, expression);
    }

    fn visit_computed_member_expression(&mut self, expression: &ComputedMemberExpression<'a>) {
        if let Expression::ThisExpression(this) = &expression.object {
            self.member_this.insert(this.span.start);
        }
        walk::walk_computed_member_expression(self, expression);
    }

    fn visit_private_field_expression(&mut self, expression: &PrivateFieldExpression<'a>) {
        if let Expression::ThisExpression(this) = &expression.object {
            self.member_this.insert(this.span.start);
        }
        walk::walk_private_field_expression(self, expression);
    }

    fn visit_this_expression(&mut self, this: &ThisExpression) {
        self.this_sites
            .push((this.span.start, self.rebinding.clone()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(source: &str, needle: &str) -> Span {
        let start = u32::try_from(source.find(needle).unwrap()).unwrap();
        Span {
            start,
            end: start + u32::try_from(needle.len()).unwrap(),
        }
    }

    fn classify(source: &str, needle: &str) -> Option<ClassObligation> {
        class_obligation(Path::new("route.js"), source, at(source, needle))
    }

    // The shape of `@tanstack/solid-router@2.0.0-rc.8`'s `dist/esm/route.js`.
    const ROUTE: &str = "import { BaseRoute, notFound } from \"dep\";\n\
function createRoute(options) {\n\treturn new Route(options);\n}\n\
var Route = class extends BaseRoute {\n\tconstructor(options) {\n\t\tsuper(options);\n\
\t\tthis.notFound = (opts) => notFound(opts);\n\t}\n\
\tlookup() {\n\t\treturn notFound(this.id);\n\t}\n};\n\
var NotFoundRoute = class extends Route {\n\tconstructor(options) {\n\t\tsuper({ ...options, id: \"404\" });\n\t}\n};\n\
export { NotFoundRoute, Route, createRoute };\n";

    #[test]
    fn a_base_binding_used_only_as_heritage_runs_at_construction() {
        let found = class_obligation(Path::new("route.js"), ROUTE, at(ROUTE, "BaseRoute")).unwrap();
        assert_eq!(found.kind, ClassObligationKind::Construction);
        assert_eq!(
            found.published,
            vec!["NotFoundRoute".to_string(), "Route".to_string()]
        );
        assert_eq!(
            found.construction_sites.len(),
            1,
            "the one `new Route(options)`"
        );
    }

    #[test]
    fn a_super_call_runs_at_construction_and_a_constructor_closure_at_member_call() {
        assert_eq!(
            classify(ROUTE, "super(options)").unwrap().kind,
            ClassObligationKind::Construction
        );
        let member = classify(ROUTE, "notFound(opts)").unwrap();
        assert_eq!(member.kind, ClassObligationKind::InstanceMember);
        assert_eq!(
            member.published,
            vec!["NotFoundRoute".to_string(), "Route".to_string()]
        );
        assert_eq!(
            classify(ROUTE, "notFound(this.id)").unwrap().kind,
            ClassObligationKind::InstanceMember
        );
    }

    #[test]
    fn a_binding_used_only_inside_instance_members_follows_them() {
        let found = class_obligation(Path::new("route.js"), ROUTE, at(ROUTE, "notFound")).unwrap();
        assert_eq!(found.kind, ClassObligationKind::InstanceMember);
        assert_eq!(
            found.published,
            vec!["NotFoundRoute".to_string(), "Route".to_string()]
        );
        // One use outside every class refuses the binding.
        let source = format!("{ROUTE}export function g() {{ return notFound(1); }}\n");
        assert_eq!(
            class_obligation(Path::new("route.js"), &source, at(&source, "notFound")),
            None
        );
    }

    #[test]
    fn anything_inexact_refuses() {
        for (source, needle) in [
            // The class escapes as a value.
            (
                "var C = class { m() { f(); } };\nconst held = C;\nexport { C, held };\n",
                "f()",
            ),
            // The instance escapes from its own constructor.
            (
                "var C = class { constructor() { register(this); this.m = () => f(); } };\nexport { C };\n",
                "f()",
            ),
            // Static members run on the class value.
            (
                "var C = class { static m() { f(); } };\nexport { C };\n",
                "f()",
            ),
            ("var C = class { static { f(); } };\nexport { C };\n", "f()"),
            // A heritage that is a call runs at module evaluation.
            (
                "var C = class extends mixin(Base) {};\nexport { C };\n",
                "mixin(Base)",
            ),
            // Not in a class at all.
            ("function g() { f(); }\nexport { g };\n", "f()"),
            // Dynamic lexical access.
            (
                "var C = class { m() { f(); } };\nexport { C };\neval(\"C\");\n",
                "f()",
            ),
        ] {
            assert_eq!(classify(source, needle), None, "{source}");
        }
        // A base binding with a use other than heritage.
        let source = "import { Base } from \"dep\";\nvar C = class extends Base {};\nconst b = Base;\nexport { C, b };\n";
        assert_eq!(classify(source, "Base"), None);
    }
}
