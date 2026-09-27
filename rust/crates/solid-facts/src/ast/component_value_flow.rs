//! Exact module-local flow of a value into a rendering prop, without package
//! or receipt authority (ADR 0138).
//!
//! `createComponent(Dynamic, { get component() { return Selected(); } })` is
//! how the Solid compiler writes `<Dynamic component={Selected()}/>`, and the
//! value `Dynamic` renders is whatever that prop yields. This module answers
//! the syntax half of "which identifier references can become that value, and
//! go nowhere else": it follows the value backwards through a closed set of
//! transparent shapes and module-local `const` holders, and it reports a
//! reference only when *every* use of every holder on the way is itself such
//! a flow. What the callees are -- whether `createComponent` is the renderer,
//! whether `Dynamic` renders `component`, whether the holder's call is a memo
//! -- is not a syntax question. Every one of those spans is handed back for the
//! caller to resolve, and a caller that cannot resolve one must drop the flow.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::core::Span;
use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_ast::ast::{
    Argument, BindingPattern, CallExpression, Expression, JSXAttributeItem, JSXAttributeName,
    JSXAttributeValue, JSXElementName, LogicalOperator, ObjectExpression, ObjectPropertyKind,
    PropertyKind, Statement, VariableDeclarationKind,
};
use oxc_parser::Parser;
use oxc_semantic::{Scoping, SemanticBuilder};
use oxc_span::{GetSpan, SourceType};
use oxc_syntax::symbol::SymbolId;

/// One identifier reference whose value reaches rendering props and nothing
/// else ([`component_value_flows`]).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComponentValueFlow {
    /// The identifier reference, e.g. `CatchNotFound` in
    /// `createMemo(() => cond() ? CatchNotFound : SafeFragment)`.
    pub reference: Span,
    /// Every rendering prop the value can reach. Never empty.
    pub sites: Vec<ComponentPropSite>,
    /// The call span of every holder call the value passes through
    /// (`createMemo(() => …)` in `const Selected = createMemo(() => …)`). The
    /// flow is exact only if each one's accessor hands its compute's result to
    /// nothing but the accessor's own reads.
    pub holder_calls: Vec<Span>,
}

/// A prop of a rendered element whose value a flow reaches.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComponentPropSite {
    /// The whole render: the call, or the JSX opening element.
    pub site: Span,
    /// The callee of the call form (`createComponent`); `None` for JSX.
    pub renderer: Option<Span>,
    /// The rendered component: the call's first argument, a bare identifier,
    /// or the JSX tag, an identifier reference.
    pub component: Span,
    /// The prop the value is written to.
    pub prop: String,
}

/// Every identifier reference in the module whose value reaches one of
/// `props` on a rendered element, and no other place, or `None`.
///
/// `None` unless the module parses with no error, has no semantic error, and
/// references no `eval`. A rendering prop is exactly one of:
///
/// - `renderer(Component, { prop: value })` or `renderer(Component, { get
///   prop() { return value; } })`: the first argument a bare identifier, the
///   second an object literal, no spread in either position, the key static
///   and not computed, a getter with no parameters and a single `return`;
/// - `<Component prop={value}/>`, the tag an identifier reference.
///
/// The value is followed through parentheses and TypeScript wrappers, both
/// branches of `?:`, both operands of `??` and `||`, the right operand of
/// `&&`, and two holders:
///
/// - `const X = value`, read as the bare identifier `X`;
/// - `const X = call(() => value)`, read only as `X()`: the call has exactly
///   that one argument, a non-async arrow with no parameters and an
///   expression body. Whether `call` is a memo is the caller's question
///   ([`ComponentValueFlow::holder_calls`]).
///
/// A holder counts only when every one of its resolved references is such a
/// read in such a position, none of them writes it, it is not redeclared, and
/// its declaration is not exported; a holder read anywhere else makes every
/// reference flowing into it unreported. Anything else a value passes through
/// -- a parameter, a member read, `let`, a call of anything that is not a
/// holder -- is not followed: the references inside it are not reported, and
/// the caller keeps treating them as escaping values.
#[must_use]
pub fn component_value_flows(
    path: &Path,
    source: &str,
    props: &[&str],
) -> Option<Vec<ComponentValueFlow>> {
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
    let nodes = built.semantic.nodes();
    let scoping = built.semantic.scoping();
    if nodes.iter().any(|node| {
        matches!(node.kind(), AstKind::IdentifierReference(identifier) if identifier.name == "eval")
    }) {
        return None;
    }

    // Holders first: the leaves of a sink depend on which symbols hold.
    let mut holders = HashMap::<SymbolId, Holder<'_, '_>>::new();
    for node in nodes.iter() {
        let AstKind::VariableDeclarator(declarator) = node.kind() else {
            continue;
        };
        if declarator.kind != VariableDeclarationKind::Const {
            continue;
        }
        let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
            continue;
        };
        let (Some(symbol), Some(init)) = (binding.symbol_id.get(), declarator.init.as_ref()) else {
            continue;
        };
        let declaration = nodes.parent_id(node.id());
        let exported = matches!(
            nodes.parent_kind(declaration),
            AstKind::ExportNamedDeclaration(_)
        );
        let holder = match memo_compute(init) {
            Some((call, body)) => Holder {
                value: body,
                call: Some(call),
                exported,
            },
            None => Holder {
                value: init,
                call: None,
                exported,
            },
        };
        holders.insert(symbol, holder);
    }

    // Sinks, and every node's leaves.
    let mut sinks = Vec::new();
    for node in nodes.iter() {
        match node.kind() {
            AstKind::CallExpression(call) => call_sinks(call, props, &mut sinks),
            AstKind::JSXOpeningElement(element) => {
                let JSXElementName::IdentifierReference(tag) = &element.name else {
                    continue;
                };
                for attribute in &element.attributes {
                    let JSXAttributeItem::Attribute(attribute) = attribute else {
                        continue;
                    };
                    let JSXAttributeName::Identifier(name) = &attribute.name else {
                        continue;
                    };
                    let Some(prop) = props.iter().find(|prop| name.name == **prop) else {
                        continue;
                    };
                    let Some(JSXAttributeValue::ExpressionContainer(container)) = &attribute.value
                    else {
                        continue;
                    };
                    let Some(value) = container.expression.as_expression() else {
                        continue;
                    };
                    sinks.push((
                        value,
                        ComponentPropSite {
                            site: span(element.span),
                            renderer: None,
                            component: span(tag.span),
                            prop: (*prop).to_string(),
                        },
                    ));
                }
            }
            _ => {}
        }
    }

    let mut graph = Graph {
        scoping,
        holders: &holders,
        covered: HashMap::new(),
        consumers: HashMap::new(),
        values: Vec::new(),
    };
    for (index, (value, _)) in sinks.iter().enumerate() {
        graph.leaves(value, Node::Sink(index));
    }
    for (symbol, holder) in &holders {
        graph.leaves(holder.value, Node::Holder(*symbol));
    }

    let mut closures = HashMap::new();
    let mut flows = Vec::<ComponentValueFlow>::new();
    let values = std::mem::take(&mut graph.values);
    for (reference, node) in values {
        let Some(reach) = graph.closure(node, &sinks, &mut closures, &mut HashSet::new()) else {
            continue;
        };
        if reach.sites.is_empty() {
            continue;
        }
        let reference = span(reference);
        if let Some(existing) = flows.iter_mut().find(|flow| flow.reference == reference) {
            merge(&mut existing.sites, reach.sites);
            merge(&mut existing.holder_calls, reach.holder_calls);
        } else {
            flows.push(ComponentValueFlow {
                reference,
                sites: reach.sites,
                holder_calls: reach.holder_calls,
            });
        }
    }
    flows.sort_by_key(|flow| (flow.reference.start, flow.reference.end));
    Some(flows)
}

fn span(span: oxc_span::Span) -> Span {
    Span {
        start: span.start,
        end: span.end,
    }
}

fn merge<T: PartialEq>(into: &mut Vec<T>, from: Vec<T>) {
    for item in from {
        if !into.contains(&item) {
            into.push(item);
        }
    }
}

/// `call(() => value)` with exactly that argument: a non-async arrow with no
/// parameters and an expression body. The call's span and the body.
fn memo_compute<'e, 'a>(init: &'e Expression<'a>) -> Option<(Span, &'e Expression<'a>)> {
    let Expression::CallExpression(call) = init.get_inner_expression() else {
        return None;
    };
    let [Argument::ArrowFunctionExpression(arrow)] = call.arguments.as_slice() else {
        return None;
    };
    if arrow.r#async
        || !arrow.expression
        || !arrow.params.items.is_empty()
        || arrow.params.rest.is_some()
    {
        return None;
    }
    let Some(Statement::ExpressionStatement(statement)) = arrow.body.statements.first() else {
        return None;
    };
    Some((span(call.span), &statement.expression))
}

/// The rendering props of a call `renderer(Component, { … })`.
fn call_sinks<'e, 'a>(
    call: &'e CallExpression<'a>,
    props: &[&str],
    sinks: &mut Vec<(&'e Expression<'a>, ComponentPropSite)>,
) {
    let (Some(Argument::Identifier(component)), Some(Argument::ObjectExpression(object))) =
        (call.arguments.first(), call.arguments.get(1))
    else {
        return;
    };
    for (prop, value) in object_prop_values(object, props) {
        sinks.push((
            value,
            ComponentPropSite {
                site: span(call.span),
                renderer: Some(span(call.callee.span())),
                component: span(component.span),
                prop: prop.to_string(),
            },
        ));
    }
}

/// `{ prop: value }` or `{ get prop() { return value; } }`, for each of
/// `props`.
fn object_prop_values<'e, 'a, 'p>(
    object: &'e ObjectExpression<'a>,
    props: &[&'p str],
) -> Vec<(&'p str, &'e Expression<'a>)> {
    let mut values = Vec::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            continue;
        };
        if property.computed {
            continue;
        }
        let Some(name) = property.key.static_name() else {
            continue;
        };
        let Some(prop) = props.iter().find(|prop| name == **prop) else {
            continue;
        };
        match property.kind {
            PropertyKind::Init if !property.method => values.push((*prop, &property.value)),
            PropertyKind::Get => {
                let Expression::FunctionExpression(getter) = &property.value else {
                    continue;
                };
                let Some(body) = getter.body.as_ref() else {
                    continue;
                };
                if !getter.params.items.is_empty() || getter.params.rest.is_some() {
                    continue;
                }
                let [Statement::ReturnStatement(statement)] = body.statements.as_slice() else {
                    continue;
                };
                if let Some(argument) = statement.argument.as_ref() {
                    values.push((*prop, argument));
                }
            }
            _ => {}
        }
    }
    values
}

struct Holder<'e, 'a> {
    /// The value it holds: the initializer, or the memo arrow's body.
    value: &'e Expression<'a>,
    /// The holder call, for a memo holder.
    call: Option<Span>,
    exported: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Node {
    Sink(usize),
    Holder(SymbolId),
}

#[derive(Clone, Default)]
struct Reach {
    sites: Vec<ComponentPropSite>,
    holder_calls: Vec<Span>,
}

struct Graph<'s, 'h, 'e, 'a> {
    scoping: &'s Scoping,
    holders: &'h HashMap<SymbolId, Holder<'e, 'a>>,
    /// The references of each holder read in a followed position.
    covered: HashMap<SymbolId, HashSet<(u32, u32)>>,
    /// The nodes whose value reads each holder.
    consumers: HashMap<SymbolId, Vec<Node>>,
    /// Every non-holder reference in a followed position, and its node.
    values: Vec<(oxc_span::Span, Node)>,
}

impl Graph<'_, '_, '_, '_> {
    fn symbol_of(&self, identifier: &oxc_ast::ast::IdentifierReference<'_>) -> Option<SymbolId> {
        let reference = identifier.reference_id.get()?;
        self.scoping.get_reference(reference).symbol_id()
    }

    fn read(&mut self, holder: SymbolId, reference: oxc_span::Span, node: Node) {
        self.covered
            .entry(holder)
            .or_default()
            .insert((reference.start, reference.end));
        let consumers = self.consumers.entry(holder).or_default();
        if !consumers.contains(&node) {
            consumers.push(node);
        }
    }

    fn leaves(&mut self, value: &Expression<'_>, node: Node) {
        match value.get_inner_expression() {
            Expression::ConditionalExpression(conditional) => {
                self.leaves(&conditional.consequent, node);
                self.leaves(&conditional.alternate, node);
            }
            Expression::LogicalExpression(logical) => {
                if matches!(
                    logical.operator,
                    LogicalOperator::Coalesce | LogicalOperator::Or
                ) {
                    self.leaves(&logical.left, node);
                }
                self.leaves(&logical.right, node);
            }
            Expression::Identifier(identifier) => match self.symbol_of(identifier) {
                Some(symbol)
                    if self
                        .holders
                        .get(&symbol)
                        .is_some_and(|holder| holder.call.is_none()) =>
                {
                    self.read(symbol, identifier.span, node);
                }
                _ => self.values.push((identifier.span, node)),
            },
            Expression::CallExpression(call) if call.arguments.is_empty() => {
                let Expression::Identifier(callee) = call.callee.get_inner_expression() else {
                    return;
                };
                if let Some(symbol) = self.symbol_of(callee)
                    && self
                        .holders
                        .get(&symbol)
                        .is_some_and(|holder| holder.call.is_some())
                {
                    self.read(symbol, callee.span, node);
                }
            }
            _ => {}
        }
    }

    /// Where `node`'s value can go: `None` when anywhere but rendering props.
    fn closure(
        &self,
        node: Node,
        sinks: &[(&Expression<'_>, ComponentPropSite)],
        memo: &mut HashMap<Node, Option<Reach>>,
        active: &mut HashSet<Node>,
    ) -> Option<Reach> {
        if let Some(known) = memo.get(&node) {
            return known.clone();
        }
        let symbol = match node {
            Node::Sink(index) => {
                return Some(Reach {
                    sites: vec![sinks[index].1.clone()],
                    holder_calls: Vec::new(),
                });
            }
            Node::Holder(symbol) => symbol,
        };
        if !active.insert(node) {
            // A holder that feeds itself has a use no sink bounds.
            return None;
        }
        let answer = self.holder_closure(symbol, sinks, memo, active);
        active.remove(&node);
        memo.insert(node, answer.clone());
        answer
    }

    fn holder_closure(
        &self,
        symbol: SymbolId,
        sinks: &[(&Expression<'_>, ComponentPropSite)],
        memo: &mut HashMap<Node, Option<Reach>>,
        active: &mut HashSet<Node>,
    ) -> Option<Reach> {
        let holder = self.holders.get(&symbol)?;
        if holder.exported || !self.scoping.symbol_redeclarations(symbol).is_empty() {
            return None;
        }
        let covered = self.covered.get(&symbol)?;
        let mut references = 0usize;
        for reference in self.scoping.get_resolved_references(symbol) {
            if reference.is_write() {
                return None;
            }
            references += 1;
        }
        // Every reference must be one of the covered reads, and the covered
        // reads are references, so equal counts mean equal sets.
        if references != covered.len() {
            return None;
        }
        let mut reach = Reach::default();
        if let Some(call) = holder.call {
            reach.holder_calls.push(call);
        }
        for consumer in self.consumers.get(&symbol)? {
            let consumed = self.closure(*consumer, sinks, memo, active)?;
            merge(&mut reach.sites, consumed.sites);
            merge(&mut reach.holder_calls, consumed.holder_calls);
        }
        Some(reach)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flows(source: &str) -> Vec<(String, Vec<String>, usize)> {
        component_value_flows(Path::new("m.js"), source, &["component"])
            .expect("the module is analyzable")
            .into_iter()
            .map(|flow| {
                let text = |span: Span| source[span.start as usize..span.end as usize].to_string();
                (
                    text(flow.reference),
                    flow.sites.iter().map(|site| text(site.component)).collect(),
                    flow.holder_calls.len(),
                )
            })
            .collect()
    }

    const HEAD: &str = "import { createComponent, Dynamic } from \"w\";\nimport { createMemo } from \"s\";\nfunction A() {}\nfunction B() {}\n";

    #[test]
    fn the_router_shapes_flow_exactly() {
        let source = format!(
            "{HEAD}function M(p) {{\n  const S = createMemo(() => p.x() ? A : B);\n  const O = createMemo(() => p.y ?? p.z ?? A);\n  return [createComponent(Dynamic, {{ get component() {{ return S(); }} }}), createComponent(Dynamic, {{ get component() {{ return O(); }} }})];\n}}\n"
        );
        assert_eq!(
            flows(&source),
            vec![
                ("A".into(), vec!["Dynamic".into()], 1),
                ("B".into(), vec!["Dynamic".into()], 1),
                ("A".into(), vec!["Dynamic".into()], 1),
            ]
        );
    }

    #[test]
    fn direct_const_and_jsx_values_flow() {
        let source = format!(
            "{HEAD}const C = A;\nexport const x = [createComponent(Dynamic, {{ component: B }}), createComponent(Dynamic, {{ get component() {{ return C; }} }})];\n"
        );
        assert_eq!(
            flows(&source),
            vec![
                ("A".into(), vec!["Dynamic".into()], 0),
                ("B".into(), vec!["Dynamic".into()], 0),
            ]
        );
        let jsx = component_value_flows(
            Path::new("m.jsx"),
            "import { Dynamic } from \"w\";\nfunction A() {}\nexport const x = <Dynamic component={A} />;\n",
            &["component"],
        )
        .unwrap();
        assert_eq!(jsx.len(), 1);
        assert_eq!(jsx[0].sites[0].renderer, None);
    }

    #[test]
    fn a_holder_read_anywhere_else_reports_nothing() {
        for rest in [
            // The memo accessor escapes as a value.
            "const S = createMemo(() => A);\nexport const x = [createComponent(Dynamic, { get component() { return S(); } }), keep(S)];\n",
            // Its result is used elsewhere too.
            "const S = createMemo(() => A);\nexport const x = [createComponent(Dynamic, { get component() { return S(); } }), S()()];\n",
            // A const read in a position no sink bounds.
            "const C = A;\nexport const x = [createComponent(Dynamic, { component: C }), C];\n",
            // An exported holder.
            "export const C = A;\nexport const x = createComponent(Dynamic, { component: C });\n",
            // A compute with a parameter can call its previous value.
            "const S = createMemo((prev) => A);\nexport const x = createComponent(Dynamic, { get component() { return S(); } });\n",
            // A second argument to the holder call.
            "const S = createMemo(() => A, { loadingValue: B });\nexport const x = createComponent(Dynamic, { get component() { return S(); } });\n",
            // `let` is not a holder.
            "let C = A;\nexport const x = createComponent(Dynamic, { component: C });\n",
            // A spread before the props, or a props value that is not a literal.
            "export const x = createComponent(...[Dynamic, { component: A }]);\n",
            "export const x = createComponent(Dynamic, merge({ component: A }));\n",
            // A computed key, or another prop.
            "export const x = createComponent(Dynamic, { [\"component\"]: A, other: B });\n",
        ] {
            let source = format!("{HEAD}{rest}");
            // The holder itself may be reported as a value (`let C`): the
            // caller resolves it to no function. What it holds never is.
            let held = flows(&source)
                .into_iter()
                .filter(|(reference, ..)| reference == "A" || reference == "B")
                .collect::<Vec<_>>();
            assert_eq!(held, vec![], "{rest}");
        }
    }

    #[test]
    fn parameters_and_member_reads_are_not_followed() {
        let source = format!(
            "{HEAD}export function M(p, Comp) {{\n  return [createComponent(Dynamic, {{ component: Comp }}), createComponent(Dynamic, {{ get component() {{ return p.c; }} }})];\n}}\n"
        );
        // `Comp` is reported: it is a reference in a followed position, and
        // the caller resolves it to no function. `p.c` is never a reference.
        assert_eq!(
            flows(&source),
            vec![("Comp".into(), vec!["Dynamic".into()], 0)]
        );
    }

    #[test]
    fn eval_or_a_parse_error_refuses() {
        assert_eq!(
            component_value_flows(Path::new("m.js"), "eval(\"x\");\n", &["component"]),
            None
        );
        assert_eq!(
            component_value_flows(Path::new("m.js"), "const = ;\n", &["component"]),
            None
        );
    }
}
