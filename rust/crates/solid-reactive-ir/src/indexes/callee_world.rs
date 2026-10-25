//! The callee world: what resolving a call's project callee asks of the
//! program (ADR 0278, 0279, 0299, 0314).
//!
//! [`crate::execution_role::project_callee_in`] and the checks under it are
//! written once, over this trait. The lookup answers from its tables; the
//! invocation-flow memos answer each question with a recorded read
//! (`execution_role/flow_memos.rs`), so a flow answer that resolves a callee
//! no longer reads the whole program.

use super::*;
use solid_facts::ast::{BindingFact, ClassFact, FunctionFact};

/// The program as callee resolution reads it. Every file handed in is one the
/// world handed out (or the asking computation's own).
pub(crate) trait CalleeWorld<'a> {
    /// ADR 0299: a literal receiver's member call that no object-member target
    /// resolves ([`crate::object_members::call_withheld`]).
    fn call_withheld(&self, file: &FileFacts, callee: Span) -> bool;
    /// The function an object-member call resolves to
    /// ([`SemanticLookup::object_member_function`]).
    fn object_member_function(
        &self,
        file: &FileFacts,
        callee: Span,
    ) -> Option<(&'a FileFacts, &'a FunctionFact)>;
    /// [`SemanticLookup::callee_symbol`].
    fn callee_symbol(&self, file: &FileFacts, callee: Span) -> Option<&'a str>;
    /// [`SemanticLookup::function_for_symbol`].
    fn function_for_symbol(&self, symbol: &str) -> Option<(&'a FileFacts, &'a FunctionFact)>;
    /// [`SemanticLookup::function_called_at`].
    fn function_called_at(
        &self,
        file: &FileFacts,
        callee: Span,
    ) -> Option<(&'a FileFacts, &'a FunctionFact)>;
    /// The entity symbol at exactly `span` of `file` (`EntitySymbols::at`).
    fn entity_symbol_at(&self, file: &FileFacts, span: Span) -> Option<&'a SymbolId>;
    /// [`SemanticLookup::binding_at_reference`], without the symbol.
    fn binding_at_reference(
        &self,
        file: &FileFacts,
        span: Span,
    ) -> Option<(&'a FileFacts, &'a BindingFact)>;
    /// [`SemanticLookup::class_for_symbol`].
    fn class_for_symbol(&self, symbol: &str) -> Option<(&'a FileFacts, &'a ClassFact)>;
    /// ADR 0209: some assignment writes a member named `name`.
    fn member_name_assigned(&self, name: &str) -> bool;
    /// ADR 0314: a write through an unproven prototype, or through a host
    /// prototype not every declaration of which is in `lib.dom.d.ts`.
    fn builtin_prototype_written(&self) -> bool;
    /// Some project constructor's prototype is written.
    fn constructor_prototypes_written(&self) -> bool;
    /// The prototype of the project constructor declared at `declaration` of
    /// `path` is written.
    fn constructor_prototype_written(&self, path: &str, declaration: Span) -> bool;
}

impl<'a> CalleeWorld<'a> for SemanticLookup<'a> {
    fn call_withheld(&self, file: &FileFacts, callee: Span) -> bool {
        crate::object_members::call_withheld(file, callee, self)
    }

    fn object_member_function(
        &self,
        file: &FileFacts,
        callee: Span,
    ) -> Option<(&'a FileFacts, &'a FunctionFact)> {
        SemanticLookup::object_member_function(self, file, callee)
    }

    fn callee_symbol(&self, file: &FileFacts, callee: Span) -> Option<&'a str> {
        SemanticLookup::callee_symbol(self, file, callee)
    }

    fn function_for_symbol(&self, symbol: &str) -> Option<(&'a FileFacts, &'a FunctionFact)> {
        SemanticLookup::function_for_symbol(self, symbol)
    }

    fn function_called_at(
        &self,
        file: &FileFacts,
        callee: Span,
    ) -> Option<(&'a FileFacts, &'a FunctionFact)> {
        SemanticLookup::function_called_at(self, file.path.as_str(), callee)
    }

    fn entity_symbol_at(&self, file: &FileFacts, span: Span) -> Option<&'a SymbolId> {
        self.entities.at(file.path.as_str(), span)
    }

    fn binding_at_reference(
        &self,
        file: &FileFacts,
        span: Span,
    ) -> Option<(&'a FileFacts, &'a BindingFact)> {
        SemanticLookup::binding_at_reference(self, file.path.as_str(), span)
            .map(|(file, binding, _)| (file, binding))
    }

    fn class_for_symbol(&self, symbol: &str) -> Option<(&'a FileFacts, &'a ClassFact)> {
        SemanticLookup::class_for_symbol(self, symbol)
    }

    fn member_name_assigned(&self, name: &str) -> bool {
        self.assigned_members().names.contains(name)
    }

    fn builtin_prototype_written(&self) -> bool {
        let assigned = self.assigned_members();
        assigned.global || assigned.hosts.iter().any(|(_, dom)| !dom)
    }

    fn constructor_prototypes_written(&self) -> bool {
        !self.assigned_members().constructors.is_empty()
    }

    fn constructor_prototype_written(&self, path: &str, declaration: Span) -> bool {
        self.assigned_members()
            .constructors
            .contains(&(path.to_string(), declaration))
    }
}

/// ADR 0314: [`SemanticLookup::member_name_may_be_reassigned`] for a value
/// whose prototype chain holds only built-in ECMAScript prototypes
/// ([`SemanticLookup::member_may_be_reassigned_on_builtin`]).
pub(crate) fn member_may_be_reassigned_on_builtin_in<'a, W: CalleeWorld<'a> + ?Sized>(
    world: &W,
    name: &str,
) -> bool {
    world.builtin_prototype_written() || world.member_name_assigned(name)
}

/// ADR 0314: [`SemanticLookup::member_may_be_reassigned_on_class`].
pub(crate) fn member_may_be_reassigned_on_class_in<'a, W: CalleeWorld<'a> + ?Sized>(
    world: &W,
    name: &str,
    file: &'a FileFacts,
    class: &'a ClassFact,
) -> bool {
    member_may_be_reassigned_on_builtin_in(world, name)
        || class_chain_prototype_written_in(world, file, class, 8)
}

/// A class's own base: the class its `extends` names in the same file, or the
/// project class its symbol names.
fn class_base_in<'a, W: CalleeWorld<'a> + ?Sized>(
    world: &W,
    file: &'a FileFacts,
    class: &'a ClassFact,
) -> Option<Option<(&'a FileFacts, &'a ClassFact)>> {
    let heritage = file.ast.peel_ts_sugar_span(class.heritage?);
    Some(
        file.ast
            .reference_declaration(heritage)
            .and_then(|declaration| {
                file.ast
                    .classes
                    .iter()
                    .find(|candidate| {
                        candidate
                            .name
                            .as_ref()
                            .is_some_and(|name| name.span == declaration)
                    })
                    .map(|base| (file, base))
            })
            .or_else(|| {
                world
                    .entity_symbol_at(file, heritage)
                    .and_then(|symbol| world.class_for_symbol(symbol.as_str()))
            }),
    )
}

fn class_chain_prototype_written_in<'a, W: CalleeWorld<'a> + ?Sized>(
    world: &W,
    file: &'a FileFacts,
    class: &'a ClassFact,
    depth: usize,
) -> bool {
    if depth == 0 {
        return true;
    }
    if world.constructor_prototypes_written() {
        // The class's own name, or the `const` a class expression is bound to.
        let declared = class.name.as_ref().map(|name| name.span).into_iter().chain(
            file.ast
                .bindings
                .iter()
                .filter(|binding| {
                    binding.initializer.is_some_and(|initializer| {
                        file.ast.peel_ts_sugar_span(initializer) == class.span
                    })
                })
                .flat_map(|binding| binding.names.iter().map(|name| name.span)),
        );
        for declaration in declared {
            if world.constructor_prototype_written(file.path.as_str(), declaration) {
                return true;
            }
        }
    }
    let Some(base) = class_base_in(world, file, class) else {
        return false;
    };
    base.is_none_or(|(base_file, base)| {
        class_chain_prototype_written_in(world, base_file, base, depth - 1)
    })
}

/// [`SemanticLookup::class_instance_is_exact`].
pub(crate) fn class_instance_is_exact_in<'a, W: CalleeWorld<'a> + ?Sized>(
    world: &W,
    file: &'a FileFacts,
    class: &'a ClassFact,
    property: &str,
    depth: usize,
) -> bool {
    use solid_facts::ast::ClassElementKind;
    if depth == 0 {
        return false;
    }
    for element in &class.elements {
        if element.r#static {
            continue;
        }
        match element.kind {
            ClassElementKind::Constructor => {
                let Some(constructor) = file
                    .ast
                    .functions
                    .iter()
                    .find(|function| Some(function.span) == element.value)
                else {
                    return false;
                };
                let returns_value = file
                    .ast
                    .own_returns(constructor)
                    .any(|returned| returned.argument.is_some());
                let parameter_property = file.ast.parameter_properties.iter().any(|name| {
                    constructor.span.contains(*name) && file.source_text(*name) == Some(property)
                });
                if returns_value || parameter_property {
                    return false;
                }
            }
            ClassElementKind::Field
                if element.computed
                    || element.key.and_then(|key| file.source_text(key)) == Some(property) =>
            {
                return false;
            }
            ClassElementKind::Method | ClassElementKind::Getter | ClassElementKind::Setter
                if element.computed =>
            {
                return false;
            }
            _ => {}
        }
    }
    let Some(base) = class_base_in(world, file, class) else {
        return true;
    };
    base.is_some_and(|(base_file, base)| {
        class_instance_is_exact_in(world, base_file, base, property, depth - 1)
    })
}

/// The property of a static member of an actual namespace import (`ns.name`,
/// not computed): where every namespace-member answer starts, whichever world
/// then answers the property's symbol and the function it names.
pub(crate) fn namespace_member_property(file: &FileFacts, callee: Span) -> Option<Span> {
    let callee = file.ast.peel_ts_sugar_span(callee);
    if file.ast.computed_members.binary_search(&callee).is_ok() {
        return None;
    }
    let member = file.ast.member_at(callee)?;
    let receiver = file.ast.peel_ts_sugar_span(member.object);
    let declaration = file.ast.reference_declaration(receiver)?;
    let namespace = file.ast.imports.iter().any(|import| {
        !import.type_only
            && import.bindings.iter().any(|binding| {
                !binding.type_only
                    && binding.kind == solid_facts::ast::ImportKind::Namespace
                    && binding.local.span == declaration
            })
    });
    namespace.then_some(member.property)
}

/// Whether the function a namespace member's symbol names is its exported
/// value: a function, not a method, whose binding is current.
pub(crate) fn namespace_member_value_is_current(
    target_file: &FileFacts,
    function: &FunctionFact,
) -> bool {
    function.method_name.is_none() && function_value_is_current_in(target_file, function)
}

/// [`SemanticLookup::namespace_member_function`]: a static member of an
/// actual namespace import names its exported value. `symbol_at` answers the
/// entity symbol at a span of `file`, `function_for` the project function a
/// symbol names; each world supplies its own.
pub(crate) fn namespace_member_function_with<'a>(
    file: &FileFacts,
    callee: Span,
    symbol_at: impl FnOnce(Span) -> Option<&'a SymbolId>,
    function_for: impl FnOnce(&str) -> Option<(&'a FileFacts, &'a FunctionFact)>,
) -> Option<(&'a FileFacts, &'a FunctionFact)> {
    let property = namespace_member_property(file, callee)?;
    let symbol = symbol_at(property)?;
    function_for(symbol.as_str())
        .filter(|(target_file, function)| namespace_member_value_is_current(target_file, function))
}

/// [`SemanticLookup::namespace_member_function`] over a callee world.
pub(crate) fn namespace_member_function_in<'a, W: CalleeWorld<'a> + ?Sized>(
    world: &W,
    file: &FileFacts,
    callee: Span,
) -> Option<(&'a FileFacts, &'a FunctionFact)> {
    namespace_member_function_with(
        file,
        callee,
        |property| world.entity_symbol_at(file, property),
        |symbol| world.function_for_symbol(symbol),
    )
}
