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
            self.member_may_be_reassigned_on_builtin("bind")
                || crate::interproc::accessor_provenance::reflective_prototype_write(self)
        })
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

    /// ADR 0209, 0314: the member names assignments write, from the base
    /// members index ([`crate::base::MemberRole::Assigned`]), and the
    /// prototypes they write through, from the base hazards index
    /// ([`crate::base::Hazard::UnownedPrototypeWrite`],
    /// [`crate::base::Hazard::PrototypeOwnerSite`]). Each owner site is
    /// proved here; [`crate::base::contributions::assignment_writes`] lists
    /// them per file.
    pub(super) fn assigned_members(&self) -> &AssignedMembers {
        self.assigned_member_names.get_or_init(|| {
            use crate::base::{Hazard, MemberName, MemberRole};
            let facts: &'a ProjectFacts = self.facts;
            let names = self
                .base_members()
                .keys()
                .filter_map(|name| match name {
                    MemberName::Named(name) => Some(name),
                    MemberName::Unnamed => None,
                })
                .filter(|name| {
                    self.base_members()
                        .values(&MemberName::Named(std::sync::Arc::clone(name)))
                        .any(|(_, used)| used.role == MemberRole::Assigned)
                })
                .map(|name| name.to_string())
                .collect::<HashSet<String>>();
            let mut prototype = self.hazard_present(Hazard::UnownedPrototypeWrite);
            let mut constructors = HashSet::default();
            let mut pending_hosts = Vec::new();
            for (file, entries) in self.slot_entries(self.base_hazards()) {
                for (hazard, member) in entries {
                    if *hazard != Hazard::PrototypeOwnerSite {
                        continue;
                    }
                    let receiver = file.ast.members[*member as usize].object;
                    match self.prototype_owner(file, receiver) {
                        Some(PrototypeOwner::Constructor(declaration)) => {
                            constructors.insert((file.path.to_string(), declaration));
                        }
                        Some(PrototypeOwner::Hosts(method, hosts)) => {
                            pending_hosts.push((method, hosts));
                        }
                        None => prototype = true,
                    }
                }
            }
            // A host proof holds only while neither the method nor the
            // global constructors it names are rewritten.
            let mut hosts = Vec::new();
            for (method, found) in pending_hosts {
                if names.contains(&method)
                    || found.iter().any(|(host, _)| {
                        names.contains(host) || global_identifier_written(facts, host)
                    })
                {
                    prototype = true;
                }
                hosts.extend(found);
            }
            AssignedMembers {
                names,
                global: prototype,
                constructors,
                hosts,
            }
        })
    }

    /// ADR 0314: whose instances a write through `receiver.prototype` can
    /// reach, when that is provable; `None` for any other receiver.
    ///
    /// - A class, a function declaration, or a `const` bound to a function or
    ///   class expression, declared in this file and never rebound: that
    ///   constructor's instances.
    /// - A parameter of a static class method nothing rebinds, when every
    ///   access to that method anywhere is a direct call through the class's
    ///   own name and passes, at that parameter, a global constructor declared
    ///   only in TypeScript's default libraries: those host prototypes.
    fn prototype_owner(&self, file: &'a FileFacts, receiver: Span) -> Option<PrototypeOwner> {
        use solid_facts::ast::BindingShape;
        let receiver = file.ast.peel_ts_sugar_span(receiver);
        if file
            .ast
            .identifiers
            .binary_search_by_key(&receiver, |identifier| identifier.span)
            .is_err()
        {
            return None;
        }
        let declaration = file.ast.reference_declaration(receiver)?;
        if let Some((function, index)) = file.ast.functions.iter().find_map(|function| {
            function
                .parameters
                .iter()
                .position(|parameter| parameter.names.iter().any(|name| name.span == declaration))
                .map(|index| (function, index))
        }) {
            return self.host_prototype_owner(file, function, index);
        }
        let declared_constructor = file.ast.classes.iter().any(|class| {
            class
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration)
        }) || file.ast.functions.iter().any(|function| {
            function.kind == solid_facts::ast::FunctionKind::Declaration
                && function
                    .name
                    .as_ref()
                    .is_some_and(|name| name.span == declaration)
        });
        if declared_constructor {
            return (!binding_written(file, declaration))
                .then_some(PrototypeOwner::Constructor(declaration));
        }
        let binding = file
            .ast
            .bindings
            .iter()
            .find(|binding| binding.names.iter().any(|name| name.span == declaration))?;
        let initializer = file.ast.peel_ts_sugar_span(binding.initializer?);
        (binding.immutable
            && binding.shape == BindingShape::Identifier
            && (file.ast.functions.iter().any(|function| {
                function.span == initializer
                    && function.kind != solid_facts::ast::FunctionKind::Arrow
            }) || file
                .ast
                .classes
                .iter()
                .any(|class| class.span == initializer)))
        .then_some(PrototypeOwner::Constructor(declaration))
    }

    fn host_prototype_owner(
        &self,
        file: &'a FileFacts,
        function: &'a solid_facts::ast::FunctionFact,
        index: usize,
    ) -> Option<PrototypeOwner> {
        use solid_facts::ast::{BindingShape, ClassElementKind};
        let parameter = function.parameters.get(index)?;
        let [name] = parameter.names.as_slice() else {
            return None;
        };
        if parameter.shape != BindingShape::Identifier
            || parameter.initializer.is_some()
            || (function.rest_parameter && index + 1 == function.parameters.len())
            || binding_written(file, name.span)
            || file.ast.identifiers.iter().any(|identifier| {
                function.span.contains(identifier.span)
                    && file.source_text(identifier.span) == Some("arguments")
            })
        {
            return None;
        }
        let (class, element) = file.ast.classes.iter().find_map(|class| {
            class
                .elements
                .iter()
                .find(|element| {
                    element.value == Some(function.span)
                        && element.r#static
                        && element.kind == ClassElementKind::Method
                        && !element.computed
                })
                .map(|element| (class, element))
        })?;
        let method = file.source_text(element.key?)?;
        let class_name = class.name.as_ref()?;
        let class_symbol = self.entities.at(file.path.as_str(), class_name.span)?;
        let is_this_class = |reference_file: &FileFacts, span: Span| {
            self.entities
                .at(reference_file.path.as_str(), span)
                .and_then(|symbol| self.class_for_symbol(symbol))
                .is_some_and(|(found_file, found)| {
                    found_file.path == file.path && found.span == class.span
                })
        };
        // The class never escapes: past its declaration and imports, every
        // reference is the object of a member access naming its key.
        for reference in self.symbol_references_if_present(class_symbol.as_str())? {
            let reference_file = self.file_by_path(reference.path.as_ref())?;
            let (Ok(start), Ok(end)) = (
                u32::try_from(reference.start_byte),
                u32::try_from(reference.end_byte),
            ) else {
                return None;
            };
            let span = Span::new(start, end);
            if (reference_file.path == file.path && span == class_name.span)
                || reference_file
                    .ast
                    .imports
                    .iter()
                    .any(|import| import.span.contains(span))
            {
                continue;
            }
            if !reference_file.ast.members.iter().any(|member| {
                reference_file.ast.peel_ts_sugar_span(member.object) == span
                    && reference_file
                        .ast
                        .computed_members
                        .binary_search(&member.span)
                        .is_err()
            }) {
                return None;
            }
        }
        let mut hosts = Vec::new();
        for other in self.files() {
            if other
                .ast
                .literal_computed_members
                .iter()
                .any(|literal| literal.key.as_str() == method)
                || other.ast.bindings.iter().any(|binding| {
                    binding
                        .object_slots
                        .iter()
                        .any(|slot| slot.property.as_str() == method)
                })
            {
                return None;
            }
            for member in other.ast.members.iter().filter(|member| {
                other
                    .ast
                    .computed_members
                    .binary_search(&member.span)
                    .is_err()
                    && other.source_text(member.property) == Some(method)
            }) {
                if !is_this_class(other, other.ast.peel_ts_sugar_span(member.object)) {
                    return None;
                }
                let call = other.ast.calls.iter().find(|call| {
                    !call.construct && other.ast.peel_ts_sugar_span(call.callee) == member.span
                })?;
                if call
                    .arguments
                    .iter()
                    .take(index + 1)
                    .any(|argument| argument.spread)
                {
                    return None;
                }
                hosts.push(self.global_constructor(other, call.arguments.get(index)?.span)?);
            }
        }
        Some(PrototypeOwner::Hosts(method.to_owned(), hosts))
    }

    /// The name of the global constructor `value` names, and whether every
    /// `var` declaration of it is in `lib.dom.d.ts`, when it is an identifier
    /// no project binding declares, every declaration of it is ambient, and
    /// its value is declared only by TypeScript's default libraries.
    fn global_constructor(&self, file: &'a FileFacts, value: Span) -> Option<(String, bool)> {
        let value = file.ast.peel_ts_sugar_span(value);
        if file
            .ast
            .identifiers
            .binary_search_by_key(&value, |identifier| identifier.span)
            .is_err()
            || file.ast.reference_declaration(value).is_some()
        {
            return None;
        }
        let symbol = self.symbol_at(file.path.as_str(), value)?;
        if !symbol.alias_target().is_empty() || symbol.declarations().is_empty() {
            return None;
        }
        // Every declaration is ambient, in a declaration file no analyzed
        // source is: none creates a runtime binding (`@webgpu/types` merges an
        // interface into `HTMLCanvasElement`; `babylonjs` re-declares
        // `var OffscreenCanvas`). The value is the host's, which a default
        // library declares, under the standard host premise (ADR 0295, 0300).
        let mut dom = true;
        let mut library = false;
        for declaration in symbol.declarations() {
            let path = declaration.location.path.as_ref();
            let name = std::path::Path::new(path).file_name()?.to_str()?;
            if self.file_by_path(path).is_some() || !name.ends_with(".d.ts") {
                return None;
            }
            if declaration.kind.as_ref() == "variable" && name.starts_with("lib.") {
                library = true;
                dom &= name == "lib.dom.d.ts";
            }
        }
        let variable = library;
        variable
            .then(|| (file.source_text(value).map(str::to_owned), dom))
            .and_then(|(name, dom)| Some((name?, dom)))
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
