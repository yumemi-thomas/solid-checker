//! ADR 0286: a typed member root whose object may have had the member replaced.
//!
//! Source discovery's type-only fallback admits a member such as `tag.label`
//! as a reactive root when Type Facts types it `Accessor<T>`. That type says
//! what the slot was declared or inferred to hold, not what it holds when it
//! is read: any code the object reaches may store another function there.
//! `target.label = () => "fixed"` type-checks against `Accessor<string>`,
//! which every `() => string` satisfies, and a write through a generic
//! parameter (`T extends { label: … }`) names the constraint's property, not
//! this one, so no census of the member's own symbol sees it.
//!
//! The rule judges every read of the member's property symbol -- every entity
//! that resolves to it -- by what its receiver is:
//!
//! - **Closed**: a `const` binding used for nothing but member reads that no
//!   write goes through, initialized by an object literal, or by a call of one
//!   exact project factory every completion of which returns a fresh object
//!   literal (written in the return, or through the factory's own `const`
//!   binding used for nothing but member reads and those returns). The literal
//!   binds no `this` -- only data properties, none a `function` expression --
//!   and spreads nothing in.
//! - **Escaped**: the same project-literal provenance, but some use of a
//!   binding on the way -- an argument, an alias, a property value, a
//!   shorthand, an export, a JSX value, a write through it -- or the literal's
//!   own `this`-binding member hands the object to code that may replace the
//!   member.
//! - **Unknown**: anything else (a parameter, a context value, an import, a
//!   call of unknown code).
//!
//! A property an object literal declares (`{ label }`, `{ label: label }`) is
//! typed by what that literal stored, so it is kept only when every read is
//! Closed over a literal that declares it. A property declared anywhere else
//! is withheld only when a read is Escaped: there the object is provably a
//! project literal that left the analysis's sight.
//!
//! A kept root is still not proven by its type (ADR 0294).
//! [`LiteralMemberSlots::closed_values`] gives source discovery the property
//! every read observes when all are Closed: a source stored there proves the
//! root, a function literal stored there is decided by its own body, and any
//! other root (an Unknown receiver among them) is only a typed candidate.

use crate::HashMap;

use solid_facts::{
    FileFacts, ProjectFacts,
    ast::{
        BindingFact, BindingShape, FunctionFact, FunctionKind, IdentifierRole, ObjectPropertyFact,
        RuntimeValueKind,
    },
    core::Span,
};

use typefacts::Location;

use crate::{SymbolId, indexes::SemanticLookup, owners::containing_ast_function};

/// One verdict per property symbol: the fallback meets the same symbol at
/// every entity Type Facts demanded for it.
pub(crate) struct LiteralMemberSlots<'l, 'a> {
    facts: &'l ProjectFacts,
    lookup: &'l SemanticLookup<'a>,
    verdicts: HashMap<SymbolId, bool>,
    /// Every entity location, by the symbol it resolves to; built on the
    /// first member candidate.
    sites: Option<HashMap<&'l SymbolId, Vec<&'l Location>>>,
    /// Each file's static members by their own span and by their property
    /// span, built on first use.
    members: HashMap<&'l str, HashMap<Span, usize>>,
}

/// The declaring object-literal keys of one property symbol.
type DeclaringKeys<'f> = Vec<(&'f str, Span)>;

/// What a member read's receiver provably holds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Receiver {
    Closed,
    Escaped,
    Unknown,
}

impl<'l, 'a> LiteralMemberSlots<'l, 'a> {
    pub(crate) fn new(facts: &'l ProjectFacts, lookup: &'l SemanticLookup<'a>) -> Self {
        Self {
            facts,
            lookup,
            verdicts: HashMap::default(),
            sites: None,
            members: HashMap::default(),
        }
    }

    /// Whether some read of the member `symbol` may observe the slot after
    /// other code replaced it, so the type-only fallback must not admit it.
    pub(crate) fn replaceable(&mut self, symbol: &SymbolId) -> bool {
        if let Some(verdict) = self.verdicts.get(symbol) {
            return *verdict;
        }
        let verdict = self.compute(symbol);
        self.verdicts.insert(symbol.clone(), verdict);
        verdict
    }

    /// ADR 0294: the property every read of the member `symbol` observes,
    /// when each read is a static member Get whose receiver is Closed over
    /// project object literals that all declare that property as an own
    /// static data property. `None` when some read is anything else. The
    /// type-only fallback asks this to judge what the slot holds instead of
    /// trusting the member's type.
    pub(crate) fn closed_values(
        &mut self,
        symbol: &SymbolId,
    ) -> Option<Vec<(&'a FileFacts, &'a ObjectPropertyFact)>> {
        let lookup = self.lookup;
        let facts = self.facts;
        let sites = self.sites.get_or_insert_with(|| {
            let mut sites = HashMap::<&SymbolId, Vec<&Location>>::default();
            for entity in facts.typescript.entities() {
                if let Some(root) = lookup.entities().get(&entity.location) {
                    sites.entry(root).or_default().push(&entity.location);
                }
            }
            sites
        });
        let sites = sites.get(symbol).cloned().unwrap_or_default();
        let declarations = lookup
            .symbol_declarations(symbol.as_str())
            .unwrap_or_default();
        let mut values = Vec::new();
        for site in sites {
            let file = lookup.file_by_path(site.path.as_ref())?;
            let span = Span::new(
                u32::try_from(site.start_byte).ok()?,
                u32::try_from(site.end_byte).ok()?,
            );
            // A declaring site (an object literal key, a type member) is no
            // read.
            if declarations.iter().any(|declaration| {
                declaration.location.path == site.path
                    && declaration.location.start_byte == site.start_byte
                    && declaration.location.end_byte == site.end_byte
            }) {
                continue;
            }
            let member = file.ast.members.iter().find(|member| {
                (member.span == span || member.property == span)
                    && file
                        .ast
                        .computed_members
                        .binary_search(&member.span)
                        .is_err()
            })?;
            if member_written(file, member.span) {
                return None;
            }
            let name = file.source_text(member.property)?;
            for (literal_file, literal) in
                self.closed_literals(file, file.ast.peel_ts_sugar_span(member.object))?
            {
                let property =
                    own_properties(literal_file, literal)
                        .into_iter()
                        .find(|property| {
                            !property.computed
                                && property.data
                                && literal_file.source_text(property.key) == Some(name)
                        })?;
                values.push((literal_file, property));
            }
        }
        Some(values)
    }

    /// The project object literals a Closed receiver holds, or `None` when
    /// the receiver is not Closed.
    fn closed_literals(
        &self,
        file: &'a FileFacts,
        receiver: Span,
    ) -> Option<Vec<(&'a FileFacts, Span)>> {
        let binding = const_binding_at(file, receiver)?;
        let initializer = file.ast.peel_ts_sugar_span(binding.initializer?);
        if !binding_closed(file, binding, None) {
            return None;
        }
        if binding.initializer_value_kind == RuntimeValueKind::Object {
            return literal_is_closed(file, initializer, None).then(|| vec![(file, initializer)]);
        }
        let (factory_file, factory) = self.factory(file, initializer)?;
        if factory_returns_literal(factory_file, factory, None) != Receiver::Closed {
            return None;
        }
        returned_literals(factory_file, factory).map(|literals| {
            literals
                .into_iter()
                .map(|literal| (factory_file, literal))
                .collect()
        })
    }

    fn compute(&mut self, symbol: &SymbolId) -> bool {
        let declarations = self
            .lookup
            .symbol_declarations(symbol.as_str())
            .unwrap_or_default();
        // A variable, function or class is no member: its reads are not
        // member Gets, so no receiver can be judged.
        if !declarations.is_empty()
            && declarations
                .iter()
                .all(|declaration| matches!(&*declaration.kind, "variable" | "function" | "class"))
        {
            return false;
        }
        let keys = declarations
            .iter()
            .filter_map(|declaration| {
                let file = self
                    .lookup
                    .file_by_path(declaration.location.path.as_ref())?;
                let span = Span::new(
                    u32::try_from(declaration.location.start_byte).ok()?,
                    u32::try_from(declaration.location.end_byte).ok()?,
                );
                file.ast
                    .object_properties
                    .iter()
                    .any(|property| property.key == span)
                    .then_some((file.path.as_str(), span))
            })
            .collect::<DeclaringKeys<'_>>();
        let literal_declared = !keys.is_empty();
        // The reads to judge are the entities that resolve to this symbol:
        // every consumer of the fallback's root finds its reads by entity
        // identity, and a span with no entity resolves to no root. Type
        // Facts' reference census is demand-driven and is not asked for a
        // property symbol, so it cannot stand in for this.
        let lookup = self.lookup;
        let facts = self.facts;
        let sites = self.sites.get_or_insert_with(|| {
            let mut sites = HashMap::<&SymbolId, Vec<&Location>>::default();
            for entity in facts.typescript.entities() {
                if let Some(root) = lookup.entities().get(&entity.location) {
                    sites.entry(root).or_default().push(&entity.location);
                }
            }
            sites
        });
        let sites = sites.get(symbol).cloned().unwrap_or_default();
        for site in sites {
            let Some(file) = self.lookup.file_by_path(site.path.as_ref()) else {
                if literal_declared {
                    return true;
                }
                continue;
            };
            let (Ok(start), Ok(end)) =
                (u32::try_from(site.start_byte), u32::try_from(site.end_byte))
            else {
                return true;
            };
            let span = Span::new(start, end);
            if keys.contains(&(file.path.as_str(), span)) {
                continue;
            }
            // A literal-declared member's every other site must be a static
            // member Get, at its whole span or its property, whose receiver
            // still holds the declaring literal. A destructuring key, a
            // computed access or any other shape is not judged here and
            // withholds the root.
            let members = self.members.entry(file.path.as_str()).or_insert_with(|| {
                let mut index = HashMap::default();
                for (position, member) in file.ast.members.iter().enumerate() {
                    if file
                        .ast
                        .computed_members
                        .binary_search(&member.span)
                        .is_err()
                    {
                        index.entry(member.span).or_insert(position);
                        index.entry(member.property).or_insert(position);
                    }
                }
                index
            });
            let Some(member) = members
                .get(&span)
                .map(|position| &file.ast.members[*position])
            else {
                if literal_declared {
                    return true;
                }
                continue;
            };
            let receiver = self.receiver(
                file,
                file.ast.peel_ts_sugar_span(member.object),
                literal_declared.then_some(&keys),
            );
            // A write through a Closed receiver makes it Escaped, so only the
            // literal-declared rule, which also refuses Unknown receivers,
            // needs the site's own write.
            let refused = if literal_declared {
                member_written(file, member.span) || receiver != Receiver::Closed
            } else {
                receiver == Receiver::Escaped
            };
            if refused {
                return true;
            }
        }
        false
    }

    /// What the receiver at `receiver` holds. `keys`, when given, are the
    /// declaring keys the literal must carry for the receiver to be Closed.
    fn receiver(
        &self,
        file: &FileFacts,
        receiver: Span,
        keys: Option<&DeclaringKeys<'_>>,
    ) -> Receiver {
        let Some(binding) = const_binding_at(file, receiver) else {
            return Receiver::Unknown;
        };
        let Some(initializer) = binding
            .initializer
            .map(|span| file.ast.peel_ts_sugar_span(span))
        else {
            return Receiver::Unknown;
        };
        let held = if binding.initializer_value_kind == RuntimeValueKind::Object {
            if literal_is_closed(file, initializer, keys) {
                Receiver::Closed
            } else {
                Receiver::Escaped
            }
        } else {
            let Some((factory_file, factory)) = self.factory(file, initializer) else {
                return Receiver::Unknown;
            };
            factory_returns_literal(factory_file, factory, keys)
        };
        if held == Receiver::Closed && !binding_closed(file, binding, None) {
            return Receiver::Escaped;
        }
        held
    }

    /// The exact synchronous project function the plain call at `initializer`
    /// runs.
    fn factory(
        &self,
        file: &FileFacts,
        initializer: Span,
    ) -> Option<(&'a FileFacts, &'a FunctionFact)> {
        let call = file
            .ast
            .calls
            .iter()
            .find(|call| call.span == initializer && !call.construct)?;
        if call.arguments.iter().any(|argument| argument.spread) {
            return None;
        }
        let (factory_file, factory) = self
            .lookup
            .callee_symbol(file, call.callee)
            .and_then(|symbol| self.lookup.function_for_symbol(symbol))?;
        (!factory.r#async
            && !factory.generator
            && factory.method_name.is_none()
            && self.lookup.function_value_is_current(factory_file, factory))
        .then_some((factory_file, factory))
    }
}

/// What every completion of `factory` returns: Closed when each returns a
/// fresh closed object literal, written in the return or bound to a `const`
/// the factory declares in its own body and uses for nothing but member reads
/// and those returns; Escaped when each returns such a literal but one of
/// them may have reached other code first; Unknown otherwise.
fn factory_returns_literal(
    file: &FileFacts,
    factory: &FunctionFact,
    keys: Option<&DeclaringKeys<'_>>,
) -> Receiver {
    let returns = factory.expression_return.as_ref().map_or_else(
        || {
            file.ast
                .returns
                .iter()
                .filter(|returned| {
                    containing_ast_function(&file.ast, returned.span)
                        .is_some_and(|owner| owner.span == factory.span)
                })
                .collect::<Vec<_>>()
        },
        |returned| vec![returned],
    );
    if returns.is_empty() {
        return Receiver::Unknown;
    }
    let mut held = Receiver::Closed;
    for returned in returns {
        let Some(argument) = returned
            .argument
            .map(|span| file.ast.peel_ts_sugar_span(span))
        else {
            return Receiver::Unknown;
        };
        let closed = if returned.runtime_value_kind == RuntimeValueKind::Object {
            literal_is_closed(file, argument, keys)
        } else {
            let Some(binding) = const_binding_at(file, argument).filter(|binding| {
                binding.initializer_value_kind == RuntimeValueKind::Object
                    && containing_ast_function(&file.ast, binding.declaration)
                        .is_some_and(|owner| owner.span == factory.span)
            }) else {
                return Receiver::Unknown;
            };
            binding.initializer.is_some_and(|initializer| {
                literal_is_closed(file, file.ast.peel_ts_sugar_span(initializer), keys)
            }) && binding_closed(file, binding, Some(factory))
        };
        if !closed {
            held = Receiver::Escaped;
        }
    }
    held
}

/// The object literal every completion of `factory` returns, written in the
/// return or through its own `const` binding, once
/// [`factory_returns_literal`] answered Closed.
fn returned_literals(file: &FileFacts, factory: &FunctionFact) -> Option<Vec<Span>> {
    let returns = factory.expression_return.as_ref().map_or_else(
        || {
            file.ast
                .returns
                .iter()
                .filter(|returned| {
                    containing_ast_function(&file.ast, returned.span)
                        .is_some_and(|owner| owner.span == factory.span)
                })
                .collect::<Vec<_>>()
        },
        |returned| vec![returned],
    );
    returns
        .into_iter()
        .map(|returned| {
            let argument = file.ast.peel_ts_sugar_span(returned.argument?);
            if returned.runtime_value_kind == RuntimeValueKind::Object {
                return Some(argument);
            }
            let binding = const_binding_at(file, argument)?;
            binding
                .initializer
                .map(|initializer| file.ast.peel_ts_sugar_span(initializer))
        })
        .collect()
}

/// The object literal's own properties: those inside it that no other
/// property inside it holds in its key or value.
fn own_properties(file: &FileFacts, literal: Span) -> Vec<&ObjectPropertyFact> {
    let inside = file
        .ast
        .object_properties
        .iter()
        .filter(|property| literal.contains(property.span))
        .collect::<Vec<_>>();
    inside
        .iter()
        .filter(|property| {
            !inside.iter().any(|other| {
                other.span != property.span
                    && (other.value.contains(property.span) || other.key.contains(property.span))
            })
        })
        .copied()
        .collect()
}

/// The unwritten `const` identifier binding the exact identifier reference at
/// `span` resolves to in this file.
fn const_binding_at(file: &FileFacts, span: Span) -> Option<&BindingFact> {
    file.ast
        .identifiers
        .binary_search_by_key(&span, |identifier| identifier.span)
        .ok()
        .filter(|index| file.ast.identifiers[*index].role == IdentifierRole::Reference)?;
    let declaration = file.ast.reference_declaration(span)?;
    let binding = file.ast.bindings.iter().find(|binding| {
        binding.immutable
            && binding.shape == BindingShape::Identifier
            && binding.names.len() == 1
            && binding.names[0].span == declaration
    })?;
    (!crate::indexes::binding_written(file, declaration)).then_some(binding)
}

/// Whether every use of `binding` is a member read through it that writes
/// nothing, or -- for a factory's own binding -- one of `factory`'s own
/// returns. An export, a shorthand and every other reference hands the object
/// to code that may replace a member.
fn binding_closed(file: &FileFacts, binding: &BindingFact, factory: Option<&FunctionFact>) -> bool {
    let declaration = binding.names[0].span;
    let refers = |span: Span| {
        span == declaration || file.ast.reference_declaration(span) == Some(declaration)
    };
    let exported = file
        .ast
        .exports
        .iter()
        .filter(|export| !export.type_only)
        .any(|export| {
            export
                .declarations
                .iter()
                .chain(&export.specifiers)
                .any(|specifier| !specifier.type_only && refers(specifier.local.span))
        });
    if exported
        || file
            .ast
            .object_properties
            .iter()
            .any(|property| property.shorthand_binding == Some(declaration))
    {
        return false;
    }
    file.ast
        .identifiers
        .iter()
        .filter(|identifier| identifier.role == IdentifierRole::Reference)
        .filter(|identifier| file.ast.reference_declaration(identifier.span) == Some(declaration))
        .all(|identifier| {
            let at = identifier.span;
            if let Some(member) = file.ast.members.iter().find(|member| member.object == at) {
                return !member_written(file, member.span);
            }
            factory.is_some_and(|factory| returned_by(file, factory, at))
        })
}

/// Whether `span` is exactly the argument of one of `factory`'s own returns.
fn returned_by(file: &FileFacts, factory: &FunctionFact, span: Span) -> bool {
    let returns_span = |argument: Option<Span>| {
        argument.is_some_and(|argument| file.ast.peel_ts_sugar_span(argument) == span)
    };
    match &factory.expression_return {
        Some(returned) => returns_span(returned.argument),
        None => file.ast.returns.iter().any(|returned| {
            returns_span(returned.argument)
                && containing_ast_function(&file.ast, returned.span)
                    .is_some_and(|owner| owner.span == factory.span)
        }),
    }
}

/// Whether the object literal at `literal` binds no `this` (no method,
/// accessor or `function` expression value), spreads nothing in and, when
/// `keys` are given, declares one of them as its own static data property.
fn literal_is_closed(file: &FileFacts, literal: Span, keys: Option<&DeclaringKeys<'_>>) -> bool {
    let inside = file
        .ast
        .object_properties
        .iter()
        .filter(|property| literal.contains(property.span))
        .collect::<Vec<_>>();
    let own = inside
        .iter()
        .filter(|property| {
            !inside.iter().any(|other| {
                other.span != property.span
                    && (other.value.contains(property.span) || other.key.contains(property.span))
            })
        })
        .copied()
        .collect::<Vec<_>>();
    let declares = keys.is_none_or(|keys| {
        own.iter().any(|property| {
            !property.computed && keys.contains(&(file.path.as_str(), property.key))
        })
    });
    let binds_this = own.iter().any(|property| {
        !property.data
            || file.ast.functions.iter().any(|function| {
                function.span == file.ast.peel_ts_sugar_span(property.value)
                    && function.kind != FunctionKind::Arrow
            })
    });
    let spreads = file.ast.spreads.iter().any(|spread| {
        literal.contains(spread.span)
            && !inside
                .iter()
                .any(|property| property.span.contains(spread.span))
    });
    declares && !binds_this && !spreads
}

/// Whether the member at `span` is written: assigned, updated, deleted or an
/// iteration target.
fn member_written(file: &FileFacts, span: Span) -> bool {
    file.ast.write_targets.contains(&span)
        || file.ast.deleted_targets.contains(&span)
        || file.ast.iteration_targets.contains(&span)
        || file
            .ast
            .assignments
            .iter()
            .any(|assignment| file.ast.peel_ts_sugar_span(assignment.target) == span)
}
