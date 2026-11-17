//! ADR 1024: an object a member write cannot reach a standard root through.
//!
//! ADR 0939's open-write census ([`super::impostor_sites`]) counts a write whose object is not made where it is written as
//! one that may put a member on a standard root (`patch(Number, "isFinite", f)` writing its parameter). [`super::fresh_object`]
//! knew one made object: a literal, or a `const` or never-written `let` bound to one. This module answers the rest of the
//! writes that only ever reach an object the program made, in two forms:
//!
//! - **Made** (the binding form): every value the object can be is a literal, `null` or `undefined` by syntax alone: a
//!   `const` bound to such a value, a `let` every assignment of which is one, `a || b`, `a ?? b`, `a && b` and `c ? a : b`
//!   of such values, and a static member path `o.p` of a `const` closed literal whose own data property `p` holds one,
//!   where nothing but this function's straight code can reach `o` before it returns it.
//! - **Factory**: the value is also one a standard function made or handed back unchanged: `Object.create(null)`,
//!   `JSON.parse(text)` (no reviver), the target `Object.assign` returns, and the accumulator of an inline `reduce`
//!   callback over an array the standard library made, whose seed and every return are such values.
//!
//! The factory form trusts the standard functions of [`FRESH_FACTORIES`]. The premise that makes that trust sound
//! (the first-patch induction, ADR 1024): take the first moment any write of the factory form puts a member on a
//! standard root. Before it no such write did, so every factory it relies on could have been replaced only by a site the
//! rest of the census sees (a patch, an override, a definer, an open write outside this form). Where the program holds
//! none that may reach a factory of the list, each factory call before that moment ran the standard code, so the object
//! written was the made one: no such first moment exists. The program side ([`fresh_factories_intact`]) asks exactly
//! that, with every site of this form left out of the open writes it reads; any doubt keeps every factory site an open
//! write.

use solid_facts::FileFacts;
use solid_facts::ast::{ArgumentLiteralFact, BindingShape, FunctionKind, MemberFact};
use solid_facts::core::Span;

/// How a written object is proven made.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Fresh {
    /// By syntax alone.
    Made,
    /// Also through a standard function of [`FRESH_FACTORIES`].
    Factory,
}

/// The standard functions the factory form trusts, by the qualified names a patch is keyed by (ADR 0781's
/// [`super::patch_vetoed`]).
pub(crate) const FRESH_FACTORIES: [&str; 8] = [
    "ObjectConstructor.create",
    "ObjectConstructor.assign",
    "ObjectConstructor.keys",
    "ObjectConstructor.values",
    "ObjectConstructor.entries",
    "JSON.parse",
    "Array.reduce",
    "Array.sort",
];

/// ADR 1024: whether the object a write at `span` writes is one the program made ([`Fresh`]), `None` when no fact
/// proves it.
pub(crate) fn fresh_written_object(file: &FileFacts, span: Span) -> Option<Fresh> {
    Walk {
        file,
        assumed: Vec::new(),
        site: span,
    }
    .value(span, 0)
}

/// Whether the program leaves every factory of [`FRESH_FACTORIES`] standard, `hazard` answering the census with every
/// factory-form site left out of its open writes (the induction's hypothesis).
pub(crate) fn fresh_factories_intact(hazard: &impl Fn(super::Hazard) -> bool) -> bool {
    FRESH_FACTORIES
        .iter()
        .all(|name| !super::patch_vetoed(name, hazard))
}

const DEPTH: u8 = 8;

struct Walk<'f> {
    file: &'f FileFacts,
    /// The `let` bindings and accumulators whose previous value the induction over their writes assumes made.
    assumed: Vec<Span>,
    /// The written object, where a member path's straight-code test is asked.
    site: Span,
}

fn both(left: Option<Fresh>, right: Option<Fresh>) -> Option<Fresh> {
    Some(left?.max(right?))
}

impl Walk<'_> {
    fn value(&mut self, span: Span, depth: u8) -> Option<Fresh> {
        if depth > DEPTH {
            return None;
        }
        let file = self.file;
        let ast = &file.ast;
        let value = ast.peel_ts_sugar_span(span);
        if literal(file, value) || function_literal(file, value) {
            return Some(Fresh::Made);
        }
        if ast.reference_declaration(value).is_none()
            && ast.member_at(value).is_none()
            && ast.call_at(value).is_none()
            && matches!(
                file.source_text(value).map(str::trim),
                Some("null" | "undefined")
            )
        {
            return Some(Fresh::Made);
        }
        if let Some(logical) = ast
            .logical_expressions
            .iter()
            .find(|logical| logical.span == value)
        {
            let (left, right) = (logical.left, logical.right);
            let left = self.value(left, depth + 1);
            return both(left, self.value(right, depth + 1));
        }
        if let Some(conditional) = ast
            .conditional_expressions
            .iter()
            .find(|conditional| conditional.span == value)
        {
            let (consequent, alternate) = (conditional.consequent, conditional.alternate);
            let consequent = self.value(consequent, depth + 1);
            return both(consequent, self.value(alternate, depth + 1));
        }
        if let Some(declaration) = ast.reference_declaration(value) {
            return self.binding(declaration, depth + 1);
        }
        if let Some(member) = ast.member_at(value) {
            return self.member_path(member, depth + 1);
        }
        if ast.call_at(value).is_some() {
            return self.factory_call(value, depth + 1);
        }
        None
    }

    /// A `const` bound to a made value; a `let` whose initial value and every assigned value are made, each assuming
    /// the binding's previous value made (an induction over the order of its writes); an accumulator of a `reduce`
    /// callback.
    fn binding(&mut self, declaration: Span, depth: u8) -> Option<Fresh> {
        if self.assumed.contains(&declaration) {
            return Some(Fresh::Made);
        }
        let file = self.file;
        let ast = &file.ast;
        // ADR 1041: a function declaration's name, never rebound, holds the function object its declaration made.
        if declared_function(file, declaration) {
            return Some(Fresh::Made);
        }
        let Some(binding) = ast
            .identifier_binding(declaration)
            .filter(|binding| binding.immutable || binding.runtime_let)
        else {
            return self.accumulator(declaration, depth);
        };
        if binding.immutable {
            return self.value(binding.initializer?, depth);
        }
        no_eval(file)?;
        self.assumed.push(declaration);
        let mut answer = match binding.initializer {
            Some(initializer) => self.value(initializer, depth),
            None => Some(Fresh::Made),
        };
        for reference in ast.references_to(declaration) {
            if answer.is_none() {
                break;
            }
            if *reference == declaration
                || !ast.any_write_target_containing(*reference)
                || ast.any_member_with_object(*reference)
            {
                continue;
            }
            // Only a plain or logical `=` whose target is the name itself: a pattern, a `for … of` head, an update or a
            // compound operator writes what no assignment value shows.
            let assigned = ast.assignments.iter().find(|assignment| {
                ast.peel_ts_sugar_span(assignment.target) == *reference
                    && (!assignment.reads_target || assignment.logical.is_some())
            });
            answer = match assigned {
                Some(assignment) => {
                    let value = assignment.value_span;
                    both(answer, self.value(value, depth))
                }
                None => None,
            };
        }
        self.assumed.pop();
        answer
    }

    /// The first parameter of an inline callback, the first argument of `R.reduce(callback, seed)` over an array the
    /// standard library made: the seed, then whatever the previous call returned. Never written, no default (an
    /// `undefined` return would hand on the default), and an arrow or a function that never names `arguments` (a
    /// sloppy function's `arguments[0]` is the parameter).
    fn accumulator(&mut self, declaration: Span, depth: u8) -> Option<Fresh> {
        let file = self.file;
        let ast = &file.ast;
        let function = ast.functions.iter().find(|function| {
            function.parameters.first().is_some_and(|parameter| {
                parameter.shape == BindingShape::Identifier
                    && parameter.initializer.is_none()
                    && parameter.names.first().map(|name| name.span) == Some(declaration)
            })
        })?;
        if function.r#async
            || function.generator
            || (function.kind != FunctionKind::Arrow
                && file
                    .source_text(function.body)
                    .is_none_or(|body| body.contains("arguments")))
            || crate::indexes::binding_written(file, declaration)
        {
            return None;
        }
        let call = ast.calls.iter().find(|call| {
            !call.construct
                && call.arguments.len() == 2
                && call.arguments.iter().all(|argument| !argument.spread)
                && ast.peel_ts_sugar_span(call.arguments[0].span) == function.span
        })?;
        let link = static_member(file, ast.peel_ts_sugar_span(call.callee))?;
        if file.source_text(link.property) != Some("reduce")
            || self.standard_array(link.object, depth).is_none()
        {
            return None;
        }
        self.assumed.push(declaration);
        let mut answer = self.value(call.arguments[1].span, depth);
        if function.expression_body {
            answer = match function
                .expression_return
                .as_ref()
                .and_then(|ret| ret.argument)
            {
                Some(returned) => both(answer, self.value(returned, depth)),
                None => None,
            };
        } else {
            for ret in ast
                .returns
                .iter()
                .filter(|ret| function.body.contains(ret.span))
            {
                if answer.is_none() {
                    break;
                }
                // A return of a function written inside the callback is that function's; one whose function no fact names keeps it open.
                match ast.innermost_function_containing(ret.span) {
                    Some(inner) if inner.span == function.span => {}
                    Some(_) => continue,
                    None => {
                        answer = None;
                        break;
                    }
                }
                answer = match ret.argument {
                    Some(returned) => both(answer, self.value(returned, depth)),
                    None => answer,
                };
            }
        }
        self.assumed.pop();
        answer.map(|_| Fresh::Factory)
    }

    /// An array the standard library made: a literal, `Object.keys`, `values` or `entries` of anything, or such an
    /// array's `sort`, which hands back its receiver.
    fn standard_array(&mut self, span: Span, depth: u8) -> Option<Fresh> {
        if depth > DEPTH {
            return None;
        }
        let file = self.file;
        let ast = &file.ast;
        let value = ast.peel_ts_sugar_span(span);
        if ast.array_literals.binary_search(&value).is_ok() {
            return Some(Fresh::Made);
        }
        let call = ast.call_at(value).filter(|call| !call.construct)?;
        let callee = ast.peel_ts_sugar_span(call.callee);
        if ["Object.keys", "Object.values", "Object.entries"]
            .iter()
            .any(|name| global_member_call(file, callee, name))
        {
            return Some(Fresh::Factory);
        }
        let link = static_member(file, callee)?;
        (file.source_text(link.property) == Some("sort")
            && call.arguments.len() <= 1
            && call.arguments.iter().all(|argument| !argument.spread))
        .then_some(())?;
        self.standard_array(link.object, depth + 1)
            .map(|_| Fresh::Factory)
    }

    /// `Object.create(null)`, `JSON.parse(text)` and `Object.assign(target, …)` of a made target.
    fn factory_call(&mut self, value: Span, depth: u8) -> Option<Fresh> {
        let file = self.file;
        let ast = &file.ast;
        let call = ast.call_at(value).filter(|call| !call.construct)?;
        let callee = ast.peel_ts_sugar_span(call.callee);
        let argument = |index: usize| {
            call.arguments
                .get(index)
                .filter(|argument| !argument.spread)
        };
        if global_member_call(file, callee, "Object.create") {
            return (call.arguments.len() == 1
                && argument(0)
                    .is_some_and(|argument| argument.literal_value == ArgumentLiteralFact::Null))
            .then_some(Fresh::Factory);
        }
        if global_member_call(file, callee, "JSON.parse") {
            return (call.arguments.len() == 1 && argument(0).is_some()).then_some(Fresh::Factory);
        }
        if global_member_call(file, callee, "Object.assign") {
            let target = argument(0)?.span;
            return self.value(target, depth).map(|_| Fresh::Factory);
        }
        None
    }

    /// `o.p`: `o` a `const` bound to a closed object literal (no spread, getter, setter, computed or duplicate key)
    /// whose own data property `p` holds a made value, every other reference to `o` a static, uncalled member access that
    /// writes no `p` (nor deletes it), or `return o` of the function that declares it, and the written object in that
    /// same function's straight code (no `finally`, which runs after the `return`): nothing but this activation's own
    /// code holds `o` until it returns it, and none of that code rebinds `p`.
    fn member_path(&mut self, member: &MemberFact, depth: u8) -> Option<Fresh> {
        let file = self.file;
        let ast = &file.ast;
        // Asked of the written object itself only: the straight-code test below is the write's.
        if member.span != ast.peel_ts_sugar_span(self.site) {
            return None;
        }
        let member = static_member(file, member.span)?;
        let name = file.source_text(member.property)?;
        let object = ast.peel_ts_sugar_span(member.object);
        let declaration = ast.reference_declaration(object)?;
        let binding = ast
            .identifier_binding(declaration)
            .filter(|binding| binding.immutable)?;
        let literal = ast.peel_ts_sugar_span(binding.initializer?);
        let property = own_data_property(file, literal, name)?;
        no_eval(file)?;
        let scope = |span: Span| {
            ast.innermost_function_containing(span)
                .map(|function| function.span)
        };
        let declared_in = scope(declaration);
        if scope(self.site) != declared_in
            || file
                .source_text(declared_in.unwrap_or(Span::new(0, u32::MAX)))
                .is_none_or(|text| text.contains("finally"))
        {
            return None;
        }
        let reached_only_here = ast.references_to(declaration).iter().all(|reference| {
            if *reference == declaration {
                return true;
            }
            if let Some(used) = static_member(
                file,
                ast.member_with_peeled_object(*reference)
                    .map(|m| m.span)
                    .unwrap_or(*reference),
            )
            .filter(|used| ast.peel_ts_sugar_span(used.object) == *reference)
            {
                let rebinds = file.source_text(used.property) == Some(name)
                    && (written(file, used.span) || ast.deleted_targets.contains(&used.span));
                return !rebinds && !ast.any_call_with_peeled_callee(used.span);
            }
            ast.returns.iter().any(|ret| {
                ret.argument
                    .map(|argument| ast.peel_ts_sugar_span(argument))
                    == Some(*reference)
                    && scope(ret.span) == declared_in
            })
        });
        if !reached_only_here {
            return None;
        }
        self.value(property, depth)
    }
}

/// Exactly an array or object literal, as [`super::fresh_object`] reads one.
fn literal(file: &FileFacts, value: Span) -> bool {
    let ast = &file.ast;
    ast.array_literals.binary_search(&value).is_ok()
        || (ast.member_at(value).is_none()
            && ast.call_at(value).is_none()
            && file
                .source_text(value)
                .map(str::trim)
                .is_some_and(|text| text.starts_with('{') && text.ends_with('}')))
}

/// ADR 1041: exactly a function or arrow expression (not a method, an accessor or a declaration): each evaluation makes a
/// new function object, whose own members a write puts on it and on no standard root.
fn function_literal(file: &FileFacts, value: Span) -> bool {
    file.ast.functions.iter().any(|function| {
        function.span == value
            && function.method_name.is_none()
            && matches!(
                function.kind,
                FunctionKind::Expression | FunctionKind::Arrow
            )
    })
}

/// ADR 1041: whether `declaration` is the name of a function declaration of this file that nothing rebinds: no
/// assignment or other write names it, no other declaration of the file binds the same name in its place (a second
/// `function` or a `var` of that name would replace it), and no direct `eval` can reach it. Its binding then holds the
/// function object the declaration made, wherever it is read (hoisting makes it so before the declaration too).
fn declared_function(file: &FileFacts, declaration: Span) -> bool {
    let ast = &file.ast;
    let Some(function) = ast.functions.iter().find(|function| {
        function.kind == FunctionKind::Declaration
            && function
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration)
    }) else {
        return false;
    };
    let Some(name) = function
        .name
        .as_ref()
        .and_then(|name| file.source_text(name.span))
    else {
        return false;
    };
    let rebound_by_name = ast.functions.iter().any(|other| {
        other.span != function.span
            && other.kind == FunctionKind::Declaration
            && other
                .name
                .as_ref()
                .is_some_and(|other| file.source_text(other.span) == Some(name))
    }) || ast.bindings.iter().any(|binding| {
        binding
            .names
            .iter()
            .any(|bound| file.source_text(bound.span) == Some(name))
    });
    !rebound_by_name
        && !crate::indexes::binding_written(file, declaration)
        && no_eval(file).is_some()
}

/// `Some(())` when no direct `eval` in the file can reach a binding by name.
fn no_eval(file: &FileFacts) -> Option<()> {
    (!crate::interproc::property_gets::direct_eval_in(file, Span::new(0, u32::MAX))).then_some(())
}

/// The member at `span` when its key is static (`o.p`, not `o[k]`).
fn static_member(file: &FileFacts, span: Span) -> Option<&MemberFact> {
    let ast = &file.ast;
    let member = ast.member_at(span)?;
    ast.computed_members
        .binary_search(&member.span)
        .is_err()
        .then_some(member)
}

/// Whether `span` is itself written: a write target, a pattern's included.
fn written(file: &FileFacts, span: Span) -> bool {
    file.ast
        .write_targets
        .iter()
        .any(|target| file.ast.peel_ts_sugar_span(*target) == span)
}

/// `callee` spells `name` (`Object.create`) on the global, which no declaration of this file shadows.
pub(super) fn global_member_call(file: &FileFacts, callee: Span, name: &str) -> bool {
    let ast = &file.ast;
    let Some(member) = static_member(file, callee) else {
        return false;
    };
    let root = ast.peel_ts_sugar_span(member.object);
    file.source_text(callee).map(str::trim) == Some(name)
        && ast.reference_declaration(root).is_none()
}

/// The value of the own data property `name` of the closed object literal at `literal`.
fn own_data_property(file: &FileFacts, literal: Span, name: &str) -> Option<Span> {
    let ast = &file.ast;
    let shape = ast
        .object_get_shapes
        .iter()
        .find(|shape| shape.span == literal && shape.closed)?;
    if shape
        .properties
        .iter()
        .any(|property| property.getter.is_some())
    {
        return None;
    }
    let inside = |span: Span| literal.contains(span) && span != literal;
    let own = ast
        .object_properties
        .iter()
        .filter(|property| inside(property.span))
        .filter(|property| {
            !ast.object_properties.iter().any(|outer| {
                outer.span != property.span
                    && inside(outer.span)
                    && outer.value.contains(property.span)
            })
        })
        .collect::<Vec<_>>();
    if own.len() != shape.properties.len()
        || own
            .iter()
            .any(|property| !property.data || property.method || property.computed)
    {
        return None;
    }
    own.into_iter()
        .find(|property| file.source_text(property.key) == Some(name))
        .map(|property| property.value)
}

#[cfg(test)]
mod tests {
    use super::Fresh;

    fn file(source: &str) -> solid_facts::FileFacts {
        use solid_facts::core::{Generation, SourceHash, SourcePath};
        let ast = solid_facts::ast::extract("fresh.ts", source).unwrap();
        solid_facts::FileFacts {
            generation: Generation::new(1).unwrap(),
            path: SourcePath::new("fresh.ts").unwrap(),
            source_hash: SourceHash::of(source),
            source: std::sync::Arc::from(source),
            ast: std::sync::Arc::new(ast),
            compiler: std::sync::Arc::new(solid_facts::compiler::ExecutionMap {
                compiler_facts_protocol: solid_facts::compiler::COMPILER_FACTS_PROTOCOL,
                source_hash: SourceHash::of(source),
                semantic_model: Default::default(),
                tracked_regions: vec![],
                untracked_regions: vec![],
                discarded_regions: vec![],
                ownership_regions: vec![],
                callback_roles: vec![],
                jsx_operations: vec![],
            }),
        }
    }

    /// The census's verdict on the program's member writes: `None` when one is an open write, else the strongest form
    /// any of them needed.
    fn verdict(source: &str) -> Option<Option<Fresh>> {
        let file = file(source);
        let (_, _, open, factory) = super::super::impostor_sites(&file);
        if !open.is_empty() {
            return None;
        }
        Some((!factory.is_empty()).then_some(Fresh::Factory))
    }

    /// ADR 1024: each form, and the shape beside it that keeps the write open.
    #[test]
    fn a_write_is_open_unless_its_object_is_proven_made() {
        for (source, expected) in [
            // The router's `createMatcher`: a static member path of a `const` closed literal, returned after the write.
            (
                "export function m(f: any, k: string, v: string) { const match = { path: '', params: {} as Record<string, string> }; match.params[k] = v; return match; }",
                Some(None),
            ),
            // The same literal handed to a call before the write: the callee may rebind `params`.
            (
                "declare function keep(o: object): void; export function m(f: any, k: string, v: string) { const match = { params: {} as Record<string, string> }; keep(match); match.params[k] = v; return match; }",
                None,
            ),
            // Rebound in place.
            (
                "export function m(f: any, t: any, k: string, v: string) { const match = { params: {} as any }; match.params = t; match.params[k] = v; }",
                None,
            ),
            // Written inside a closure, which may run after the return.
            (
                "export function m(f: any, k: string, v: string) { const match = { params: {} as any }; const later = () => { match.params[k] = v; }; later(); return match; }",
                None,
            ),
            // A computed member of the holder may be `params`.
            (
                "export function m(f: any, k: string, v: any) { const match = { params: {} as any } as any; match[k] = v; match.params[k] = v; }",
                None,
            ),
            // A `let` every assignment of which is a literal.
            (
                "export function m(f: any, k: string, s: boolean) { let o: any = {}; if (s) o = []; o[k] = f; }",
                Some(None),
            ),
            // A `let` assigned a parameter.
            (
                "export function m(f: any, t: any, k: string) { let o: any = {}; o = t; o[k] = f; }",
                None,
            ),
            // The router's scroll positions: `JSON.parse(…) || {}`.
            (
                "export function m(f: any, s: string, k: string) { let p: any = {}; p = JSON.parse(s) || {}; p[k] = f; }",
                Some(Some(Fresh::Factory)),
            ),
            // A reviver hands back what it likes.
            (
                "export function m(f: any, s: string, k: string, r: any) { const p = JSON.parse(s, r); p[k] = f; }",
                None,
            ),
            // The router's `useSearchParams`: a `let` bound only to `Object.assign` of itself or a fresh spread.
            (
                "export function m(f: any, raw: any, v: any, k: string) { let r: any; r = Object.assign(r || { ...raw }, v); r[k] = f; }",
                Some(Some(Fresh::Factory)),
            ),
            // `Object.assign` of a target the program does not make.
            (
                "export function m(f: any, t: any, v: any, k: string) { const r = Object.assign(t, v); r[k] = f; }",
                None,
            ),
            // `Object.create(null)`, and of anything else.
            (
                "export function m(f: any, k: string) { const o = Object.create(null); o[k] = f; return { ...o }; }",
                Some(Some(Fresh::Factory)),
            ),
            (
                "export function m(f: any, t: any, k: string) { const o = Object.create(t); o[k] = f; }",
                None,
            ),
            // A shadowed `Object`.
            (
                "export function m(f: any, t: any, k: string) { const Object = { create: (_: null) => t }; const o = Object.create(null); o[k] = f; }",
                None,
            ),
            // The router's `hashKey`: a reduce accumulator over a standard array, seeded fresh, returned.
            (
                "export function m(f: any, v: any) { return Object.keys(v).sort().reduce((result: any, key) => { result[key] = v[key]; return result; }, Object.create(null)); }",
                Some(Some(Fresh::Factory)),
            ),
            // A receiver no fact makes: its `reduce` may be anything.
            (
                "export function m(f: any, xs: any, v: any) { return xs.reduce((result: any, key: string) => { result[key] = v[key]; return result; }, {}); }",
                None,
            ),
            // A default stands in for an `undefined` return.
            (
                "export function m(f: any, t: any, v: any) { return [1].reduce((result: any = t, key: number) => { result[key] = v; return undefined; }, {}); }",
                None,
            ),
            // A return of something else.
            (
                "export function m(f: any, t: any, v: any) { return [1].reduce((result: any, key: number) => { result[key] = v; return t; }, {}); }",
                None,
            ),
        ] {
            assert_eq!(verdict(source), expected, "{source}");
        }
    }

    /// ADR 1041: a function object the program made (a function or arrow expression, a function declaration nothing
    /// rebinds), each beside the shape that keeps the write open.
    #[test]
    fn a_write_to_a_function_object_the_program_made_reaches_no_root() {
        for (source, expected) in [
            // The router's `toAction`: a `const` bound to a function expression, members and symbol keys written.
            (
                "const s = Symbol('s'); export function m(u: string) { const fn: any = function (...a: unknown[]) { return a; }; fn.toString = () => u; fn[s] = u; return fn; }",
                Some(None),
            ),
            // An arrow.
            (
                "export function m(u: string) { const fn: any = () => u; fn.toString = () => u; return fn; }",
                Some(None),
            ),
            // The router's `query.get = …`: a function declaration, exported, never rebound.
            (
                "export function query(k: string) { return k; } query.get = (k: string) => k; query.clear = () => {};",
                Some(None),
            ),
            // The router's `Object.assign(RouterComponent, { match })`: a declaration inside a function.
            (
                "export function r() { function RouterComponent() { return null; } return Object.assign(RouterComponent, { match(u: string) { return u; } }); }",
                Some(None),
            ),
            // A parameter: the caller's object, which may be a standard root.
            (
                "export function m(fn: any, u: string) { fn.toString = () => u; }",
                None,
            ),
            (
                "export function m(fn: any, u: string) { Object.assign(fn, { match: () => u }); }",
                None,
            ),
            // A declaration rebound before the write.
            (
                "export function query(k: string) { return k; } export function set(t: any) { (query as any) = t; } query.get = (k: string) => k;",
                None,
            ),
            // A second declaration of the name replaces the first.
            (
                "function query(k: string) { return k; } function query(k: string) { return k + k; } query.get = (k: string) => k;",
                None,
            ),
            // A `var` of the name.
            (
                "declare const t: any; function query(k: string) { return k; } var query = t; query.get = (k: string) => k;",
                None,
            ),
            // A direct `eval` may rebind it.
            (
                "export function m(s: string) { function query(k: string) { return k; } eval(s); (query as any).get = (k: string) => k; }",
                None,
            ),
        ] {
            assert_eq!(verdict(source), expected, "{source}");
        }
    }

    /// A direct `eval` reaches a `let` or a `const`'s holder by name.
    #[test]
    fn a_direct_eval_keeps_the_binding_forms_open() {
        assert_eq!(
            verdict(
                "export function m(f: any, k: string, s: string) { let o: any = {}; eval(s); o[k] = f; }"
            ),
            None
        );
    }
}
