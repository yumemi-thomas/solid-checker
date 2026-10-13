//! What a class export's construction keeps on its instance for its own
//! members, from the module's bytes alone (ADR 0139). A proposal input for the
//! generator; the Type Facts producer's census of the class decides the claim.

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    ArrowFunctionExpression, AssignmentExpression, BindingPattern, CallExpression, Class,
    ClassElement, ComputedMemberExpression, Declaration, Expression, Function, IdentifierReference,
    ImportOrExportKind, MethodDefinitionKind, ModuleExportName, NewExpression,
    PrivateFieldExpression, PropertyKey, ReturnStatement, SimpleAssignmentTarget, Statement,
    StaticMemberExpression, ThisExpression, UnaryExpression,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};
use oxc_syntax::operator::{AssignmentOperator, UnaryOperator};
use oxc_syntax::reference::ReferenceId;
use oxc_syntax::symbol::SymbolId;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

/// One constructor parameter of a class export, as its construction treats
/// it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RetainedParameter {
    /// Stored once, as a top-level `this.<key> = p` statement of the
    /// constructor, and otherwise unused; every other access of the key is a
    /// member call nothing at construction reaches.
    Kept { key: String },
    /// Never referenced.
    Unused,
}

/// Every constructor parameter of a class export that keeps at least one of
/// them for its members, in parameter order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedConstructorArguments {
    pub parameters: Vec<RetainedParameter>,
}

/// The constructor parameters of the module-level class this module publishes
/// as `export_name`, when its construction keeps at least one for its members
/// and every one is either kept or unused -- or `None`.
///
/// The answer is exact or absent, on the rules the producer's census states
/// (`retainedArgumentsLocked`), restated here on Oxc's resolved references so
/// the generator proposes what the census can confirm:
///
/// - the module parses with no error, has no semantic error and no `eval`;
/// - the class has no heritage, decorator, static member, static block,
///   accessor property, computed key, field initializer or nested class; one
///   constructor with a body; parameters that are plain identifiers with no
///   default, no rest and no parameter property;
/// - every `this` in the class body is the object of a member access with a
///   literal key, so the instance never escapes; the constructor returns no
///   value;
/// - every write of a member of `this` is a plain assignment whose value is a
///   constructor parameter, a literal, a template, an object or array literal,
///   a `new` result, a `void`/`typeof`/`!` expression, or -- as a top-level
///   statement of the constructor -- a function literal it installs;
/// - every reference to the class in this module is an export specifier, a
///   `new` callee or a `void` operand;
/// - each kept parameter's key names no class element or installed member, is
///   not `__proto__`, is written nowhere else, and every other access of it is
///   the callee of a call outside the code a construction reaches (the
///   constructor less its installations and, transitively, the body of every
///   member whose key reached code names on `this`).
///
/// A parameter the constructor also calls directly is not described here --
/// the call item's arguments are not derived -- so such a class answers
/// `None`, and so does one that keeps nothing.
#[must_use]
pub fn retained_constructor_arguments(
    path: &Path,
    source: &str,
    export_name: &str,
) -> Option<RetainedConstructorArguments> {
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
    let scoping = built.semantic.scoping();
    let mut module = ModuleWalk::default();
    module.visit_program(&parsed.program);
    if module.eval {
        return None;
    }

    // The class this module publishes under `export_name`.
    let mut classes = Vec::<(SymbolId, &Class<'_>)>::new();
    let mut declared_names = HashMap::<SymbolId, Vec<String>>::new();
    let mut specifiers = HashMap::<ReferenceId, String>::new();
    for statement in &parsed.program.body {
        match statement {
            Statement::ClassDeclaration(class) => classes.extend(declared_class(class)),
            Statement::VariableDeclaration(declaration) => {
                for declarator in &declaration.declarations {
                    classes.extend(declarator_class(&declarator.id, declarator.init.as_ref()));
                }
            }
            Statement::ExportNamedDeclaration(export) => {
                if export.export_kind == ImportOrExportKind::Type {
                    continue;
                }
                match &export.declaration {
                    Some(Declaration::ClassDeclaration(class)) => {
                        if let Some((symbol, class)) = declared_class(class) {
                            let name = scoping.symbol_name(symbol).to_string();
                            declared_names.entry(symbol).or_default().push(name);
                            classes.push((symbol, class));
                        }
                    }
                    Some(Declaration::VariableDeclaration(declaration)) => {
                        for declarator in &declaration.declarations {
                            if let Some((symbol, class)) =
                                declarator_class(&declarator.id, declarator.init.as_ref())
                            {
                                let name = scoping.symbol_name(symbol).to_string();
                                declared_names.entry(symbol).or_default().push(name);
                                classes.push((symbol, class));
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
                                specifiers.insert(reference, specifier.exported.name().to_string());
                            }
                        }
                    }
                    None => {}
                }
            }
            _ => {}
        }
    }
    let (symbol, class) = classes.iter().copied().find(|(symbol, _)| {
        declared_names
            .get(symbol)
            .is_some_and(|names| names.iter().any(|name| name == export_name))
            || scoping
                .get_resolved_reference_ids(*symbol)
                .iter()
                .any(|reference| {
                    specifiers
                        .get(reference)
                        .is_some_and(|name| name == export_name)
                })
    })?;
    if !scoping.symbol_redeclarations(symbol).is_empty() {
        return None;
    }
    // Every reference publishes, constructs, or reads and discards the class.
    for reference in scoping.get_resolved_reference_ids(symbol) {
        if !(specifiers.contains_key(reference)
            || module.new_callees.contains(reference)
            || module.void_operands.contains(reference))
        {
            return None;
        }
    }

    // Class elements.
    if class.super_class.is_some() || !class.decorators.is_empty() {
        return None;
    }
    let mut declared = HashSet::<String>::new();
    let mut bodies = HashMap::<String, Vec<oxc_span::Span>>::new();
    let mut constructor = None::<&Function<'_>>;
    for element in &class.body.body {
        match element {
            ClassElement::MethodDefinition(method) => {
                if method.r#static || method.computed || !method.decorators.is_empty() {
                    return None;
                }
                if method.kind == MethodDefinitionKind::Constructor {
                    if method.value.body.is_some() {
                        if constructor.is_some() {
                            return None;
                        }
                        constructor = Some(&method.value);
                    }
                    continue;
                }
                let key = property_key_name(&method.key)?;
                declared.insert(key.clone());
                if method.value.body.is_some() {
                    bodies.entry(key).or_default().push(method.value.span);
                }
            }
            ClassElement::PropertyDefinition(property) => {
                if property.r#static
                    || property.computed
                    || property.value.is_some()
                    || !property.decorators.is_empty()
                {
                    return None;
                }
                declared.insert(property_key_name(&property.key)?);
            }
            ClassElement::StaticBlock(_) | ClassElement::AccessorProperty(_) => return None,
            ClassElement::TSIndexSignature(_) => {}
        }
    }
    let constructor = constructor?;
    let statements = &constructor.body.as_ref()?.statements;
    if constructor.params.rest.is_some() {
        return None;
    }
    let mut parameters = Vec::new();
    for parameter in &constructor.params.items {
        let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
            return None;
        };
        if parameter.initializer.is_some()
            || !parameter.decorators.is_empty()
            || parameter.accessibility.is_some()
            || parameter.readonly
            || parameter.r#override
        {
            return None;
        }
        parameters.push(identifier.symbol_id.get()?);
    }
    let parameter_indices = parameters
        .iter()
        .enumerate()
        .map(|(index, symbol)| (*symbol, index))
        .collect::<HashMap<_, _>>();

    // Installed members and parameter stores: top-level `this.<key> = …`.
    let mut installed = Vec::<oxc_span::Span>::new();
    let mut installation_statements = HashSet::<(u32, u32)>::new();
    let mut installation_targets = HashSet::<(u32, u32)>::new();
    // parameter reference span -> (key, target span)
    let mut stores = HashMap::<(u32, u32), (String, oxc_span::Span)>::new();
    for statement in statements {
        let Some((key, target, right)) = this_store_statement(statement) else {
            continue;
        };
        match right.get_inner_expression() {
            Expression::ArrowFunctionExpression(arrow) => {
                if declared.contains(&key) || bodies.contains_key(&key) {
                    return None;
                }
                bodies.entry(key).or_default().push(arrow.span);
                installed.push(arrow.span);
                installation_statements.insert((statement.span().start, statement.span().end));
                installation_targets.insert((target.start, target.end));
            }
            Expression::FunctionExpression(function) => {
                if declared.contains(&key) || bodies.contains_key(&key) {
                    return None;
                }
                bodies.entry(key).or_default().push(function.span);
                installed.push(function.span);
                installation_statements.insert((statement.span().start, statement.span().end));
                installation_targets.insert((target.start, target.end));
            }
            Expression::Identifier(identifier) => {
                stores.insert((identifier.span.start, identifier.span.end), (key, target));
            }
            _ => {}
        }
    }

    // One walk over the class body.
    let mut body = ClassWalk {
        class: class.span,
        ..ClassWalk::default()
    };
    for element in &class.body.body {
        body.visit_class_element(element);
    }
    if !body.exact
        || body
            .this_sites
            .iter()
            .any(|start| !body.member_this.contains(start))
        || body.accesses.iter().any(|(_, key)| key.is_none())
    {
        return None;
    }
    // A value returned from the constructor's own frame replaces the instance.
    if body.returns.iter().any(|at| {
        body.functions
            .iter()
            .filter(|function| contains(**function, *at))
            .min_by_key(|function| function.end - function.start)
            == Some(&constructor.span)
    }) {
        return None;
    }
    // Every write of a member of `this` is a plain assignment of a value no
    // hidden code can hang on the instance.
    let assignment_targets = body
        .assignments
        .iter()
        .map(|assignment| (assignment.target.start, assignment.target.end))
        .collect::<HashSet<_>>();
    if body
        .write_targets
        .iter()
        .any(|target| !assignment_targets.contains(&(target.start, target.end)))
    {
        return None;
    }
    for assignment in &body.assignments {
        if assignment.key.as_deref() == Some("__proto__") {
            return None;
        }
        match assignment.value {
            AssignedValue::Function(span) if installed.contains(&span) => {}
            AssignedValue::Reference(Some(reference))
                if scoping
                    .get_reference(reference)
                    .symbol_id()
                    .is_some_and(|symbol| parameter_indices.contains_key(&symbol)) => {}
            AssignedValue::Plain => {}
            _ => return None,
        }
    }

    // What a construction reaches.
    let mut regions = statements
        .iter()
        .filter(|statement| {
            !installation_statements.contains(&(statement.span().start, statement.span().end))
        })
        .map(GetSpan::span)
        .collect::<Vec<_>>();
    let mut reached = BTreeSet::<String>::new();
    let mut index = 0;
    while index < regions.len() {
        let region = regions[index];
        index += 1;
        for (span, key) in &body.accesses {
            let Some(key) = key else { continue };
            if !contains(region, *span)
                || installation_targets.contains(&(span.start, span.end))
                || !reached.insert(key.clone())
            {
                continue;
            }
            regions.extend(bodies.get(key).into_iter().flatten().copied());
        }
    }

    // Each parameter: kept, or unused.
    let mut answer = Vec::new();
    let mut keys = BTreeMap::<String, usize>::new();
    for (index, symbol) in parameters.iter().enumerate() {
        let references = scoping.get_resolved_reference_ids(*symbol);
        if references.is_empty() {
            answer.push(RetainedParameter::Unused);
            continue;
        }
        let [reference] = references else {
            return None;
        };
        let node = scoping.get_reference(*reference).node_id();
        let at = built.semantic.nodes().get_node(node).kind().span();
        let (key, target) = stores.get(&(at.start, at.end))?.clone();
        if key == "__proto__"
            || declared.contains(&key)
            || bodies.contains_key(&key)
            || keys.insert(key.clone(), index).is_some()
        {
            return None;
        }
        for (span, access) in &body.accesses {
            if access.as_deref() != Some(key.as_str()) || *span == target {
                continue;
            }
            if !body.callees.contains(&(span.start, span.end))
                || regions.iter().any(|region| contains(*region, *span))
            {
                return None;
            }
        }
        answer.push(RetainedParameter::Kept { key });
    }
    answer
        .iter()
        .any(|parameter| matches!(parameter, RetainedParameter::Kept { .. }))
        .then_some(RetainedConstructorArguments { parameters: answer })
}

fn declared_class<'c, 'a>(class: &'c Class<'a>) -> Option<(SymbolId, &'c Class<'a>)> {
    Some((class.id.as_ref()?.symbol_id.get()?, class))
}

fn declarator_class<'c, 'a>(
    id: &BindingPattern<'a>,
    init: Option<&'c Expression<'a>>,
) -> Option<(SymbolId, &'c Class<'a>)> {
    let BindingPattern::BindingIdentifier(identifier) = id else {
        return None;
    };
    let Expression::ClassExpression(class) = init?.get_inner_expression() else {
        return None;
    };
    Some((identifier.symbol_id.get()?, class))
}

fn property_key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::PrivateIdentifier(identifier) => Some(format!("#{}", identifier.name)),
        _ => key.static_name().map(|name| name.into_owned()),
    }
}

/// `this.<key> = <right>` as a top-level expression statement: the key, the
/// target's span and the right-hand side.
fn this_store_statement<'s, 'a>(
    statement: &'s Statement<'a>,
) -> Option<(String, oxc_span::Span, &'s Expression<'a>)> {
    let Statement::ExpressionStatement(statement) = statement else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = statement.expression.get_inner_expression()
    else {
        return None;
    };
    if assignment.operator != AssignmentOperator::Assign {
        return None;
    }
    let member = assignment
        .left
        .as_simple_assignment_target()?
        .as_member_expression()?;
    if !matches!(member.object(), Expression::ThisExpression(_)) {
        return None;
    }
    let key = match member {
        oxc_ast::ast::MemberExpression::PrivateFieldExpression(field) => {
            format!("#{}", field.field.name)
        }
        _ => member.static_property_name()?.to_owned(),
    };
    Some((key, member.span(), &assignment.right))
}

fn contains(outer: oxc_span::Span, inner: oxc_span::Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

/// Module-wide references the class check reads.
#[derive(Default)]
struct ModuleWalk {
    eval: bool,
    new_callees: HashSet<ReferenceId>,
    void_operands: HashSet<ReferenceId>,
}

impl<'a> Visit<'a> for ModuleWalk {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.eval |= identifier.name == "eval";
        walk::walk_identifier_reference(self, identifier);
    }

    fn visit_new_expression(&mut self, expression: &NewExpression<'a>) {
        if let Expression::Identifier(identifier) = expression.callee.get_inner_expression()
            && let Some(reference) = identifier.reference_id.get()
        {
            self.new_callees.insert(reference);
        }
        walk::walk_new_expression(self, expression);
    }

    fn visit_unary_expression(&mut self, expression: &UnaryExpression<'a>) {
        if expression.operator == UnaryOperator::Void
            && let Expression::Identifier(identifier) = expression.argument.get_inner_expression()
            && let Some(reference) = identifier.reference_id.get()
        {
            self.void_operands.insert(reference);
        }
        walk::walk_unary_expression(self, expression);
    }
}

#[derive(Clone, Copy)]
enum AssignedValue {
    Function(oxc_span::Span),
    Reference(Option<ReferenceId>),
    Plain,
    Other,
}

struct ThisAssignment {
    target: oxc_span::Span,
    key: Option<String>,
    value: AssignedValue,
}

/// One walk over a class body for everything the class check reads.
struct ClassWalk {
    class: oxc_span::Span,
    exact: bool,
    functions: Vec<oxc_span::Span>,
    returns: Vec<oxc_span::Span>,
    this_sites: Vec<u32>,
    member_this: HashSet<u32>,
    /// Every member access of `this`, with its key (`None`: computed).
    accesses: Vec<(oxc_span::Span, Option<String>)>,
    /// The span of every member access that is the direct callee of a call.
    callees: HashSet<(u32, u32)>,
    /// Every member of `this` in a write position.
    write_targets: Vec<oxc_span::Span>,
    assignments: Vec<ThisAssignment>,
}

impl Default for ClassWalk {
    fn default() -> Self {
        Self {
            class: oxc_span::Span::default(),
            exact: true,
            functions: Vec::new(),
            returns: Vec::new(),
            this_sites: Vec::new(),
            member_this: HashSet::new(),
            accesses: Vec::new(),
            callees: HashSet::new(),
            write_targets: Vec::new(),
            assignments: Vec::new(),
        }
    }
}

fn this_member_key(target: &SimpleAssignmentTarget<'_>) -> Option<Option<String>> {
    let member = target.as_member_expression()?;
    if !matches!(member.object(), Expression::ThisExpression(_)) {
        return None;
    }
    Some(match member {
        oxc_ast::ast::MemberExpression::PrivateFieldExpression(field) => {
            Some(format!("#{}", field.field.name))
        }
        _ => member.static_property_name().map(str::to_owned),
    })
}

impl<'a> Visit<'a> for ClassWalk {
    fn visit_class(&mut self, class: &Class<'a>) {
        if class.span != self.class {
            self.exact = false;
        }
        walk::walk_class(self, class);
    }

    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        self.functions.push(function.span);
        walk::walk_function(self, function, flags);
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.functions.push(arrow.span);
        walk::walk_arrow_function_expression(self, arrow);
    }

    fn visit_return_statement(&mut self, statement: &ReturnStatement<'a>) {
        if statement.argument.is_some() {
            self.returns.push(statement.span);
        }
        walk::walk_return_statement(self, statement);
    }

    fn visit_this_expression(&mut self, this: &ThisExpression) {
        self.this_sites.push(this.span.start);
    }

    fn visit_static_member_expression(&mut self, expression: &StaticMemberExpression<'a>) {
        if let Expression::ThisExpression(this) = &expression.object {
            self.member_this.insert(this.span.start);
            self.accesses
                .push((expression.span, Some(expression.property.name.to_string())));
        }
        walk::walk_static_member_expression(self, expression);
    }

    fn visit_computed_member_expression(&mut self, expression: &ComputedMemberExpression<'a>) {
        if let Expression::ThisExpression(this) = &expression.object {
            self.member_this.insert(this.span.start);
            self.accesses.push((
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
            self.accesses
                .push((expression.span, Some(format!("#{}", expression.field.name))));
        }
        walk::walk_private_field_expression(self, expression);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let Some(member) = call.callee.as_member_expression() {
            self.callees
                .insert((member.span().start, member.span().end));
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        if let Some(target) = assignment.left.as_simple_assignment_target()
            && let Some(key) = this_member_key(target)
        {
            let value = match assignment.right.get_inner_expression() {
                Expression::ArrowFunctionExpression(arrow) => AssignedValue::Function(arrow.span),
                Expression::FunctionExpression(function) => AssignedValue::Function(function.span),
                Expression::Identifier(identifier) => {
                    AssignedValue::Reference(identifier.reference_id.get())
                }
                Expression::StringLiteral(_)
                | Expression::NumericLiteral(_)
                | Expression::BigIntLiteral(_)
                | Expression::BooleanLiteral(_)
                | Expression::NullLiteral(_)
                | Expression::RegExpLiteral(_)
                | Expression::TemplateLiteral(_)
                | Expression::ObjectExpression(_)
                | Expression::ArrayExpression(_)
                | Expression::NewExpression(_) => AssignedValue::Plain,
                Expression::UnaryExpression(unary)
                    if matches!(
                        unary.operator,
                        UnaryOperator::Void | UnaryOperator::Typeof | UnaryOperator::LogicalNot
                    ) =>
                {
                    AssignedValue::Plain
                }
                _ => AssignedValue::Other,
            };
            self.assignments.push(ThisAssignment {
                target: target.span(),
                key,
                value,
            });
        }
        walk::walk_assignment_expression(self, assignment);
    }

    fn visit_simple_assignment_target(&mut self, target: &SimpleAssignmentTarget<'a>) {
        if this_member_key(target).is_some() {
            self.write_targets.push(target.span());
        }
        walk::walk_simple_assignment_target(self, target);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEMBER: &str = "\trun(value) {\n\t\treturn this.callback(value);\n\t}\n";

    fn answer(source: &str) -> Option<Vec<RetainedParameter>> {
        retained_constructor_arguments(Path::new("index.js"), source, "C")
            .map(|found| found.parameters)
    }

    fn class(constructor: &str, members: &str) -> String {
        format!("var C = class {{\n\tconstructor{constructor}\n{members}}};\nexport {{ C }};\n")
    }

    fn kept(key: &str) -> RetainedParameter {
        RetainedParameter::Kept { key: key.into() }
    }

    /// ADR 0139's positive shapes: kept once for a member nothing at
    /// construction reaches, beside an unused parameter, or for an installed
    /// closure; published directly or by an aliased specifier.
    #[test]
    fn a_callable_kept_for_members_is_described() {
        let keeper = class("(callback) {\n\t\tthis.callback = callback;\n\t}", MEMBER);
        assert_eq!(answer(&keeper), Some(vec![kept("callback")]));
        assert_eq!(
            answer(&class(
                "(unused, callback) {\n\t\tthis.callback = callback;\n\t}",
                MEMBER
            )),
            Some(vec![RetainedParameter::Unused, kept("callback")])
        );
        assert_eq!(
            answer(&class(
                "(callback) {\n\t\tthis.callback = callback;\n\t\tthis.fire = () => this.callback(1);\n\t}",
                ""
            )),
            Some(vec![kept("callback")])
        );
        let aliased = keeper.replace("export { C };", "export { C as D };");
        assert_eq!(
            retained_constructor_arguments(Path::new("index.js"), &aliased, "D")
                .map(|found| found.parameters),
            Some(vec![kept("callback")])
        );
        assert_eq!(answer(&aliased), None, "published as D only");
        // `void C` and `new C(…)` read and construct, and hand nothing on.
        let used = format!("{keeper}void C;\nexport function make(f) {{ return new C(f); }}\n");
        assert_eq!(answer(&used), Some(vec![kept("callback")]));
    }

    /// Every row fails on a walk that dropped the premise it names.
    #[test]
    fn anything_inexact_describes_nothing() {
        for (source, why) in [
            (
                class("(callback) {\n\t\tthis.callback = callback;\n\t\tthis.run(0);\n\t}", MEMBER),
                "construction reaches the member that calls the key",
            ),
            (
                class(
                    "(callback) {\n\t\tthis.callback = callback;\n\t\tthis.fire = () => this.callback(1);\n\t\tthis.fire();\n\t}",
                    "",
                ),
                "construction calls the installed closure that calls the key",
            ),
            (
                class(
                    "(callback) {\n\t\tthis.callback = callback;\n\t\t[1].forEach(() => this.callback(0));\n\t}",
                    "",
                ),
                "a closure the constructor hands on may run at construction",
            ),
            (
                class(
                    "(callback) {\n\t\tthis.callback = callback;\n\t}",
                    "\tlater() {\n\t\treturn [1].map(this.callback);\n\t}\n",
                ),
                "a member hands the callable on",
            ),
            (
                class(
                    "(callback) {\n\t\tthis.callback = callback;\n\t}",
                    &format!("\treset(next) {{\n\t\tthis.callback = next;\n\t}}\n{MEMBER}"),
                ),
                "the key is written again",
            ),
            (
                format!(
                    "const registry = [];\n{}",
                    class(
                        "(callback) {\n\t\tthis.callback = callback;\n\t\tregistry.push(this);\n\t}",
                        MEMBER
                    )
                ),
                "the instance escapes",
            ),
            (
                format!(
                    "{}C.prototype.run = function (value) {{ return this.callback(value); }};\n",
                    class("(callback) {\n\t\tthis.callback = callback;\n\t}", "")
                ),
                "the prototype is augmented",
            ),
            (
                format!(
                    "function helper() {{ return [this.callback]; }}\n{}",
                    class(
                        "(callback) {\n\t\tthis.callback = callback;\n\t\tthis.helper = helper;\n\t}",
                        MEMBER
                    )
                ),
                "a function hung on the instance by name",
            ),
            (
                class(
                    "(callback, key) {\n\t\tthis.callback = callback;\n\t\tthis[key] = 1;\n\t}",
                    MEMBER,
                ),
                "a computed member of `this`",
            ),
            (
                class(
                    "(callback) {\n\t\tthis.callback = callback;\n\t\treturn {};\n\t}",
                    MEMBER,
                ),
                "the constructor returns another object",
            ),
            (
                class("(callback = () => 0) {\n\t\tthis.callback = callback;\n\t}", MEMBER),
                "a defaulted parameter",
            ),
            (
                class(
                    "(callback) {\n\t\tif (callback) this.callback = callback;\n\t}",
                    MEMBER,
                ),
                "a conditional store, and a test of the argument",
            ),
            (
                class(
                    "(callback) {\n\t\tcallback(0);\n\t\tthis.callback = callback;\n\t}",
                    MEMBER,
                ),
                "a direct call, whose item's arguments are not derived",
            ),
            (
                format!(
                    "var C = class {{\n\tstatic make(callback) {{ return new C(callback); }}\n\tconstructor(callback) {{\n\t\tthis.callback = callback;\n\t}}\n{MEMBER}}};\nexport {{ C }};\n"
                ),
                "a static member",
            ),
            (
                class("(callback) {\n\t\tthis.other = 1;\n\t}", ""),
                "nothing is kept",
            ),
            (
                "import { Base } from \"dep\";\nvar C = class extends Base {\n\tconstructor(callback) {\n\t\tsuper();\n\t\tthis.callback = callback;\n\t}\n\trun() { return this.callback(); }\n};\nexport { C };\n"
                    .to_owned(),
                "a heritage runs a constructor this walk cannot see",
            ),
            (
                format!(
                    "{}eval(\"C\");\n",
                    class("(callback) {\n\t\tthis.callback = callback;\n\t}", MEMBER)
                ),
                "dynamic lexical access",
            ),
        ] {
            assert_eq!(answer(&source), None, "{why}: {source}");
        }
    }
}
