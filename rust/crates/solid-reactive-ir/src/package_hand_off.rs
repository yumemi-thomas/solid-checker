//! Batch 11, slice 5 follow-up (the review of #213, B3; ADR 0967): whether a package the build does not read as source may run a function of the project.
//!
//! The flush accounting's program censuses (`flush_accounting::FlushSafe::NoSteppedAction` and its two siblings) read the primitives the build's own table names. An
//! installed package whose source the build does not read can run a function the project hands it as an action's step, an effect's apply or a leaf callback (the
//! package calls the primitive itself), and nothing in the table shows it. The owner's ruling (option b, fail closed, no premise): a flush in a function such a package may
//! run gets no census reason, so it is uncertifiable, never silent.
//!
//! A package runs a function it was handed, and a function that function calls. The hand-off routes followed here, each by the facts of the program and never by a spelling:
//!
//! - the function written as an argument of a call (or of a `new`, or a tagged template's substitution), or inside one: a member of an object literal, an element of an array
//!   literal, a branch of a conditional, a spread, whatever the argument builds (`pkg.run({ done: () => flush() })`);
//! - a reference to its binding (a function declaration, a `const` arrow, a named function expression) written as such an argument or inside one, through every `const`
//!   copy, assignment and holder (`const holder = { run }`, `let kept; kept = run`), in the file or, through an import, in another;
//! - a JSX prop, a spread attribute or a child expression of a component the program does not read (`<Pkg render={() => flush()} />`, `<Pkg>{() => flush()}</Pkg>`),
//!   and a `use:` directive;
//! - a project function's parameter that goes on to any of these (`function wrap(f) { pkg.run(f) }`);
//! - a caller: a function whose exact call edge reaches the function (a helper the handed callback calls), asked the same way.
//!
//! What receives the value decides ([`Receiver`]): a dialect primitive, a reviewed test runner's registration (by the callee's identity, whatever the project proves of when
//! the runner runs a suite body: `reviewed_test_runner_registration`), the standard library, a package the build reads as source, a DOM element and a project function
//! whose parameter is followed know their behavior; a declaration in a package the build does not read, a callee nothing resolves and a
//! project slot no function body defines (a function-typed parameter or field: it may hold a package's function) do not.
//!
//! Not followed, named in `docs/open-work.md`: a value a project function returns (`pkg.run(makeCallback())`), a method of a class whose instance is handed, an assignment into
//! an object a package exports, a project component's props, and an export a framework enters from outside.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use solid_facts::FileFacts;
use solid_facts::ast::{AstFacts, CallFact, FunctionFact, JsxAttributeFact, JsxElementFact};
use solid_facts::core::Span;

use crate::indexes::SemanticLookup;
use crate::local_access::HandOff;

/// What receives a function value at a hand-off position.
enum Receiver<'a> {
    /// Code whose behavior the program knows: it runs no action's step, effect's apply or leaf callback of its own.
    Known,
    /// A project function: the value goes on in the parameter at the index (a project function's body is analyzed).
    Parameter(&'a FileFacts, &'a FunctionFact, usize),
    /// Code nothing here reads: a declaration in a package the build does not read as source, a callee nothing resolves, a slot no function body defines.
    Unread,
}

type Key = (Arc<str>, Span);

/// What one question has visited, so that a cycle of copies or calls ends.
#[derive(Default)]
struct Walk {
    positions: HashSet<Key>,
    functions: HashSet<Key>,
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
        let mut positions: Vec<(&'a FileFacts, Span)> = vec![(file, function.span)];
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

    /// Whether the value written (or named) at `at` reaches a receiver nothing here reads: as an argument, a prop, a child, a substitution, or in the holder (a `const`,
    /// an assignment, an object or array literal) one of those hands on.
    fn position(&self, file: &'a FileFacts, at: Span, walk: &mut Walk) -> bool {
        let ast = &file.ast;
        let at = ast.peel_ts_sugar_span(at);
        if !walk.positions.insert((file.path.shared(), at)) {
            return false;
        }
        // The value is called, not handed: `f()`, `(() => …)()`, a tag.
        if ast.calls_with_peeled_callee(at).next().is_some()
            || ast
                .tagged_templates
                .iter()
                .any(|template| template.tag == at)
        {
            return false;
        }
        for (call, index) in ast.arguments_containing(at) {
            if crosses(ast, call.arguments[index].span, at) {
                continue;
            }
            match self.call_receiver(file, call, index) {
                Receiver::Unread => return true,
                Receiver::Parameter(callee_file, callee, parameter) => {
                    if self.parameter(callee_file, callee, parameter, walk) {
                        return true;
                    }
                }
                Receiver::Known => {}
            }
        }
        for template in &ast.tagged_templates {
            if template
                .substitutions
                .iter()
                .any(|substitution| substitution.contains(at) && !crosses(ast, *substitution, at))
                && matches!(self.tag_receiver(file, template.tag), Receiver::Unread)
            {
                return true;
            }
        }
        for (index, element) in ast.jsx_elements.iter().enumerate() {
            if element.span == at || !element.span.contains(at) {
                continue;
            }
            if self.element_hands(file, index, element, at) {
                return true;
            }
        }
        for binding in &ast.bindings {
            let held = binding.initializer.is_some_and(|initializer| {
                initializer.contains(at) && !crosses(ast, initializer, at)
            });
            if held
                && binding
                    .names
                    .iter()
                    .any(|name| self.binding(file, name.span, walk))
            {
                return true;
            }
        }
        for assignment in &ast.assignments {
            let value = ast.peel_ts_sugar_span(assignment.value_span);
            if !value.contains(at) || crosses(ast, value, at) {
                continue;
            }
            // `kept = f` hands it to every reference of `kept`; `holder.slot = f`, to every reference of `holder`.
            let target = root_identifier(ast, assignment.target);
            if target
                .and_then(|target| ast.reference_declaration(target))
                .is_some_and(|declaration| self.binding(file, declaration, walk))
            {
                return true;
            }
        }
        false
    }

    /// Every read of the binding named at `name`: in the file, and in the files that import it.
    fn binding(&self, file: &'a FileFacts, name: Span, walk: &mut Walk) -> bool {
        let ast = &file.ast;
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

    /// The parameter at `index` of a project function the value is passed to: every read of the names it binds (a rest parameter takes the tail).
    fn parameter(
        &self,
        file: &'a FileFacts,
        function: &'a FunctionFact,
        index: usize,
        walk: &mut Walk,
    ) -> bool {
        let names: Vec<Span> = match function.parameters.get(index) {
            Some(parameter) => parameter.names.iter().map(|name| name.span).collect(),
            None if function.rest_parameter => function
                .rest_parameter_names
                .iter()
                .map(|name| name.span)
                .collect(),
            None => Vec::new(),
        };
        names.into_iter().any(|name| self.binding(file, name, walk))
    }

    /// What receives argument `index` of `call`.
    fn call_receiver(&self, file: &'a FileFacts, call: &CallFact, index: usize) -> Receiver<'a> {
        let lookup = self.lookup;
        if lookup.primitive_at_call(file, call.span).is_some()
            || lookup.reviewed_test_runner_registration(file, call)
        {
            return Receiver::Known;
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
        self.declaration_receiver(lookup.resolved_callee_call(file, call.callee))
    }

    /// The receiver a resolved call's declaration names: the standard library and a package the build reads as source are known; a declaration in a package it does not
    /// read, a project declaration no function body defines (a function-typed parameter or field) and a call nothing resolves are not.
    fn declaration_receiver(&self, resolved: Option<&typefacts::ResolvedCall>) -> Receiver<'a> {
        let Some(declaration) = resolved.and_then(|resolved| resolved.declaration.as_ref()) else {
            return Receiver::Unread;
        };
        if declaration.standard_library
            || self
                .lookup
                .in_dependency_source(declaration.location.path.as_ref())
        {
            Receiver::Known
        } else {
            Receiver::Unread
        }
    }

    /// What a tagged template's tag receives its substitutions as: the same declaration question as a call's.
    fn tag_receiver(&self, file: &'a FileFacts, tag: Span) -> Receiver<'a> {
        self.declaration_receiver(self.lookup.resolved_callee_call(file, tag))
    }

    /// Whether the element hands the value at `at` (a prop's value, a spread's argument, a child expression, a directive's value) to a component nothing here reads.
    fn element_hands(
        &self,
        file: &'a FileFacts,
        index: usize,
        element: &JsxElementFact,
        at: Span,
    ) -> bool {
        let ast = &file.ast;
        let holds = |outer: Span| outer.contains(at) && !crosses(ast, outer, at);
        let attribute_holds = |attribute: &JsxAttributeFact| {
            attribute.value.or(attribute.expression).is_some_and(holds)
        };
        // A `use:name` directive is the one namespaced attribute whose local name the binder resolves to a declaration (`directive_binding`).
        let directive = |attribute: &&JsxAttributeFact| attribute.directive_binding.is_some();
        // A `use:` directive is a call of the function the name resolves to, with the attribute's value as its accessor.
        if element
            .attributes
            .iter()
            .filter(directive)
            .any(|attribute| attribute_holds(attribute) && self.directive_unread(file, attribute))
        {
            return true;
        }
        let component_gets_it = element
            .attributes
            .iter()
            .filter(|attribute| !directive(attribute))
            .any(attribute_holds)
            || element.spreads.iter().any(|spread| holds(spread.argument))
            || element.children.iter().any(|child| holds(*child));
        component_gets_it
            && matches!(
                self.element_receiver(file, index, element),
                Receiver::Unread
            )
    }

    /// A `use:name` directive whose function is no project function: the declaration the binder resolved its name to (`directive_binding`) is no project function's
    /// binding (a package's import, or nothing the program declares).
    fn directive_unread(&self, file: &'a FileFacts, attribute: &JsxAttributeFact) -> bool {
        attribute
            .directive_binding
            .and_then(|declaration| self.lookup.entities().at(file.path.as_str(), declaration))
            .and_then(|symbol| self.lookup.function_for_symbol(symbol.as_str()))
            .is_none()
    }

    /// What a JSX element's props, children and spreads are handed to: an intrinsic element is the DOM's (its handlers run on the browser's own stack); a dialect export is
    /// known; a project function's parameter is the props object (not followed, named in the open work); a component declared in a package the build reads as source is
    /// known; any other is not.
    fn element_receiver(
        &self,
        file: &'a FileFacts,
        index: usize,
        element: &JsxElementFact,
    ) -> Receiver<'a> {
        let lookup = self.lookup;
        let name = file.source_text(element.name.span).unwrap_or_default();
        // The language's own rule of a tag: a plain name that starts with a lowercase letter is an intrinsic element, anything else is a value.
        if element.member_object.is_none()
            && name
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_lowercase())
        {
            return Receiver::Known;
        }
        if lookup
            .primitives(file)
            .jsx
            .get(index)
            .is_some_and(Option::is_some)
        {
            return Receiver::Known;
        }
        let Some(symbol) = lookup.entities().value_symbol(file, element.name.span) else {
            return Receiver::Unread;
        };
        if lookup.function_for_symbol(symbol.as_str()).is_some() {
            return Receiver::Known;
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

/// The identifier a write target is rooted at: `kept` in `kept = f`, `holder` in `holder.slot.inner = f`.
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
