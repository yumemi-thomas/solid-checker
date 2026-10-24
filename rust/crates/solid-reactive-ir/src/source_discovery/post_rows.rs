//! What each source-discovery post-pass reads of one file (or one TypeScript
//! path), as rows the pass folds in file order (S7a; S7 design §2).
//!
//! Each row set is a function of its file's own facts and of the reads its
//! computation records ([`super::trace`], [`crate::base::reads`]); a pass's
//! state is never read here except through those records, so a row is kept
//! while its file and its reads answer as they did
//! ([`super::post_passes`]). The passes' own order rules (first wins, last
//! wins, the alias rounds) are applied when the rows are folded.

use std::sync::Arc;

use super::trace;
use super::*;

/// The rows of one analyzed file that read no source table: what passes 3
/// to 10 ask of the file itself.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct FileRows {
    /// Pass 3: each accessor parameter of a children callback, in element
    /// order: (symbol, display, declaration).
    pub(super) children: Vec<(SymbolId, SymbolId, Location)>,
    /// Pass 4: each effect whose compute returns a candidate source and whose
    /// apply takes a parameter, in call order.
    pub(super) effects: Vec<EffectRow>,
    /// Pass 5: each binding initialized by an identifier with a symbol, in
    /// binding order.
    pub(super) aliases: Vec<AliasRow>,
    /// Pass 6: each binding initialized by a call that may be a view of a
    /// store, in binding order.
    pub(super) views: Vec<ViewRow>,
    /// Pass 7: each component's props parameter, in function order.
    pub(super) prop_roots: Vec<PropRoot>,
    /// Pass 8: each binding that may alias or view a props object, in
    /// binding order.
    pub(super) prop_aliases: Vec<ViewRow>,
    /// Pass 9: the symbols the file writes, sorted, each once.
    pub(super) written: Vec<SymbolId>,
    /// Pass 10: each JSX element whose tag names a project function: its
    /// ordinal, and the function's file and span.
    pub(super) renders: Vec<(u32, Arc<str>, Span)>,
    /// Pass 10: the symbols the file exports by a value specifier or
    /// declaration, sorted, each once.
    pub(super) exports: Vec<SymbolId>,
}

/// Pass 4: `createEffect(compute, apply)` whose compute returns `source`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct EffectRow {
    pub(super) source: SymbolId,
    /// Apply's first parameter.
    pub(super) parameter: SymbolId,
    /// The parameter's own display and declaration, used when `source` has
    /// no accessor entry.
    pub(super) display: SymbolId,
    pub(super) declaration: Location,
}

/// Pass 5: `const name = source`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AliasRow {
    pub(super) source: SymbolId,
    pub(super) immutable: bool,
    /// Each name with a symbol: (symbol, display).
    pub(super) names: Vec<(SymbolId, SymbolId)>,
}

/// Passes 6 and 8: a binding whose initializer may name or view a source:
/// the symbols its pass tests in order (the first that is a source is its
/// root), and its names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ViewRow {
    pub(super) candidates: Vec<SymbolId>,
    pub(super) names: Vec<(SymbolId, SymbolId)>,
}

/// Pass 7: a component's first identifier parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PropRoot {
    pub(super) symbol: SymbolId,
    pub(super) display: SymbolId,
    pub(super) declaration: Location,
    pub(super) uncertain: bool,
}

/// The component status of `function`, the `ordinal`-th function of the
/// file in slot `slot` ([`SemanticLookup::function_component_status`]), read
/// from the file's own census answer so a reader records that file's
/// statuses, not every file's.
pub(super) fn component_status(
    lookup: &SemanticLookup<'_>,
    statuses: Option<&[crate::indexes::ComponentIdentity]>,
    file: &FileFacts,
    ordinal: usize,
    function: &solid_facts::ast::FunctionFact,
) -> crate::indexes::ComponentStatus {
    use crate::indexes::{ComponentIdentity, ComponentStatus};
    match statuses.and_then(|statuses| statuses.get(ordinal)) {
        Some(ComponentIdentity::Proven | ComponentIdentity::Escaped) => ComponentStatus::Proven,
        Some(ComponentIdentity::Possible) => ComponentStatus::Uncertain,
        Some(ComponentIdentity::No) => {
            crate::indexes::unproven_component_status(lookup.dialect, file, function)
        }
        None => lookup.function_component_status(file, function),
    }
}

/// The component census's answer for the file in `slot`, when its functions
/// are keyed by distinct spans (the lookup's statuses are keyed by span, so
/// two functions with one span share an answer there).
pub(super) fn file_statuses(
    lookup: &SemanticLookup<'_>,
    slot: usize,
    file: &FileFacts,
) -> Option<Arc<Vec<crate::indexes::ComponentIdentity>>> {
    let mut spans = file
        .ast
        .functions
        .iter()
        .map(|function| function.span)
        .collect::<Vec<_>>();
    spans.sort_unstable();
    if spans.windows(2).any(|pair| pair[0] == pair[1]) {
        return None;
    }
    let key = lookup.memo_slot_of(slot);
    let statuses = lookup
        .memo_world()
        .answer::<crate::indexes::FileComponentStatuses>(&key);
    (statuses.len() == file.ast.functions.len()).then_some(statuses)
}

/// Pass 4's fallback for a returned identifier with no entity: the symbol
/// whose source declaration in `path` is named `name`, the one at the least
/// location when several are (a `HashMap` scan used to pick any of them;
/// none of the fixtures or corpus projects reaches the fallback).
pub(super) fn declared_symbol(
    source_declarations: &HashMap<SymbolId, typefacts::Declaration>,
    path: &str,
    name: &str,
) -> Option<SymbolId> {
    source_declarations
        .iter()
        .filter(|(_, declaration)| {
            &*declaration.name == name && &*declaration.location.path == path
        })
        .min_by(|(left_symbol, left), (right_symbol, right)| {
            (
                left.location.start_byte,
                left.location.end_byte,
                left_symbol,
            )
                .cmp(&(
                    right.location.start_byte,
                    right.location.end_byte,
                    right_symbol,
                ))
        })
        .map(|(symbol, _)| symbol.clone())
}

/// The largest function an argument holds.
fn argument_function<'f>(
    file: &'f FileFacts,
    argument: &solid_facts::ast::ArgumentFact,
) -> Option<&'f solid_facts::ast::FunctionFact> {
    file.ast
        .functions
        .iter()
        .filter(|function| argument.span.contains(function.span))
        .max_by_key(|function| function.span.end - function.span.start)
}

/// The rows of the file in `slot`.
pub(super) fn file_rows(ctx: &StageContext<'_>, slot: usize, file: &FileFacts) -> FileRows {
    let StageContext {
        entities,
        source_declarations,
        symbol_names,
        semantic_lookup: lookup,
        ..
    } = *ctx;
    let path = file.path.shared();
    let dialect = lookup.dialect;
    let name_of = |span: Span| symbol_id(file.source_text(span).unwrap_or_default());
    let names = |binding: &solid_facts::ast::BindingFact| {
        binding
            .names
            .iter()
            .filter_map(|name| {
                entities
                    .get(&location(path.clone(), name.span))
                    .map(|symbol| (symbol.clone(), name_of(name.span)))
            })
            .collect::<Vec<_>>()
    };
    let mut rows = FileRows::default();
    // Pass 3: children callbacks' accessor parameters.
    for element in &file.ast.jsx_elements {
        let primitive = jsx_primitive_name(file, element, entities, symbol_names, dialect);
        let keyed = element
            .boolean_properties
            .iter()
            .find(|property| file.source_text(property.name) == Some("keyed"))
            .map(|property| property.value);
        let key = match keyed {
            Some(true) => solid_dialect::KeyForm::Keyed,
            Some(false) => solid_dialect::KeyForm::Unkeyed,
            None if element
                .properties
                .iter()
                .any(|property| file.source_text(*property) == Some("keyed")) =>
            {
                dynamic_key_form(file, element, lookup)
            }
            None => solid_dialect::KeyForm::Absent,
        };
        // Which children parameters are accessors is the dialect's question.
        let parameter_indices = known_primitive(&primitive).map_or(&[][..], |primitive| {
            dialect.children_accessor_parameters(primitive, key)
        });
        if parameter_indices.is_empty() {
            continue;
        }
        // The accessor parameters belong to the function that IS this
        // element's children value, and to no other function inside it.
        let children_functions = jsx_children_function_spans(file, element);
        for function in file
            .ast
            .functions
            .iter()
            .filter(|function| children_functions.contains(&function.span))
        {
            for index in parameter_indices {
                let Some(parameter) = function
                    .parameters
                    .get(*index)
                    .and_then(|parameter| parameter.names.first())
                else {
                    continue;
                };
                let declaration = location(path.clone(), parameter.span);
                if let Some(symbol) = entities.get(&declaration) {
                    rows.children
                        .push((symbol.clone(), name_of(parameter.span), declaration));
                }
            }
        }
    }
    // Pass 4: effect apply parameters of store-returning computes.
    for call in &file.ast.calls {
        if !crate::call_primitive_name_cached(lookup, file, call, entities, symbol_names, dialect)
            .as_ref()
            .and_then(PrimitiveName::primitive)
            .is_some_and(|primitive| {
                matches!(
                    primitive,
                    Primitive::CreateEffect | Primitive::CreateRenderEffect
                )
            })
        {
            continue;
        }
        let Some(compute) = call
            .arguments
            .first()
            .and_then(|argument| argument_function(file, argument))
        else {
            continue;
        };
        let returned = compute.expression_return.as_ref().or_else(|| {
            file.ast.returns.iter().find(|returned| {
                compute.body.contains(returned.span)
                    && containing_ast_function(&file.ast, returned.span)
                        .is_some_and(|owner| owner.span == compute.span)
            })
        });
        let Some(source) = returned.and_then(|returned| {
            entities
                .get(&location(path.clone(), returned.span))
                .cloned()
                .or_else(|| {
                    let name = (returned.value == solid_facts::ast::ReturnValueKind::Identifier)
                        .then_some(returned.span)
                        .and_then(|span| file.source_text(span))?;
                    let symbol = declared_symbol(source_declarations, file.path.as_str(), name);
                    trace::note(trace::Dep::DeclaredIn(
                        path.clone(),
                        Arc::from(name),
                        symbol.clone(),
                    ));
                    symbol
                })
        }) else {
            continue;
        };
        let Some(apply) = call
            .arguments
            .get(1)
            .and_then(|argument| argument_function(file, argument))
        else {
            continue;
        };
        let Some(parameter) = apply
            .parameters
            .first()
            .and_then(|parameter| parameter.names.first())
        else {
            continue;
        };
        let declaration = location(path.clone(), parameter.span);
        let Some(parameter_symbol) = entities.get(&declaration) else {
            continue;
        };
        rows.effects.push(EffectRow {
            source,
            parameter: parameter_symbol.clone(),
            display: name_of(parameter.span),
            declaration,
        });
    }
    // Passes 5, 6 and 8: bindings that may alias a setter, an action, a
    // store or a props object.
    for binding in &file.ast.bindings {
        let source = binding
            .initializer_identifier
            .as_ref()
            .and_then(|identifier| entities.get(&location(path.clone(), identifier.span)));
        let views = binding
            .call_initializer
            .map(|initializer| view_candidates(file, initializer, entities, lookup))
            .unwrap_or_default();
        if source.is_none() && views.is_empty() {
            continue;
        }
        let names = names(binding);
        if names.is_empty() {
            continue;
        }
        if let Some(source) = source {
            rows.aliases.push(AliasRow {
                source: source.clone(),
                immutable: binding.immutable,
                names: names.clone(),
            });
        }
        if !views.is_empty() {
            rows.views.push(ViewRow {
                candidates: views.clone(),
                names: names.clone(),
            });
        }
        let mut candidates = source.cloned().into_iter().collect::<Vec<_>>();
        candidates.extend(views);
        rows.prop_aliases.push(ViewRow { candidates, names });
    }
    // Pass 7: props parameters of possible components.
    let statuses = file_statuses(lookup, slot, file);
    for (ordinal, function) in file.ast.functions.iter().enumerate() {
        let status = component_status(
            lookup,
            statuses.as_deref().map(Vec::as_slice),
            file,
            ordinal,
            function,
        );
        if status == crate::indexes::ComponentStatus::No {
            continue;
        }
        let Some(parameter) = function
            .parameters
            .first()
            .filter(|parameter| parameter.shape == solid_facts::ast::BindingShape::Identifier)
            .and_then(|parameter| parameter.names.first())
        else {
            continue;
        };
        let declaration = location(path.clone(), parameter.span);
        if let Some(symbol) = entities.get(&declaration) {
            rows.prop_roots.push(PropRoot {
                symbol: symbol.clone(),
                display: name_of(parameter.span),
                declaration,
                uncertain: status == crate::indexes::ComponentStatus::Uncertain,
            });
        }
    }
    // Pass 9: every symbol the file writes.
    let mut written = crate::value_identity::written_symbols(file, entities)
        .cloned()
        .collect::<Vec<_>>();
    for target in &file.ast.iteration_targets {
        for identifier in file.ast.identifiers_within(*target) {
            written.extend(entities.at(file.path.as_str(), identifier.span).cloned());
        }
    }
    written.sort_unstable();
    written.dedup();
    rows.written = written;
    // Pass 10: the project functions JSX tags render, and the exports.
    for (ordinal, element) in file.ast.jsx_elements.iter().enumerate() {
        if let Some((target_file, target)) = trace::render_target(lookup, file, element.name.span) {
            rows.renders.push((
                u32::try_from(ordinal).expect("fewer than 2^32 elements"),
                target_file,
                target,
            ));
        }
    }
    let mut exports = file
        .ast
        .exports
        .iter()
        .flat_map(|export| export.specifiers.iter().chain(export.declarations.iter()))
        .filter(|specifier| !specifier.type_only)
        .filter_map(|specifier| {
            entities
                .get(&location(path.clone(), specifier.local.span))
                .cloned()
        })
        .collect::<Vec<_>>();
    exports.sort_unstable();
    exports.dedup();
    rows.exports = exports;
    rows
}

/// Pass 1: a former tuple-slot source candidate (ADR 0172's withdrawn
/// identities), kept unless `unless_runtime` and the symbol is a runtime
/// source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TupleRow {
    pub(super) symbol: SymbolId,
    pub(super) source: TypedSource,
    pub(super) unless_runtime: bool,
}

/// Pass 1's rows of one file, under the tuple-iteration veto: a tuple type
/// describes the factory's result, not the values a replaced iterator hands
/// the binding, so former candidates stay typed sources.
pub(super) fn tuple_rows(ctx: &StageContext<'_>, file: &FileFacts) -> Vec<TupleRow> {
    let StageContext {
        project_indexes,
        entities,
        symbol_names,
        semantic_lookup,
        ..
    } = *ctx;
    let mut rows = Vec::new();
    let ast_index = &project_indexes.ast_files_by_path[file.path.as_str()];
    for call in &file.ast.calls {
        for (name, value) in
            crate::callback_return::withdrawn_tuple_candidates(semantic_lookup, file, call)
        {
            let kind = match value.kind.as_str() {
                "accessor" => ReactiveSourceKind::Accessor,
                "store-path" => ReactiveSourceKind::Store,
                _ => continue,
            };
            if let Some(symbol) = entities.at(file.path.as_str(), name) {
                rows.push(TupleRow {
                    symbol: symbol.clone(),
                    source: TypedSource {
                        name: symbol_id(file.source_text(name).unwrap_or_default()),
                        declaration: location(file.path.shared(), name),
                        kind,
                        implementation_visible: false,
                        held_body_unfollowed: false,
                    },
                    unless_runtime: false,
                });
            }
        }
    }
    // Identify only former candidates under the pre-veto premise. Never
    // install that derivation as runtime source authority: a primitive tuple
    // consumed inside a wrapper can also be replaced.
    for binding in file.ast.bindings.iter().filter(|binding| {
        binding.immutable
            && binding.shape == solid_facts::ast::BindingShape::Identifier
            && binding.names.len() == 1
    }) {
        let name = &binding.names[0];
        let Some(symbol) = entities.at(file.path.as_str(), name.span) else {
            continue;
        };
        if crate::indexes::binding_written(file, name.span) {
            continue;
        }
        let Some(call) = binding
            .call_initializer
            .and_then(|span| ast_index.call_by_span(span))
        else {
            continue;
        };
        let Some((_, _, primitive)) = returned_source_through(
            semantic_lookup,
            file,
            call,
            entities,
            symbol_names,
            (RETURNED_SOURCE_FORWARDS, false),
        ) else {
            continue;
        };
        rows.push(TupleRow {
            symbol: symbol.clone(),
            source: TypedSource {
                name: symbol_id(file.source_text(name.span).unwrap_or_default()),
                declaration: location(file.path.shared(), name.span),
                kind: if semantic_lookup.dialect.returns_store(primitive) {
                    ReactiveSourceKind::Store
                } else {
                    ReactiveSourceKind::Accessor
                },
                implementation_visible: false,
                held_body_unfollowed: false,
            },
            unless_runtime: true,
        });
    }
    rows
}

/// What a typed implementation proves (pass 2's [`TypedImplementation`],
/// as data).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Typed {
    Source(SymbolId),
    Visible,
    HeldUnfollowed,
    Unknown,
}

/// Pass 2: one entity a dialect source or setter alias types, in entity
/// order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum TypedRow {
    /// An `Accessor`, `Signal` or `Store` typed value: admitted unless the
    /// symbol already is an accessor.
    Source {
        symbol: SymbolId,
        kind: ReactiveSourceKind,
        name: SymbolId,
        declaration: Location,
        typed: Typed,
    },
    /// A `Setter` or `StoreSetter` typed value: first wins.
    Setter {
        symbol: SymbolId,
        name: SymbolId,
        declaration: Location,
        kind: ReactiveSourceKind,
    },
}

/// Pass 2's rows of one TypeScript path's entities, judged against pass 0's
/// runtime sources and kinds (the judge records what it read of them).
pub(super) fn typed_rows(
    ctx: &StageContext<'_>,
    entities_at_path: &[typefacts::EntityFact],
    runtime_sources: &HashSet<SymbolId>,
    source_kinds: &HashMap<SymbolId, ReactiveSourceKind>,
    literal_member_slots: &crate::literal_member_slots::LiteralMemberSlots<'_, '_>,
) -> Vec<TypedRow> {
    let StageContext {
        entities,
        source_declarations,
        semantic_lookup,
        ..
    } = *ctx;
    let mut rows = Vec::new();
    for entity in entities_at_path {
        let Some(descriptor) = &entity.type_descriptor else {
            continue;
        };
        if !semantic_lookup
            .dialect
            .owns_module(descriptor.origin_module.as_ref())
        {
            continue;
        }
        let Some(symbol) = entities.exact(&entity.location) else {
            continue;
        };
        let Some((role, type_declaration)) =
            descriptor
                .alias_declarations
                .iter()
                .find_map(|declaration| {
                    semantic_lookup
                        .dialect
                        .type_role(descriptor.origin_module.as_ref(), declaration.name.as_ref())
                        .map(|role| (role, declaration.location.clone()))
                })
        else {
            continue;
        };
        if matches!(
            role,
            solid_dialect::TypeRole::Component | solid_dialect::TypeRole::Owner
        ) {
            continue;
        }
        if semantic_lookup
            .file_by_path(entity.location.path.as_ref())
            .is_some_and(|file| {
                let (Ok(start), Ok(end)) = (
                    u32::try_from(entity.location.start_byte),
                    u32::try_from(entity.location.end_byte),
                ) else {
                    return true;
                };
                crate::callback_return::binding_is_callback_result(
                    semantic_lookup,
                    file,
                    solid_facts::core::Span::new(start, end),
                )
            })
        {
            continue;
        }
        // ADR 0172: a tuple containing an accessor is not itself an accessor.
        // Demanded member/call spans may carry the member's type beside a
        // receiver symbol. Only an exact value's own callable type can
        // introduce the typed root; a member must also resolve separately
        // from its receiver. Nested alias declarations cannot do so.
        if matches!(
            role,
            solid_dialect::TypeRole::Accessor | solid_dialect::TypeRole::Signal
        ) && (entity.callability != Some(typefacts::Callability::Callable)
            || semantic_lookup
                .file_by_path(entity.location.path.as_ref())
                .is_some_and(|file| {
                    crate::value_identity::binding_has_write(file, entities, symbol)
                })
            || !semantic_lookup
                .file_by_path(entity.location.path.as_ref())
                .is_some_and(|file| {
                    file.ast.identifiers.iter().any(|identifier| {
                        u64::from(identifier.span.start) == entity.location.start_byte
                            && u64::from(identifier.span.end) == entity.location.end_byte
                    }) || file.ast.members.iter().any(|member| {
                        ((u64::from(member.span.start) == entity.location.start_byte
                            && u64::from(member.span.end) == entity.location.end_byte)
                            || (u64::from(member.property.start) == entity.location.start_byte
                                && u64::from(member.property.end) == entity.location.end_byte))
                            && !file.ast.computed_members.contains(&member.span)
                            && entities.at(file.path.as_str(), member.property) == Some(symbol)
                            && entities.at(file.path.as_str(), member.object) != Some(symbol)
                    })
                }))
        {
            continue;
        }
        // ADR 0286: a property an object literal declares is typed by what
        // the literal stored, which is not what its slot holds once the
        // object reached code that may replace the member.
        if literal_member_slots.replaceable(symbol) {
            continue;
        }
        let declaration = source_declarations.get(symbol);
        let (name, local_location) = declaration.map_or_else(
            || ("accessor".into(), entity.location.clone()),
            |declaration| (declaration.name.clone(), declaration.location.clone()),
        );
        let declaration_location = if type_declaration.path.is_empty() {
            local_location
        } else {
            type_declaration
        };
        // ADR 0294: the type names what the value may be, never that calling
        // or reading it reads reactive state. A proven source keeps what
        // proved it. Otherwise the value the slot holds decides.
        let typed_kind = match role {
            solid_dialect::TypeRole::Accessor | solid_dialect::TypeRole::Signal => {
                Some(ReactiveSourceKind::Accessor)
            }
            solid_dialect::TypeRole::Store => Some(ReactiveSourceKind::Store),
            _ => None,
        };
        if let Some(kind) = typed_kind {
            // Judged even when the symbol already is an accessor: the fold
            // skips the row then, and the judge reads nothing it writes.
            let typed = match typed_implementation(
                semantic_lookup,
                entities,
                literal_member_slots,
                &entity.location,
                symbol,
                kind,
                runtime_sources,
                source_kinds,
            ) {
                TypedImplementation::Source(source) => Typed::Source(source),
                TypedImplementation::Visible => Typed::Visible,
                TypedImplementation::HeldUnfollowed => Typed::HeldUnfollowed,
                TypedImplementation::Unknown => Typed::Unknown,
            };
            rows.push(TypedRow::Source {
                symbol: symbol.clone(),
                kind,
                name: symbol_id(name.as_ref()),
                declaration: declaration_location,
                typed,
            });
            continue;
        }
        match role {
            solid_dialect::TypeRole::Accessor
            | solid_dialect::TypeRole::Signal
            | solid_dialect::TypeRole::Store => unreachable!(),
            solid_dialect::TypeRole::Setter | solid_dialect::TypeRole::StoreSetter => {
                let kind = if role == solid_dialect::TypeRole::StoreSetter {
                    ReactiveSourceKind::Store
                } else {
                    ReactiveSourceKind::Accessor
                };
                rows.push(TypedRow::Setter {
                    symbol: symbol.clone(),
                    name: symbol_id(name.as_ref()),
                    declaration: declaration_location,
                    kind,
                });
            }
            solid_dialect::TypeRole::Component | solid_dialect::TypeRole::Owner => unreachable!(),
        }
    }
    rows
}
