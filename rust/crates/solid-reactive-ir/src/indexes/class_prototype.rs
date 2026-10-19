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

    pub(super) fn assigned_members(&self) -> &AssignedMembers {
        self.assigned_member_names.get_or_init(|| {
            let facts: &'a ProjectFacts = self.facts;
            let mut names = HashSet::new();
            let mut prototype = false;
            let mut constructors = HashSet::new();
            let mut pending_hosts = Vec::new();
            let mut record =
                |file: &'a FileFacts, owner: Option<PrototypeOwner>, prototype: &mut bool| {
                    match owner {
                        Some(PrototypeOwner::Constructor(declaration)) => {
                            constructors.insert((file.path.to_string(), declaration));
                        }
                        Some(PrototypeOwner::Hosts(method, hosts)) => {
                            pending_hosts.push((method, hosts));
                        }
                        None => *prototype = true,
                    }
                };
            for file in &facts.files {
                let targets = file
                    .ast
                    .assignments
                    .iter()
                    .map(|assignment| assignment.target)
                    .chain(file.ast.iteration_targets.iter().copied());
                for target in targets {
                    let target = file.ast.peel_ts_sugar_span(target);
                    let text = file.source_text(target).unwrap_or_default();
                    // Every member written by the target: the target itself,
                    // or a leaf of a destructuring pattern (`[c.run] = …`) or
                    // loop head (`for (c.run of …)`).
                    // A member that is another written member's object
                    // (`C.prototype` in `C.prototype.m = f`) is read, not
                    // written.
                    let inside = file
                        .ast
                        .members
                        .iter()
                        .filter(|member| target.contains(member.span))
                        .collect::<Vec<_>>();
                    let mut written = inside
                        .iter()
                        .copied()
                        .filter(|member| {
                            !inside.iter().any(|outer| {
                                file.ast.peel_ts_sugar_span(outer.object) == member.span
                            })
                        })
                        .peekable();
                    if written.peek().is_none() && text.contains("prototype") {
                        prototype = true;
                    }
                    for member in written {
                        if file
                            .ast
                            .computed_members
                            .binary_search(&member.span)
                            .is_ok()
                        {
                            // A computed write names its key only when the key is
                            // a literal, or a `const` bound to a string literal.
                            // Any other key, on a prototype, may be any member.
                            if let Some(literal) = file
                                .ast
                                .literal_computed_members
                                .iter()
                                .find(|literal| literal.span == member.span)
                            {
                                names.insert(literal.key.to_string());
                                if literal.key.as_str() == "prototype" {
                                    prototype = true;
                                }
                            } else if let Some(key) = constant_string_key(file, member.property) {
                                if key == "prototype" {
                                    prototype = true;
                                }
                                names.insert(key);
                            } else if file
                                .source_text(member.object)
                                .unwrap_or_default()
                                .contains("prototype")
                            {
                                // `R.prototype[key] = …`: narrowed to `R`'s
                                // instances when `R` is proven (ADR 0314).
                                let object = file.ast.peel_ts_sugar_span(member.object);
                                match file.ast.members.iter().find(|prototype_member| {
                                    prototype_member.span == object
                                        && file
                                            .ast
                                            .computed_members
                                            .binary_search(&prototype_member.span)
                                            .is_err()
                                        && file.source_text(prototype_member.property)
                                            == Some("prototype")
                                }) {
                                    Some(prototype_member) => record(
                                        file,
                                        self.prototype_owner(file, prototype_member.object),
                                        &mut prototype,
                                    ),
                                    None => prototype = true,
                                }
                            }
                        } else if let Some(property) = file.source_text(member.property) {
                            names.insert(property.to_string());
                            if property == "prototype" {
                                // `R.prototype = …` (ADR 0314).
                                record(
                                    file,
                                    self.prototype_owner(file, member.object),
                                    &mut prototype,
                                );
                            }
                        }
                    }
                    // A dynamic key not spelled through a prototype (`c[key] = f`
                    // with `key: keyof C`, or through a prototype alias) is not
                    // modeled. Like `Object.defineProperty`, it is a reflective
                    // write this trust boundary leaves out (ADR 0215): vetoing
                    // every dynamic write would refuse every `xs[i] = v`.
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
    pub(crate) fn property_may_be_defined_reflectively(&self) -> bool {
        const NAMES: [&str; 3] = ["defineProperty", "defineProperties", "__defineGetter__"];
        *self.reflective_property_definition.get_or_init(|| {
            self.files().iter().any(|file| {
                file.ast.members.iter().any(|member| {
                    file.source_text(member.property).is_some_and(|property| {
                        NAMES.contains(&property.trim_matches(['\'', '"', '`']))
                    })
                }) || file.ast.bindings.iter().any(|binding| {
                    binding
                        .object_slots
                        .iter()
                        .any(|slot| NAMES.contains(&slot.property.as_str()))
                }) || file.ast.calls.iter().any(|call| {
                    file.source_text(call.callee)
                        .is_some_and(|callee| NAMES.iter().any(|name| callee.ends_with(name)))
                })
            })
        })
    }

    /// ADR 0314: [`Self::property_may_be_defined_reflectively`] for one
    /// property name. Every spelling of a reflective definer must be the
    /// callee of a call whose key argument (the second) is a string literal
    /// other than `name`; any other use -- an alias, a destructuring, a
    /// computed or missing key, `defineProperties`, `__defineGetter__` with
    /// this name -- may define `name`.
    pub(crate) fn property_may_be_defined_reflectively_as(&self, name: &str) -> bool {
        use solid_facts::ast::ArgumentLiteralFact;
        const NAMES: [&str; 3] = ["defineProperty", "defineProperties", "__defineGetter__"];
        if !self.property_may_be_defined_reflectively() {
            return false;
        }
        self.files().iter().any(|file| {
            let spelled = |text: &str| NAMES.contains(&text.trim_matches(['\'', '"', '`']));
            file.ast.bindings.iter().any(|binding| {
                binding
                    .object_slots
                    .iter()
                    .any(|slot| NAMES.contains(&slot.property.as_str()))
            }) || file.ast.members.iter().any(|member| {
                file.source_text(member.property).is_some_and(spelled)
                    && !file.ast.calls.iter().any(|call| {
                        file.ast.peel_ts_sugar_span(call.callee) == member.span
                            && file.source_text(member.property).is_some_and(|text| {
                                text.trim_matches(['\'', '"', '`']) != "defineProperties"
                            })
                            && matches!(
                                call.arguments.get(1).map(|key| (&key.literal_value, key.spread)),
                                Some((ArgumentLiteralFact::String(key), false)) if key.as_str() != name
                            )
                            && !call.arguments.first().is_some_and(|target| target.spread)
                    })
            }) || file.ast.calls.iter().any(|call| {
                file.source_text(call.callee).is_some_and(|callee| {
                    NAMES.iter().any(|definer| callee.ends_with(definer))
                }) && !file
                    .ast
                    .members
                    .iter()
                    .any(|member| file.ast.peel_ts_sugar_span(call.callee) == member.span)
            })
        })
    }
}
