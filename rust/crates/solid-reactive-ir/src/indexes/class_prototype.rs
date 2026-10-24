//! Class and prototype writes: whether a member may be reassigned, and who owns a prototype or global constructor.

use super::*;

impl<'a> SemanticLookup<'a> {
    /// ADR 0209: whether some assignment in the project writes a property
    /// named `name` on any object, or writes through any `prototype`.
    pub(crate) fn member_name_may_be_reassigned(&self, name: &str) -> bool {
        let assigned = self.assigned_members();
        assigned.global
            || !assigned.constructors.is_empty()
            || !assigned.hosts.is_empty()
            || assigned.names.contains(name)
    }

    /// ADR 0338: whether any analysed file may replace `Function.prototype.bind`,
    /// which builds every `read.bind(null, node)` getter: a plain write to a
    /// member named `bind` (`Function.prototype.bind = …`), a write through a
    /// prototype whose key or owner is not proven, or a reflective write
    /// spelled through a `prototype` (`Object.defineProperty(Function.prototype,
    /// …)`, `Object.assign`, `Reflect.set`; ADR 0292's veto). A tripwire over
    /// what the project visibly writes, not a proof about the builtin.
    pub(crate) fn function_bind_may_be_patched(&self) -> bool {
        *self.function_bind_patched.get_or_init(|| {
            self.whole_program_answer::<super::project_memos::FunctionBindPatched>()
        })
    }

    /// [`Self::function_bind_may_be_patched`]'s computation (the memo's).
    pub(super) fn compute_function_bind_patched(&self) -> bool {
        self.member_may_be_reassigned_on_builtin("bind")
            || crate::interproc::accessor_provenance::reflective_prototype_write(self)
    }

    /// ADR 0314: [`Self::member_name_may_be_reassigned`] for a value whose
    /// prototype chain holds only built-in ECMAScript prototypes -- a fresh
    /// literal, a primitive, a built-in collection (the ADR 0211 `Builtin`
    /// origin). A write through a project constructor's prototype cannot
    /// reach it, and neither can one through a host prototype every
    /// declaration of which is in `lib.dom.d.ts`: no ECMAScript value inherits
    /// from a DOM interface.
    pub(crate) fn member_may_be_reassigned_on_builtin(&self, name: &str) -> bool {
        let assigned = self.assigned_members();
        assigned.global
            || assigned.hosts.iter().any(|(_, dom)| !dom)
            || assigned.names.contains(name)
    }

    /// ADR 0314: [`Self::member_name_may_be_reassigned`] for an instance of
    /// the project class `class`: a write through a prototype reaches it only
    /// when its chain holds that prototype. Every ancestor must be a project
    /// class this resolves; an unresolved `extends` may be anything.
    pub(crate) fn member_may_be_reassigned_on_class(
        &self,
        name: &str,
        file: &'a FileFacts,
        class: &'a solid_facts::ast::ClassFact,
    ) -> bool {
        let assigned = self.assigned_members();
        assigned.global
            || assigned.hosts.iter().any(|(_, dom)| !dom)
            || assigned.names.contains(name)
            || self.class_chain_prototype_written(file, class, 8)
    }

    fn class_chain_prototype_written(
        &self,
        file: &'a FileFacts,
        class: &'a solid_facts::ast::ClassFact,
        depth: usize,
    ) -> bool {
        let assigned = self.assigned_members();
        if depth == 0 {
            return true;
        }
        if !assigned.constructors.is_empty() {
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
                if assigned
                    .constructors
                    .contains(&(file.path.to_string(), declaration))
                {
                    return true;
                }
            }
        }
        let Some(heritage) = class.heritage else {
            return false;
        };
        let heritage = file.ast.peel_ts_sugar_span(heritage);
        let base = file
            .ast
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
                self.entities
                    .at(file.path.as_str(), heritage)
                    .and_then(|symbol| self.class_for_symbol(symbol))
            });
        base.is_none_or(|(base_file, base)| {
            self.class_chain_prototype_written(base_file, base, depth - 1)
        })
    }

    /// ADR 0209, 0314: the member names assignments write, and the prototypes
    /// they write through: the [`crate::object_members::AssignedMembersAnswer`]
    /// memo over the base members and hazards indexes, as sets for the
    /// per-name queries above.
    pub(super) fn assigned_members(&self) -> &AssignedMembers {
        self.assigned_member_names.get_or_init(|| {
            let answer = self
                .memo_world()
                .answer::<crate::object_members::AssignedMembersAnswer>(&());
            AssignedMembers {
                names: answer.names.iter().map(|name| name.to_string()).collect(),
                global: answer.global,
                constructors: answer
                    .constructors
                    .iter()
                    .map(|(path, declaration)| (path.to_string(), *declaration))
                    .collect(),
                hosts: answer.hosts,
            }
        })
    }

    /// Review round 1 of ADR 0293: whether any analyzed file may define a
    /// property reflectively, installing an accessor where a literal wrote
    /// data. Deliberately syntactic and wide: any member access, destructured
    /// property or callee spelled `defineProperty`, `defineProperties` or
    /// `__defineGetter__`, whatever its receiver, so an alias
    /// (`const define = Object.defineProperty`) is caught where it is taken.
    ///
    /// The census is [`crate::base::Hazard::ReflectiveDefinitionSpelling`].
    pub(crate) fn property_may_be_defined_reflectively(&self) -> bool {
        self.hazard_present(crate::base::Hazard::ReflectiveDefinitionSpelling)
    }

    /// ADR 0314: [`Self::property_may_be_defined_reflectively`] for one
    /// property name. Every spelling of a reflective definer must be the
    /// callee of a call whose key argument (the second) is a string literal
    /// other than `name`; any other use -- an alias, a destructuring, a
    /// computed or missing key, `defineProperties`, `__defineGetter__` with
    /// this name -- may define `name`.
    pub(crate) fn property_may_be_defined_reflectively_as(&self, name: &str) -> bool {
        if !self.property_may_be_defined_reflectively() {
            return false;
        }
        let names = self
            .reflective_definition_names
            .get_or_init(|| self.reflective_definition_names());
        names.any || names.sole_keys.contains(name)
    }

    /// The per-name answer of
    /// [`Self::property_may_be_defined_reflectively_as`] as data, from the
    /// base definers index ([`crate::base::contributions::definers`]).
    fn reflective_definition_names(&self) -> super::ReflectiveDefinitionNames {
        use crate::base::DefinerKey;
        let mut result = super::ReflectiveDefinitionNames::default();
        for key in self.base_definers().keys() {
            match key {
                DefinerKey::Any => result.any = true,
                DefinerKey::Sole(name) => {
                    result.sole_keys.insert(name.to_string());
                }
            }
        }
        result
    }
}
