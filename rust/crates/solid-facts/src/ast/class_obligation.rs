//! Where an obligation inside, or at the base of, a module-level class runs,
//! without package or receipt authority (ADR 0134).

use crate::core::Span;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, ArrowFunctionExpression, AssignmentOperator, BindingPattern, CallExpression, Class,
    ClassElement, ComputedMemberExpression, Declaration, Expression, Function, IdentifierReference,
    ImportDeclarationSpecifier, ImportOrExportKind, MemberExpression, MethodDefinitionKind,
    ModuleExportName, NewExpression, PrivateFieldExpression, PropertyKey, SimpleAssignmentTarget,
    Statement, StaticMemberExpression, ThisExpression,
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
    /// included), an instance field's initializer, a closure either of them
    /// creates without installing it as a member, or a member construction
    /// can invoke (ADR 0134, amendment of 2026-09-28).
    Construction,
    /// It runs only when a member of an instance is invoked later: a method
    /// or accessor body, a closure the constructor installs
    /// (`this.notFound = (opts) => notFound(…)`), or a field whose
    /// initializer is a function -- and no construction can invoke it.
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
/// - for [`ClassObligationKind::InstanceMember`], the members constructing an
///   affected class can invoke are exact (`construction_reaches`): every
///   class on the way, its module-level bases included, is a module-level
///   class; no instance escapes (every `this`, outside nested non-arrow
///   functions and nested classes, which rebind it, is the object of a member
///   access, or the `thisArg` of `this.m.call`/`.apply`); and the member the
///   obligation sits in is not one of them, which would make it construction.
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

    // Step 1: which class(es) the obligation belongs to, and where in them it
    // sits: construction code, or the body of one named instance member.
    let mut positions = Vec::<Position>::new();
    let seeds = if let Some(symbol) = import_bindings.get(&(location.start, location.end)) {
        let references = scoping.get_resolved_reference_ids(*symbol);
        if references.is_empty() {
            return None;
        }
        // Every use of the binding must be one this classification answers:
        // a bare heritage, or a position inside a module-level class. The
        // binding's obligation then runs exactly when one of those does.
        let mut seeds = BTreeSet::new();
        for reference in references {
            if let Some(index) = heritage.get(reference) {
                seeds.insert(*index);
                positions.push(Position::Construction);
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
            positions.push(class.classify(at, &census)?);
            seeds.insert(index);
        }
        seeds
    } else {
        let (index, class) = classes
            .iter()
            .enumerate()
            .find(|(_, class)| contains(class.class.span, location))?;
        positions.push(class.classify(location, &census)?);
        BTreeSet::from([index])
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
    }

    // Step 3: a construction-time position makes the whole obligation one:
    // its domains are a superset of the instance-member `returns`. A member
    // position is construction too when construction can invoke that member
    // (amendment of 2026-09-28), and answers nothing when which members
    // construction can invoke is not exact.
    let kind = if positions.contains(&Position::Construction) {
        ClassObligationKind::Construction
    } else {
        let mut involved = affected.clone();
        for index in &affected {
            let mut current = *index;
            while classes[current].class.super_class.is_some() {
                // A base that is not a module-level class runs a constructor
                // this module cannot see, and it may call any member of the
                // instance, an override included.
                let reference = classes[current].heritage_reference()?;
                let base = scoping.get_reference(reference).symbol_id()?;
                current = classes.iter().position(|class| class.symbol == base)?;
                if !involved.insert(current) {
                    break;
                }
            }
        }
        let reached = construction_reaches(&classes, &involved, &census)?;
        if positions
            .iter()
            .any(|position| matches!(position, Position::Member(key) if reached.contains(key)))
        {
            ClassObligationKind::Construction
        } else {
            ClassObligationKind::InstanceMember
        }
    };
    Some(ClassObligation {
        kind,
        published: published.into_iter().collect(),
        construction_sites: construction_sites
            .into_iter()
            .map(|(start, end)| Span { start, end })
            .collect(),
    })
}

/// Where inside a class an obligation sits.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Position {
    /// Code that runs when an instance is constructed.
    Construction,
    /// The body of the instance member with this key: a method or accessor,
    /// a closure the constructor installs as `this.key = …`, or a field whose
    /// initializer is a function.
    Member(String),
}

/// The key a class element or a member access names, when it names one
/// exactly: `#key` for a private name.
fn property_key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::PrivateIdentifier(identifier) => Some(format!("#{}", identifier.name)),
        _ => key.static_name().map(|name| name.into_owned()),
    }
}

fn member_key(member: &MemberExpression<'_>) -> Option<String> {
    match member {
        MemberExpression::PrivateFieldExpression(expression) => {
            Some(format!("#{}", expression.field.name))
        }
        _ => member.static_property_name().map(str::to_owned),
    }
}

fn is_instance_receiver(expression: &Expression<'_>) -> bool {
    matches!(
        expression,
        Expression::ThisExpression(_) | Expression::Super(_)
    )
}

fn function_span(expression: &Expression<'_>) -> Option<oxc_span::Span> {
    match expression.get_inner_expression() {
        Expression::FunctionExpression(function) => Some(function.span),
        Expression::ArrowFunctionExpression(arrow) => Some(arrow.span),
        _ => None,
    }
}

/// A closure a constructor installs on its instance: a top-level
/// `this.key = <function>` statement of the constructor body.
struct Installation {
    key: String,
    /// The assignment target, `this.key`.
    target: oxc_span::Span,
    function: oxc_span::Span,
}

fn installations(constructor: &Function<'_>) -> Vec<Installation> {
    let mut found = Vec::new();
    let Some(body) = &constructor.body else {
        return found;
    };
    for statement in &body.statements {
        let Statement::ExpressionStatement(statement) = statement else {
            continue;
        };
        let Expression::AssignmentExpression(assignment) =
            statement.expression.get_inner_expression()
        else {
            continue;
        };
        if assignment.operator != AssignmentOperator::Assign {
            continue;
        }
        let Some(target) = assignment.left.as_simple_assignment_target() else {
            continue;
        };
        let Some(member) = target.as_member_expression() else {
            continue;
        };
        if !matches!(member.object(), Expression::ThisExpression(_)) {
            continue;
        }
        let (Some(key), Some(function)) = (member_key(member), function_span(&assignment.right))
        else {
            continue;
        };
        found.push(Installation {
            key,
            target: member.span(),
            function,
        });
    }
    found
}

/// The keys of the instance members that constructing any of `involved` can
/// invoke, directly or through other members -- or `None` when that set is
/// not exact (ADR 0134, amendment of 2026-09-28).
///
/// Construction code is every involved class's constructor body and instance
/// field initializers, less the closures they install as members. A member is
/// reached when reached code names its key on `this` or `super` -- a call, a
/// `.call`/`.apply`, a read that hands it on, or a setter write, all alike --
/// and its body is then reached code too. Keys are matched by name across the
/// whole hierarchy, which can only reach more members than run. It refuses
/// on a computed non-literal member access of `this` or `super`, an instance
/// that escapes as a value, a member written anywhere other than its own
/// installation or declared twice, a static member or static block, and a
/// reached key that two involved classes both define, which is an override a
/// base constructor would run in place of its own.
fn construction_reaches(
    classes: &[ModuleClass<'_, '_>],
    involved: &BTreeSet<usize>,
    census: &Census,
) -> Option<BTreeSet<String>> {
    let mut construction = Vec::<oxc_span::Span>::new();
    let mut excluded = Vec::<oxc_span::Span>::new();
    let mut targets = HashSet::<(u32, u32)>::new();
    let mut members = HashMap::<String, Vec<oxc_span::Span>>::new();
    let mut definers = HashMap::<String, BTreeSet<usize>>::new();
    // Keys a write may not touch: methods, installed closures, function fields.
    let mut callable = HashSet::<String>::new();
    let mut accessors = HashSet::<String>::new();
    let mut fields = HashSet::<String>::new();
    for index in involved {
        let class = &classes[*index];
        if census.instance_escapes(class.class.span) {
            return None;
        }
        // A callable key declared twice in one class: the later declaration
        // replaces the earlier one.
        let mut own = HashSet::<String>::new();
        let mut define = |key: String, region: oxc_span::Span| {
            members.entry(key.clone()).or_default().push(region);
            definers.entry(key).or_default().insert(*index);
        };
        for element in &class.class.body.body {
            match element {
                ClassElement::MethodDefinition(method) => {
                    if method.r#static || method.computed {
                        return None;
                    }
                    match method.kind {
                        MethodDefinitionKind::Constructor => {
                            construction.push(method.value.span);
                            for installation in installations(&method.value) {
                                if !own.insert(installation.key.clone()) {
                                    return None;
                                }
                                excluded.push(installation.function);
                                targets
                                    .insert((installation.target.start, installation.target.end));
                                define(installation.key, installation.function);
                            }
                        }
                        MethodDefinitionKind::Method => {
                            let key = property_key_name(&method.key)?;
                            if !own.insert(key.clone()) {
                                return None;
                            }
                            define(key, method.value.span);
                        }
                        MethodDefinitionKind::Get | MethodDefinitionKind::Set => {
                            let key = property_key_name(&method.key)?;
                            accessors.insert(key.clone());
                            define(key, method.value.span);
                        }
                    }
                }
                ClassElement::PropertyDefinition(property) => {
                    if property.r#static || property.computed {
                        return None;
                    }
                    let key = property_key_name(&property.key)?;
                    match property.value.as_ref() {
                        Some(value) => match function_span(value) {
                            Some(function) => {
                                if !own.insert(key.clone()) {
                                    return None;
                                }
                                excluded.push(function);
                                define(key, function);
                            }
                            None => {
                                construction.push(value.span());
                                fields.insert(key);
                            }
                        },
                        None => {
                            fields.insert(key);
                        }
                    }
                }
                ClassElement::StaticBlock(_) | ClassElement::AccessorProperty(_) => return None,
                ClassElement::TSIndexSignature(_) => {}
            }
        }
        callable.extend(own);
    }
    // A key that is both a callable member and an accessor or a data field
    // is a member some other declaration replaces.
    if callable
        .iter()
        .any(|key| accessors.contains(key) || fields.contains(key))
    {
        return None;
    }
    let within = |span: oxc_span::Span| {
        involved
            .iter()
            .any(|index| contains_span(classes[*index].class.span, span))
    };
    for (span, key) in &census.this_writes {
        if !within(*span) || targets.contains(&(span.start, span.end)) {
            continue;
        }
        match key {
            None => return None,
            Some(key) if callable.contains(key) => return None,
            Some(_) => {}
        }
    }
    if census
        .this_members
        .iter()
        .any(|(span, key)| key.is_none() && within(*span))
    {
        return None;
    }
    let mut reached = BTreeSet::<String>::new();
    let mut regions = construction
        .into_iter()
        .map(|region| (region, true))
        .collect::<Vec<_>>();
    while let Some((region, is_construction)) = regions.pop() {
        for (span, key) in &census.this_members {
            if !contains_span(region, *span)
                || targets.contains(&(span.start, span.end))
                || (is_construction
                    && excluded
                        .iter()
                        .any(|closure| contains_span(*closure, *span)))
            {
                continue;
            }
            let Some(key) = key else {
                return None;
            };
            let Some(bodies) = members.get(key) else {
                continue;
            };
            if !reached.insert(key.clone()) {
                continue;
            }
            if definers.get(key).is_some_and(|classes| classes.len() > 1) {
                return None;
            }
            regions.extend(bodies.iter().map(|body| (*body, false)));
        }
    }
    Some(reached)
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

    /// Where the code at `location`, inside this class's span, sits.
    fn classify(&self, location: Span, census: &Census) -> Option<Position> {
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
            return self.heritage_reference().map(|_| Position::Construction);
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
                            // The outermost function nested in the
                            // constructor is a member only when the
                            // constructor installs it as one. Any other
                            // closure -- a callback it hands on, an IIFE --
                            // may run during construction.
                            let outermost = census
                                .functions
                                .iter()
                                .filter(|span| {
                                    contains_span(method.value.span, **span)
                                        && contains(**span, location)
                                })
                                .max_by_key(|span| span.end - span.start);
                            Some(
                                outermost
                                    .and_then(|closure| {
                                        installations(&method.value)
                                            .into_iter()
                                            .find(|installation| installation.function == *closure)
                                    })
                                    .map_or(Position::Construction, |installation| {
                                        Position::Member(installation.key)
                                    }),
                            )
                        }
                        MethodDefinitionKind::Method
                        | MethodDefinitionKind::Get
                        | MethodDefinitionKind::Set => {
                            Some(Position::Member(property_key_name(&method.key)?))
                        }
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
                    // A field whose initializer is a function is a member;
                    // a closure anywhere else in an initializer may run
                    // during construction.
                    match function_span(value) {
                        Some(function) if contains(function, location) => {
                            Some(Position::Member(property_key_name(&property.key)?))
                        }
                        _ => Some(Position::Construction),
                    }
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
    /// Every member access of `this` or `super`, with the key it names;
    /// `None` for a computed key that is not a literal.
    this_members: Vec<(oxc_span::Span, Option<String>)>,
    /// The subset of `this_members` in a write position.
    this_writes: Vec<(oxc_span::Span, Option<String>)>,
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
        if is_instance_receiver(&expression.object) {
            self.this_members
                .push((expression.span, Some(expression.property.name.to_string())));
        }
        walk::walk_static_member_expression(self, expression);
    }

    fn visit_computed_member_expression(&mut self, expression: &ComputedMemberExpression<'a>) {
        if let Expression::ThisExpression(this) = &expression.object {
            self.member_this.insert(this.span.start);
        }
        if is_instance_receiver(&expression.object) {
            self.this_members.push((
                expression.span,
                expression
                    .static_property_name()
                    .map(|name| name.to_string()),
            ));
        }
        walk::walk_computed_member_expression(self, expression);
    }

    fn visit_private_field_expression(&mut self, expression: &PrivateFieldExpression<'a>) {
        if let Expression::ThisExpression(this) = &expression.object {
            self.member_this.insert(this.span.start);
        }
        if is_instance_receiver(&expression.object) {
            self.this_members
                .push((expression.span, Some(format!("#{}", expression.field.name))));
        }
        walk::walk_private_field_expression(self, expression);
    }

    // `this.m.call(this, …)` and `this.m.apply(this, …)` hand the instance
    // only to its own member, as `this.m(…)` does: not an escape.
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let Expression::StaticMemberExpression(callee) = call.callee.get_inner_expression()
            && matches!(callee.property.name.as_str(), "call" | "apply")
            && let Some(member) = callee.object.get_inner_expression().as_member_expression()
            && is_instance_receiver(member.object())
            && let Some(Argument::ThisExpression(this)) = call.arguments.first()
        {
            self.member_this.insert(this.span.start);
        }
        walk::walk_call_expression(self, call);
    }

    // Every write position -- an assignment's target, an update's operand, a
    // destructuring element, a `for … in`/`of` head -- is a simple target.
    fn visit_simple_assignment_target(&mut self, target: &SimpleAssignmentTarget<'a>) {
        if let Some(member) = target.as_member_expression()
            && is_instance_receiver(member.object())
        {
            self.this_writes.push((member.span(), member_key(member)));
        }
        walk::walk_simple_assignment_target(self, target);
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

    // `route.js` with a base this module declares, so the members every
    // construction can invoke are exactly enumerable.
    const LOCAL_ROUTE: &str = "import { notFound } from \"dep\";\n\
var BaseRoute = class {\n\tconstructor(options) {\n\t\tthis.options = options;\n\t}\n};\n\
function createRoute(options) {\n\treturn new Route(options);\n}\n\
var Route = class extends BaseRoute {\n\tconstructor(options) {\n\t\tsuper(options);\n\
\t\tthis.notFound = (opts) => notFound(opts);\n\t}\n\
\tlookup() {\n\t\treturn notFound(this.id);\n\t}\n};\n\
export { Route, createRoute };\n";

    #[test]
    fn a_super_call_runs_at_construction_and_a_constructor_closure_at_member_call() {
        assert_eq!(
            classify(ROUTE, "super(options)").unwrap().kind,
            ClassObligationKind::Construction
        );
        let member = classify(LOCAL_ROUTE, "notFound(opts)").unwrap();
        assert_eq!(member.kind, ClassObligationKind::InstanceMember);
        assert_eq!(member.published, vec!["Route".to_string()]);
        assert_eq!(
            classify(LOCAL_ROUTE, "notFound(this.id)").unwrap().kind,
            ClassObligationKind::InstanceMember
        );
    }

    #[test]
    fn a_binding_used_only_inside_instance_members_follows_them() {
        let found = class_obligation(
            Path::new("route.js"),
            LOCAL_ROUTE,
            at(LOCAL_ROUTE, "notFound"),
        )
        .unwrap();
        assert_eq!(found.kind, ClassObligationKind::InstanceMember);
        assert_eq!(found.published, vec!["Route".to_string()]);
        // One use outside every class refuses the binding.
        let source = format!("{LOCAL_ROUTE}export function g() {{ return notFound(1); }}\n");
        assert_eq!(
            class_obligation(Path::new("route.js"), &source, at(&source, "notFound")),
            None
        );
    }

    // Amendment of 2026-09-28: a base this module cannot see may invoke any
    // member during `super(…)`, so no member position of its subclasses is
    // exact.
    #[test]
    fn a_member_of_a_class_on_an_imported_base_refuses() {
        for needle in ["notFound(opts)", "notFound(this.id)"] {
            assert_eq!(classify(ROUTE, needle), None, "{needle}");
        }
        assert_eq!(
            class_obligation(Path::new("route.js"), ROUTE, at(ROUTE, "notFound")),
            None
        );
    }

    // The shape of `@tanstack/router-core@1.171.22`'s `RouterCore`: the
    // constructor installs `update` and then calls it.
    const CORE: &str = "import { f, g, h } from \"dep\";\n\
var Core = class {\n\tconstructor(options) {\n\
\t\tthis.update = (next) => {\n\t\t\tf(next);\n\t\t\tthis.helper();\n\t\t};\n\
\t\tthis.later = () => h();\n\
\t\tthis.update(options);\n\t}\n\
\thelper() {\n\t\tg(1);\n\t}\n\
\tother() {\n\t\tg(2);\n\t}\n};\n\
export { Core };\n";

    #[test]
    fn a_member_the_constructor_invokes_runs_at_construction() {
        // Directly, and transitively through `update`.
        assert_eq!(
            classify(CORE, "f(next)").unwrap().kind,
            ClassObligationKind::Construction
        );
        assert_eq!(
            classify(CORE, "g(1)").unwrap().kind,
            ClassObligationKind::Construction
        );
        // Members nothing at construction names stay instance members.
        assert_eq!(
            classify(CORE, "h()").unwrap().kind,
            ClassObligationKind::InstanceMember
        );
        assert_eq!(
            classify(CORE, "g(2)").unwrap().kind,
            ClassObligationKind::InstanceMember
        );
        // A binding with one use in a reached member is construction.
        let found = class_obligation(Path::new("core.js"), CORE, at(CORE, "g")).unwrap();
        assert_eq!(found.kind, ClassObligationKind::Construction);
        // `.call`, a field initializer, and a closure the constructor hands
        // on all reach construction too.
        for (source, needle) in [
            (
                CORE.replace("this.update(options);", "this.update.call(this, options);"),
                "f(next)",
            ),
            (
                CORE.replace(
                    "\tconstructor(options) {",
                    "\tready = this.other();\n\tconstructor(options) {",
                ),
                "g(2)",
            ),
            (
                CORE.replace(
                    "this.update(options);",
                    "[options].forEach(() => this.other());",
                ),
                "g(2)",
            ),
            (
                CORE.replace("this.later = () => h();", "[options].forEach(() => h());"),
                "h()",
            ),
        ] {
            assert_eq!(
                classify(&source, needle).unwrap().kind,
                ClassObligationKind::Construction,
                "{source}"
            );
        }
    }

    #[test]
    fn a_base_constructor_that_calls_an_overridden_member_refuses() {
        let source = "import { f } from \"dep\";\n\
var Base = class {\n\tconstructor() {\n\t\tthis.init();\n\t}\n\tinit() {\n\t\tf(0);\n\t}\n};\n\
var Sub = class extends Base {\n\tinit() {\n\t\tf(1);\n\t}\n};\n\
export { Base, Sub };\n";
        assert_eq!(classify(source, "f(1)"), None);
        assert_eq!(classify(source, "f(0)"), None);
        // Not reached from any construction, an override is harmless.
        let quiet = source.replace("this.init();", "this.ready = true;");
        assert_eq!(
            classify(&quiet, "f(1)").unwrap().kind,
            ClassObligationKind::InstanceMember
        );
    }

    #[test]
    fn an_inexact_member_set_refuses() {
        for (source, needle) in [
            // A computed member of `this` could name any member.
            (
                CORE.replace("this.update(options);", "this[options]();"),
                "g(2)",
            ),
            // A member written after its declaration is not the member.
            (
                CORE.replace("\t\tg(2);", "\t\tg(2);\n\t\tthis.helper = null;"),
                "h()",
            ),
            // The instance escapes to code this module cannot see.
            (
                CORE.replace("this.update(options);", "register(this);"),
                "h()",
            ),
        ] {
            assert_eq!(classify(&source, needle), None, "{source}");
        }
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
