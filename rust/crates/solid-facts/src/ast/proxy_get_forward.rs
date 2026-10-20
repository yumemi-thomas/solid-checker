//! ADR 0287: whether a `new Proxy(target, handler)` expression reads exactly
//! like its target. The syntax half only: which binding `target` names, and
//! whether that binding is a reactive store, belong to the caller.
//!
//! A proxy whose handler defines no trap but `get` forwards every other
//! operation to its target. When every value `get` returns is
//! `Reflect.get(target, prop[, receiver])` over its own unwritten parameters,
//! a member read of the proxy is a member read of the target, with extra code
//! run first. A throw is a completion that returns nothing, so it is allowed;
//! falling off the end of the trap returns `undefined` and is not.
//!
//! Only the text as analyzed counts. A branch a host-constant fold removed
//! (ADR 0166, for a selected dependency under ADR 0284) is already blank here;
//! any other branch is live, whatever its condition looks like.

use std::collections::HashSet;
use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_ast::ast::{
    Argument, BindingPattern, Expression, FormalParameters, IdentifierReference,
    ObjectPropertyKind, PropertyKey, PropertyKind, ReturnStatement, Statement,
};
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_semantic::{Scoping, SemanticBuilder, SymbolId};
use oxc_span::{GetSpan, SourceType};
use oxc_syntax::scope::ScopeFlags;

use crate::core::Span;

const MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;

/// The span of the target argument of the `new Proxy(target, handler)`
/// expression at exactly `construct`, when the handler forwards every read to
/// it. `None` refuses. The expression must be:
///
/// - `new Proxy(identifier, { get … })`, where `Proxy` is the unbound global,
///   with exactly those two arguments and no spread;
/// - a handler written in place as an object literal holding one property: a
///   non-computed `get` (identifier or string key) whose value is a
///   non-async, non-generator function or arrow written in place;
/// - a trap with two or three plain parameters, no default, no rest, none
///   written or redeclared;
/// - a trap that names its first parameter only as the first argument of a
///   returned forward, and holds no `this`, `super`, `arguments` or `eval`;
/// - a trap every normal completion of which returns
///   `Reflect.get(target, prop)` or `Reflect.get(target, prop, receiver)`
///   over those parameters, `Reflect` being the unbound global. Any other
///   returned value, a fallthrough, or control flow the completion walk does
///   not model (`try`, loops, `switch`, labels) refuses.
#[must_use]
pub fn proxy_get_forward_target(path: &Path, source: &str, construct: Span) -> Option<Span> {
    if source.len() > MAX_SOURCE_BYTES {
        return None;
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(
        &allocator,
        source,
        SourceType::from_path(path).ok()?.with_module(true),
    )
    .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return None;
    }
    let built = SemanticBuilder::new().build(&parsed.program);
    if !built.errors.is_empty() {
        return None;
    }
    let scoping = built.semantic.scoping();
    let new = built
        .semantic
        .nodes()
        .iter()
        .find_map(|node| match node.kind() {
            AstKind::NewExpression(new)
                if new.span.start == construct.start && new.span.end == construct.end =>
            {
                Some(new)
            }
            _ => None,
        })?;
    let Expression::Identifier(callee) = super::peel_ts_sugar(&new.callee) else {
        return None;
    };
    if callee.name != "Proxy" || !unbound(scoping, callee) {
        return None;
    }
    let [target, handler] = new.arguments.as_slice() else {
        return None;
    };
    let (Some(target), Some(handler)) = (argument(target), argument(handler)) else {
        return None;
    };
    let Expression::Identifier(target) = super::peel_ts_sugar(target) else {
        return None;
    };
    let Expression::ObjectExpression(handler) = super::peel_ts_sugar(handler) else {
        return None;
    };
    let [ObjectPropertyKind::ObjectProperty(trap)] = handler.properties.as_slice() else {
        return None;
    };
    let named_get = match &trap.key {
        PropertyKey::StaticIdentifier(key) => key.name == "get",
        PropertyKey::StringLiteral(key) => key.value == "get",
        _ => false,
    };
    if trap.kind != PropertyKind::Init || trap.computed || trap.shorthand || !named_get {
        return None;
    }
    let (params, body, concise) = match super::peel_ts_sugar(&trap.value) {
        Expression::FunctionExpression(function) if !function.r#async && !function.generator => {
            (&*function.params, function.body.as_deref()?, false)
        }
        Expression::ArrowFunctionExpression(arrow) if !arrow.r#async => {
            (&*arrow.params, &*arrow.body, arrow.expression)
        }
        _ => return None,
    };
    let parameters = Parameters::of(params, scoping)?;
    // The whole trap: references, `this`, `arguments` and `eval`, nested
    // functions included, since a closure can capture the target too.
    let mut names = TrapNames {
        scoping,
        target: parameters.target,
        target_references: Vec::new(),
        refused: false,
    };
    names.visit_function_body(body);
    if names.refused {
        return None;
    }
    let forwards = if concise {
        let [Statement::ExpressionStatement(statement)] = body.statements.as_slice() else {
            return None;
        };
        let forward = forward_call(&statement.expression, scoping, &parameters)?;
        vec![forward]
    } else {
        let mut returns = TrapReturns {
            scoping,
            parameters: &parameters,
            forwards: Vec::new(),
        };
        returns.visit_function_body(body);
        let forwards = returns.forwards;
        let candidates = forwards
            .iter()
            .map(|forward| (forward.call.start, forward.call.end))
            .collect::<HashSet<_>>();
        if super::completion_call_cover::statements_return_cover(&body.statements, candidates)
            != Some(true)
        {
            return None;
        }
        forwards
    };
    // The target parameter is read only as the first argument of a forward
    // every completion returns.
    let allowed = forwards
        .iter()
        .map(|forward| (forward.target.start, forward.target.end))
        .collect::<HashSet<_>>();
    if !names
        .target_references
        .iter()
        .all(|reference| allowed.contains(reference))
    {
        return None;
    }
    Some(Span::new(target.span.start, target.span.end))
}

fn argument<'a>(argument: &'a Argument<'a>) -> Option<&'a Expression<'a>> {
    (!matches!(argument, Argument::SpreadElement(_))).then(|| argument.to_expression())
}

fn unbound(scoping: &Scoping, reference: &IdentifierReference<'_>) -> bool {
    reference
        .reference_id
        .get()
        .is_some_and(|id| scoping.get_reference(id).symbol_id().is_none())
}

fn resolves_to(scoping: &Scoping, expression: &Expression<'_>, symbol: SymbolId) -> bool {
    matches!(super::peel_ts_sugar(expression), Expression::Identifier(reference)
        if reference
            .reference_id
            .get()
            .and_then(|id| scoping.get_reference(id).symbol_id())
            == Some(symbol))
}

struct Parameters {
    target: SymbolId,
    prop: SymbolId,
    receiver: Option<SymbolId>,
}

impl Parameters {
    fn of(params: &FormalParameters<'_>, scoping: &Scoping) -> Option<Self> {
        if params.rest.is_some() || !(2..=3).contains(&params.items.len()) {
            return None;
        }
        let mut symbols = Vec::new();
        for item in &params.items {
            let BindingPattern::BindingIdentifier(identifier) = &item.pattern else {
                return None;
            };
            if item.initializer.is_some() {
                return None;
            }
            let symbol = identifier.symbol_id.get()?;
            if !scoping.symbol_redeclarations(symbol).is_empty()
                || scoping
                    .get_resolved_references(symbol)
                    .any(|reference| reference.is_write())
            {
                return None;
            }
            symbols.push(symbol);
        }
        Some(Self {
            target: symbols[0],
            prop: symbols[1],
            receiver: symbols.get(2).copied(),
        })
    }
}

struct Forward {
    call: Span,
    target: Span,
}

/// `Reflect.get(target, prop)` or `Reflect.get(target, prop, receiver)`,
/// exactly, over the trap's own parameters.
fn forward_call(
    expression: &Expression<'_>,
    scoping: &Scoping,
    parameters: &Parameters,
) -> Option<Forward> {
    let Expression::CallExpression(call) = super::peel_ts_sugar(expression) else {
        return None;
    };
    if call.optional {
        return None;
    }
    let Expression::StaticMemberExpression(member) = super::peel_ts_sugar(&call.callee) else {
        return None;
    };
    let Expression::Identifier(reflect) = super::peel_ts_sugar(&member.object) else {
        return None;
    };
    if member.optional
        || member.property.name != "get"
        || reflect.name != "Reflect"
        || !unbound(scoping, reflect)
    {
        return None;
    }
    let arguments = call
        .arguments
        .iter()
        .map(argument)
        .collect::<Option<Vec<_>>>()?;
    let (target, prop, receiver) = match arguments.as_slice() {
        [target, prop] => (*target, *prop, None),
        [target, prop, receiver] => (*target, *prop, Some(*receiver)),
        _ => return None,
    };
    let receiver_matches = match (receiver, parameters.receiver) {
        (None, _) => true,
        (Some(receiver), Some(symbol)) => resolves_to(scoping, receiver, symbol),
        (Some(_), None) => false,
    };
    if !resolves_to(scoping, target, parameters.target)
        || !resolves_to(scoping, prop, parameters.prop)
        || !receiver_matches
    {
        return None;
    }
    let target = super::peel_ts_sugar(target).span();
    Some(Forward {
        call: Span::new(call.span.start, call.span.end),
        target: Span::new(target.start, target.end),
    })
}

/// Every reference in the trap that names its target parameter, and whether
/// anything in it can reach the handler (`this`, `super`) or rewrite a
/// parameter behind the scoping's back (`arguments`, a direct `eval`).
struct TrapNames<'s> {
    scoping: &'s Scoping,
    target: SymbolId,
    target_references: Vec<(u32, u32)>,
    refused: bool,
}

impl<'a> Visit<'a> for TrapNames<'_> {
    fn visit_identifier_reference(&mut self, reference: &IdentifierReference<'a>) {
        if reference.name == "arguments" || reference.name == "eval" {
            self.refused = true;
        }
        if reference
            .reference_id
            .get()
            .and_then(|id| self.scoping.get_reference(id).symbol_id())
            == Some(self.target)
        {
            self.target_references
                .push((reference.span.start, reference.span.end));
        }
    }
    fn visit_this_expression(&mut self, _: &oxc_ast::ast::ThisExpression) {
        self.refused = true;
    }
    fn visit_super(&mut self, _: &oxc_ast::ast::Super) {
        self.refused = true;
    }
}

/// The forwards the trap itself returns, outside every nested function. A
/// return of anything else is not a candidate, so the completion walk
/// refuses it.
struct TrapReturns<'s> {
    scoping: &'s Scoping,
    parameters: &'s Parameters,
    forwards: Vec<Forward>,
}

impl<'a> Visit<'a> for TrapReturns<'_> {
    fn visit_return_statement(&mut self, statement: &ReturnStatement<'a>) {
        if let Some(argument) = &statement.argument
            && let Some(forward) = forward_call(argument, self.scoping, self.parameters)
        {
            self.forwards.push(forward);
        }
    }
    fn visit_function(&mut self, _: &oxc_ast::ast::Function<'a>, _: ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &oxc_ast::ast::ArrowFunctionExpression<'a>) {}
    fn visit_class(&mut self, _: &oxc_ast::ast::Class<'a>) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The answer for the one `new Proxy(` in `body`, as the span text of
    /// the target it names.
    fn target(body: &str) -> Option<String> {
        let source = format!("declare const sink: any, flag: boolean, other: any;\n{body}\n");
        let start = source.find("new Proxy(").expect("a proxy");
        // The construct ends at the `)` closing `new Proxy(`.
        let mut depth = 0_i32;
        let mut end = None;
        for (offset, byte) in source.as_bytes()[start..].iter().enumerate() {
            match byte {
                b'(' | b'{' | b'[' => depth += 1,
                b')' | b'}' | b']' => {
                    depth -= 1;
                    if depth == 0 && *byte == b')' {
                        end = Some(start + offset + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        let span = Span::new(start as u32, end.expect("closed") as u32);
        proxy_get_forward_target(Path::new("hook.ts"), &source, span)
            .map(|span| source[span.start as usize..span.end as usize].to_owned())
    }

    #[test]
    fn a_get_only_handler_returning_the_forward_reads_its_target() {
        for body in [
            // The `@tanstack/solid-query` 6.0.0-rc.0 shape, server branch folded.
            "function f(state: any, errors: Set<string>) { return new Proxy(state, {\n\
               get(target, prop, receiver) {\n\
                 if (typeof prop === \"symbol\") { return Reflect.get(target, prop, receiver); }\n\
                 if (!1) ;\n\
                 if (errors.has(prop)) { return Reflect.get(target, prop, receiver); }\n\
                 if (state.isError && !state.isFetching) { throw state.error; }\n\
                 return Reflect.get(target, prop, receiver);\n\
               }\n\
             }); }",
            "function f(state: any) { return new Proxy(state, { get(t, p) { return Reflect.get(t, p); } }); }",
            "function f(state: any) { return new Proxy(state, { get: (t, p, r) => Reflect.get(t, p, r) }); }",
            "function f(state: any) { return new Proxy(state, { \"get\": function (t, p, r) { sink(p); return Reflect.get(t, p); } }); }",
            "function f(state: any) { return new Proxy(state, { get(t, p, r) { if (flag) throw 0; else return (Reflect.get(t, p, r)); } }); }",
        ] {
            assert_eq!(target(body).as_deref(), Some("state"), "{body}");
        }
    }

    #[test]
    fn any_other_trap_return_or_handler_shape_refuses() {
        for body in [
            // Other traps, or an extra one.
            "function f(s: any) { return new Proxy(s, { set() { return true; } }); }",
            "function f(s: any) { return new Proxy(s, { get(t, p) { return Reflect.get(t, p); }, set() { return true; } }); }",
            "function f(s: any) { return new Proxy(s, { get(t, p) { return Reflect.get(t, p); }, has() { return true; } }); }",
            "function f(s: any) { return new Proxy(s, { get(t, p) { return Reflect.get(t, p); }, ownKeys() { return []; } }); }",
            // Spread, computed, accessor, shorthand or out-of-place handlers.
            "function f(s: any, h: any) { return new Proxy(s, { ...h }); }",
            "function f(s: any, h: any) { return new Proxy(s, { get(t, p) { return Reflect.get(t, p); }, ...h }); }",
            "function f(s: any) { return new Proxy(s, { [\"get\"](t: any, p: any) { return Reflect.get(t, p); } }); }",
            "function f(s: any) { return new Proxy(s, { get get() { return () => 0; } }); }",
            "function f(s: any, get: any) { return new Proxy(s, { get }); }",
            "function f(s: any, h: any) { return new Proxy(s, h); }",
            // A returned value other than the forward.
            "function f(s: any) { return new Proxy(s, { get(t, p) { if (flag) return Reflect.get(other, p); return Reflect.get(t, p); } }); }",
            "function f(s: any) { return new Proxy(s, { get(t, p) { if (typeof window === \"undefined\") { return Reflect.get(other, p); } return Reflect.get(t, p); } }); }",
            "function f(s: any) { return new Proxy(s, { get(t, p) { return Reflect.get(t, \"x\"); } }); }",
            "function f(s: any) { return new Proxy(s, { get(t, p, r) { return Reflect.get(t, p, other); } }); }",
            "function f(s: any) { return new Proxy(s, { get(t, p) { return Reflect.get(t, p, other); } }); }",
            "function f(s: any) { return new Proxy(s, { get(t, p) { return t[p]; } }); }",
            "function f(s: any) { return new Proxy(s, { get(t, p) { return Reflect.get(t, p)?.x; } }); }",
            "function f(s: any) { return new Proxy(s, { get(t, p) { return Reflect?.get(t, p); } }); }",
            "function f(s: any) { return new Proxy(s, { get(t, p) { if (flag) return; return Reflect.get(t, p); } }); }",
            // Fallthrough, throw-only, and unmodeled flow.
            "function f(s: any) { return new Proxy(s, { get(t, p) { if (flag) return Reflect.get(t, p); } }); }",
            "function f(s: any) { return new Proxy(s, { get(t, p) { throw 0; } }); }",
            "function f(s: any) { return new Proxy(s, { get(t, p) { try { return Reflect.get(t, p); } finally { sink(0); } } }); }",
            "function f(s: any) { return new Proxy(s, { get(t, p) { for (;;) return Reflect.get(t, p); } }); }",
            // Written, redeclared, defaulted or escaping parameters.
            "function f(s: any) { return new Proxy(s, { get(t, p) { t = other; return Reflect.get(t, p); } }); }",
            "function f(s: any) { return new Proxy(s, { get(t, p) { p = \"x\"; return Reflect.get(t, p); } }); }",
            "function f(s: any) { return new Proxy(s, { get(t, p, r) { r = other; return Reflect.get(t, p, r); } }); }",
            "function f(s: any) { return new Proxy(s, { get(t = other, p) { return Reflect.get(t, p); } }); }",
            "function f(s: any) { return new Proxy(s, { get(t, p) { sink(t); return Reflect.get(t, p); } }); }",
            "function f(s: any) { return new Proxy(s, { get(t, p) { const read = () => t; return Reflect.get(t, p); } }); }",
            "function f(s: any) { return new Proxy(s, { get(t, p) { sink(t.x); return Reflect.get(t, p); } }); }",
            "function f(s: any) { return new Proxy(s, { get(...args: any[]) { return Reflect.get(args[0], args[1]); } }); }",
            "function f(s: any) { return new Proxy(s, { get(t) { return Reflect.get(t, \"x\"); } }); }",
            // The handler, or the parameters, reachable another way.
            "function f(s: any) { return new Proxy(s, { get(t, p) { (this as any).get = sink; return Reflect.get(t, p); } }); }",
            "function f(s: any) { return new Proxy(s, { get(t, p) { arguments[1] = \"x\"; return Reflect.get(t, p); } }); }",
            "function f(s: any) { return new Proxy(s, { get(t, p) { eval(\"p = 1\"); return Reflect.get(t, p); } }); }",
            "function f(s: any) { return new Proxy(s, { async get(t: any, p: any) { return Reflect.get(t, p); } }); }",
            "function f(s: any) { return new Proxy(s, { *get(t: any, p: any) { return Reflect.get(t, p); } }); }",
            // Not the global `Proxy` or `Reflect`, or not a plain target.
            "function f(s: any, Proxy: any) { return new Proxy(s, { get(t: any, p: any) { return Reflect.get(t, p); } }); }",
            "function f(s: any, Reflect: any) { return new Proxy(s, { get(t: any, p: any) { return Reflect.get(t, p); } }); }",
            "function f(s: any) { return new Proxy(s.items, { get(t, p) { return Reflect.get(t, p); } }); }",
            "function f(s: any) { return new Proxy(s, { get(t, p) { return Reflect.get(t, p); } }, other); }",
            "function f(s: any[]) { return new Proxy(...s); }",
        ] {
            assert_eq!(target(body), None, "{body}");
        }
    }
}
