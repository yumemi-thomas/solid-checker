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
//!   is handed when a package may run *that* function (the same question, asked of the owner). A component's closing tag (`</Card>`) names it as the opening one does.
//!   Code in an element's prop, spread or child expression (no function between) runs where that element evaluates it: the compiler makes it a getter of a component's
//!   props, read when the component reads the prop, so under a component nothing here reads (`<Pkg>{helper()}</Pkg>`, `<Pkg>{list.map(() => <Own />)}</Pkg>`) it is
//!   handed;
//!   An accessor runs where it is read: a project component's props (whose expressions are getters) and an object literal with a getter are followed as objects of
//!   accessors, so a reference of them in code a package may run hands what the accessor runs (`runStep(() => { void props.children; })`); a method call on them
//!   (`o.m()`, `props.pick()`) binds `this` to the object, and the method may read an accessor through `this` or `super` (which reads the prototype's member with the
//!   same `this`): it is handed unless the member resolves to exactly one function that never names either;
//! - a receiver reaches members through no reference of their object (the check of #227, G1): a value placed directly in an object literal that writes `this` or `super` (a
//!   property's value, a spread's argument, a `__proto__:` value) is handed, as is a prototype installed by a write of `prototype` or `__proto__` (a `const` literal that writes
//!   no receiver takes the write as a `__proto__:` key, and the value goes on in the reads of the object), and a call of a member the project declares hands its holder when the
//!   callee names `this` or `super` or nothing resolves it to exactly one function that never does (a method written outside the literal, a parameter, a call's result). The
//!   object a member reaches a receiver through is no reference either (the review of #229, B2: a member assigned after the literal, written by a project function, on a
//!   derived literal, an `Object.create` result or a class's instance), so the key is followed instead: a function stored under a key (a property of an object literal, an
//!   assignment's target) is handed when a use of a receiver, in code a package may run, reads the key (`this.k`, `super.k`, `const { k } = this`) or any key (`this[key]`, the
//!   receiver kept in a local, reflected, spread or taken whole by a rest: followed as an object of accessors; a key the program does not name is any key); none of it withholds a hand-off;
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
use std::collections::{BTreeMap, HashMap, HashSet};
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
#[derive(Clone, Copy)]
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

/// The spans of the `this` and `super` expressions of one file, shared; `None` when the file does not parse.
type ReceiverSpans = Option<Arc<Vec<Span>>>;

/// The spans of the object literals of one file that write a receiver, shared; `None` when the file's receivers cannot be read.
type ReceiverLiterals = Option<Arc<Vec<Span>>>;

/// One use of a receiver (`this`, `super`) in a file of the program: what it reads, and where.
#[derive(Clone, Copy)]
struct ReceiverRead<'a> {
    file: &'a FileFacts,
    at: Span,
    kind: ReadKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReadKind {
    /// `this.k`, `super.k`, `this["k"]`, `this[key]`: the member expression, read (or called) there.
    Member,
    /// The member is the target of a write (`this.k = v`, `super.k = v`): a setter of the key may run there, and nothing is read out.
    MemberWrite,
    /// `const { k } = this`: the local the pattern binds to the member.
    Local,
    /// The receiver is used as a value (`const self = this`, `f(this)`, `{ ...this }`, `return this`, a pattern with a rest): any member may be read from it, anywhere it goes.
    Escape,
    /// The file's receivers cannot be read: any member may be read.
    Unreadable,
}

/// Every use of a receiver in the files of the program, by the key it reads (the review of #229, B2): the members `this` and `super` read are reached through no reference of
/// the object that holds them, so a value stored under a key (by an assignment, a literal's property) is read by a receiver that names the key, wherever the object came from.
#[derive(Default)]
struct ReceiverReads<'a> {
    named: BTreeMap<String, Vec<ReceiverRead<'a>>>,
    /// A read of a key the program does not name (`this[key]`), and every use of the receiver as a value: they read any key.
    any: Vec<ReceiverRead<'a>>,
}

/// What one question has visited, so that a cycle of copies or calls ends.
#[derive(Default)]
struct Walk {
    /// The value followed is an object whose members are accessors (a component's props, whose expressions the compiler makes getters; an object literal with a
    /// getter): reading a member runs the accessor where the read is written, so a reference of the object in code a package may run hands what the accessor runs
    /// (the review of #222's check 3, R1 and R2). The mode is set and restored on the walk of the question that asks (`as_accessors`), so a cycle through it ends
    /// in the one walk (the review of #226, B1); a position is visited once per mode.
    accessor: bool,
    /// In that mode, the component whose props are followed: a reference written directly in its body, outside every JSX expression and class, reads them when the
    /// component runs, which is where its element is created (already asked).
    accessor_owner: Option<Key>,
    positions: HashSet<(Key, bool, Option<Key>)>,
    /// A function's own question ("may a package run it") is the same in every mode: `function_in` sets the mode off while it asks, so one visit serves all.
    functions: HashSet<Key>,
    /// What a callback's return reaches follows the value in the mode the walk is in (it asks `position` of each place the return is at), so these are visited
    /// per mode and owner as `positions` is: a place visited following a function value is not visited following an accessor object (the review of #226's
    /// re-check, N2: `[0].map(() => (c ? helper : o))` cut the object's flow after the function's).
    returned_positions: HashSet<((Key, Key), bool, Option<Key>)>,
    returned_bindings: HashSet<((Key, Key), bool, Option<Key>)>,
}

impl Walk {
    /// The mode the walk is in: whether it follows an object of accessors, and the component whose props it is.
    fn mode(&self) -> (bool, Option<Key>) {
        (self.accessor, self.accessor_owner.clone())
    }
}

/// Whether the function is the value of an accessor property of an object literal (`get g() { … }`, `set g(v) { … }`: neither a data property nor a method),
/// and the literal: `Some(None)` when no literal fact holds the property, which is handed (a missing fact is no proof).
fn accessor_object(ast: &AstFacts, function: &FunctionFact) -> Option<Option<Span>> {
    let property = ast
        .object_properties
        .iter()
        .find(|property| !property.data && !property.method && property.value == function.span)?;
    Some(
        ast.object_get_shapes
            .iter()
            .filter(|shape| shape.span.contains(property.span))
            .min_by_key(|shape| shape.span.end - shape.span.start)
            .map(|shape| shape.span),
    )
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
    /// The spans of every `this` and `super` expression of a file, parsed once (`None`: the file does not parse, so nothing is known of its receiver).
    receiver_spans: RefCell<HashMap<Arc<str>, ReceiverSpans>>,
    /// The object literals of a file that write `this` or `super` anywhere in their text (`None`: the file's receivers cannot be read, so every literal may).
    receiver_literals: RefCell<HashMap<Arc<str>, ReceiverLiterals>>,
    /// Whether a file of the program (a dependency source file too: a key it exports names the property as well) spells the `__proto__` key anywhere: what makes a write
    /// through a key the program does not name a possible prototype install.
    proto_key_spelled: std::cell::OnceCell<bool>,
    /// The uses of receivers in the files of the program, read once (`receiver_reads`).
    receiver_reads: std::cell::OnceCell<ReceiverReads<'a>>,
    /// Whether an object literal, asked as a holder, is provably confined (`literal_confined`), kept: the answer is the syntax's and the call graph's, not a walk's.
    confined: RefCell<HashMap<Key, bool>>,
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
            receiver_spans: RefCell::new(HashMap::new()),
            receiver_literals: RefCell::new(HashMap::new()),
            proto_key_spelled: std::cell::OnceCell::new(),
            receiver_reads: std::cell::OnceCell::new(),
            confined: RefCell::new(HashMap::new()),
        }
    }

    /// Whether a function that owns `span` (or one that, through exact call edges, runs it) may be run by a package nothing here reads, or the element whose prop or
    /// child expression holds `span` may evaluate it there (`<Pkg>{(flush(), "x")}</Pkg>`).
    pub(crate) fn may_run(&self, file: &'a FileFacts, span: Span) -> bool {
        file.ast
            .functions_owning(span)
            .any(|function| self.function(file, function))
            || self.evaluated_by_element(file, span, &mut Walk::default())
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

    /// The function as a value (written where it is, or named by its binding's references), and the functions that call it. Its positions are asked of the
    /// function itself, not of an object of accessors, whatever mode the asking walk is in.
    fn function_in(
        &self,
        file: &'a FileFacts,
        function: &'a FunctionFact,
        walk: &mut Walk,
    ) -> bool {
        let saved = (walk.accessor, walk.accessor_owner.take());
        walk.accessor = false;
        let answer = self.function_value_in(file, function, walk);
        walk.accessor = saved.0;
        walk.accessor_owner = saved.1;
        answer
    }

    fn function_value_in(
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
        // An accessor of an object literal (`{ get g() { … } }`) runs where a member of the object is read, not where the function is a value: the object is followed as
        // an object of accessors (the review of #222's check 3, R2).
        if let Some(object) = accessor_object(ast, function) {
            // No literal fact holds the property: handed.
            let Some(object) = object else {
                return true;
            };
            // `this` or `super` in the literal's own functions reads its members through no reference of the object (the review of #226, B3; `super.x` runs the
            // prototype's `x` against the object, which a getter of the literal may be read through): handed, as is a file whose receivers cannot be read.
            let Some(receiver_spans) = self.receiver_spans(file) else {
                return true;
            };
            let receiver_in_literal = receiver_spans.iter().any(|span| object.contains(*span));
            if receiver_in_literal
                || self.as_accessors(walk, None, |walk| self.position(file, object, walk))
            {
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
        // A call site runs where it is written: in the function that owns it, or where the element whose prop or child expression holds it evaluates it
        // (`<Pkg>{helper()}</Pkg>`: the re-review of #222, N1).
        self.lookup
            .function_call_sites(file.path.as_str(), function.span)
            .into_iter()
            .any(|(caller_file, call_site)| self.runs_where_written(caller_file, call_site, walk))
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
        let (accessor, owner) = walk.mode();
        if !walk
            .positions
            .insert(((file.path.shared(), at), accessor, owner))
        {
            return false;
        }
        // A reference of an object whose members are accessors may read one where it is written: a member read, a destructuring, a spread, an iteration.
        if walk.accessor {
            // In a class (a field initializer, a method): the class's code runs where an instance is made or a method called, which nothing here follows, as a
            // class's functions are handed whole.
            if ast.classes.iter().any(|class| class.span.contains(at)) {
                return true;
            }
            let in_owner = walk.accessor_owner.as_ref().is_some_and(|(path, owner)| {
                path.as_ref() == file.path.as_str()
                    && ast
                        .innermost_function_owning(at)
                        .is_some_and(|function| function.span == *owner)
            });
            // In the component's own body the read runs with the component, unless it is written in a JSX prop, spread or child expression, which the element it is
            // in evaluates where it chooses (`return <Pkg>{typeof props.children}</Pkg>`: the review of #226, B2).
            let runs = if in_owner {
                self.evaluated_by_element(file, at, walk)
            } else {
                self.runs_where_written(file, at, walk)
            };
            if runs {
                return true;
            }
        }
        // `ReturnType<typeof f>` is erased: nothing runs there.
        if ast.type_queries.iter().any(|query| query.contains(at)) {
            return false;
        }
        // A value placed directly in an object literal that writes `this` or `super` may be run, or read, through that receiver, which no reference of the object shows:
        // a method called by `this.m()` in a closure a package runs, a prototype's getter read by `super.g` (the review of #227's check, G1).
        if self.placed_in_receiver_literal(file, at) {
            return true;
        }
        // A value stored under a key a receiver reads, in a function a package may run, is reached by that read wherever the object it is stored in came from (an
        // object assigned after the literal, a prototype, an instance of a class, a parameter: the review of #229, B2).
        if self.placed_under_read_key(file, at, walk) {
            return true;
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
            // A write of `__proto__` or `prototype` installs the value as the prototype of objects nothing here follows: their members are read through receivers, not
            // through the value's references (the review of #227's check, G1).
            if self.installs_prototype(file, assignment.target)
                || self.assigned_under_read_key(file, assignment.target, walk)
                || self.assignment_hands(file, assignment.target, walk)
            {
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
            // A method call on an object of accessors binds `this` to it: the method may read an accessor through `this`, which no reference of the object shows.
            if walk.accessor && self.method_call_may_read_receiver(file, member.span) {
                return true;
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

    /// The spans of every `this` and `super` expression of the file: syntax only, from a parse of the file's own text, kept per file. `None` when the file does not parse.
    fn receiver_spans(&self, file: &FileFacts) -> ReceiverSpans {
        let path = file.path.shared();
        if let Some(known) = self.receiver_spans.borrow().get(&path) {
            return known.clone();
        }
        let spans = solid_facts::ast::receiver_expression_spans(
            std::path::Path::new(file.path.as_str()),
            &file.source,
        )
        .map(Arc::new);
        self.receiver_spans.borrow_mut().insert(path, spans.clone());
        spans
    }

    /// Whether `function` names `this` or `super` anywhere in its text (an arrow's is its enclosing function's, a nested function's its own: both count, which can only
    /// add). A file that does not parse names them.
    fn function_names_receiver(&self, file: &FileFacts, function: &FunctionFact) -> bool {
        let Some(spans) = self.receiver_spans(file) else {
            return true;
        };
        spans.iter().any(|span| function.span.contains(*span))
    }

    /// The object literals of the file that write `this` or `super` anywhere in their text (an accessor's, a method's, an arrow's, a nested literal's: which can only add),
    /// kept per file. `None` when the file's receivers cannot be read.
    fn receiver_literals(&self, file: &FileFacts) -> ReceiverLiterals {
        let path = file.path.shared();
        if let Some(known) = self.receiver_literals.borrow().get(&path) {
            return known.clone();
        }
        let literals = self.receiver_spans(file).map(|spans| {
            Arc::new(
                file.ast
                    .object_get_shapes
                    .iter()
                    .map(|shape| shape.span)
                    .filter(|literal| spans.iter().any(|span| literal.contains(*span)))
                    .collect::<Vec<_>>(),
            )
        });
        self.receiver_literals
            .borrow_mut()
            .insert(path, literals.clone());
        literals
    }

    /// Whether the value written (or named) at `at` is placed directly in an object literal that writes a receiver: a property's value, a spread's argument, a `__proto__:`
    /// value (the contexts an object literal carries, [`Self::carries`]). A literal whose file's receivers cannot be read is one.
    fn placed_in_receiver_literal(&self, file: &'a FileFacts, at: Span) -> bool {
        let ast = &file.ast;
        let placed = |literal: Span| {
            literal != at
                && literal.contains(at)
                && !crosses(ast, literal, at)
                && self.carries(file, literal, at)
        };
        match self.receiver_literals(file) {
            Some(literals) => literals.iter().any(|literal| placed(*literal)),
            None => ast.object_get_shapes.iter().any(|shape| placed(shape.span)),
        }
    }

    /// Whether an assignment to `target` installs a prototype that nothing here follows: a write of `prototype` (`F.prototype = p`), of `__proto__` into anything but a `const` that
    /// holds one object literal writing no receiver (`o.__proto__ = p`, `o["__proto__"] = p`: that one is a `__proto__:` key written late, and the value goes on in the reads of
    /// the object as it does there), or a write through a key the program does not name where a file of the project spells `__proto__` (the key may be that one: ADR 0643's
    /// net for a handed key).
    fn installs_prototype(&self, file: &FileFacts, target: Span) -> bool {
        let ast = &file.ast;
        let Some(member) = ast.member_at(ast.peel_ts_sugar_span(target)) else {
            return false;
        };
        match crate::installed_members::member_name(file, member) {
            Some("__proto__") => !self.receiverless_literal_binding(file, member.object),
            Some("prototype") => true,
            Some(_) => false,
            // Every file of the program spells the key as well as the project's: a name a dependency source file exports (`o[KEY] = base`) is the key too (the review of
            // #229, S2). A key no file spells is a write nothing here reads as an install: the stores under a key a receiver reads are the question of
            // [`Self::placed_under_read_key`] and [`Self::assigned_under_read_key`].
            None => {
                *self.proto_key_spelled.get_or_init(|| {
                    self.files
                        .values()
                        .any(|file| file.source.contains("__proto__"))
                }) && ast.computed_members.binary_search(&member.span).is_ok()
            }
        }
    }

    /// Whether the expression at `object` is a reference to a `const` whose initializer is one object literal that writes no `this` and no `super`, so that nothing reaches
    /// the literal's members through a receiver (a method written elsewhere that names one is the call's question: [`Self::method_call_may_read_receiver`]).
    fn receiverless_literal_binding(&self, file: &FileFacts, object: Span) -> bool {
        let ast = &file.ast;
        let Some(declaration) = ast.reference_declaration(ast.peel_ts_sugar_span(object)) else {
            return false;
        };
        let Some(literal) = ast
            .bindings
            .iter()
            .find(|binding| {
                binding.immutable
                    && binding.shape == solid_facts::ast::BindingShape::Identifier
                    && binding.names.iter().any(|name| name.span == declaration)
            })
            .and_then(|binding| binding.initializer)
            .map(|initializer| ast.peel_ts_sugar_span(initializer))
            .filter(|initializer| {
                ast.object_get_shapes
                    .iter()
                    .any(|shape| shape.span == *initializer)
            })
        else {
            return false;
        };
        self.receiver_literals(file)
            .is_some_and(|literals| !literals.contains(&literal))
    }

    /// Every use of a receiver in the files of the program (a dependency source file too), read once and kept: [`ReceiverReads`].
    fn receiver_reads(&self) -> &ReceiverReads<'a> {
        self.receiver_reads.get_or_init(|| {
            let mut files: Vec<(&str, &'a FileFacts)> = self
                .files
                .iter()
                .map(|(path, file)| (*path, *file))
                .collect();
            files.sort_unstable_by_key(|(path, _)| *path);
            let mut reads = ReceiverReads::default();
            for (_, file) in files {
                self.collect_receiver_reads(file, &mut reads);
            }
            reads
        })
    }

    /// The uses of `this` and `super` in one file, by what each reads: a member (`this.k`, `super.k`, `this["k"]`; a computed key the program does not name reads any
    /// key), a key a pattern destructures (`const { k } = this`), or the receiver as a value (`const self = this`, `f(this)`, `return this`, `{ ...this }`, a pattern with a
    /// rest, an iteration), which reads any key anywhere it goes. A call of `super(…)`, a private member (`this.#k`: only the class's own code names it) and an operand nothing
    /// takes (`typeof this`) read no key.
    fn collect_receiver_reads(&self, file: &'a FileFacts, reads: &mut ReceiverReads<'a>) {
        if !file.source.contains("this") && !file.source.contains("super") {
            return;
        }
        // A file of another language (a JSON or CSS module the program reads) holds no receiver; a script that does not parse may hold any.
        let script = std::path::Path::new(file.path.as_str())
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(
                    extension,
                    "ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "mjs" | "cjs"
                )
            });
        if !script {
            return;
        }
        let Some(spans) = self.receiver_spans(file) else {
            reads.any.push(ReceiverRead {
                file,
                at: Span::new(0, 0),
                kind: ReadKind::Unreadable,
            });
            return;
        };
        let mut seen: HashSet<(Arc<str>, Span)> = HashSet::new();
        for &receiver in spans.iter() {
            // `super(…)` runs the parent's constructor against the object: no member is read through it.
            if file.ast.calls_with_peeled_callee(receiver).next().is_some() {
                continue;
            }
            self.follow_receiver(file, receiver, 0, &mut seen, reads);
        }
    }

    /// The uses of `value`, the receiver or a name for it (a local it is kept in, a parameter it is passed to): a member read through it is a read of the key, a pattern that
    /// destructures it reads the keys it names, a local that keeps it and a call of a function the build reads whose parameter receives it are followed to their uses, and any
    /// other use hands it where nothing here follows (`return this`, `{ ...this }`, an argument of a callee nothing resolves exactly, an iteration): it reads any key there.
    fn follow_receiver(
        &self,
        file: &'a FileFacts,
        value: Span,
        depth: usize,
        seen: &mut HashSet<(Arc<str>, Span)>,
        reads: &mut ReceiverReads<'a>,
    ) {
        const DEPTH: usize = 6;
        let ast = &file.ast;
        let value = ast.peel_ts_sugar_span(value);
        if !seen.insert((file.path.shared(), value)) {
            return;
        }
        let mut member_read = false;
        for member in ast.members_with_peeled_object(value) {
            member_read = true;
            let write = ast
                .write_targets
                .iter()
                .any(|target| ast.peel_ts_sugar_span(*target) == member.span);
            let read = ReceiverRead {
                file,
                at: member.span,
                kind: if write {
                    ReadKind::MemberWrite
                } else {
                    ReadKind::Member
                },
            };
            // `this.constructor` and `this.__proto__` are other objects of the receiver (its class, its prototype), whose members are read from the result: any key.
            match self
                .member_key(file, member)
                .filter(|key| !matches!(key.as_str(), "constructor" | "__proto__"))
            {
                Some(key) => reads.named.entry(key).or_default().push(read),
                None => reads.any.push(read),
            }
        }
        if member_read || private_member_of(file, value) {
            return;
        }
        if ast
            .inert_operands
            .iter()
            .any(|operand| ast.peel_ts_sugar_span(*operand) == value)
        {
            return;
        }
        if let Some(binding) = ast.bindings.iter().find(|binding| {
            binding
                .initializer
                .is_some_and(|initializer| ast.peel_ts_sugar_span(initializer) == value)
        }) {
            // `const { k } = this`: the pattern reads each key it names, and binds the local to it.
            if binding.shape == solid_facts::ast::BindingShape::Object
                && !binding.object_slots.is_empty()
                && binding.object_slots.len() == binding.names.len()
                && binding
                    .object_slots
                    .iter()
                    .all(|slot| slot.plain_identifier_key)
            {
                for slot in &binding.object_slots {
                    reads
                        .named
                        .entry(slot.property.to_string())
                        .or_default()
                        .push(ReceiverRead {
                            file,
                            at: slot.local.span,
                            kind: ReadKind::Local,
                        });
                }
                return;
            }
            // `const self = this`: the receiver under another name, read where the name is.
            if binding.shape == solid_facts::ast::BindingShape::Identifier
                && depth < DEPTH
                && binding.names.iter().all(|name| {
                    !crate::indexes::binding_exported(file, name.span)
                        && self.foreign_references(file, name.span).is_empty()
                })
            {
                for name in &binding.names {
                    for reference in ast.references_to(name.span) {
                        if !ast
                            .write_targets
                            .iter()
                            .any(|target| ast.peel_ts_sugar_span(*target) == *reference)
                        {
                            self.follow_receiver(file, *reference, depth + 1, seen, reads);
                        }
                    }
                }
                return;
            }
        }
        let argument = ast.arguments_containing(value).find(|(call, index)| {
            !call.arguments[*index].spread
                && ast.peel_ts_sugar_span(call.arguments[*index].span) == value
        });
        // `held.has(router)`, `requests.set(router, …)` on a weak collection: the object is a key whose identity is compared, and no key of one can be got back.
        if let Some((call, index)) = argument
            && self.unread_argument(file, call, index, false)
        {
            return;
        }
        // `f(this)`: the parameter of a function the build reads, which receives it.
        if depth < DEPTH
            && let Some((call, index)) = argument
            && let Receiver::Parameter(callee_file, callee, parameter) =
                self.call_receiver(file, call, index)
            && !names_arguments(callee_file, callee)
        {
            let names: Option<Vec<Span>> = match callee.parameters.get(parameter) {
                Some(declared) => Some(declared.names.iter().map(|name| name.span).collect()),
                None if callee.rest_parameter => Some(
                    callee
                        .rest_parameter_names
                        .iter()
                        .map(|name| name.span)
                        .collect(),
                ),
                // An index past the list is no name; a function other than an arrow has an `arguments` object to read it through (`Self::parameter` hands it).
                None => (callee.kind == FunctionKind::Arrow).then(Vec::new),
            };
            if let Some(names) = names {
                for name in names {
                    for reference in callee_file.ast.references_to(name) {
                        self.follow_receiver(callee_file, *reference, depth + 1, seen, reads);
                    }
                }
                return;
            }
        }
        reads.any.push(ReceiverRead {
            file,
            at: value,
            kind: ReadKind::Escape,
        });
    }

    /// Whether a use of a receiver in code a package may run reads `key` (`None`: a key the program does not name, which any read may be), or the receiver as a value.
    fn read_through_receiver(&self, key: Option<&str>, walk: &mut Walk) -> bool {
        let reads = self.receiver_reads();
        let by_name: Box<dyn Iterator<Item = &ReceiverRead<'a>> + '_> = match key {
            Some(key) => Box::new(reads.named.get(key).into_iter().flatten()),
            None => Box::new(reads.named.values().flatten()),
        };
        by_name
            .chain(reads.any.iter())
            .any(|read| self.receiver_read_runs(*read, walk))
    }

    /// Whether the use of a receiver runs on a package's stack, or hands what it reads to one. The member that is read (a getter runs where the read is written; a function
    /// read is called there or goes on from there), the local a pattern binds, and the receiver used as a value, whose reads go wherever the value goes (followed as an object
    /// of accessors, which reads whatever member it holds).
    fn receiver_read_runs(&self, read: ReceiverRead<'a>, walk: &mut Walk) -> bool {
        let saved = (walk.accessor, walk.accessor_owner.take());
        walk.accessor = false;
        let answer = match read.kind {
            ReadKind::Unreadable => true,
            ReadKind::MemberWrite => self.runs_on_package_stack(read.file, read.at, walk),
            // A member that is called runs the function where the call is written; one that is only read goes on from there (the value the read gives is another's).
            ReadKind::Member => {
                self.runs_on_package_stack(read.file, read.at, walk)
                    || (!self.called_as_method(read.file, read.at)
                        && self.position(read.file, read.at, walk))
            }
            ReadKind::Local => {
                self.runs_on_package_stack(read.file, read.at, walk)
                    || self.binding(read.file, read.at, walk)
            }
            ReadKind::Escape => {
                self.as_accessors(walk, None, |walk| self.position(read.file, read.at, walk))
            }
        };
        walk.accessor = saved.0;
        walk.accessor_owner = saved.1;
        answer
    }

    /// Whether the code at `at` runs on a package's stack: the innermost function that owns it may be run by one (an arrow a package is handed, a getter or a method of an
    /// object that writes a receiver, a function written in a class), or the element whose prop or child expression holds it evaluates it there. Not the functions around that
    /// function: a closure that runs on a timer's stack in a getter a package may run does not run on the package's, and one run inline is the enclosing function's question
    /// (`function_in` asks it of the closure's own positions).
    fn runs_on_package_stack(&self, file: &'a FileFacts, at: Span, walk: &mut Walk) -> bool {
        let ast = &file.ast;
        let by_function = match ast.innermost_function_owning(at) {
            Some(function) => self.function_in(file, function, walk),
            // A field initializer or a static block runs where an instance is made or the class is defined: a class's code is handed whole.
            None => ast.classes.iter().any(|class| class.span.contains(at)),
        };
        by_function || self.evaluated_by_element(file, at, walk)
    }

    /// Whether the value written (or named) at `at` is stored under a key a receiver reads: a property of an object literal that carries it (a nested literal's value
    /// carries it up to the literal it is in, a spread's argument stores it under the keys of the object spread), or the target of an assignment that does. The object that
    /// holds it is no reference the read follows (a prototype, an instance of a class, a parameter, an object the program builds elsewhere): the key is.
    fn placed_under_read_key(&self, file: &'a FileFacts, at: Span, walk: &mut Walk) -> bool {
        let reads = self.receiver_reads();
        if reads.named.is_empty() && reads.any.is_empty() {
            return false;
        }
        let ast = &file.ast;
        let mut keys: Vec<Option<String>> = Vec::new();
        for shape in &ast.object_get_shapes {
            let literal = shape.span;
            if literal == at
                || !literal.contains(at)
                || crosses(ast, literal, at)
                || !self.carries(file, literal, at)
            {
                continue;
            }
            // A literal that provably cannot be the receiver of a function that reads members through `this` stores nothing a receiver reads.
            if self.literal_confined(file, literal) {
                continue;
            }
            let direct = |inner: Span| {
                literal.contains(inner)
                    && inner != literal
                    && !ast.object_properties.iter().any(|property| {
                        property.span != inner
                            && property.span.contains(inner)
                            && literal.contains(property.span)
                    })
            };
            for property in &ast.object_properties {
                if direct(property.span) && self.carries(file, property.value, at) {
                    keys.push(self.property_key(file, property));
                }
            }
            for spread in &ast.spreads {
                if direct(spread.span) && self.carries(file, spread.argument, at) {
                    keys.push(None);
                }
            }
        }
        keys.iter()
            .any(|key| self.read_through_receiver(key.as_deref(), walk))
    }

    /// Whether an assignment to `target` (`holder.m = f`, `holder[key] = f`) stores under a key a receiver reads: [`Self::placed_under_read_key`]'s question of an
    /// assignment's target. A target that is no member (`kept = f`) stores under no key.
    fn assigned_under_read_key(&self, file: &'a FileFacts, target: Span, walk: &mut Walk) -> bool {
        let reads = self.receiver_reads();
        if reads.named.is_empty() && reads.any.is_empty() {
            return false;
        }
        let Some(member) = file.ast.member_at(file.ast.peel_ts_sugar_span(target)) else {
            return false;
        };
        // A holder that provably cannot be the receiver of a function that reads members through `this` stores nothing a receiver reads.
        if self.holder_confined(file, member.object) {
            return false;
        }
        let key = self.member_key(file, member);
        self.read_through_receiver(key.as_deref(), walk)
    }

    /// Whether the object the expression at `holder` names provably cannot be the receiver of a function that reads members through `this` or `super`, nor inherit from
    /// one: a plain object literal (no `this` or `super` anywhere in it, no `__proto__` or computed key) bound by a `const` name, whose every use is one that makes it neither
    /// (a member read, a member call whose callee names no receiver, a write of a member under a key the program names that is not `__proto__`, a destructuring, a copy, a
    /// call of a function the build reads whose parameter is confined in turn). An escape is no proof: a value passed to, returned to or stored where code nothing here
    /// follows can reach it (a package, a callee nothing resolves exactly, `return o`, `[o]`, `x.p = o`, an export) may be given a prototype, have a member called with it
    /// as `this`, or be made the receiver of a function with `.call`, and what is not proved is handed.
    fn holder_confined(&self, file: &'a FileFacts, holder: Span) -> bool {
        let ast = &file.ast;
        let holder = ast.peel_ts_sugar_span(holder);
        if ast
            .object_get_shapes
            .iter()
            .any(|shape| shape.span == holder)
        {
            return self.literal_confined(file, holder);
        }
        let Some(declaration) = ast.reference_declaration(holder) else {
            return false;
        };
        let key = (file.path.shared(), declaration);
        if let Some(known) = self.confined.borrow().get(&key) {
            return *known;
        }
        let answer = self.named_object_confined(file, declaration, &mut HashSet::new());
        self.confined.borrow_mut().insert(key, answer);
        answer
    }

    /// [`Self::holder_confined`] of an object literal: where it is written decides (the binding that names it, the parameter that receives it, the literal that holds it).
    fn literal_confined(&self, file: &'a FileFacts, literal: Span) -> bool {
        let key = (file.path.shared(), literal);
        if let Some(known) = self.confined.borrow().get(&key) {
            return *known;
        }
        let answer = self.literal_context_confined(file, literal, &mut HashSet::new());
        self.confined.borrow_mut().insert(key, answer);
        answer
    }

    fn literal_context_confined(
        &self,
        file: &'a FileFacts,
        literal: Span,
        seen: &mut HashSet<(Arc<str>, Span)>,
    ) -> bool {
        let ast = &file.ast;
        if !self.literal_plain(file, literal) {
            return false;
        }
        // Named by a `const`: its uses.
        if let Some(binding) = ast.bindings.iter().find(|binding| {
            binding
                .initializer
                .is_some_and(|initializer| ast.peel_ts_sugar_span(initializer) == literal)
        }) {
            return binding.shape == solid_facts::ast::BindingShape::Identifier
                && binding
                    .names
                    .iter()
                    .all(|name| self.named_object_confined(file, name.span, seen));
        }
        // An argument of a call of a function the build reads: that parameter's uses.
        if let Some((call, index)) = ast.arguments_containing(literal).find(|(call, index)| {
            !call.arguments[*index].spread
                && ast.peel_ts_sugar_span(call.arguments[*index].span) == literal
        }) {
            return match self.call_receiver(file, call, index) {
                Receiver::Parameter(callee_file, callee, parameter) => {
                    self.parameter_confined(callee_file, callee, parameter, seen)
                }
                _ => false,
            };
        }
        // A property's value of a literal that holds it: confined as that literal is.
        let outer = ast
            .object_get_shapes
            .iter()
            .filter(|shape| shape.span != literal && shape.span.contains(literal))
            .min_by_key(|shape| shape.span.end - shape.span.start)
            .map(|shape| shape.span);
        match outer {
            Some(outer)
                if direct_properties(ast, outer)
                    .any(|property| ast.peel_ts_sugar_span(property.value) == literal) =>
            {
                self.literal_context_confined(file, outer, seen)
            }
            _ => false,
        }
    }

    /// An object literal that writes no receiver and sets no prototype: no `this` or `super` anywhere in it, and no property under `__proto__` or under a key the program
    /// does not name (which may be `__proto__`).
    fn literal_plain(&self, file: &FileFacts, literal: Span) -> bool {
        let Some(receivers) = self.receiver_literals(file) else {
            return false;
        };
        !receivers.contains(&literal)
            && direct_properties(&file.ast, literal).all(|property| {
                self.property_key(file, property)
                    .is_some_and(|key| key != "__proto__")
            })
    }

    /// A `const` whose initializer is a plain literal (or another name for one), with every use confined.
    fn named_object_confined(
        &self,
        file: &'a FileFacts,
        declaration: Span,
        seen: &mut HashSet<(Arc<str>, Span)>,
    ) -> bool {
        if !seen.insert((file.path.shared(), declaration)) {
            return true;
        }
        let ast = &file.ast;
        let Some(binding) = ast.identifier_binding(declaration) else {
            return false;
        };
        let Some(initializer) = binding
            .initializer
            .map(|initializer| ast.peel_ts_sugar_span(initializer))
        else {
            return false;
        };
        if !binding.immutable {
            return false;
        }
        if ast
            .object_get_shapes
            .iter()
            .any(|shape| shape.span == initializer)
        {
            if !self.literal_plain(file, initializer) {
                return false;
            }
        } else if let Some(alias) = ast.reference_declaration(initializer) {
            // `const p = o`: the same object; `o`'s uses include this one, which walks `p`'s.
            if !self.named_object_confined(file, alias, seen) {
                return false;
            }
        } else {
            return false;
        }
        self.references_confined(file, declaration, seen)
    }

    /// Every use of the name declared at `declaration` (in the file, and in the files that import it) is one that makes the object neither a receiver nor reachable by code
    /// nothing here follows. An exported name is reached by the program's importers, which a program whose boundary is open does not enumerate.
    fn references_confined(
        &self,
        file: &'a FileFacts,
        declaration: Span,
        seen: &mut HashSet<(Arc<str>, Span)>,
    ) -> bool {
        let ast = &file.ast;
        if !self.lookup.program_closed && crate::indexes::binding_exported(file, declaration) {
            return false;
        }
        ast.references_to(declaration)
            .iter()
            .filter(|reference| {
                !ast.write_targets
                    .iter()
                    .any(|target| ast.peel_ts_sugar_span(*target) == **reference)
            })
            .all(|reference| self.use_confined(file, *reference, seen))
            && self
                .foreign_references(file, declaration)
                .into_iter()
                .all(|(holder, at, import)| {
                    if import {
                        self.references_confined(holder, at, seen)
                    } else {
                        self.use_confined(holder, at, seen)
                    }
                })
    }

    /// Whether one use of an object (a reference of the name that holds it) leaves it confined: see [`Self::holder_confined`].
    fn use_confined(
        &self,
        file: &'a FileFacts,
        reference: Span,
        seen: &mut HashSet<(Arc<str>, Span)>,
    ) -> bool {
        let ast = &file.ast;
        let reference = ast.peel_ts_sugar_span(reference);
        if let Some(member) = ast.members_with_peeled_object(reference).next() {
            let key = crate::installed_members::member_name(file, member);
            if ast
                .write_targets
                .iter()
                .any(|target| ast.peel_ts_sugar_span(*target) == member.span)
            {
                // A write sets a member, or the prototype when the key is `__proto__` (which a key the program does not name may be).
                return key.is_some_and(|key| key != "__proto__");
            }
            // A call binds `this` to the object: a callee that may name it reads members through it.
            return !self.method_call_may_read_receiver(file, member.span);
        }
        if ast
            .inert_operands
            .iter()
            .any(|operand| ast.peel_ts_sugar_span(*operand) == reference)
        {
            return true;
        }
        if ast
            .spreads
            .iter()
            .any(|spread| ast.peel_ts_sugar_span(spread.argument) == reference)
        {
            return true;
        }
        if let Some(binding) = ast.bindings.iter().find(|binding| {
            binding
                .initializer
                .is_some_and(|initializer| ast.peel_ts_sugar_span(initializer) == reference)
        }) {
            return match binding.shape {
                solid_facts::ast::BindingShape::Identifier => binding.names.iter().all(|name| {
                    !seen.insert((file.path.shared(), name.span))
                        || self.references_confined(file, name.span, seen)
                }),
                // A pattern reads the members it names (a rest copies them).
                _ => true,
            };
        }
        if let Some((call, index)) = ast.arguments_containing(reference).find(|(call, index)| {
            !call.arguments[*index].spread
                && ast.peel_ts_sugar_span(call.arguments[*index].span) == reference
        }) {
            if self.unread_argument(file, call, index, true) {
                return true;
            }
            if let Receiver::Parameter(callee_file, callee, parameter) =
                self.call_receiver(file, call, index)
            {
                return self.parameter_confined(callee_file, callee, parameter, seen);
            }
        }
        false
    }

    /// Whether the identifier at `identifier` is the library's global `name` (`WeakMap`, `Object`): the compiler resolves it, and every declaration it resolves to is the
    /// standard library's (a script of a package has no resolved call of its own: `requests.set` and `Object.defineProperty` there name no standard declaration, but the
    /// global they are read from does).
    fn is_library_global(&self, file: &FileFacts, identifier: Span, name: &str) -> bool {
        file.source_text(identifier) == Some(name)
            && self
                .lookup
                .entities()
                .value_symbol(file, identifier)
                .and_then(|symbol| self.lookup.symbol_declarations(symbol.as_str()))
                .is_some_and(|declarations| {
                    !declarations.is_empty()
                        && declarations
                            .iter()
                            .all(|declaration| declaration.location.path.starts_with("bundled:"))
                })
    }

    /// Whether the callee is a method of a weak collection the file makes itself (`held.delete`, `requests.set` where `held` is a `const` bound to `new WeakSet(…)` and
    /// `requests` to `new WeakMap(…)`, the constructors the library's, resolved by the compiler), for the script of a package whose calls resolve to no declaration.
    fn weak_collection_method(&self, file: &FileFacts, callee: Span) -> bool {
        let ast = &file.ast;
        let Some(member) = ast.member_at(ast.peel_ts_sugar_span(callee)) else {
            return false;
        };
        let Some(method) = crate::installed_members::member_name(file, member) else {
            return false;
        };
        let Some(declaration) = ast.reference_declaration(ast.peel_ts_sugar_span(member.object))
        else {
            return false;
        };
        let Some(binding) = ast.identifier_binding(declaration) else {
            return false;
        };
        let Some(constructed) = binding
            .initializer
            .filter(|_| binding.immutable)
            .and_then(|initializer| ast.call_at(ast.peel_ts_sugar_span(initializer)))
            .filter(|call| call.construct)
        else {
            return false;
        };
        let constructor = ast.peel_ts_sugar_span(constructed.callee);
        if self.is_library_global(file, constructor, "WeakMap") {
            matches!(method, "get" | "has" | "delete" | "set")
        } else if self.is_library_global(file, constructor, "WeakSet") {
            matches!(method, "has" | "delete" | "add")
        } else {
            false
        }
    }

    /// The method a callee names on the library's global `Object` (resolved by the compiler), for the script of a package whose calls resolve to no declaration.
    fn global_object_method<'f>(&self, file: &'f FileFacts, callee: Span) -> Option<&'f str> {
        let ast = &file.ast;
        let member = ast.member_at(ast.peel_ts_sugar_span(callee))?;
        self.is_library_global(file, ast.peel_ts_sugar_span(member.object), "Object")
            .then(|| crate::installed_members::member_name(file, member))
            .flatten()
    }

    /// Whether the argument at `index` of `call` is an object the call reads no member of, and gives no other name to: a key of a collection that compares its identity and never
    /// gives it back (the first argument of a call of `WeakMap`'s `get`, `has`, `delete` and `set`, of `WeakSet`'s `has`, `delete` and `add`, and of `Map`'s and `Set`'s `get`,
    /// `has` and `delete`: a `Map.set` and a `Set.add` keep it where an iteration gets it back), or the object `Object.isFrozen`, `isSealed`, `isExtensible`, `keys` and
    /// `getOwnPropertyNames` ask of (names and booleans, no member read). `Object.defineProperty`, `defineProperties`, `assign`, `freeze`, `seal` and `preventExtensions` read none
    /// either, but they return the object they are given, so the call is one only when its value is dropped (a statement); a prototype set (`Object.setPrototypeOf`) is the same, and it
    /// makes the object the derived one, which `confining` asks (a holder is no longer provably no receiver, nor is one a descriptor may give a getter of its own). The declaration is
    /// the standard library's, resolved by the compiler, or, in a script of a package whose calls resolve to no declaration, the library's global `Object` or a weak collection the file
    /// makes (the compiler resolves the global).
    fn unread_argument(
        &self,
        file: &'a FileFacts,
        call: &CallFact,
        index: usize,
        confining: bool,
    ) -> bool {
        if index != 0 {
            return false;
        }
        let ast = &file.ast;
        let dropped = !confining
            && ast
                .inert_operands
                .iter()
                .any(|operand| ast.peel_ts_sugar_span(*operand) == call.span);
        let resolved = self
            .lookup
            .resolved_callee_call(file, call.callee)
            .and_then(crate::runtime_semantics::standard_call_name);
        if let Some(name) = resolved {
            return match name {
                "WeakMap.get" | "WeakMap.has" | "WeakMap.delete" | "WeakMap.set"
                | "WeakSet.has" | "WeakSet.delete" | "WeakSet.add" | "Map.get" | "Map.has"
                | "Map.delete" | "Set.has" | "Set.delete" => true,
                "ObjectConstructor.isFrozen"
                | "ObjectConstructor.isSealed"
                | "ObjectConstructor.isExtensible"
                | "ObjectConstructor.getOwnPropertyNames"
                | "ObjectConstructor.keys" => true,
                "ObjectConstructor.defineProperty"
                | "ObjectConstructor.defineProperties"
                | "ObjectConstructor.freeze"
                | "ObjectConstructor.seal"
                | "ObjectConstructor.preventExtensions"
                | "ObjectConstructor.assign"
                | "ObjectConstructor.setPrototypeOf" => dropped,
                _ => false,
            };
        }
        self.weak_collection_method(file, call.callee)
            || self
                .global_object_method(file, call.callee)
                .is_some_and(|method| match method {
                    "isFrozen" | "isSealed" | "isExtensible" | "getOwnPropertyNames" | "keys" => {
                        true
                    }
                    "defineProperty" | "defineProperties" | "freeze" | "seal"
                    | "preventExtensions" | "assign" | "setPrototypeOf" => dropped,
                    _ => false,
                })
    }

    /// [`Self::holder_confined`] of an object passed as the argument at `index` of a function the build reads: the uses of the parameter.
    fn parameter_confined(
        &self,
        file: &'a FileFacts,
        function: &'a FunctionFact,
        index: usize,
        seen: &mut HashSet<(Arc<str>, Span)>,
    ) -> bool {
        if names_arguments(file, function) {
            return false;
        }
        let names: Vec<Span> = match function.parameters.get(index) {
            Some(parameter) => parameter.names.iter().map(|name| name.span).collect(),
            None if function.rest_parameter => function
                .rest_parameter_names
                .iter()
                .map(|name| name.span)
                .collect(),
            None => return function.kind == FunctionKind::Arrow,
        };
        names.into_iter().all(|name| {
            !seen.insert((file.path.shared(), name)) || self.references_confined(file, name, seen)
        })
    }

    /// The key a property of an object literal stores its value under: its name, a quoted name's text, a unique symbol (`symbol_key`), or `None` for a key the program does not
    /// name (a computed key that is no plain string literal).
    fn property_key(
        &self,
        file: &FileFacts,
        property: &solid_facts::ast::ObjectPropertyFact,
    ) -> Option<String> {
        key_of_text(file.source_text(property.key)?, property.computed).or_else(|| {
            property
                .computed
                .then(|| self.symbol_key(file, property.key))
                .flatten()
        })
    }

    /// The key a member reads or stores under: its name, a computed member's literal key, or a unique symbol the file makes (`[_m$]` where `var _m$ = Symbol()`).
    fn member_key(
        &self,
        file: &FileFacts,
        member: &solid_facts::ast::MemberFact,
    ) -> Option<String> {
        crate::installed_members::member_name(file, member)
            .map(str::to_owned)
            .or_else(|| {
                file.ast
                    .computed_members
                    .binary_search(&member.span)
                    .is_ok()
                    .then(|| self.symbol_key(file, member.property))
                    .flatten()
            })
    }

    /// The key of a computed expression that names a unique symbol: a reference to a name the file binds once, to a call of the library's global `Symbol` (the compiler resolves
    /// it). No other key is that symbol (a string name is not, and two calls of `Symbol` are two symbols), so it is read and stored under that name alone; an imported symbol, and
    /// a symbol any other way got, name nothing.
    fn symbol_key(&self, file: &FileFacts, expression: Span) -> Option<String> {
        let ast = &file.ast;
        let declaration = ast.reference_declaration(ast.peel_ts_sugar_span(expression))?;
        let binding = ast.identifier_binding(declaration)?;
        // Bound once and never written again (a compiled script declares it with `var`).
        if ast.references_to(declaration).iter().any(|reference| {
            ast.write_targets
                .iter()
                .any(|target| ast.peel_ts_sugar_span(*target) == *reference)
        }) {
            return None;
        }
        let call = ast.call_at(ast.peel_ts_sugar_span(binding.initializer?))?;
        let callee = ast.peel_ts_sugar_span(call.callee);
        (!call.construct && self.is_library_global(file, callee, "Symbol"))
            .then(|| format!("\0symbol {} {}", file.path.as_str(), declaration.start))
    }

    /// Whether the member at `member` is called as a method (`o.m(…)`, `o.m\`…\``, `o.m?.()`; not `new o.m()`): the call binds `this` to the object.
    fn called_as_method(&self, file: &FileFacts, member: Span) -> bool {
        let ast = &file.ast;
        ast.calls_with_peeled_callee(member)
            .next()
            .is_some_and(|call| !call.construct)
            || ast
                .tagged_templates
                .iter()
                .any(|template| ast.peel_ts_sugar_span(template.tag) == member)
    }

    /// Whether the member at `member`, read from an object of accessors or of functions, is called as a method (`o.m(…)`, `o.m\`…\``, `o.m?.()`), which makes the object the
    /// callee's `this`, and the callee is not proved to be a function that never names `this` or `super`: a method written outside the literal (`{ m }`, `m: m`, an import), one
    /// the program replaces, a prop of a component, a parameter, a call's result, an alias, an array's element: anything the call graph does not resolve to exactly one
    /// function. `this.g` or `this.m()` in such a method reads a member through no reference of the object (the review of #226's re-check, N1; of #229, B1: a name key
    /// that withheld the call where nothing resolved the callee certified four shapes Chrome reports).
    fn method_call_may_read_receiver(&self, file: &'a FileFacts, member: Span) -> bool {
        if !self.called_as_method(file, member) {
            return false;
        }
        // A callee no fact resolves to exactly one function is handed.
        let Some((callee_file, callee)) = self.callback_target(file, member) else {
            return true;
        };
        self.function_names_receiver(callee_file, callee)
    }

    /// `question` asked of an object of accessors (the props of `owner`, or an object literal), on the walk that asks: its visited positions and functions are
    /// the same, so a cycle through the object (a getter that names its own object, a component that renders itself) ends (the review of #226, B1).
    fn as_accessors(
        &self,
        walk: &mut Walk,
        owner: Option<Key>,
        question: impl FnOnce(&mut Walk) -> bool,
    ) -> bool {
        let saved = (
            walk.accessor,
            std::mem::replace(&mut walk.accessor_owner, owner),
        );
        walk.accessor = true;
        let answer = question(walk);
        walk.accessor = saved.0;
        walk.accessor_owner = saved.1;
        answer
    }

    /// Whether a function that owns `at` (a call, a tag, a carrier) may be run by a package: what is called there runs on its stack.
    fn runs_where_written(&self, file: &'a FileFacts, at: Span, walk: &mut Walk) -> bool {
        file.ast
            .functions_owning(at)
            .any(|owner| self.function_in(file, owner, walk))
            || self.evaluated_by_element(file, at, walk)
    }

    /// Whether the element whose prop, spread or child expression holds `at` (the innermost one, with no function between) may evaluate it on a package's stack. The
    /// compiler makes such an expression a getter of a component's props, so it runs when the component reads the prop, not where it is written: an unread package's
    /// component may read it anywhere (`<Pkg>{helper()}</Pkg>`, `<Pkg>{list.map(() => <Own />)}</Pkg>`, `<Pkg render={make()} />`), a project component where its
    /// first parameter goes, a dialect's where the element is. An intrinsic element evaluates it where the element is created, and so does any element besides (the
    /// review of #222, N1).
    fn evaluated_by_element(&self, file: &'a FileFacts, at: Span, walk: &mut Walk) -> bool {
        let ast = &file.ast;
        let Some((index, element)) = ast
            .jsx_elements
            .iter()
            .enumerate()
            .filter(|(_, element)| {
                element.span != at
                    && element.span.contains(at)
                    && !ast
                        .functions_containing(at)
                        .any(|function| element.span.contains(function.span))
            })
            .min_by_key(|(_, element)| element.span.end - element.span.start)
        else {
            return false;
        };
        // The tag names the component: not an expression the element evaluates.
        if element.name.span.contains(at)
            || element.closing_name.is_some_and(|name| name.contains(at))
        {
            return false;
        }
        if self.element_created_in(file, element.span, walk) {
            return true;
        }
        if is_intrinsic_tag(file, element) || self.dynamic_string_tag(file, element) {
            return false;
        }
        let receiver = self.element_receiver(file, index, element);
        if self.receiver_hands(file, receiver, walk) {
            return true;
        }
        // A project component reads its props wherever it reads them: the expression is a getter of the props object, which runs at the read (`runStep(() => {
        // void props.children; })` in the component, or a closure handed to a package's component). Its first parameter is followed as an object of accessors.
        // An unread package's component was answered by `receiver_hands` (handed); a known receiver or a dialect's element reads no props of a project function.
        if let Receiver::Parameter(component_file, component, parameter) = receiver {
            let owner = Some((component_file.path.shared(), component.span));
            return self.as_accessors(walk, owner, |walk| {
                self.parameter(component_file, component, parameter, walk)
            });
        }
        false
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
            // The call binds `this` to the holder, and a callee that names `this` or `super`, or that nothing resolves to one function (a parameter, a call's result, an
            // alias, an array's element: the review of #229, B1), may run its members through no reference of the holder (the review of #226's re-check, N1, for a holder of
            // accessors, here for a holder of functions).
            self.runs_where_written(file, call.span, walk)
                || self
                    .method_call_may_read_receiver(file, file.ast.peel_ts_sugar_span(call.callee))
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
        imported
            || self.binding(file, declaration, walk)
            || self.parameter_holder_hands(file, declaration, walk)
    }

    /// A store into a member of a parameter (`t.m = f` in `function put(t) { … }`) stores into the object the callers pass, whose reads the walk of the parameter's own does not
    /// reach (`put(o)`, then `runStep(() => { o.m(); })`): the holder is each caller's argument, followed like a store into it. Every use of the function must be a direct call
    /// the build resolves (a use that is no call, an export a program may enter, a function nothing names, a parameter that is a pattern or a rest: the callers are not known,
    /// and the store is handed). An argument that is a fresh object literal is reached through the parameter only.
    fn parameter_holder_hands(
        &self,
        file: &'a FileFacts,
        declaration: Span,
        walk: &mut Walk,
    ) -> bool {
        let ast = &file.ast;
        let plain = ast.functions.iter().find_map(|function| {
            function
                .parameters
                .iter()
                .position(|parameter| {
                    parameter.shape == solid_facts::ast::BindingShape::Identifier
                        && parameter.names.len() == 1
                        && parameter.names[0].span == declaration
                })
                .map(|index| (function, index))
        });
        let Some((function, index)) = plain else {
            // A parameter that is a pattern or a rest is no name of the argument; a name that is no parameter is a local, which `binding` followed.
            return ast.functions.iter().any(|function| {
                function
                    .parameters
                    .iter()
                    .any(|parameter| parameter.names.iter().any(|name| name.span == declaration))
                    || function
                        .rest_parameter_names
                        .iter()
                        .any(|name| name.span == declaration)
            });
        };
        let (accessor, owner) = walk.mode();
        if !walk
            .positions
            .insert(((file.path.shared(), declaration), accessor, owner))
        {
            return false;
        }
        let Some(callers) = self.exact_callers(file, function) else {
            return true;
        };
        callers.into_iter().any(|(caller_file, call)| {
            let Some(argument) = call.arguments.get(index) else {
                return false;
            };
            if argument.spread {
                return true;
            }
            let argument = caller_file.ast.peel_ts_sugar_span(argument.span);
            if caller_file
                .ast
                .object_get_shapes
                .iter()
                .any(|shape| shape.span == argument)
            {
                return false;
            }
            self.assignment_hands(caller_file, argument, walk)
        })
    }

    /// The calls that run `function`, when every use of it is a direct call; `None` when anything else may call it: a use that is no call (a copy, an argument, a tag), an
    /// export a program may enter, a function no binding names.
    fn exact_callers(
        &self,
        file: &'a FileFacts,
        function: &'a FunctionFact,
    ) -> Option<Vec<(&'a FileFacts, &'a CallFact)>> {
        if self.exported(file, function)
            && (!self.lookup.program_closed || self.lookup.escapes_unnamed(file.path.as_str()))
        {
            return None;
        }
        let placed =
            crate::local_access::function_reference_hand_offs(file, function, self.lookup)?;
        let foreign = crate::local_access::foreign_reference_hand_offs(file, function, self.lookup);
        if placed
            .iter()
            .any(|(_, hand_off)| *hand_off != HandOff::Call)
            || foreign
                .iter()
                .any(|(_, _, hand_off)| *hand_off != HandOff::Call)
        {
            return None;
        }
        let mut callers = Vec::new();
        for (at, _) in placed {
            callers.push((file, file.ast.calls_with_peeled_callee(at).next()?));
        }
        for (holder, at, _) in foreign {
            callers.push((holder, holder.ast.calls_with_peeled_callee(at).next()?));
        }
        Some(callers)
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
        let (accessor, owner) = walk.mode();
        if !walk.returned_positions.insert((
            (target.clone(), (file.path.shared(), at)),
            accessor,
            owner,
        )) || ast.type_queries.iter().any(|query| query.contains(at))
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
        let (accessor, owner) = walk.mode();
        if !walk.returned_bindings.insert((
            (target.clone(), (file.path.shared(), name)),
            accessor,
            owner,
        )) {
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

/// The properties written directly in the object literal at `literal` (not those of a literal nested in one of its values).
fn direct_properties(
    ast: &AstFacts,
    literal: Span,
) -> impl Iterator<Item = &solid_facts::ast::ObjectPropertyFact> {
    ast.object_properties.iter().filter(move |property| {
        literal.contains(property.span)
            && property.span != literal
            && !ast.object_properties.iter().any(|outer| {
                outer.span != property.span
                    && outer.span.contains(property.span)
                    && literal.contains(outer.span)
            })
    })
}

/// The key a property's key text names: `m`, `"m"` and `["m"]` name `m`, `[m]` and `[\`m\`]` name nothing the program fixes, and a quoted text with an escape is no plain name.
fn key_of_text(text: &str, computed: bool) -> Option<String> {
    let quoted = ["\"", "'"].into_iter().find_map(|quote| {
        text.strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
            .filter(|inner| !inner.contains(['\\', '"', '\'']))
    });
    match (computed, quoted) {
        (_, Some(inner)) => Some(inner.to_owned()),
        (false, None) if !text.starts_with(['"', '\'']) => Some(text.to_owned()),
        _ => None,
    }
}

/// Whether the receiver at `receiver` is the object of a private member (`this.#k`, `this?.#k`): a slot only the class's own code names, which no store from outside reaches.
fn private_member_of(file: &FileFacts, receiver: Span) -> bool {
    file.source
        .get(receiver.end as usize..)
        .is_some_and(private_member_follows)
}

/// Whether the text after a receiver reads a private member (`.#k`, `?.#k`).
fn private_member_follows(rest: &str) -> bool {
    let rest = rest.trim_start();
    rest.starts_with(".#") || rest.starts_with("?.#")
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

#[cfg(test)]
mod tests {
    use super::{key_of_text, private_member_follows};

    #[test]
    fn a_property_key_is_a_name_or_a_plain_string() {
        for (text, computed, key) in [
            ("m", false, Some("m")),
            ("\"m\"", false, Some("m")),
            ("'m'", false, Some("m")),
            ("0", false, Some("0")),
            ("\"m\"", true, Some("m")),
            ("'a b'", true, Some("a b")),
            ("m", true, None),
            ("k + \"x\"", true, None),
            ("`m`", true, None),
            ("\"a\\\"b\"", false, None),
            ("\"a'b\"", false, None),
            ("\"\\u006d\"", true, None),
        ] {
            assert_eq!(
                key_of_text(text, computed).as_deref(),
                key,
                "{text} (computed: {computed})"
            );
        }
    }

    #[test]
    fn a_private_member_is_told_from_a_public_one() {
        for (rest, private) in [
            (".#k", true),
            (" .#k", true),
            ("?.#k", true),
            (".k", false),
            ("?.k", false),
            ("[\"#k\"]", false),
            (" as any).#k", false),
            ("", false),
        ] {
            assert_eq!(private_member_follows(rest), private, "{rest:?}");
        }
    }
}
