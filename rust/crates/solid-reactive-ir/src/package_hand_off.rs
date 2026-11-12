//! Batch 11, slice 5 follow-up (the review of #213, B3; ADR 0967): whether a package the build does not read as source may run a function of the project.
//!
//! The flush accounting's program censuses (`flush_accounting::FlushSafe::NoSteppedAction` and its two siblings) read the primitives the build's own table names. An
//! installed package whose source the build does not read can run a function the project hands it as an action's step, an effect's apply or a leaf callback (the
//! package calls the primitive itself), and nothing in the table shows it. The owner's ruling (option b, fail closed, no premise): a flush in a function such a package may
//! run gets no census reason, so it is uncertifiable, never silent.
//!
//! The default is the owner's second ruling on the review of #216 (the flip): **a value counts as handed unless a fact of the program proves where it goes**. Each reference
//! of a function (and of every holder it is copied into) is read at its position, and a position is one of
//!
//! - a call (`f()`, `new C()`, a tag, `f.call(…)`, a method of a holder the project declares): the value runs on the stack of the function the call is written in, so it
//!   is handed when a package may run *that* function (the same question, asked of the owner). A component's closing tag (`</Card>`) names it as the opening one does;
//! - an argument of a call: the receiver decides ([`Receiver`]): a dialect primitive and the standard library's inline callbacks run it before they return, on the stack
//!   of the function the call is written in; a reviewed fresh-stack scheduler, a DOM listener registration and a reviewed test runner's registration run it elsewhere and
//!   keep nothing; a project function the call graph resolves exactly goes on in its parameter (a component's props are its first parameter, a reflective call's forwarded
//!   arguments are shifted by the thisArg; a function that names `arguments` hands every argument); the rest (a collection's insertion, a package, a callee nothing
//!   resolves, a spread) is handed;
//! - a substitution of a tagged template, a JSX prop, a spread attribute, a child expression or a `use:` directive: the same question of what the element is (an
//!   intrinsic element is the DOM's; `Dynamic` is the component its `component` prop names; a package the build does not read is handed). The `ref` callback and the
//!   children (a function child is called as an accessor) of an intrinsic element (or of `Dynamic` with a string tag) are applied where the element is created, on the
//!   stack of the function the element is written in, and so are its spreads and every attribute the compiler does not record as an event handler;
//! - the initializer of a binding, the value of an assignment, or a function's `return`: the value goes on in every read of that holder (in the file and in the files that
//!   import it), of the function's calls, and in the function itself when a package may run it (`pkg(() => cb)`); a callback given literally or by an exact reference returns
//!   the value to its receiving call too (`p.then(() => cb)`, `p.then(give)`). Copies use a separate return-flow walk; an unresolved target retains the obligation.
//!   An assignment expression's own value is asked as any value is (`const first = (last = cb)`).
//!   An assignment into an object nothing here declares (an import, `this`, a call's
//!   result) is handed, one into a property of a local holder goes on in the holder's reads (the table has no row for a property that is only written, so
//!   `el.onclick = f` is followed through `el`);
//! - an operand nothing takes (`typeof f`, `f === g`, an expression statement): not handed;
//! - **anything else** (an iterable, a receiver, a `yield`, a default value, a class field): handed.
//!
//! A context *places* the value only when it carries it (`carries`): nothing between the context and the value computes another value from it, so parentheses,
//! TypeScript's wrappers, a conditional's arms, a logical expression's operands, a comma's last operand, an array literal's elements, an object literal's property values,
//! a spread's argument and the arguments of a dialect primitive's call (`action(step)` returns the function that runs the step) carry it, and nothing else does. In
//! `[cb].map(f)` the literal is the receiver of a call, whose value is another one: no context places `cb`, so it is **handed**, as is a value in a callee's object, an
//! iterable, an operand of `await` or `yield` or a part of a template (the review of #217, A).
//!
//! A function is also handed when it is written in a class (an instance, a subclass or `this` may hand it from where nothing here follows), when it is exported by name
//! from a module a namespace import hands whole (`runNs(NS)`), and when it is exported from a program whose boundary is open.
//!
//! What receives the value decides ([`Receiver`]): a dialect primitive, a reviewed test runner's registration (by the callee's identity, whatever the project proves of when
//! the runner runs a suite body: `reviewed_test_runner_registration`), the standard library's reviewed argument behaviors, a package the build reads as source, a DOM
//! element and a project function whose parameter is followed know their behavior; a declaration in a package the build does not read, a callee nothing resolves and a
//! project slot no function body defines (a function-typed parameter or field: it may hold a package's function) do not.
//!
//! Not followed, named in `docs/open-work.md`: `this` handed from inside a class (a class's functions are handed whole), an instance a framework builds, and a value that
//! reaches a package through a global (`globalThis.x = f` with a hand-off the program does not spell).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use solid_facts::FileFacts;
use solid_facts::ast::{
    ArrayElementKind, AstFacts, CallFact, FunctionFact, FunctionKind, JsxAttributeFact,
    JsxAttributeValueKind, JsxElementFact,
};
use solid_facts::compiler::CompilerOperationKind;
use solid_facts::core::Span;

use crate::indexes::SemanticLookup;
use crate::local_access::HandOff;
use crate::runtime_semantics::RuntimeArgumentBehavior;

/// What receives a function value at a hand-off position.
enum Receiver<'a> {
    /// Code whose behavior the program knows: it runs the value elsewhere (a host scheduler, a DOM listener registration, a reviewed runner) or never, and keeps nothing.
    Known,
    /// Code that runs the value before it returns, on the stack of the function the call is written in: the span of that call. A dialect primitive and the standard
    /// library's inline callbacks (`forEach`, `Array.from`'s map function, a `Promise` executor, `Reflect.apply`'s target).
    Inline(Span),
    /// A project function: the value goes on in the parameter at the index (a project function's body is analyzed).
    Parameter(&'a FileFacts, &'a FunctionFact, usize),
    /// Code nothing here follows: a declaration in a package the build does not read as source, a callee nothing resolves, a slot no function body defines, a
    /// collection's insertion, a spread.
    Unread,
}

type Key = (Arc<str>, Span);

/// What one question has visited, so that a cycle of copies or calls ends.
#[derive(Default)]
struct Walk {
    positions: HashSet<Key>,
    functions: HashSet<Key>,
    returned_positions: HashSet<(Key, Key)>,
    returned_bindings: HashSet<(Key, Key)>,
}

/// How a reflective call forwards its arguments to its target.
#[derive(Clone, Copy)]
enum Forward {
    /// `f.call(thisArg, …args)`, `f.bind(thisArg, …args)`: the arguments after the thisArg are the target's, shifted by one.
    Spread,
    /// `f.apply(thisArg, [args])`, `Reflect.apply(f, thisArg, [args])`, `Reflect.construct(f, [args])`: the arguments are an array no position maps.
    Array,
}

/// The question "may a package nothing here reads run this function", answered per function and kept.
pub(crate) struct UnreadPackageHandOff<'l, 'a> {
    lookup: &'l SemanticLookup<'a>,
    /// The files of the program by path (`SemanticLookup::files`, read once by the pass).
    files: &'l HashMap<&'l str, &'a FileFacts>,
    asked: RefCell<HashMap<Key, bool>>,
}

impl<'l, 'a> UnreadPackageHandOff<'l, 'a> {
    pub(crate) fn new(
        lookup: &'l SemanticLookup<'a>,
        files: &'l HashMap<&'l str, &'a FileFacts>,
    ) -> Self {
        Self {
            lookup,
            files,
            asked: RefCell::new(HashMap::new()),
        }
    }

    /// Whether a function that owns `span` (or one that, through exact call edges, runs it) may be run by a package nothing here reads.
    pub(crate) fn may_run(&self, file: &'a FileFacts, span: Span) -> bool {
        file.ast
            .functions_owning(span)
            .any(|function| self.function(file, function))
    }

    fn function(&self, file: &'a FileFacts, function: &'a FunctionFact) -> bool {
        let key = (file.path.shared(), function.span);
        if let Some(answer) = self.asked.borrow().get(&key) {
            return *answer;
        }
        let answer = self.function_in(file, function, &mut Walk::default());
        self.asked.borrow_mut().insert(key, answer);
        answer
    }

    /// The function as a value (written where it is, or named by its binding's references), and the functions that call it.
    fn function_in(
        &self,
        file: &'a FileFacts,
        function: &'a FunctionFact,
        walk: &mut Walk,
    ) -> bool {
        let key = (file.path.shared(), function.span);
        if !walk.functions.insert(key.clone()) {
            return false;
        }
        if self.asked.borrow().get(&key) == Some(&true) {
            return true;
        }
        let ast = &file.ast;
        // A function written in a class (a method, an arrow in a field, a function in either): an instance, a subclass or `this` may hand it on from where nothing here
        // follows (the review of #216, r2a).
        if ast
            .classes
            .iter()
            .any(|class| class.span.contains(function.span))
        {
            return true;
        }
        if self.exported(file, function) {
            // An export of a program whose boundary is open is entered by code this build cannot see. In a closed program a namespace that escapes with no name the
            // analysis can read (`runNs(NS)`: passed along, spread, an alias, an element access) hands out every export of the module (the namespace census, ADR 0347).
            if !self.lookup.program_closed || self.lookup.escapes_unnamed(file.path.as_str()) {
                return true;
            }
        }
        // A function declaration is hoisted, not written at a position: only its references are.
        let mut positions: Vec<(&'a FileFacts, Span)> = Vec::new();
        if function.kind != FunctionKind::Declaration {
            positions.push((file, function.span));
        }
        if let Some(placed) =
            crate::local_access::function_reference_hand_offs(file, function, self.lookup)
        {
            positions.extend(
                placed
                    .into_iter()
                    .filter(|(_, hand_off)| *hand_off != HandOff::Call)
                    .map(|(at, _)| (file, at)),
            );
        }
        positions.extend(
            crate::local_access::foreign_reference_hand_offs(file, function, self.lookup)
                .into_iter()
                .filter(|(_, _, hand_off)| *hand_off != HandOff::Call)
                .map(|(holder, at, _)| (holder, at)),
        );
        if positions
            .into_iter()
            .any(|(holder, at)| self.position(holder, at, walk))
        {
            return true;
        }
        self.lookup
            .function_call_sites(file.path.as_str(), function.span)
            .into_iter()
            .any(|(caller_file, call_site)| {
                caller_file
                    .ast
                    .functions_owning(call_site)
                    .any(|caller| self.function_in(caller_file, caller, walk))
            })
    }

    /// Whether the module exports the function: `export function f`, `export const f = () => …`, `export default () => …`, an export specifier of its binding, or a
    /// member of an exported object literal (the export statement holds it outside every function of its own).
    fn exported(&self, file: &'a FileFacts, function: &'a FunctionFact) -> bool {
        let ast = &file.ast;
        let written_in_export = ast.exports.iter().any(|export| {
            export.module.is_none()
                && export.span.contains(function.span)
                && !ast
                    .functions_containing(function.span)
                    .any(|outer| outer.span != function.span && export.span.contains(outer.span))
        });
        written_in_export
            || crate::owners::function_binding_name(file, function)
                .is_some_and(|name| crate::indexes::binding_exported(file, name.span))
    }

    /// Whether the value written (or named) at `at` goes where a package nothing here reads may run it: see the module's list of positions.
    fn position(&self, file: &'a FileFacts, at: Span, walk: &mut Walk) -> bool {
        let ast = &file.ast;
        let at = ast.peel_ts_sugar_span(at);
        if !walk.positions.insert((file.path.shared(), at)) {
            return false;
        }
        // `ReturnType<typeof f>` is erased: nothing runs there.
        if ast.type_queries.iter().any(|query| query.contains(at)) {
            return false;
        }
        // The value is called, not handed: `f()`, `new C()`, `(() => …)()`, a tag. It runs on the stack of the function the call is written in; an instance a constructor
        // builds is a value of its own.
        if let Some(call) = ast.calls_with_peeled_callee(at).next() {
            return self.runs_where_written(file, call.span, walk)
                || (call.construct && self.position(file, call.span, walk));
        }
        if let Some(template) = ast
            .tagged_templates
            .iter()
            .find(|template| template.tag == at)
        {
            return self.runs_where_written(file, template.span, walk);
        }
        // A component named as a tag (`<Own />`) runs where the element is created (the call graph's render site). The closing tag of `<Own>…</Own>` names the same component and
        // is a reference of it too: answered as the opening name is (the review of #217, E: Mac 4's finding). An element written as a child or a prop of another element is
        // created where that element evaluates it: `<Pkg><Own>x</Own></Pkg>` runs `Own` when `Pkg` reads its children (the review of #222, E).
        if let Some(element) = ast
            .jsx_containing(at)
            .find(|element| element.name.span == at || element.closing_name == Some(at))
        {
            return self.element_created_in(file, element.span, walk);
        }
        // The property of a member (`NS.step`, `holder.run`) names the function the member reads: the member is the value there.
        if ast.reference_declaration(at).is_none()
            && let Some(member) = ast
                .members
                .iter()
                .find(|member| ast.peel_ts_sugar_span(member.property) == at)
        {
            return self.position(file, member.span, walk);
        }
        // Whether a context the analysis follows holds the value: the contexts decide, and a value in none of them is handed unless it is an operand nothing takes.
        let mut placed = false;
        for (call, index) in ast.arguments_containing(at) {
            if crosses(ast, call.arguments[index].span, at)
                || !self.carries(file, call.arguments[index].span, at)
            {
                continue;
            }
            placed = true;
            let receiver = self.call_receiver(file, call, index);
            if self.receiver_hands(file, receiver, walk) {
                return true;
            }
        }
        for template in &ast.tagged_templates {
            if template.substitutions.iter().any(|substitution| {
                substitution.contains(at)
                    && !crosses(ast, *substitution, at)
                    && self.carries(file, *substitution, at)
            }) {
                placed = true;
                let receiver = self.tag_receiver(file, template.tag);
                if self.receiver_hands(file, receiver, walk) {
                    return true;
                }
            }
        }
        for (index, element) in ast.jsx_elements.iter().enumerate() {
            if element.span == at || !element.span.contains(at) {
                continue;
            }
            if let Some(handed) = self.element_hands(file, index, element, at, walk) {
                placed = true;
                if handed {
                    return true;
                }
            }
        }
        for binding in &ast.bindings {
            let held = binding.initializer.is_some_and(|initializer| {
                initializer.contains(at)
                    && !crosses(ast, initializer, at)
                    && self.carries(file, initializer, at)
            });
            if held {
                placed = true;
                if binding
                    .names
                    .iter()
                    .any(|name| self.binding(file, name.span, walk))
                {
                    return true;
                }
            }
        }
        for assignment in &ast.assignments {
            let value = ast.peel_ts_sugar_span(assignment.value_span);
            if !value.contains(at) || crosses(ast, value, at) || !self.carries(file, value, at) {
                continue;
            }
            placed = true;
            if self.assignment_hands(file, assignment.target, walk) {
                return true;
            }
            // The assignment expression's own value is the value it writes (a plain `=`; a logical one's value is the target's or this one): `const first = (last = cb)`,
            // `first = last = cb` and `[(last = cb)].map(f)` take it on from there, so the expression is asked as any value is, and one written as a statement is an operand
            // nothing takes (the review of #222, B1). An arithmetic assignment computes another value. The expression is the target's start to the value's end: a span that
            // is no expression (a parenthesized target) is in no context, so it is handed.
            if (!assignment.reads_target || assignment.logical.is_some())
                && !assignment.value_span.contains(assignment.target)
                && self.position(
                    file,
                    Span::new(assignment.target.start, assignment.value_span.end),
                    walk,
                )
            {
                return true;
            }
        }
        let returned = self.returned(file, at, walk);
        if returned == Some(true) {
            return true;
        }
        placed |= returned.is_some();
        // A holder a member of which is read, called or written: `holder.run()` runs a function the project declares on the stack of the function the call is written in;
        // `holder.slot` is the member's own value (the same question of the member); any other method of a holder (`list.forEach(pkg.run)`) may pass its elements on.
        for member in ast.members_with_peeled_object(at) {
            // A write through the holder (`holder.slot = v`) stores into it and takes nothing out.
            placed = true;
            if ast
                .write_targets
                .iter()
                .any(|target| ast.peel_ts_sugar_span(*target) == member.span)
            {
                continue;
            }
            let handed = match ast.calls_with_peeled_callee(member.span).next() {
                Some(call) => self.member_call_hands(file, call, walk),
                None => self.position(file, member.span, walk),
            };
            if handed {
                return true;
            }
        }
        if placed {
            return false;
        }
        // The value itself is an operand nothing takes (`typeof f`, `f === g`, `f;`): only the whole operand, since a value inside a larger expression (`[f].forEach(g)`)
        // flows on in what that expression does.
        let inert = ast
            .inert_operands
            .iter()
            .any(|operand| ast.peel_ts_sugar_span(*operand) == at);
        !inert
    }

    /// Whether the expression `outer` is, or holds as an element or a property value, the value written at `at`, through nothing that computes another value from it:
    /// parentheses and TypeScript's wrappers, a conditional's arms, a logical expression's operands, a comma's last operand, an array literal's elements and a spread's
    /// argument, an object literal's property values, and the arguments of a call of a dialect primitive. A context places the value only when it carries it: `[cb].map(f)`
    /// does not (the literal is the receiver of a call, and the call's value is another one), and neither does a template's part, an operand of `await` or `yield`, nor an
    /// iterable: the value there is handed (the review of #217, A).
    ///
    /// A dialect primitive's call carries what it is handed because the dialect's table does not say whether the call returns something that runs the callback later
    /// ([`Self::call_receiver`] asks the stack of the call): `action(function* () { … })` returns the function that runs the step, on the stack of whoever calls it, so
    /// `const run = action(step)` places `step` in `run` and a function that returns `action(step)` returns it.
    fn carries(&self, file: &'a FileFacts, outer: Span, at: Span) -> bool {
        let ast = &file.ast;
        let outer = ast.peel_ts_sugar_span(outer);
        let at = ast.peel_ts_sugar_span(at);
        if outer == at {
            return true;
        }
        if !outer.contains(at) {
            return false;
        }
        if let Some(array) = ast
            .array_literal_elements
            .iter()
            .find(|array| array.span == outer)
        {
            return array.elements.iter().any(|element| {
                element.kind != ArrayElementKind::Hole && self.carries(file, element.span, at)
            });
        }
        if let Some(conditional) = ast
            .conditional_expressions
            .iter()
            .find(|conditional| conditional.span == outer)
        {
            return self.carries(file, conditional.consequent, at)
                || self.carries(file, conditional.alternate, at);
        }
        if let Some(logical) = ast
            .logical_expressions
            .iter()
            .find(|logical| logical.span == outer)
        {
            return self.carries(file, logical.left, at) || self.carries(file, logical.right, at);
        }
        if let Some(sequence) = ast.sequences.iter().find(|sequence| sequence.span == outer) {
            return self.carries(file, sequence.tail, at);
        }
        if let Some(call) = ast.calls.iter().find(|call| call.span == outer) {
            return self.lookup.primitive_at_call(file, call.span).is_some()
                && call
                    .arguments
                    .iter()
                    .any(|argument| !argument.spread && self.carries(file, argument.span, at));
        }
        // An object literal: the values of the properties written directly in it, and the argument of a spread written directly in it.
        if ast
            .object_get_shapes
            .iter()
            .any(|shape| shape.span == outer)
        {
            let direct = |inner: Span| {
                outer.contains(inner)
                    && inner != outer
                    && !ast.object_properties.iter().any(|property| {
                        property.span != inner
                            && property.span.contains(inner)
                            && outer.contains(property.span)
                    })
            };
            return ast
                .object_properties
                .iter()
                .any(|property| direct(property.span) && self.carries(file, property.value, at))
                || ast
                    .spreads
                    .iter()
                    .any(|spread| direct(spread.span) && self.carries(file, spread.argument, at));
        }
        false
    }

    /// Whether a package may run the code that creates the JSX element at `span`: the function it is written in, and, when it is written inside another element with no
    /// function between (a child, a prop's value), wherever that element evaluates it, which is the question of the element's own value there (the review of #222, E).
    fn element_created_in(&self, file: &'a FileFacts, span: Span, walk: &mut Walk) -> bool {
        let ast = &file.ast;
        if self.runs_where_written(file, span, walk) {
            return true;
        }
        let nested = ast.jsx_elements.iter().any(|outer| {
            outer.span != span
                && outer.span.contains(span)
                && !ast
                    .functions_containing(span)
                    .any(|function| outer.span.contains(function.span))
        });
        nested && self.position(file, span, walk)
    }

    /// Whether a function that owns `at` (a call, a tag, a carrier) may be run by a package: what is called there runs on its stack.
    fn runs_where_written(&self, file: &'a FileFacts, at: Span, walk: &mut Walk) -> bool {
        file.ast
            .functions_owning(at)
            .any(|owner| self.function_in(file, owner, walk))
    }

    /// Whether what the receiver does with the value hands it to a package.
    fn receiver_hands(&self, file: &'a FileFacts, receiver: Receiver<'a>, walk: &mut Walk) -> bool {
        match receiver {
            Receiver::Unread => true,
            Receiver::Known => false,
            Receiver::Inline(call) => self.runs_where_written(file, call, walk),
            Receiver::Parameter(callee_file, callee, parameter) => {
                self.parameter(callee_file, callee, parameter, walk)
            }
        }
    }

    /// A call of a member of the holder at `call.callee`'s object: whether it hands the holder's members on. The member the project declares runs where the call is
    /// written; `Function.prototype.call` and `apply` run their receiver there; `bind` makes a function of it; any other declaration (a collection's method, a package's,
    /// nothing that resolves) may pass the members on.
    fn member_call_hands(&self, file: &'a FileFacts, call: &CallFact, walk: &mut Walk) -> bool {
        let lookup = self.lookup;
        let resolved = lookup.resolved_callee_call(file, call.callee);
        let standard = resolved.and_then(crate::runtime_semantics::standard_call_name);
        if matches!(
            standard,
            Some(
                "Function.call"
                    | "CallableFunction.call"
                    | "NewableFunction.call"
                    | "Function.apply"
                    | "CallableFunction.apply"
                    | "NewableFunction.apply"
            )
        ) {
            return self.runs_where_written(file, call.span, walk);
        }
        if matches!(
            standard,
            Some("Function.bind" | "CallableFunction.bind" | "NewableFunction.bind")
        ) {
            return self.position(file, call.span, walk);
        }
        // The member the project declares: its property symbol's declarations all lie in analyzed files (a property of an interface or a class the project writes, an
        // object literal's member; the call through a function-typed property has no declaration of its own to resolve).
        let analyzed = |path: &str| self.files.contains_key(path);
        let property_in_project = ast_member_property(&file.ast, call.callee)
            .and_then(|property| lookup.entities().at(file.path.as_str(), property))
            .and_then(|symbol| lookup.symbol_declarations(symbol.as_str()))
            .is_some_and(|declarations| {
                !declarations.is_empty()
                    && declarations
                        .iter()
                        .all(|declaration| analyzed(declaration.location.path.as_ref()))
            });
        let declared_in_project = property_in_project
            || resolved
                .and_then(|resolved| resolved.declaration.as_ref())
                .is_some_and(|declaration| {
                    !declaration.standard_library && analyzed(declaration.location.path.as_ref())
                });
        if declared_in_project {
            self.runs_where_written(file, call.span, walk)
        } else {
            true
        }
    }

    /// Whether the assignment of the value to `target` hands it to a package: a variable or a property of a project binding is followed in every read of the holder; an
    /// import, `this` and a call's result name a holder nothing here follows (handed). A property the standard library declares is the host's (`window.onerror = f`), which
    /// runs it elsewhere and keeps nothing for project code: asked of the compiler's table, which has a row for the property only when something else demanded it.
    fn assignment_hands(&self, file: &'a FileFacts, target: Span, walk: &mut Walk) -> bool {
        let ast = &file.ast;
        let target = ast.peel_ts_sugar_span(target);
        if let Some(member) = ast.member_at(target) {
            let property_symbol = self
                .lookup
                .entities()
                .at(file.path.as_str(), member.property);
            let declarations = property_symbol
                .and_then(|symbol| self.lookup.symbol_declarations(symbol.as_str()))
                .unwrap_or_default();
            if !declarations.is_empty()
                && declarations
                    .iter()
                    .all(|declaration| declaration.location.path.starts_with("bundled:"))
            {
                return false;
            }
        }
        let Some(root) = root_identifier(ast, target) else {
            return true;
        };
        let Some(declaration) = ast.reference_declaration(root) else {
            return true;
        };
        // An import names a module's binding, an object another file (or a package) holds.
        let imported = ast
            .imports
            .iter()
            .flat_map(|import| &import.bindings)
            .any(|binding| binding.local.span == declaration);
        imported || self.binding(file, declaration, walk)
    }

    /// A function returns the value (a `return`, an arrow's expression body): the calls of that function evaluate to it (`make()`, an immediately invoked function), so
    /// each is a position the value is at, and a package that runs the function may call what it returns (`pkg(() => cb)`). `None` when no function returns it.
    fn returned(&self, file: &'a FileFacts, at: Span, walk: &mut Walk) -> Option<bool> {
        let ast = &file.ast;
        let returns = |ret: &solid_facts::ast::ReturnFact| {
            ret.argument.is_some_and(|argument| {
                let argument = ast.peel_ts_sugar_span(argument);
                argument.contains(at)
                    && !crosses(ast, argument, at)
                    && self.carries(file, argument, at)
            })
        };
        let mut any = false;
        for function in ast.functions_containing(at) {
            if function.span == at {
                continue;
            }
            let owned = |ret: &solid_facts::ast::ReturnFact| {
                ast.innermost_function_owning(ret.span)
                    .is_some_and(|owner| owner.span == function.span)
            };
            let returned = function.expression_return.as_ref().is_some_and(returns)
                || ast
                    .returns_within(function.body)
                    .any(|ret| returns(ret) && owned(ret));
            if !returned {
                continue;
            }
            any = true;
            // The function may itself be run by a package, which may call what it returns.
            if self.function_in(file, function, walk) {
                return Some(true);
            }
            let mut calls: Vec<(&'a FileFacts, Span)> = ast
                .calls_with_peeled_callee(function.span)
                .map(|call| (file, call.span))
                .collect();
            if self.callback_return_hands(file, function, walk) {
                return Some(true);
            }
            for (caller_file, callee) in self
                .lookup
                .function_call_sites(file.path.as_str(), function.span)
            {
                if let Some(call) = caller_file.ast.calls_with_peeled_callee(callee).next() {
                    calls.push((caller_file, call.span));
                }
            }
            if calls
                .into_iter()
                .any(|(holder, span)| self.position(holder, span, walk))
            {
                return Some(true);
            }
        }
        any.then_some(false)
    }

    /// A callback's return reaches its receiving call whether the callback is a literal, a named binding, or a closed object's member. Follow its value copies with a
    /// separate visited set: asking where the function runs and asking where its return goes are different questions at the same reference.
    fn callback_return_hands(
        &self,
        file: &'a FileFacts,
        function: &'a FunctionFact,
        walk: &mut Walk,
    ) -> bool {
        let target = (file.path.shared(), function.span);
        if function.kind == FunctionKind::Declaration
            && !function.name.as_ref().is_some_and(|name| {
                self.lookup
                    .entities()
                    .exact(&name.span.location(file.path.shared()))
                    .is_some()
            })
        {
            return true;
        }
        let mut positions = Vec::new();
        if function.kind != FunctionKind::Declaration {
            positions.push((file, function.span));
        }
        if let Some(references) =
            crate::local_access::function_reference_hand_offs(file, function, self.lookup)
        {
            positions.extend(references.into_iter().map(|(at, _)| (file, at)));
        }
        positions.extend(
            crate::local_access::foreign_reference_hand_offs(file, function, self.lookup)
                .into_iter()
                .map(|(holder, at, _)| (holder, at)),
        );
        positions
            .into_iter()
            .any(|(holder, at)| self.callback_return_at(holder, at, &target, walk))
    }

    fn callback_return_at(
        &self,
        file: &'a FileFacts,
        at: Span,
        target: &Key,
        walk: &mut Walk,
    ) -> bool {
        let ast = &file.ast;
        let at = ast.peel_ts_sugar_span(at);
        if !walk
            .returned_positions
            .insert((target.clone(), (file.path.shared(), at)))
            || ast.type_queries.iter().any(|query| query.contains(at))
        {
            return false;
        }
        let exact_target = |value| {
            let value = ast.peel_ts_sugar_span(value);
            self.callback_target(file, value)
                .is_some_and(|(holder, function)| {
                    holder.path.as_str() == target.0.as_ref() && function.span == target.1
                })
        };
        if let Some(call) = ast.calls_with_peeled_callee(at).next() {
            return !exact_target(at) || self.position(file, call.span, walk);
        }
        let mut placed = false;
        for (call, index) in ast.arguments_containing(at) {
            let argument = &call.arguments[index];
            if crosses(ast, argument.span, at) || !self.carries(file, argument.span, at) {
                continue;
            }
            placed = true;
            // A parameter's later consumers are not the enclosing call's result. Without that return-flow proof, retain the obligation.
            if argument.spread
                || !exact_target(argument.span)
                || matches!(
                    self.call_receiver(file, call, index),
                    Receiver::Parameter(..)
                )
                || self.position(file, call.span, walk)
            {
                return true;
            }
        }
        for binding in &ast.bindings {
            if binding.initializer.is_some_and(|initializer| {
                !crosses(ast, initializer, at) && self.carries(file, initializer, at)
            }) {
                placed = true;
                if binding
                    .names
                    .iter()
                    .any(|name| self.callback_return_binding(file, name.span, target, walk))
                {
                    return true;
                }
            }
        }
        for member in ast.members_with_peeled_object(at) {
            placed = true;
            if !exact_target(member.span)
                || self.callback_return_at(file, member.span, target, walk)
            {
                return true;
            }
        }
        !placed
            && !ast
                .inert_operands
                .iter()
                .any(|operand| ast.peel_ts_sugar_span(*operand) == at)
    }

    fn callback_return_binding(
        &self,
        file: &'a FileFacts,
        name: Span,
        target: &Key,
        walk: &mut Walk,
    ) -> bool {
        if !walk
            .returned_bindings
            .insert((target.clone(), (file.path.shared(), name)))
        {
            return false;
        }
        if self
            .lookup
            .entities()
            .exact(&name.location(file.path.shared()))
            .is_none()
            || (!self.lookup.program_closed && crate::indexes::binding_exported(file, name))
        {
            return true;
        }
        file.ast.references_to(name).iter().any(|reference| {
            !file
                .ast
                .write_targets
                .iter()
                .any(|written| file.ast.peel_ts_sugar_span(*written) == *reference)
                && self.callback_return_at(file, *reference, target, walk)
        }) || self
            .foreign_references(file, name)
            .into_iter()
            .any(|(holder, at, import)| {
                if import {
                    self.callback_return_binding(holder, at, target, walk)
                } else {
                    self.callback_return_at(holder, at, target, walk)
                }
            })
    }

    /// Resolve the whole callback value. A structural property declaration alone does not prove which function a mutable object holds.
    fn callback_target(
        &self,
        file: &'a FileFacts,
        value: Span,
    ) -> Option<(&'a FileFacts, &'a FunctionFact)> {
        let value = file.ast.peel_ts_sugar_span(value);
        if let Some(function) = file.ast.function_at(value) {
            return Some((file, function));
        }
        if let Some(member) = file.ast.member_at(value) {
            let property = self
                .lookup
                .ast_file_index(file.path.as_str())?
                .member_property(value)?;
            let name = file.source_text(property)?;
            let mut consulted = Vec::new();
            let symbols = self.lookup.member_value_symbols_at_tracked(
                file,
                member.object,
                name,
                &mut consulted,
            );
            for (path, _) in consulted {
                let consulted_file = self.files.get(path.as_str()).copied()?;
                self.lookup.note_file_read(consulted_file);
            }
            return match symbols.as_slice() {
                [symbol] => self.lookup.function_for_symbol(symbol.as_str()),
                _ => None,
            };
        }
        self.project_function_at(file, value)
            .filter(|(holder, function)| self.lookup.function_value_is_current(holder, function))
    }

    /// Every read of the binding named at `name`: in the file, and in the files that import it. A binding the module exports from a program whose boundary is open is
    /// read by code this build cannot see.
    fn binding(&self, file: &'a FileFacts, name: Span, walk: &mut Walk) -> bool {
        let ast = &file.ast;
        if !self.lookup.program_closed && crate::indexes::binding_exported(file, name) {
            return true;
        }
        if ast
            .references_to(name)
            .iter()
            .filter(|reference| {
                !ast.write_targets
                    .iter()
                    .any(|target| ast.peel_ts_sugar_span(*target) == **reference)
            })
            .any(|reference| self.position(file, *reference, walk))
        {
            return true;
        }
        self.foreign_references(file, name)
            .into_iter()
            .any(|(holder, at, import)| {
                if import {
                    self.binding(holder, at, walk)
                } else {
                    self.position(holder, at, walk)
                }
            })
    }

    /// The references to the binding named at `name` in other files (a reference to an import's local name is the import itself: `true`, its uses are the local
    /// binding's), as `foreign_reference_hand_offs` reads them for a function.
    fn foreign_references(
        &self,
        file: &'a FileFacts,
        name: Span,
    ) -> Vec<(&'a FileFacts, Span, bool)> {
        let lookup = self.lookup;
        let Some(symbol) = lookup.entities().exact(&name.location(file.path.shared())) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for reference in lookup.symbol_references_through_redirects(symbol.as_str()) {
            if reference.path.as_ref() == file.path.as_str() {
                continue;
            }
            let Some(holder) = self.files.get(reference.path.as_ref()).copied() else {
                continue;
            };
            let (Ok(start), Ok(end)) = (
                u32::try_from(reference.start_byte),
                u32::try_from(reference.end_byte),
            ) else {
                continue;
            };
            let at = Span::new(start, end);
            let ast = &holder.ast;
            if ast
                .exports
                .iter()
                .flat_map(|export| &export.specifiers)
                .any(|specifier| specifier.local.span == at)
            {
                continue;
            }
            let import = ast
                .imports
                .iter()
                .flat_map(|import| &import.bindings)
                .any(|binding| binding.local.span == at);
            out.push((holder, at, import));
        }
        out
    }

    /// The parameter at `index` of a project function the value is passed to: every read of the names it binds (a rest parameter takes the tail). A function that names
    /// `arguments` hands every argument, and an index past the list of a function that has an `arguments` object is read through it, which nothing here follows.
    fn parameter(
        &self,
        file: &'a FileFacts,
        function: &'a FunctionFact,
        index: usize,
        walk: &mut Walk,
    ) -> bool {
        // A function that names `arguments` reads every argument through it, and no declared name shows that (the review of #217, B).
        if names_arguments(file, function) {
            return true;
        }
        let names: Vec<Span> = match function.parameters.get(index) {
            Some(parameter) => parameter.names.iter().map(|name| name.span).collect(),
            None if function.rest_parameter => function
                .rest_parameter_names
                .iter()
                .map(|name| name.span)
                .collect(),
            None => return function.kind != FunctionKind::Arrow,
        };
        names.into_iter().any(|name| self.binding(file, name, walk))
    }

    /// What receives argument `index` of `call`.
    fn call_receiver(&self, file: &'a FileFacts, call: &CallFact, index: usize) -> Receiver<'a> {
        let lookup = self.lookup;
        // A spread argument hands the elements of what it spreads, and an index no argument fills is a position nothing here knows.
        let Some(argument) = call.arguments.get(index) else {
            return Receiver::Unread;
        };
        if argument.spread {
            return Receiver::Unread;
        }
        if lookup.reviewed_test_runner_registration(file, call) {
            return Receiver::Known;
        }
        // A dialect primitive runs a callback it is handed inline or later, which the dialect's table does not distinguish from a stack: the stack of the call is asked.
        if lookup.primitive_at_call(file, call.span).is_some() {
            return Receiver::Inline(call.span);
        }
        // A project function the call graph resolves exactly: its body is analyzed, and the value goes on in its parameter.
        if let Some((callee_file, callee, exact)) =
            crate::execution_role::project_callee_at_parameter(file, call, index, lookup)
        {
            return if exact {
                Receiver::Parameter(callee_file, callee, index)
            } else {
                Receiver::Unread
            };
        }
        let resolved = lookup.resolved_callee_call(file, call.callee);
        // `f.call(this, …)`, `f.apply(this, […])`, `f.bind(this, …)`, `Reflect.apply(f, this, […])` hand their arguments to `f`: the standard declaration is the
        // forwarder's, the receiver of the value is the function it forwards to.
        if let Some((target, forward, target_index)) =
            resolved.and_then(|resolved| forwarding(file, call, resolved))
        {
            return self.forwarded(file, target, forward, target_index, index, call.span);
        }
        self.declaration_receiver(file, call, index, resolved)
    }

    /// What argument `index` of a reflective call reaches. The target itself (`Reflect.apply`'s first argument) is invoked before the call returns. A forwarded
    /// argument goes to the target's parameter shifted by the thisArg when the target is a project function; every other argument (the thisArg, an arguments array,
    /// a target no project function names) is a value nothing here follows.
    fn forwarded(
        &self,
        file: &'a FileFacts,
        target: Span,
        forward: Forward,
        target_index: Option<usize>,
        index: usize,
        call: Span,
    ) -> Receiver<'a> {
        if target_index == Some(index) {
            return Receiver::Inline(call);
        }
        match (forward, target_index) {
            // `f.call(thisArg, …)`, `f.bind(thisArg, …)`: argument 0 is the thisArg, argument `i + 1` is the target's parameter `i`.
            (Forward::Spread, None) if index >= 1 => self
                .project_function_at(file, target)
                .map_or(Receiver::Unread, |(target_file, function)| {
                    Receiver::Parameter(target_file, function, index - 1)
                }),
            _ => Receiver::Unread,
        }
    }

    /// The project function the expression at `at` names (a binding of one, an imported one), when the compiler resolves it to a function with a body.
    fn project_function_at(
        &self,
        file: &'a FileFacts,
        at: Span,
    ) -> Option<(&'a FileFacts, &'a FunctionFact)> {
        self.lookup
            .entities()
            .value_symbol(file, at)
            .and_then(|symbol| self.lookup.function_for_symbol(symbol.as_str()))
    }

    /// The receiver a resolved call's declaration names: a standard-library declaration by its reviewed argument behavior (a collection's insertion, a function that
    /// copies or returns its argument and one the table does not list are not known), a package the build reads as source; a declaration in a package it does not
    /// read, a project declaration no function body defines (a function-typed parameter or field) and a call nothing resolves are not.
    fn declaration_receiver(
        &self,
        file: &'a FileFacts,
        call: &CallFact,
        index: usize,
        resolved: Option<&typefacts::ResolvedCall>,
    ) -> Receiver<'a> {
        let Some(resolved) = resolved else {
            return Receiver::Unread;
        };
        let Some(declaration) = resolved.declaration.as_ref() else {
            return Receiver::Unread;
        };
        if declaration.standard_library {
            let callability = self
                .lookup
                .entity_at(file.path.as_str(), call.arguments[index].span)
                .and_then(|entity| entity.callability);
            return match crate::runtime_semantics::argument_behavior(resolved, callability, index) {
                Some(RuntimeArgumentBehavior::InlineCallback) => Receiver::Inline(call.span),
                Some(
                    RuntimeArgumentBehavior::FreshStackCallback
                    | RuntimeArgumentBehavior::DeferredCallback,
                ) => Receiver::Known,
                // Every DOM redeclaration of `addEventListener` registers its listener the same way, and every redeclaration of `removeEventListener` only removes it
                // (the reference classifier's own answers, `hand_off_at`).
                None if crate::runtime_semantics::runs_on_invoker_stack(
                    resolved,
                    callability,
                    index,
                ) =>
                {
                    Receiver::Known
                }
                None if index == 1
                    && declaration.qualified_name.rsplit_once('.').is_some_and(
                        |(owner, member)| !owner.is_empty() && member == "removeEventListener",
                    ) =>
                {
                    Receiver::Known
                }
                // A value the call may keep or return (`Set.add`, `Array.of`, `Promise.resolve`, `Object.assign`) and a call the table does not list.
                Some(
                    RuntimeArgumentBehavior::RetainedValue | RuntimeArgumentBehavior::ValueOnly,
                )
                | None => Receiver::Unread,
            };
        }
        if self
            .lookup
            .in_dependency_source(declaration.location.path.as_ref())
        {
            Receiver::Known
        } else {
            Receiver::Unread
        }
    }

    /// What a tagged template's tag receives its substitutions as: the standard library's tag (`String.raw`) and a package the build reads as source are known, any other
    /// is not.
    fn tag_receiver(&self, file: &'a FileFacts, tag: Span) -> Receiver<'a> {
        let declaration = self
            .lookup
            .resolved_callee_call(file, tag)
            .and_then(|resolved| resolved.declaration.as_ref());
        match declaration {
            Some(declaration)
                if declaration.standard_library
                    || self
                        .lookup
                        .in_dependency_source(declaration.location.path.as_ref()) =>
            {
                Receiver::Known
            }
            _ => Receiver::Unread,
        }
    }

    /// What the element does with the value at `at` (a prop's value, a spread's argument, a child expression, a directive's value): `None` when no attribute, spread or
    /// child of the element holds it, otherwise whether it goes where a package nothing here reads may run it.
    fn element_hands(
        &self,
        file: &'a FileFacts,
        index: usize,
        element: &JsxElementFact,
        at: Span,
        walk: &mut Walk,
    ) -> Option<bool> {
        let ast = &file.ast;
        let carried = |outer: Span| {
            outer.contains(at) && !crosses(ast, outer, at) && self.carries(file, outer, at)
        };
        let attribute_holds = |attribute: &JsxAttributeFact| {
            attribute
                .expression
                .or(attribute.value)
                .is_some_and(carried)
        };
        // A child is its expression container, braces included: the expression inside is what carries the value. A child that is an element is the value when the value
        // is that element (the question of where a nested element is created).
        let child_holds = |container: Span| {
            if container == at {
                return true;
            }
            let expression = ast
                .jsx_child_expressions
                .iter()
                .copied()
                .filter(|child| container.contains(*child) && *child != container)
                .max_by_key(|child| child.end - child.start)
                .unwrap_or(container);
            expression.contains(at)
                && !crosses(ast, container, at)
                && self.carries(file, expression, at)
        };
        // A `use:name` directive is the one namespaced attribute whose local name the binder resolves to a declaration (`directive_binding`), and a call of the function the
        // name resolves to with the attribute's value as its accessor: argument 1.
        let directive = |attribute: &&JsxAttributeFact| attribute.directive_binding.is_some();
        let mut placed = false;
        for attribute in element.attributes.iter().filter(directive) {
            if attribute_holds(attribute) {
                placed = true;
                let receiver = self.directive_receiver(file, attribute);
                if self.receiver_hands(file, receiver, walk) {
                    return Some(true);
                }
            }
        }
        // An intrinsic element's `ref` callback is applied where the element is created: `ref` of `@solidjs/web` (rc.21) resolves its accessor and calls `applyRef` at once, on the
        // stack of the code that creates the element (the review of #217, D; Chrome, the `ph-refprop` and `ph-dynref` cells). `Dynamic` with a string tag creates one.
        // Every other attribute, its children and its spreads are applied there too: rc.21 inserts the children (a function child is called as an accessor: `<div>{cb}</div>`,
        // `children={cb}`), and `spread`'s apply calls the spread's `ref` and inserts its `children` (Chrome, FLUSH_IN_ACTION in a step). Only an attribute the compiler
        // records as an event handler is the DOM's, run on its own stack; a spread's keys are not read here (the review of #222, D).
        let applied_at_creation = |attribute: &&JsxAttributeFact| {
            !directive(attribute)
                && !compiled_operation(file, attribute, CompilerOperationKind::EventHandler)
        };
        if (is_intrinsic_tag(file, element) || self.dynamic_string_tag(file, element))
            && (element
                .attributes
                .iter()
                .filter(applied_at_creation)
                .any(attribute_holds)
                || element.children.iter().any(|child| child_holds(*child))
                || element
                    .spreads
                    .iter()
                    .any(|spread| carried(spread.argument)))
        {
            placed = true;
            if self.element_created_in(file, element.span, walk) {
                return Some(true);
            }
        }
        let component_gets_it = element
            .attributes
            .iter()
            .filter(|attribute| !directive(attribute))
            .any(attribute_holds)
            || element
                .spreads
                .iter()
                .any(|spread| carried(spread.argument))
            || element.children.iter().any(|child| child_holds(*child));
        if component_gets_it {
            placed = true;
            let receiver = self.element_receiver(file, index, element);
            if self.receiver_hands(file, receiver, walk) {
                return Some(true);
            }
        }
        placed.then_some(false)
    }

    /// A `use:name` directive's function: a project function takes the attribute's value as its accessor (parameter 1), any other (a package's import, nothing the
    /// program declares) is not followed.
    fn directive_receiver(
        &self,
        file: &'a FileFacts,
        attribute: &JsxAttributeFact,
    ) -> Receiver<'a> {
        attribute
            .directive_binding
            .and_then(|declaration| self.lookup.entities().at(file.path.as_str(), declaration))
            .and_then(|symbol| self.lookup.function_for_symbol(symbol.as_str()))
            .map_or(Receiver::Unread, |(directive_file, function)| {
                Receiver::Parameter(directive_file, function, 1)
            })
    }

    /// What a JSX element's props, children and spreads are handed to: an intrinsic element is the DOM's (its handlers run on the browser's own stack); a dialect export
    /// runs its callbacks where it is written; the dialect's component that renders the component a prop names (`Dynamic`) is that component; a project function's first
    /// parameter is the props object; a component declared in a package the build reads as source is known; any other is not.
    fn element_receiver(
        &self,
        file: &'a FileFacts,
        index: usize,
        element: &JsxElementFact,
    ) -> Receiver<'a> {
        let lookup = self.lookup;
        // The language's own rule of a tag: a plain name that starts with a lowercase letter is an intrinsic element, anything else is a value.
        if is_intrinsic_tag(file, element) {
            return Receiver::Known;
        }
        // `<Dynamic component={…}/>` renders what its `component` prop names, and hands it every other prop: a string names an intrinsic element, a component is that
        // component's props, anything else is not followed (a package's component, a value nothing resolves).
        if let Some(prop) = lookup.intrinsic_tag_prop(file, element.name.span) {
            return self.dynamic_receiver(file, element, prop);
        }
        if lookup
            .primitives(file)
            .jsx
            .get(index)
            .is_some_and(Option::is_some)
        {
            return Receiver::Inline(element.span);
        }
        let Some(symbol) = lookup.entities().value_symbol(file, element.name.span) else {
            return Receiver::Unread;
        };
        // A project component: its props are the first parameter, read as any parameter is.
        if let Some((component_file, component)) = lookup.function_for_symbol(symbol.as_str()) {
            return Receiver::Parameter(component_file, component, 0);
        }
        match lookup.symbol_declarations(symbol.as_str()) {
            Some(declarations)
                if !declarations.is_empty()
                    && declarations.iter().all(|declaration| {
                        lookup.in_dependency_source(declaration.location.path.as_ref())
                    }) =>
            {
                Receiver::Known
            }
            _ => Receiver::Unread,
        }
    }

    /// Whether the element is a `Dynamic` (the dialect's component that renders what its `component` prop names) whose prop is a string: an intrinsic element's tag.
    fn dynamic_string_tag(&self, file: &'a FileFacts, element: &JsxElementFact) -> bool {
        self.lookup
            .intrinsic_tag_prop(file, element.name.span)
            .is_some_and(|prop| {
                element.attributes.iter().any(|attribute| {
                    attribute.namespace.is_none()
                        && file.source_text(attribute.name) == Some(prop)
                        && attribute.value_kind == JsxAttributeValueKind::String
                })
            })
    }

    /// The receiver of a `Dynamic` element's props: the component its `prop` attribute names. A string attribute is an intrinsic element's tag (the DOM's), an
    /// expression a project function resolves to is that component (its props are the first parameter), anything else is not followed.
    fn dynamic_receiver(
        &self,
        file: &'a FileFacts,
        element: &JsxElementFact,
        prop: &str,
    ) -> Receiver<'a> {
        let Some(attribute) = element.attributes.iter().find(|attribute| {
            attribute.namespace.is_none() && file.source_text(attribute.name) == Some(prop)
        }) else {
            return Receiver::Unread;
        };
        if attribute.value_kind == JsxAttributeValueKind::String {
            return Receiver::Known;
        }
        let Some(value) = attribute.expression.or(attribute.value) else {
            return Receiver::Unread;
        };
        self.project_function_at(file, file.ast.peel_ts_sugar_span(value))
            .map_or(Receiver::Unread, |(component_file, component)| {
                Receiver::Parameter(component_file, component, 0)
            })
    }
}

/// The expression a standard reflective call forwards its arguments to, how it forwards them, and the argument index that is the target itself (`Reflect.apply(f, …)`
/// and `Reflect.construct(f, …)` invoke their first argument; `f.call(…)`, `f.apply(…)` and `f.bind(…)` hold it as their receiver). The declaration is the resolved
/// standard one (ECMAScript 20.2.3, 28.1), never a spelling.
fn forwarding(
    file: &FileFacts,
    call: &CallFact,
    resolved: &typefacts::ResolvedCall,
) -> Option<(Span, Forward, Option<usize>)> {
    let receiver = || {
        file.ast
            .member_at(file.ast.peel_ts_sugar_span(call.callee))
            .map(|member| member.object)
    };
    match crate::runtime_semantics::standard_call_name(resolved)? {
        "Function.call"
        | "CallableFunction.call"
        | "NewableFunction.call"
        | "Function.bind"
        | "CallableFunction.bind"
        | "NewableFunction.bind" => receiver().map(|target| (target, Forward::Spread, None)),
        "Function.apply" | "CallableFunction.apply" | "NewableFunction.apply" => {
            receiver().map(|target| (target, Forward::Array, None))
        }
        "Reflect.apply" | "Reflect.construct" => call
            .arguments
            .first()
            .map(|argument| (argument.span, Forward::Array, Some(0))),
        _ => None,
    }
}

/// The property span of the member a call's callee names, under TypeScript's wrappers.
fn ast_member_property(ast: &AstFacts, callee: Span) -> Option<Span> {
    ast.member_at(ast.peel_ts_sugar_span(callee))
        .map(|member| member.property)
}

/// Whether the pinned compiler's semantic trace records an operation of `kind` at the attribute's expression (a `ref` callback, an event handler: the compiler reads the
/// attribute's own name, for a native element and for a component alike), so no spelling is read here.
fn compiled_operation(
    file: &FileFacts,
    attribute: &JsxAttributeFact,
    kind: CompilerOperationKind,
) -> bool {
    attribute.expression.is_some_and(|expression| {
        file.compiler
            .semantic_model
            .operations
            .iter()
            .any(|operation| operation.kind == kind && operation.span == expression)
    })
}

/// Whether the element's tag is an intrinsic element's: a plain name that starts with a lowercase letter (the language's own rule of a tag).
fn is_intrinsic_tag(file: &FileFacts, element: &JsxElementFact) -> bool {
    element.member_object.is_none()
        && file
            .source_text(element.name.span)
            .unwrap_or_default()
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_lowercase())
}

/// Whether a function that has an `arguments` object of its own (any but an arrow, which reads its enclosing function's) reads it: the object holds every argument, which no
/// parameter's references show (`value_use::reads_arguments`: a reference the binder finds, in the function's text).
fn names_arguments(file: &FileFacts, function: &FunctionFact) -> bool {
    function.kind != FunctionKind::Arrow && crate::value_use::reads_arguments(file, function.span)
}

/// Whether a function or a JSX element lies between `outer` (an argument, an initializer, a prop's value) and `at`: the value at `at` is that function's or that element's
/// business, not `outer`'s (a callback written in a callback, a handler written in a child element).
fn crosses(ast: &AstFacts, outer: Span, at: Span) -> bool {
    ast.functions_containing(at)
        .any(|function| function.span != at && outer.contains(function.span))
        || ast.jsx_elements.iter().any(|element| {
            element.span != at && outer.contains(element.span) && element.span.contains(at)
        })
}

/// The identifier a write target is rooted at: `kept` in `kept = f`, `holder` in `holder.slot.inner = f`. `None` when the root is no identifier (`this`, a call's result) or
/// no binding the scope tree resolves (a global).
fn root_identifier(ast: &AstFacts, target: Span) -> Option<Span> {
    let mut at = ast.peel_ts_sugar_span(target);
    for _ in 0..32 {
        match ast.member_at(at) {
            Some(member) => at = ast.peel_ts_sugar_span(member.object),
            None => return ast.reference_declaration(at).map(|_| at),
        }
    }
    None
}
