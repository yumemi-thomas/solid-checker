//! ADR 0980: whether the name a function is stored through is the only name of the object (alias tracking of stored functions).
//!
//! A store `X.m = function () { flush(); }` is followed in every read of the name `X`. A name is not the object: `const a = o; a.m = f` stores into the object `o` names, and
//! `o` is never read (the review of #232, B6 and S2). The store is followed only when the holder is **sole-named**, and any other store is handed:
//!
//! - a local: a `const`, or a `let` or `var` nothing reassigns, bound to a fresh object (an object literal whose keys are all spelled and none `__proto__`, an array literal, or an instance of a class the
//!   project declares in this file that has no constructor, no superclass and no `this`), declared in no export, with every reference of the name the object of a member
//!   access (a write through any key; a read of a key the program spells, not `valueOf`, which returns the object), an operand nothing takes (`typeof o`, `o === x`) or a type query;
//! - a parameter (`function put(t) { t.m = f; }`): a plain identifier no assignment reassigns, whose references are all of that kind. The callers are the next question
//!   ([`UnreadPackageHandOff::exact_callers`] and `parameter_holder_hands`: each argument is a sole-named name except for the one call).
//!
//! Anything that copies the object (an initializer or an assignment's value, an argument, an element or a property's value, a `return`, an iterable, a spread, a
//! destructuring source, an export, a getter's return, a `Proxy`, a comma, conditional or nullish expression) is a use that is none of these, so the store is handed. A holder
//! whose own initializer is another name (`const a = o`, `hh.o`, `id(o)`, `const [x] = [o]`, a loop variable, a destructured binding) is handed too: its initializer is no
//! fresh object. And only a store into a name directly (`a.m = f`, `a["m"] = f`) is followed: `box.o.m = f` stores into the object `box.o` names, which is no name.

use super::*;

impl<'l, 'a> UnreadPackageHandOff<'l, 'a> {
    /// Whether the name declared at `declaration` is the only name of the object it holds, except for the references at `allowed` (the one call that passes it on).
    pub(super) fn sole_named(
        &self,
        file: &'a FileFacts,
        declaration: Span,
        allowed: &[Span],
    ) -> bool {
        let ast = &file.ast;
        let parameter = ast.functions.iter().any(|function| {
            function
                .parameters
                .iter()
                .any(|parameter| parameter.names.iter().any(|name| name.span == declaration))
                || function
                    .rest_parameter_names
                    .iter()
                    .any(|name| name.span == declaration)
        });
        if parameter {
            let plain = ast.functions.iter().any(|function| {
                function.parameters.iter().any(|parameter| {
                    parameter.shape == solid_facts::ast::BindingShape::Identifier
                        && parameter.names.len() == 1
                        && parameter.names[0].span == declaration
                })
            });
            return plain && self.only_member_uses(file, declaration, allowed);
        }
        let Some(binding) = ast.identifier_binding(declaration) else {
            return false;
        };
        if binding.shape != solid_facts::ast::BindingShape::Identifier || binding.names.len() != 1 {
            return false;
        }
        let Some(initializer) = binding
            .initializer
            .map(|initializer| ast.peel_ts_sugar_span(initializer))
        else {
            return false;
        };
        if crate::indexes::binding_exported(file, declaration) {
            return false;
        }
        let fresh = match binding.initializer_value_kind {
            solid_facts::ast::RuntimeValueKind::Object => {
                ast.object_get_shapes
                    .iter()
                    .any(|shape| shape.span == initializer)
                    // Every key is spelled and none is `__proto__` (a computed key may be one: the prototype's setter would take the store). A receiver the literal
                    // writes (`this` in a method) is the receiver machinery's: a store under a key a receiver reads is handed there (`placed_under_read_key`).
                    && direct_properties(ast, initializer).all(|property| {
                        self.property_key(file, property)
                            .is_some_and(|key| key != "__proto__")
                    })
            }
            solid_facts::ast::RuntimeValueKind::Array => ast.array_literals.contains(&initializer),
            _ => self.fresh_instance(file, initializer),
        };
        fresh && self.only_member_uses(file, declaration, allowed)
    }

    /// An instance of a class this file declares that no code of its own can leak: no superclass, no constructor (which may return another object), no `this` or `super`
    /// anywhere in its text.
    fn fresh_instance(&self, file: &'a FileFacts, initializer: Span) -> bool {
        let ast = &file.ast;
        let Some(call) = ast.call_at(initializer) else {
            return false;
        };
        if !call.construct || !call.arguments.is_empty() {
            return false;
        }
        let callee = ast.peel_ts_sugar_span(call.callee);
        let Some(class) = ast.reference_declaration(callee).and_then(|declaration| {
            ast.classes.iter().find(|class| {
                class
                    .name
                    .as_ref()
                    .is_some_and(|name| name.span == declaration)
            })
        }) else {
            return false;
        };
        if class.heritage.is_some()
            || class
                .elements
                .iter()
                .any(|element| element.kind == solid_facts::ast::ClassElementKind::Constructor)
        {
            return false;
        }
        let Some(receivers) = self.receiver_spans(file) else {
            return false;
        };
        !receivers.iter().any(|span| class.span.contains(*span))
    }

    /// Whether every reference of the name declared at `declaration` (other than `allowed`) is the object of a member access, an operand nothing takes or a type query,
    /// and none writes the name.
    fn only_member_uses(&self, file: &'a FileFacts, declaration: Span, allowed: &[Span]) -> bool {
        let ast = &file.ast;
        ast.references_to(declaration).iter().all(|reference| {
            let at = ast.peel_ts_sugar_span(*reference);
            if allowed.contains(reference) || allowed.contains(&at) {
                return true;
            }
            if ast
                .write_targets
                .iter()
                .any(|target| ast.peel_ts_sugar_span(*target) == at)
            {
                return false;
            }
            if ast.type_queries.iter().any(|query| query.contains(at)) {
                return true;
            }
            if let Some(member) = ast.members_with_peeled_object(at).next() {
                // A write through any key reads nothing out; a read gives out a member (the object itself for `valueOf`, and for a key the program does not spell).
                return ast
                    .write_targets
                    .iter()
                    .any(|target| ast.peel_ts_sugar_span(*target) == member.span)
                    || member_spelled_key(file, member).is_some_and(|key| key != "valueOf");
            }
            ast.inert_operands
                .iter()
                .any(|operand| ast.peel_ts_sugar_span(*operand) == at)
        })
    }
}
