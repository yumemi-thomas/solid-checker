//! The local-access stage: per-file reads and writes of reactive
//! sources, classified by execution role, with per-file reuse.

use crate::cache::{
    CachedLocalAccessFile, CachedLocalAccesses, LocalAccessBuild, LocalAccessResult,
    LocalAccessSymbolState, SourceDiscoveryTypeScriptDelta, same_compiler_semantics,
};
use crate::owners::{
    LoadingCover, analysis_context, async_read_role, containing_ast_function,
    containing_leaf_owner, counts_as_strict_read_root, enclosing_render_function,
    function_binding_name, inside_known_value_function_argument, inside_non_component_function,
    read_loading_cover, typed_accessor_descriptor_at,
};
use crate::pipeline::{parallel_file_chunk_results, parallel_slice_results};
use crate::source_discovery::{AsyncSourceOptions, PropUse, PropsReactivityIndex};
use crate::{
    ActionInvocation, AsyncRead, ExecutionRole, ReactiveRead, ReactiveSourceKind, ReactiveWrite,
    location,
};

use crate::{HashMap, HashSet};
use std::sync::Arc;

use crate::execution_role::{
    RootBodyGuard, allowed_callback_spans, async_execution_role, callee_callback_timing,
    control_flow_execution_role, direct_control_flow_body_role, host_callback_timing,
    missing_jsx_census, named_callback_execution_role, nested_literal_runs_during_body,
    pending_accessor_probe, read_analysis_context, semantic_execution_role,
    semantic_write_execution_role, unclassified_callback_reaches_modelled_consumer,
};
use crate::identity::SymbolId;
use crate::indexes::{EntitySymbols, SemanticLookup};
use solid_facts::ProjectFacts;
use typefacts::{Declaration, Location};

/// The obligation context of a member call on a caller-supplied value
/// (ADR 0509), dropped where accessor transport resolves the call.
pub(crate) const UNRESOLVED_HANDED_MEMBER: &str = "handed-member-dispatch";

/// [`UNRESOLVED_HANDED_MEMBER`] where the call may only read (its role reports
/// no write or action): a read row at the call speaks for it.
pub(crate) const UNRESOLVED_HANDED_READ: &str = "handed-member-read";

/// How many member, alias and destructuring steps [`UNRESOLVED_HANDED_MEMBER`]
/// follows from a callee to the value a caller supplied.
pub(crate) const HANDED_DEPTH: usize = 6;

/// The tables [`handed_value`] reads.
pub(crate) struct HandedTables<'a, 'l> {
    pub(crate) lookup: &'a SemanticLookup<'l>,
    pub(crate) entities: &'a EntitySymbols,
    pub(crate) prop_sources: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
    pub(crate) accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) setters: &'a HashMap<
        SymbolId,
        (
            SymbolId,
            Location,
            crate::OwnedWriteOption,
            ReactiveSourceKind,
        ),
    >,
    pub(crate) actions: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) props_reactivity: &'a PropsReactivityIndex,
}

/// ADR 0509: whether the value at `span` is one a caller supplied
/// ([`UNRESOLVED_HANDED_MEMBER`]), through at most `depth` steps.
pub(crate) fn handed_value(
    t: &HandedTables<'_, '_>,
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    depth: usize,
) -> bool {
    if depth == 0 {
        return false;
    }
    let span = file.ast.peel_ts_sugar_span(span);
    if let Some(member) = file.ast.member_at(span) {
        // A computed member too (`pair[1]`): this only opens obligations.
        return handed_value(t, file, member.object, depth - 1);
    }
    // A local literal destructured or indexed later (`const [, f] = [0,
    // ctx.read]`, `const { f } = { f: imported }`): one of its values.
    if let Some(literal) = file.ast.array_literal_at(span) {
        return literal
            .elements
            .iter()
            .any(|element| handed_value(t, file, element.span, depth - 1));
    }
    // A local object literal exactly at `span`: its own top-level values
    // (an open literal, with a spread or a computed key, may hold anything).
    if let Some(shape) = file
        .ast
        .object_get_shapes
        .iter()
        .find(|shape| shape.span == span)
    {
        if !shape.closed {
            return true;
        }
        let own = |property: &&solid_facts::ast::ObjectPropertyFact| {
            span.contains(property.span)
                && !file.ast.object_properties.iter().any(|outer| {
                    outer.span != property.span
                        && span.contains(outer.span)
                        && outer.span.contains(property.span)
                })
        };
        return file
            .ast
            .object_properties
            .iter()
            .filter(own)
            .any(|property| {
                handed_value(
                    t,
                    file,
                    property.shorthand_binding.unwrap_or(property.value),
                    depth - 1,
                )
            });
    }
    if !file.ast.identifiers.iter().any(|identifier| {
        identifier.span == span && identifier.role == solid_facts::ast::IdentifierRole::Reference
    }) {
        return false;
    }
    if t.entities
        .get(&location(file.path.shared(), span))
        .is_some_and(|symbol| {
            t.prop_sources.contains_key(symbol)
                && t.source_kinds.get(symbol) != Some(&ReactiveSourceKind::Store)
        })
    {
        return true;
    }
    // An alias whose own binding has no row of a value that has one (`const
    // { f } = { f: imported }`): the alias was not followed, so the call may
    // read, write or act. The value's symbol is the one a call of it
    // resolves to (through an import).
    let is_source = |symbol: &str| {
        t.accessors.contains_key(symbol)
            || t.setters.contains_key(symbol)
            || t.actions.contains_key(symbol)
    };
    if depth < HANDED_DEPTH
        && (t.lookup.callee_symbol(file, span).is_some_and(is_source)
            // A value position no symbol was demanded at: its binder's
            // declaration (an import's local name) carries it.
            || file
                .ast
                .reference_declaration(span)
                .and_then(|declaration| t.lookup.callee_symbol(file, declaration))
                .is_some_and(is_source))
    {
        return true;
    }
    let Some(declaration) = file.ast.reference_declaration(span) else {
        return false;
    };
    // A slot of a component's destructured props parameter.
    if file.ast.functions.iter().any(|function| {
        function.parameters.first().is_some_and(|parameter| {
            parameter.shape == solid_facts::ast::BindingShape::Object
                && parameter
                    .object_slots
                    .iter()
                    .any(|slot| slot.local.span == declaration)
                && t.props_reactivity
                    .for_declaration(&location(file.path.shared(), parameter.pattern))
                    .is_some()
        })
    }) {
        return true;
    }
    let Some(binding) = file
        .ast
        .bindings
        .iter()
        .find(|binding| binding.names.iter().any(|name| name.span == declaration))
    else {
        return false;
    };
    if let Some(call) = binding
        .call_initializer
        .and_then(|initializer| file.ast.call_at(initializer))
    {
        if call.construct {
            return false;
        }
        return match t.lookup.primitive_at_call(file, call.span) {
            Some(primitive) => {
                t.lookup
                    .dialect
                    .name_of(primitive)
                    .and_then(|name| t.lookup.dialect.context_role(name))
                    == Some(solid_dialect::ContextRole::Read)
            }
            None => t
                .lookup
                .function_called_at(file.path.as_str(), file.ast.peel_ts_sugar_span(call.callee))
                .is_some_and(|(hook_file, hook)| {
                    !returned_slot_is_literal_function(hook_file, hook, binding, declaration)
                }),
        };
    }
    binding
        .initializer
        .is_some_and(|initializer| handed_value(t, file, initializer, depth - 1))
}

/// Whether the slot of `binding` named at `declaration` is, in the one
/// return of `hook` (a literal), a function literal written there: its body
/// is visible, and the call graph follows a call of it (ADR 0282), so the
/// handed-member net is not needed for it.
fn returned_slot_is_literal_function(
    file: &solid_facts::FileFacts,
    hook: &solid_facts::ast::FunctionFact,
    binding: &solid_facts::ast::BindingFact,
    declaration: solid_facts::core::Span,
) -> bool {
    let Some(returned) = crate::interproc::accessor_provenance::single_return(file, hook) else {
        return false;
    };
    let returned = file.ast.peel_ts_sugar_span(returned);
    // The whole result (`const task = make(); task()`): a function literal
    // the hook returns, directly or through a `const` it binds to one.
    if binding.shape == solid_facts::ast::BindingShape::Identifier {
        if file.ast.function_at(returned).is_some() {
            return true;
        }
        let Some(declaration) = file.ast.reference_declaration(returned) else {
            return false;
        };
        return file.ast.bindings.iter().any(|local| {
            local.immutable
                && local.shape == solid_facts::ast::BindingShape::Identifier
                && local.names.iter().any(|name| name.span == declaration)
                && !crate::indexes::binding_written(file, declaration)
                && local.initializer.is_some_and(|initializer| {
                    file.ast
                        .function_at(file.ast.peel_ts_sugar_span(initializer))
                        .is_some()
                })
        });
    }
    let Some(structure) = file
        .ast
        .returns
        .iter()
        .chain(
            file.ast
                .functions
                .iter()
                .filter_map(|function| function.expression_return.as_ref()),
        )
        .find(|candidate| {
            candidate
                .argument
                .is_some_and(|argument| file.ast.peel_ts_sugar_span(argument) == returned)
        })
        .and_then(|candidate| candidate.structure.as_deref())
        .filter(|structure| structure.complete_literal)
    else {
        return false;
    };
    let value = match binding.shape {
        solid_facts::ast::BindingShape::Array => binding
            .array_slots
            .iter()
            .position(|slot| slot.as_ref().is_some_and(|slot| slot.span == declaration))
            .and_then(|index| structure.elements.get(index).copied().flatten()),
        solid_facts::ast::BindingShape::Object => binding
            .object_slots
            .iter()
            .find(|slot| slot.local.span == declaration)
            .and_then(|slot| {
                structure
                    .properties
                    .iter()
                    .find(|property| property.name.as_str() == slot.property.as_str())
                    .map(|property| property.value)
            }),
        solid_facts::ast::BindingShape::Identifier => None,
    };
    value.is_some_and(|value| {
        file.ast
            .function_at(file.ast.peel_ts_sugar_span(value))
            .is_some()
    })
}

/// Do not apply an incoming-prop witness to a replaced or escaping root.
/// Unknown writes through aliases/views need their own value-flow proof.
pub(super) fn props_root_is_current(
    file: &solid_facts::FileFacts,
    declaration: &Location,
    symbol: &SymbolId,
    entities: &EntitySymbols,
    lookup: &SemanticLookup<'_>,
) -> bool {
    props_root_unaccounted_uses(file, declaration, symbol, entities, lookup)
        .is_some_and(|uses| uses.is_empty())
}

/// The `const` identifier bindings that are exactly the props root, directly
/// or through other such aliases, each with the span of the root identifier in
/// its initializer.
fn props_root_aliases(
    file: &solid_facts::FileFacts,
    symbol: &SymbolId,
    entities: &EntitySymbols,
) -> Vec<(SymbolId, solid_facts::core::Span)> {
    let mut aliases: Vec<(SymbolId, solid_facts::core::Span)> = Vec::new();
    loop {
        let before = aliases.len();
        for binding in &file.ast.bindings {
            let (Some(initializer), [name]) =
                (&binding.initializer_identifier, binding.names.as_slice())
            else {
                continue;
            };
            if !binding.immutable || binding.shape != solid_facts::ast::BindingShape::Identifier {
                continue;
            }
            let (Some(alias), Some(source)) = (
                entities.at(file.path.as_str(), name.span),
                entities.at(file.path.as_str(), initializer.span),
            ) else {
                continue;
            };
            if alias == symbol || aliases.iter().any(|(known, _)| known == alias) {
                continue;
            }
            if source == symbol || aliases.iter().any(|(known, _)| known == source) {
                aliases.push((alias.clone(), initializer.span));
            }
        }
        if aliases.len() == before {
            return aliases;
        }
    }
}

/// Whether the identifier at `span` is a spread attribute of an intrinsic
/// (DOM) element. `@solidjs/web` `spread` (client) and `ssrElement` (server),
/// `2.0.0-rc.13`, only read the object: they enumerate its keys, read each
/// value, and write into their own `newProps`/`prevProps` and the node. A
/// `ref` or `on*` value is called with the node or an event, never the props.
/// A multi-attribute spread compiles to a `merge(..)` view first. Neither
/// path writes, deletes or retains-and-mutates the props object.
fn is_intrinsic_spread_argument(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
) -> bool {
    file.ast.jsx_containing(span).any(|element| {
        crate::execution_role::intrinsic_element(file, element)
            && element
                .spreads
                .iter()
                .any(|spread| file.ast.peel_ts_sugar_span(spread.argument) == span)
    })
}

/// The depth a view call's nested view arguments are followed to.
const VIEW_CALL_DEPTH: usize = 8;

/// Whether the call at `call` provably builds a *view*: a new object whose
/// writes and deletes never reach its sources. This is the one owner of the
/// "inert whole-object use" decision for `omit` and `merge`.
///
/// Both are exact dialect primitives (`SemanticLookup::primitive_at_call`,
/// never a name). On `@solidjs/signals@2.0.0-rc.13` (`dist/dev.js`):
///
/// * `omit(props, ...keys)` always returns `new Proxy(new OmitView(..),
///   omitTraps)` whatever the keys (zero keys, a predicate), and the traps
///   have `set: trueFn, deleteProperty: trueFn`. The first argument must not
///   be a spread, which could supply no source at all.
/// * `merge(...sources)` returns the one non-falsy source *itself* ("The one
///   non-falsy source ... IS the merge"), so `merge(props)` and
///   `merge(props, maybeUndefined)` are aliases. It is a view
///   (`mergeTraps`: `set: trueFn, deleteProperty: trueFn`) only when two
///   sources are non-falsy. A props object always is, so the call is a view
///   when no argument is a spread and some argument is syntactically never
///   falsy: an object or function literal (a function becomes a memo source)
///   or a call that itself builds a view. Anything else could be `undefined`
///   at run time and stays an alias.
///
/// Both premises assume `Proxy` exists (`SUPPORTS_PROXY`); the no-Proxy
/// branch copies descriptors and can return its last source itself.
pub(super) fn call_builds_view(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    lookup: &SemanticLookup<'_>,
    depth: usize,
) -> bool {
    if call.construct {
        return false;
    }
    let Some(primitive) = lookup.primitive_at_call(file, call.span) else {
        return false;
    };
    if lookup.dialect.splits_props(primitive) {
        return call.arguments.first().is_some_and(|first| !first.spread);
    }
    if !lookup.dialect.merges_props_reactivity(primitive) {
        return false;
    }
    call.arguments.len() >= 2
        && call.arguments.iter().all(|argument| !argument.spread)
        && call.arguments.iter().any(|argument| {
            use solid_facts::ast::RuntimeValueKind;
            matches!(
                argument.runtime_value_kind,
                RuntimeValueKind::Object | RuntimeValueKind::Function
            ) || (depth < VIEW_CALL_DEPTH
                && file
                    .ast
                    .call_at(file.ast.peel_ts_sugar_span(argument.span))
                    .is_some_and(|inner| call_builds_view(file, inner, lookup, depth + 1)))
        })
}

/// Whether a component element hands its callee a `merge` *view* rather than
/// possibly the object it spreads. Both lowerings (the native compiler's
/// `shared/component.rs` and `@solidjs/babel-plugin`'s `shared/component.ts`)
/// build one source per run of attributes or children (an object literal) and
/// one per spread, and wrap several sources in `mergeProps`; a lone source is
/// passed as is. rc.13 `merge` returns its one non-falsy source itself, so the
/// same never-falsy test as [`call_builds_view`] applies: the call is a view
/// when some source is provably never falsy, here
///
/// * a non-`ref` attribute (an object literal source; `ref` may lower to no
///   property). A `children` attribute does not count when the element has
///   any raw child node, even whitespace: Babel drops it
///   (`hasChildren && key === "children"`);
/// * a child that renders: whitespace and `{/* */}` lower to nothing, and a
///   spread child is not followed; or
/// * a spread of a member expression (`{...props.extra}`): dynamic, so lowered
///   to a function source (`() => props.extra`), unless the object is an import
///   binding (a namespace member is static and passes through unwrapped) or any
///   comment sits in the spread (Babel's `isDynamic` reads a leading
///   `/*@static*/` marker, `staticMarker` is configurable, and it passes the
///   expression raw).
///
/// A spread of a bare identifier is not: it may be `undefined`, leaving the
/// spread props object as the receiver's own.
pub(super) fn component_element_builds_view(
    file: &solid_facts::FileFacts,
    element: &solid_facts::ast::JsxElementFact,
) -> bool {
    let has_raw_children = !element.children.is_empty();
    if element.attributes.iter().any(|attribute| {
        attribute.namespace.is_some()
            || match file.source_text(attribute.name) {
                Some("ref") => false,
                Some("children") => !has_raw_children,
                _ => true,
            }
    }) {
        return true;
    }
    if element.children.iter().any(|child| {
        // An empty container (`{}`, `{/* c */}`, or one the parser reads as
        // whitespace, e.g. `{\u{200B}}`) is dropped by both lowerings; the
        // parser decides that, not the text.
        if element.empty_children.contains(child) {
            return false;
        }
        file.source_text(*child).is_some_and(|text| {
            // JavaScript's `\s` (Babel's `filterChildren`) also counts U+FEFF
            // as whitespace; Rust's `trim` does not.
            let js_space = |c: char| c.is_whitespace() || c == '\u{FEFF}';
            let text = text.trim_matches(js_space);
            match text.strip_prefix('{') {
                // A container with an expression is an object-literal source,
                // except a spread child, which is passed through raw.
                Some(inner) => !inner.trim_start_matches(js_space).starts_with("..."),
                None => !text.is_empty(),
            }
        })
    }) {
        return true;
    }
    element.spreads.iter().any(|spread| {
        if file
            .source_text(spread.span)
            .is_none_or(|text| text.contains("/*") || text.contains("//"))
        {
            return false;
        }
        let argument = file.ast.peel_ts_sugar_span(spread.argument);
        file.ast.member_at(argument).is_some_and(|member| {
            let object = file.ast.peel_ts_sugar_span(member.object);
            let object_text = file.source_text(object);
            !file.ast.imports.iter().any(|import| {
                import
                    .bindings
                    .iter()
                    .any(|binding| file.source_text(binding.local.span) == object_text)
            })
        })
    })
}

/// Whether the identifier at `span` is passed to a call that builds a view
/// of it (`omit(props, ..)` as the first argument, `merge(.., props, ..)`),
/// so the call neither writes it nor hands out an alias that can.
fn is_view_argument(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    // An argument that peels to `span` lies inside the argument's own span, so
    // only the arguments containing `span` can match. They arrive ordered by
    // (call, argument); the first match of each call decides that call.
    let mut decided: Option<&solid_facts::ast::CallFact> = None;
    file.ast.arguments_containing(span).any(|(call, index)| {
        let argument = &call.arguments[index];
        if argument.spread
            || file.ast.peel_ts_sugar_span(argument.span) != span
            || decided.is_some_and(|seen| std::ptr::eq(seen, call))
        {
            return false;
        }
        decided = Some(call);
        call_builds_view(file, call, lookup, 0)
            && (index == 0
                || lookup
                    .primitive_at_call(file, call.span)
                    .is_some_and(|primitive| lookup.dialect.merges_props_reactivity(primitive)))
    })
}

/// Whether every other prop source attached to the props declaration is a
/// `const` alias of the root (see [`props_root_aliases`]) or a `const`
/// identifier bound directly to a call that builds a view. Writes through a view are no-ops;
/// writes through an alias or a destructured rest reach the object, so those
/// still block.
pub(super) fn props_root_attachments_are_views(
    file: &solid_facts::FileFacts,
    prop_sources: &HashMap<SymbolId, (SymbolId, Location)>,
    declaration: &Location,
    symbol: &SymbolId,
    entities: &EntitySymbols,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let aliases = props_root_aliases(file, symbol, entities);
    prop_sources
        .iter()
        .filter(|(other, (_, attached))| *other != symbol && attached == declaration)
        .all(|(other, _)| {
            aliases.iter().any(|(alias, _)| alias == other)
                || file.ast.bindings.iter().any(|binding| {
                    binding.immutable
                        && binding.shape == solid_facts::ast::BindingShape::Identifier
                        && binding.names.len() == 1
                        && binding.names.first().is_some_and(|name| {
                            entities.at(file.path.as_str(), name.span) == Some(other)
                        })
                        && binding
                            .call_initializer
                            .and_then(|initializer| file.ast.call_at(initializer))
                            .is_some_and(|call| call_builds_view(file, call, lookup, 0))
                })
        })
}

/// The uses of a props root that may alias or escape it (every use that is
/// not a member access, erased TypeScript syntax, or an argument of a call
/// that builds a view of it, see [`call_builds_view`]), or `None` when the
/// binding or a member through it is written in its file.
pub(super) fn props_root_unaccounted_uses(
    file: &solid_facts::FileFacts,
    declaration: &Location,
    symbol: &SymbolId,
    entities: &EntitySymbols,
    lookup: &SemanticLookup<'_>,
) -> Option<Vec<solid_facts::core::Span>> {
    use solid_facts::core::Span;
    if declaration.path.as_ref() != file.path.as_str() {
        return None;
    }
    let (Ok(start), Ok(end)) = (
        u32::try_from(declaration.start_byte),
        u32::try_from(declaration.end_byte),
    ) else {
        return None;
    };
    if crate::indexes::binding_written(file, Span::new(start, end)) {
        return None;
    }
    // `arguments[0]` is the props object under another name (`delete
    // arguments[0].count` removes the caller's getter). Any `arguments` inside
    // the declaring function leaves the root unprovable.
    let declaration_span = Span::new(start, end);
    if file
        .ast
        .functions
        .iter()
        .filter(|function| {
            function
                .parameters
                .iter()
                .any(|parameter| parameter.pattern.contains(declaration_span))
        })
        .any(|function| {
            file.ast
                .identifiers_within(function.span)
                .any(|identifier| file.source_text(identifier.span) == Some("arguments"))
        })
    {
        return None;
    }
    // A `const` alias (`const local = props`) is the same root: its own uses
    // are judged exactly like the parameter's, and it is never a separate
    // witness (the read rule only certifies the parameter's own reads).
    let aliases = props_root_aliases(file, symbol, entities);
    let names_root = |span| {
        let span = file.ast.peel_ts_sugar_span(span);
        file.ast.has_reference_identifier_at(span)
            && entities.at(file.path.as_str(), span).is_some_and(|named| {
                named == symbol || aliases.iter().any(|(alias, _)| alias == named)
            })
    };
    let member_written = file
        .ast
        .assignments
        .iter()
        .map(|assignment| assignment.target)
        .chain(file.ast.deleted_targets.iter().copied())
        .chain(file.ast.iteration_targets.iter().copied())
        .any(|target| {
            file.ast
                .members_within(target)
                .any(|member| names_root(member.object))
        });
    // Every unaccounted use of the object can alias or escape it, including
    // shorthand properties, returns and nested parameter defaults. Erased
    // parts of transparent TS wrappers do not evaluate the object.
    if member_written {
        return None;
    }
    // The spans some member access takes as its (peeled) object, built when a
    // root use first needs it.
    let mut member_objects: Option<HashSet<solid_facts::core::Span>> = None;
    let escapes = file
        .ast
        .identifiers
        .iter()
        .filter(|identifier| {
            names_root(identifier.span)
                && !file.ast.transparent_wrappers.iter().any(|wrapper| {
                    wrapper.span.contains(identifier.span)
                        && !wrapper.inner.contains(identifier.span)
                })
                && !member_objects
                    .get_or_insert_with(|| {
                        file.ast
                            .members
                            .iter()
                            .map(|member| file.ast.peel_ts_sugar_span(member.object))
                            .collect()
                    })
                    .contains(&identifier.span)
                && !is_view_argument(file, identifier.span, lookup)
                && !aliases
                    .iter()
                    .any(|(_, initializer)| *initializer == identifier.span)
                && !is_intrinsic_spread_argument(file, identifier.span)
        })
        .map(|identifier| identifier.span)
        .collect();
    Some(escapes)
}

/// Whether `site` runs while a call of `function` does: written directly in
/// its body (no nested function), not in JSX the body writes (it may lower to
/// a getter), not in a binding or assignment pattern, and the function neither
/// async nor a generator (ADR 0462). The function may itself sit in JSX.
fn site_runs_during_local_call(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    site: solid_facts::core::Span,
) -> bool {
    !function.r#async
        && !function.generator
        && crate::owners::written_directly_in(&file.ast, function, site)
        && !file
            .ast
            .jsx_containing(site)
            .any(|element| function.body.contains(element.span))
        && !file
            .ast
            .bindings
            .iter()
            .any(|binding| binding.pattern.contains(site))
        && !file
            .ast
            .assignments
            .iter()
            .any(|assignment| assignment.target.contains(site))
}

/// How many named local functions an async read is followed through to the
/// rendered call that runs it (ADR 0462).
const CALL_ATTRIBUTED_DEPTH: usize = 4;

pub(crate) struct LocalAccessContext<'a, 'facts> {
    pub(crate) facts: &'a ProjectFacts,
    pub(crate) lookup: &'a SemanticLookup<'facts>,
    pub(crate) entities: &'a EntitySymbols,
    pub(crate) symbol_names: &'a HashMap<SymbolId, SymbolId>,
    pub(crate) reachable_calls: &'a HashMap<Location, usize>,
    pub(crate) accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    /// ADR 0294: typed candidates. A read of one whose implementation is not
    /// visible is an obligation, never a read.
    pub(crate) typed_sources: &'a HashMap<SymbolId, crate::source_discovery::TypedSource>,
    pub(crate) accessor_origins: &'a HashMap<SymbolId, (SymbolId, SymbolId, Location)>,
    pub(crate) setters: &'a HashMap<
        SymbolId,
        (
            SymbolId,
            Location,
            crate::OwnedWriteOption,
            ReactiveSourceKind,
        ),
    >,
    pub(crate) actions: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) source_primitives: &'a HashMap<SymbolId, SymbolId>,
    pub(crate) async_sources: &'a HashSet<SymbolId>,
    pub(crate) source_async_options: &'a HashMap<SymbolId, AsyncSourceOptions>,
    /// Whether any file in the analyzed project imports a server rendering
    /// entry point. False means unresolved, not proven CSR: the server entry
    /// may live in another tsconfig or package.
    pub(crate) server_rendering: crate::source_discovery::ServerRenderingPremise,
    pub(crate) source_declarations: &'a HashMap<SymbolId, Declaration>,
    pub(crate) source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
    pub(crate) prop_sources: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) uncertain_prop_sources: &'a HashSet<SymbolId>,
    pub(crate) props_reactivity: &'a PropsReactivityIndex,
}

/// Whether a `parameter-member` argument is proven not to be a reactive
/// source, so the package's member call on it cannot read reactively.
///
/// A Solid store is a proxy typed as the object it wraps, so a declared type
/// never proves the negative. Two things do:
///
/// * the argument's own syntax -- an inline literal is a value created at the
///   call site, and `createStore` never produced it. This holds for a literal
///   that *spreads* a store too, and deliberately so: `[...storeArray]` copies
///   out of the proxy at the call site, so what the callee receives really is
///   snapshot data, and the reactive read is the spread itself. The spread
///   pass below owns that read and reports it in its own execution role;
///   claiming the callee's parameter read as well would report one dependency
///   twice. What stays uncovered is a *nested* proxy surviving the shallow
///   copy; and
/// * an analyzed local binding initialized by a resolved standard-library
///   call, such as `document.createElement("button")`. That value's origin is
///   platform code, which cannot return a Solid store.
///
/// Anything the project cannot see the origin of -- a parameter, a prop, an
/// import, a bare `declare const` -- stays unproven and keeps its SC9012
/// obligation. This is the negative proof, never a default.
pub(crate) fn argument_proves_non_reactive(
    file: &solid_facts::FileFacts,
    argument: &solid_facts::ast::ArgumentFact,
    entities: &EntitySymbols,
    source_kinds: &impl crate::interproc::SourceKindTable,
    lookup: &SemanticLookup<'_>,
) -> bool {
    use solid_facts::ast::RuntimeValueKind;
    if matches!(
        argument.runtime_value_kind,
        RuntimeValueKind::Primitive
            | RuntimeValueKind::Array
            | RuntimeValueKind::Object
            | RuntimeValueKind::Nullish
    ) {
        return true;
    }
    let Some(symbol) = entities.get(&location(file.path.shared(), argument.span)) else {
        return false;
    };
    if source_kinds.source_kind(symbol.as_str()).is_some() {
        return false;
    }
    file.ast.bindings.iter().any(|binding| {
        binding.names.iter().any(|name| {
            entities
                .at(file.path.as_str(), name.span)
                .is_some_and(|candidate| candidate == symbol)
        }) && binding.call_initializer.is_some_and(|initializer| {
            file.ast.call_at(initializer).is_some_and(|call| {
                lookup
                    .resolved_callee_call(file, call.callee)
                    .and_then(|resolved| resolved.declaration.as_ref())
                    .is_some_and(|declaration| declaration.standard_library)
            })
        })
    })
}

pub(crate) struct LocalAccessReuse<'a> {
    pub(crate) typescript_unchanged: bool,
    pub(crate) source_discovery_delta: Option<&'a SourceDiscoveryTypeScriptDelta>,
    /// The files whose own facts are the last build's
    /// ([`crate::source_discovery::SourceDiscovery::unchanged_paths`]).
    pub(crate) unchanged_paths: &'a HashSet<String>,
}

impl LocalAccessContext<'_, '_> {
    pub(crate) fn build(
        &self,
        cache: Option<&mut CachedLocalAccesses>,
        reuse: LocalAccessReuse<'_>,
    ) -> LocalAccessBuild {
        let LocalAccessReuse {
            typescript_unchanged,
            source_discovery_delta,
            unchanged_paths,
        } = reuse;
        let mut result = LocalAccessResult::default();
        let mut reused_files = 0;
        let mut recomputed_files = 0;
        if let Some(cache) = cache {
            let exact_typescript_delta = typescript_unchanged || source_discovery_delta.is_some();
            // The symbols whose source-discovery entries moved since the
            // files were last checked, by value: whichever file's discovery
            // or project pass moved them (ADR 0397).
            let sources = self.sources();
            let mut candidate_dependencies = match &cache.sources {
                Some(previous) => previous.changed_symbols(&sources),
                None => cache.dependency_states.keys().cloned().collect(),
            };
            if let Some(delta) = source_discovery_delta {
                candidate_dependencies.extend(delta.semantic_symbol_ids.iter().cloned());
            }
            for (symbol, previous) in &cache.prop_sources {
                if self.prop_state(symbol) != Some(previous.clone()) {
                    candidate_dependencies.insert(symbol.clone());
                }
            }
            for symbol in self.prop_sources.keys() {
                if !cache.prop_sources.contains_key(symbol) {
                    candidate_dependencies.insert(symbol.clone());
                }
            }
            let changed_dependencies = candidate_dependencies
                .into_iter()
                .filter(|symbol| {
                    cache
                        .dependency_states
                        .get(symbol)
                        .is_some_and(|previous| *previous != self.symbol_state(symbol))
                })
                .collect::<HashSet<_>>();
            let current_paths = self
                .facts
                .files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<HashSet<_>>();
            cache
                .files
                .retain(|path, _| current_paths.contains(path.as_str()));
            let mut recompute = Vec::new();
            for file in &self.facts.files {
                if let Some(cached) = cache.files.get(file.path.as_str())
                    && exact_typescript_delta
                    && self.cached_matches(
                        file,
                        cached,
                        unchanged_paths.contains(file.path.as_str()),
                        &changed_dependencies,
                    )
                {
                    reused_files += 1;
                    continue;
                }
                recompute.push(file);
            }
            // Each file's contribution reads only this build's facts and
            // lookup, so the files are discovered in parallel and recorded in
            // their order.
            let fresh = parallel_slice_results(&recompute, |file| {
                crate::base::reads::record(|| {
                    (
                        self.discover(file),
                        self.dependencies(file),
                        self.call_multiplicities(file),
                    )
                })
            });
            for (file, ((contribution, dependencies, call_multiplicities), reads)) in
                recompute.into_iter().zip(fresh)
            {
                cache.files.insert(
                    file.path.clone(),
                    CachedLocalAccessFile {
                        source_hash: file.source_hash.clone(),
                        proof: crate::base::reads::ReadProof::new(reads, self.lookup),
                        compiler: file.compiler.clone(),
                        dependencies,
                        call_multiplicities,
                        contribution,
                    },
                );
                recomputed_files += 1;
            }
            let current_dependencies = cache
                .files
                .values()
                .flat_map(|file| file.dependencies.iter().cloned())
                .collect::<HashSet<_>>();
            cache
                .dependency_states
                .retain(|symbol, _| current_dependencies.contains(symbol));
            for symbol in current_dependencies {
                if !cache.dependency_states.contains_key(symbol.as_str())
                    || changed_dependencies.contains(symbol.as_str())
                {
                    cache
                        .dependency_states
                        .insert(symbol.clone(), self.symbol_state(&symbol));
                }
            }
            cache
                .prop_sources
                .retain(|symbol, _| self.prop_sources.contains_key(symbol));
            for symbol in self.prop_sources.keys() {
                let state = self
                    .prop_state(symbol)
                    .expect("prop state exists for every prop source");
                if cache.prop_sources.get(symbol) != Some(&state) {
                    cache.prop_sources.insert(symbol.clone(), state);
                }
            }
            let local_access_files = &cache.files;
            for partial in parallel_file_chunk_results(&self.facts.files, |files| {
                let mut partial = LocalAccessResult::default();
                for file in files {
                    if let Some(cached) = local_access_files.get(file.path.as_str()) {
                        append_local_access_result(&mut partial, &cached.contribution);
                    }
                }
                partial
            }) {
                append_local_access_result_owned(&mut result, partial);
            }
            cache.sources = Some(sources);
        } else {
            for contribution in
                parallel_slice_results(&self.facts.files, |file| self.discover(file))
            {
                append_local_access_result(&mut result, &contribution);
                recomputed_files += 1;
            }
        }
        LocalAccessBuild {
            result,
            reused_files,
            recomputed_files,
        }
    }

    /// The tables a file's chunk reads by symbol, as this build has them.
    fn sources(&self) -> crate::cache::LocalAccessSources {
        crate::cache::LocalAccessSources {
            accessors: self.accessors.clone(),
            typed_sources: self.typed_sources.clone(),
            accessor_origins: self.accessor_origins.clone(),
            setters: self.setters.clone(),
            actions: self.actions.clone(),
            source_primitives: self.source_primitives.clone(),
            async_sources: self.async_sources.clone(),
            source_async_options: self.source_async_options.clone(),
            source_kinds: self.source_kinds.clone(),
            server_rendering: self.server_rendering,
        }
    }

    pub(crate) fn dependencies(&self, file: &solid_facts::FileFacts) -> HashSet<SymbolId> {
        file.ast
            .calls
            .iter()
            .map(|call| call.callee)
            .chain(file.ast.members.iter().map(|member| member.object))
            .chain(
                file.ast
                    .members
                    .iter()
                    .map(|member| file.ast.peel_ts_sugar_span(member.object)),
            )
            .chain(file.ast.spreads.iter().map(|spread| spread.argument))
            .chain(
                file.ast
                    .jsx_elements
                    .iter()
                    .map(|element| element.name.span),
            )
            .filter_map(|span| {
                self.entities
                    .get(&location(file.path.shared(), span))
                    .cloned()
            })
            .chain(file.ast.calls.iter().filter_map(|call| {
                self.lookup
                    .callee_symbol(file, call.callee)
                    .map(SymbolId::from)
            }))
            .collect()
    }

    /// The source's declared async/hydration options with the project-level
    /// server-render fact folded into a violation/uncertifiable distinction.
    fn effective_async_options(&self, symbol: &str) -> AsyncSourceOptions {
        let mut options = self
            .source_async_options
            .get(symbol)
            .copied()
            .unwrap_or_default();
        // Three states, not two. A bare `ssrSource: "client"` source is a
        // server-render hole only where server rendering is proven; it is a
        // proof obligation only where the premise is unresolved. Where an
        // explicit rendering selector proves the application is client-only,
        // the hole cannot exist and neither claim is made.
        use crate::source_discovery::ServerRenderingPremise;
        options.server_rendering_unresolved =
            options.ssr_client_bare && self.server_rendering == ServerRenderingPremise::Unresolved;
        options.ssr_client_bare &= self.server_rendering.renders();
        options
    }

    /// The prop-source identity plus its caller classification, the pair the
    /// incremental cache fingerprints: a call site in one file deciding a
    /// prop's reactivity must invalidate the component's file.
    pub(crate) fn prop_state(
        &self,
        symbol: &str,
    ) -> Option<(
        SymbolId,
        Location,
        Option<crate::source_discovery::PropsReactivity>,
        bool,
    )> {
        self.prop_sources.get(symbol).map(|(name, declaration)| {
            (
                name.clone(),
                declaration.clone(),
                self.props_reactivity.for_declaration(declaration).cloned(),
                self.uncertain_prop_sources.contains(symbol),
            )
        })
    }

    pub(crate) fn symbol_state(&self, symbol: &str) -> LocalAccessSymbolState {
        LocalAccessSymbolState {
            accessor: self.accessors.get(symbol).cloned(),
            typed_source: self.typed_sources.get(symbol).cloned(),
            accessor_origin: self.accessor_origins.get(symbol).cloned(),
            setter: self.setters.get(symbol).cloned(),
            action: self.actions.get(symbol).cloned(),
            source_primitive: self.source_primitives.get(symbol).cloned(),
            async_source: self.async_sources.contains(symbol),
            async_options: self.effective_async_options(symbol),
            source_kind: self.source_kinds.get(symbol).copied(),
            prop_source: self.prop_state(symbol),
            source_declaration: self.source_declarations.get(symbol).cloned(),
            symbol_name: self.symbol_names.get(symbol).cloned(),
        }
    }

    pub(crate) fn call_multiplicities(
        &self,
        file: &solid_facts::FileFacts,
    ) -> Vec<(Location, Option<usize>)> {
        file.ast
            .calls
            .iter()
            .map(|call| {
                let callee = location(file.path.shared(), call.callee);
                let multiplicity = self.reachable_calls.get(&callee).copied();
                (callee, multiplicity)
            })
            .collect()
    }

    pub(crate) fn cached_matches(
        &self,
        file: &solid_facts::FileFacts,
        cached: &CachedLocalAccessFile,
        unchanged_path: bool,
        changed_dependencies: &HashSet<SymbolId>,
    ) -> bool {
        unchanged_path
            && cached.source_hash == file.source_hash
            && cached.proof.holds(self.lookup)
            && (Arc::ptr_eq(&cached.compiler, &file.compiler)
                || same_compiler_semantics(&cached.compiler, &file.compiler))
            // A file's async reads depend on the whole program's async
            // context, which no chunk proof covers.
            && cached.contribution.async_reads.is_empty()
            && cached.dependencies.is_disjoint(changed_dependencies)
            && cached
                .call_multiplicities
                .iter()
                .all(|(callee, previous)| self.reachable_calls.get(callee).copied() == *previous)
    }

    /// Own body only: nested parameter defaults retain ADR 0204's existing
    /// authored-site proof. Select the innermost function before admitting a
    /// named helper, so an anonymous nested closure is not its parent's read.
    /// ADR 0323: whether the view expression at `expression` hides `key`: an
    /// `omit` call (written there, or the initializer of the `const` the
    /// expression names) with `key` among its string-literal hidden arguments,
    /// or whose first argument is such a view. Such a view has no `key`:
    /// reading it reads nothing of its source. A spread, a computed or a
    /// predicate argument hides nothing nameable, so the read stays a read, and
    /// a `merge` between the layers stops the walk (another argument may
    /// supply the key).
    fn view_hides_key(
        &self,
        file: &solid_facts::FileFacts,
        expression: solid_facts::core::Span,
        key: &str,
        depth: usize,
    ) -> bool {
        if depth > 8 {
            return false;
        }
        let expression = file.ast.peel_ts_sugar_span(expression);
        // A `const` view name: continue at its initializer.
        let named =
            crate::source_discovery::reference_symbol(file, expression, self.entities, self.lookup)
                .and_then(|symbol| {
                    file.ast.bindings.iter().find(|binding| {
                        binding.immutable
                            && binding.shape == solid_facts::ast::BindingShape::Identifier
                            && binding.names.first().is_some_and(|name| {
                                self.entities.get(&location(file.path.shared(), name.span))
                                    == Some(&symbol)
                            })
                    })
                })
                .and_then(|binding| binding.call_initializer);
        let Some(call) = file.ast.call_at(named.map_or(expression, |initializer| {
            file.ast.peel_ts_sugar_span(initializer)
        })) else {
            return false;
        };
        let Some(primitive) = self.lookup.primitive_at_call(file, call.span) else {
            return false;
        };
        if !self.lookup.dialect.splits_props(primitive)
            || call.arguments.iter().any(|argument| argument.spread)
        {
            return false;
        }
        let Some((source, hidden)) = call.arguments.split_first() else {
            return false;
        };
        if hidden.iter().any(|argument| {
            matches!(
                &argument.literal_value,
                solid_facts::ast::ArgumentLiteralFact::String(hidden) if hidden.as_str() == key
            )
        }) {
            return true;
        }
        self.view_hides_key(file, source.span, key, depth + 1)
    }

    /// The source an inline `omit` or `merge` view expression reads through to
    /// ([`crate::source_discovery::view_root`]): a props parameter or a store.
    fn inline_view_root(
        &self,
        file: &solid_facts::FileFacts,
        expression: solid_facts::core::Span,
    ) -> Option<SymbolId> {
        crate::source_discovery::view_root(
            file,
            expression,
            self.entities,
            self.lookup,
            &|symbol| {
                self.prop_sources.contains_key(symbol)
                    || (self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store)
                        && self.accessors.contains_key(symbol))
            },
            0,
        )
    }

    fn component_prop_helper<'file>(
        &self,
        file: &'file solid_facts::FileFacts,
        site: solid_facts::core::Span,
    ) -> Option<&'file solid_facts::ast::FunctionFact> {
        let helper = file
            .ast
            .functions
            .iter()
            .filter(|function| function.span.contains(site))
            .min_by_key(|function| function.span.end - function.span.start)?;
        (helper.body.contains(site)
            && function_binding_name(file, helper).is_some()
            && !self.lookup.function_may_be_component(file, helper)
            && file.ast.functions.iter().any(|component| {
                component.body.contains(helper.span)
                    && self.lookup.function_is_component(file, component)
            }))
        .then_some(helper)
    }

    /// An exact callback slot's execution role, independent of any other use
    /// of the binding. Arbitrary callback props and returned invokers remain
    /// escapes. An effect apply slot is strict, despite being deferred.
    fn helper_value_execution(
        &self,
        file: &solid_facts::FileFacts,
        reference: solid_facts::core::Span,
    ) -> Option<ExecutionRole> {
        if file.compiler.callback_roles.iter().any(|callback| {
            callback.span == reference
                && callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
        }) {
            return Some(ExecutionRole::EventCallback);
        }
        file.ast
            .arguments_containing(reference)
            .find_map(|(call, index)| {
                let argument = &call.arguments[index];
                if argument.spread
                    || file.ast.peel_ts_sugar_span(argument.span) != reference
                    || !self
                        .lookup
                        .resolved_callee_call(file, call.callee)
                        .is_some_and(|resolved| {
                            resolved.validity == typefacts::ResolvedCallValidity::Valid
                        })
                {
                    return None;
                }
                let primitive = self.lookup.primitive_at_call(file, call.span)?;
                let dialect = self.lookup.dialect;
                let semantics =
                    dialect.callback_semantics_at(primitive, index, call.arguments.len());
                let execution = semantics.execution?;
                // ADR 0323: a callback that runs synchronously in the call's
                // own strict-read window (`createRoot`, `runWithOwner`) runs
                // in the role of the call.
                if dialect.callback_inherits_calling_window(primitive, index) {
                    return match semantic_execution_role(
                        file,
                        call.callee,
                        &allowed_callback_spans(file, self.lookup),
                        self.entities,
                        self.symbol_names,
                        self.lookup,
                    ) {
                        // ADR 0443: `runWithOwner` clears tracking in a window
                        // the tracked call does not have.
                        ExecutionRole::TrackedJsx
                            if dialect
                                .callback_clears_tracking_in_calling_window(primitive, index) =>
                        {
                            match crate::execution_role::unwindowed_role_at_call(
                                file,
                                call.callee,
                                self.lookup,
                            ) {
                                ExecutionRole::Unknown | ExecutionRole::Unreported => None,
                                role => Some(role),
                            }
                        }
                        ExecutionRole::TrackedJsx => Some(ExecutionRole::DeferredCallback),
                        ExecutionRole::ModuleInitialization
                        | ExecutionRole::Unknown
                        | ExecutionRole::Unreported => None,
                        other => Some(other),
                    };
                }
                if semantics.requires_return_invocation
                    || semantics.stores_as_value
                    || dialect.callback_runs_on_result_access(
                        primitive,
                        index,
                        call.arguments.len(),
                    )
                {
                    return None;
                }
                if dialect.reports_untracked_reads_at(primitive, index, call.arguments.len()) {
                    return Some(
                        if dialect.apply_callback_argument(primitive) == Some(index) {
                            ExecutionRole::EffectApply
                        } else if dialect.callback_runs_untracked_without_window(primitive, index) {
                            ExecutionRole::UntrackedUnwindowed
                        } else {
                            ExecutionRole::UntrackedCallback
                        },
                    );
                }
                if semantics.tracks_reads {
                    return Some(ExecutionRole::TrackedJsx);
                }
                if execution == solid_dialect::Execution::Deferred
                    || dialect.runs_callback_deferred(primitive)
                {
                    return Some(ExecutionRole::DeferredCallback);
                }
                None
            })
    }

    /// A named event caller's role is not exclusive when the same binding is
    /// also invoked directly (MixedNamedCaller). Refuse that deeper chain;
    /// do not let its named callback role erase a possible render-time call.
    fn exclusive_event_caller(
        &self,
        file: &solid_facts::FileFacts,
        caller: &solid_facts::ast::FunctionFact,
    ) -> bool {
        if file.compiler.callback_roles.iter().any(|callback| {
            callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
                && callback.span.contains(caller.span)
                && file
                    .ast
                    .functions_within(callback.span)
                    .max_by_key(|function| function.span.end - function.span.start)
                    .is_some_and(|function| function.span == caller.span)
        }) {
            return true;
        }
        let Some(name) = function_binding_name(file, caller) else {
            return false;
        };
        let mut observed = false;
        for identifier in &file.ast.identifiers {
            if identifier.role != solid_facts::ast::IdentifierRole::Reference
                || file.ast.reference_declaration(identifier.span) != Some(name.span)
            {
                continue;
            }
            observed = true;
            if !file.compiler.callback_roles.iter().any(|callback| {
                callback.span == identifier.span
                    && callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
            }) {
                return false;
            }
        }
        observed
    }

    /// Each exact local call has its own execution proof. An escape opens a
    /// separate definition-site obligation; it cannot erase another call's
    /// valid immutable target, own-body prefix, or incoming-prop witness.
    #[allow(clippy::too_many_arguments)]
    fn project_helper_prop_read(
        &self,
        file: &solid_facts::FileFacts,
        helper: &solid_facts::ast::FunctionFact,
        site: solid_facts::core::Span,
        allowed: &[solid_facts::core::Span],
        read: ReactiveRead,
        definition_admitted: bool,
        result: &mut LocalAccessResult,
    ) {
        if read.execution == ExecutionRole::DiscardedRendering {
            return;
        }
        let Some(name) = function_binding_name(file, helper) else {
            return;
        };
        // Dynamic lexical observation may replace a declaration or the
        // captured props root without an ordinary reference/write fact.
        let dynamic_lexical_observation = file.ast.identifiers.iter().any(|identifier| {
            identifier.role == solid_facts::ast::IdentifierRole::Reference
                && matches!(
                    file.source_text(identifier.span),
                    Some("eval" | "arguments")
                )
                && file.ast.functions.iter().any(|component| {
                    self.lookup.function_is_component(file, component)
                        && component.body.contains(helper.span)
                        && component.span.contains(identifier.span)
                })
        });
        let symbol = self.entities.at(file.path.as_str(), name.span);
        let current = !dynamic_lexical_observation
            && symbol.is_some_and(|symbol| {
                self.lookup.function_value_is_current(file, helper)
                    && !crate::value_identity::binding_has_write(file, self.entities, symbol)
                    && self
                        .lookup
                        .function_for_symbol(symbol.as_str())
                        .is_some_and(|(target_file, target)| {
                            target_file.path == file.path && target.span == helper.span
                        })
            });
        let body_runs = crate::interproc::body_site_runs_during_call(file, helper, site);
        // The lexical census includes undemanded references. Missing call
        // facts open the proof rather than hiding an observed call or escape.
        let mut unproven_use = dynamic_lexical_observation;
        let mut retained_in_layer = false;
        // ADR 0509: an unproven call from another named local function
        // (`const outer = () => helper()`) runs the read wherever that
        // function runs, which nothing here follows.
        let mut nested_in_named = false;
        for reference in file.ast.identifiers.iter().filter(|identifier| {
            identifier.role == solid_facts::ast::IdentifierRole::Reference
                && file.ast.reference_declaration(identifier.span) == Some(name.span)
        }) {
            if crate::execution_role::discarded_region_contains(file, reference.span)
                || file.ast.transparent_wrappers.iter().any(|wrapper| {
                    wrapper.span.contains(reference.span) && !wrapper.inner.contains(reference.span)
                })
            {
                continue;
            }
            let Some(call) = file.ast.calls.iter().find(|call| {
                call.direct_callee
                    && !call.construct
                    && file.ast.peel_ts_sugar_span(call.callee) == reference.span
            }) else {
                if current
                    && body_runs
                    && let Some(execution) = self.helper_value_execution(file, reference.span)
                {
                    if execution.reports_untracked_read() {
                        let mut attributed = read.clone();
                        attributed.location = location(file.path.shared(), reference.span);
                        attributed.execution = execution;
                        attributed.context =
                            read_analysis_context(file, site, execution, self.lookup).into();
                        attributed.via =
                            file.source_text(name.span).unwrap_or("local helper").into();
                        attributed.origin = Some(read.location.clone());
                        attributed.origin_context = attributed.via.clone();
                        attributed.summary_attributed = false;
                        attributed.missing_jsx_census =
                            missing_jsx_census(file, reference.span, execution);
                        attributed.host_callback_timing = false;
                        attributed.callee_callback_timing = false;
                        result.reads.push(Arc::new(attributed));
                        result.strict_read_obligations += 1;
                    }
                } else {
                    unproven_use = true;
                    // ADR 0323 (review round 10): a reference that keeps the
                    // helper (an alias, a store, a return, a property) inside
                    // an admitted anonymous layer may run it where nothing
                    // here sees. A pass to a call (an unknown consumer) is
                    // option B's silence; this is an obligation.
                    if !file
                        .ast
                        .arguments_containing(reference.span)
                        .any(|(outer, index)| {
                            file.ast.peel_ts_sugar_span(outer.arguments[index].span)
                                == reference.span
                        })
                        && unclassified_callback_reaches_modelled_consumer(
                            file,
                            reference.span,
                            self.lookup,
                        )
                    {
                        retained_in_layer = true;
                    }
                }
                continue;
            };
            let execution = semantic_execution_role(
                file,
                call.span,
                allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            );
            if execution == ExecutionRole::DiscardedRendering {
                continue;
            }
            let candidates = self.lookup.callee_symbols(file, call.callee);
            let exact = current
                && candidates.len() == 1
                && symbol.is_some_and(|symbol| candidates.first() == Some(symbol))
                && self
                    .lookup
                    .resolved_callee_call(file, call.callee)
                    .is_some_and(|resolved| {
                        resolved.validity == typefacts::ResolvedCallValidity::Valid
                    });
            let caller = containing_ast_function(&file.ast, call.span);
            let control = direct_control_flow_body_role(
                file,
                call.span,
                self.entities,
                self.symbol_names,
                self.lookup.dialect,
            );
            let component =
                caller.is_some_and(|caller| self.lookup.function_is_component(file, caller));
            let primitive_callback = caller.is_some_and(|caller| {
                file.ast
                    .arguments_containing(caller.span)
                    .any(|(outer, index)| {
                        !outer.arguments[index].spread
                            && file.ast.peel_ts_sugar_span(outer.arguments[index].span)
                                == caller.span
                            && self.lookup.primitive_at_call(file, outer.span).is_some_and(
                                |primitive| {
                                    self.lookup
                                        .dialect
                                        .callback_semantics_at(
                                            primitive,
                                            index,
                                            outer.arguments.len(),
                                        )
                                        .execution
                                        .is_some()
                                },
                            )
                    })
            });
            let event = caller.is_some_and(|caller| self.exclusive_event_caller(file, caller));
            let non_strict_role = matches!(
                execution,
                ExecutionRole::TrackedJsx
                    | ExecutionRole::DeferredCallback
                    | ExecutionRole::EventCallback
            );
            // JSX-contained tracked expressions, control-flow bodies and
            // events execute in the compiler/dialect-proven scope, not while
            // constructing the outer JSX. Do not use ADR 0201's no-JSX entry
            // gate to decide that these known scopes are unproven.
            let prefix = caller.is_some_and(|caller| {
                crate::interproc::body_site_runs_during_call(file, caller, call.span)
                    || (!caller.r#async
                        && !caller.generator
                        && crate::owners::written_directly_in(&file.ast, caller, call.span)
                        && (control.is_some() || (non_strict_role && (component || event))))
            });
            // ADR 0323: an anonymous callback that reaches a modelled consumer
            // (a project callee, an inline standard-library callback, an IIFE)
            // is a layer of its own: a helper called directly in its body runs
            // when the callback does. Whether that is during the body is still
            // `callee_timing`'s answer below.
            let anonymous_layer = caller.is_some_and(|caller| {
                caller.method_name.is_none()
                    && function_binding_name(file, caller).is_none()
                    && self.lookup.function_component_status(file, caller)
                        == crate::indexes::ComponentStatus::No
                    && unclassified_callback_reaches_modelled_consumer(file, call.span, self.lookup)
            });
            let admitted = component
                || primitive_callback
                || control.is_some()
                || (non_strict_role && event)
                || anonymous_layer;
            let host_timing = host_callback_timing(file, call.span, execution, self.lookup);
            let callee_timing = callee_callback_timing(file, call.span, execution, self.lookup);
            let timing_proven =
                exact && body_runs && prefix && admitted && !host_timing && !callee_timing;
            if timing_proven && non_strict_role {
                continue;
            }
            let direct = timing_proven && execution.reports_untracked_read();
            if !direct {
                unproven_use = true;
                // The call's own role reports, but its timing is unproven; or
                // it is in another named local function, which runs where
                // nothing here follows.
                if execution.reports_untracked_read()
                    || (!non_strict_role
                        && caller.is_some_and(|caller| {
                            caller.span != helper.span
                                && function_binding_name(file, caller).is_some()
                                && !component
                        }))
                {
                    nested_in_named = true;
                }
                continue;
            }
            let mut attributed = read.clone();
            attributed.location = location(file.path.shared(), call.span);
            attributed.execution = execution;
            attributed.context =
                read_analysis_context(file, call.span, execution, self.lookup).into();
            attributed.via = file
                .source_text(call.callee)
                .unwrap_or("local helper")
                .into();
            attributed.origin = Some(read.location.clone());
            attributed.origin_context =
                file.source_text(name.span).unwrap_or("local helper").into();
            attributed.summary_attributed = false;
            attributed.missing_jsx_census = missing_jsx_census(file, call.span, execution);
            attributed.host_callback_timing = host_timing;
            attributed.callee_callback_timing = callee_timing;
            result.reads.push(Arc::new(attributed));
            result.strict_read_obligations += 1;
        }
        // An unproven use (an escape, an unproven call) adds no new
        // obligation: the read keeps what the definition site answered before
        // this path existed, and nothing where that gate refused it. A new
        // definition-site obligation for every such helper buried the corpus
        // in derived getters and handler helpers (ADR 0222).
        if unproven_use && definition_admitted {
            let counts = counts_as_strict_read_root(file, site, read.execution, self.lookup);
            result.reads.push(Arc::new(read));
            if counts {
                result.strict_read_obligations += 1;
            }
        } else if retained_in_layer || nested_in_named {
            // The helper may run in the render window through the reference
            // that keeps it: not proven, not silent.
            let mut retained = read;
            retained.callee_callback_timing = true;
            let counts = counts_as_strict_read_root(file, site, retained.execution, self.lookup);
            result.reads.push(Arc::new(retained));
            if counts {
                result.strict_read_obligations += 1;
            }
        }
    }

    pub(crate) fn discover(&self, file: &solid_facts::FileFacts) -> LocalAccessResult {
        let mut result = LocalAccessResult::default();
        let mut seen = HashSet::default();
        let allowed = allowed_callback_spans(file, self.lookup);
        for call in &file.ast.calls {
            self.discover_call(file, call, &allowed, &mut result, &mut seen);
        }
        for member in &file.ast.members {
            self.discover_member(file, member, &allowed, &mut result, &mut seen);
        }
        self.discover_spreads(file, &allowed, &mut result);
        self.discover_dynamic_jsx(file, &mut result);
        result
    }

    /// ADR 0468: a comma, conditional or logical callee (`(c && setX)(1)`)
    /// resolves to no symbol, so the call has no row; when an identifier it
    /// may evaluate to is a local setter, action or accessor, the call may
    /// write, act or read here, and an absent row would say it does not. The
    /// call is an obligation instead.
    fn push_composite_callee_obligation(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        result: &mut LocalAccessResult,
    ) {
        let reactive = file
            .ast
            .composite_callee_references(call.callee)
            .into_iter()
            .filter_map(|reference| self.lookup.callee_symbol(file, reference))
            .any(|symbol| {
                self.setters.contains_key(symbol)
                    || self.actions.contains_key(symbol)
                    || self.accessors.contains_key(symbol)
            });
        if !reactive {
            return;
        }
        result.dispatch_obligations.push(crate::StaticDefect {
            kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                callee: file
                    .source_text(call.callee)
                    .unwrap_or("this callee")
                    .to_owned(),
                member: None,
            },
            location: location(file.path.shared(), call.callee),
            analysis_context: "composite-callee-unresolved".to_owned(),
            fixes: vec![],
            uncertain: true,
        });
    }

    fn discover_call(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        allowed: &[solid_facts::core::Span],
        result: &mut LocalAccessResult,
        seen: &mut HashSet<(Arc<str>, u64, u64)>,
    ) {
        let callee = location(file.path.shared(), call.callee);
        let Some(symbol) = self.lookup.callee_symbol(file, call.callee) else {
            self.push_composite_callee_obligation(file, call, result);
            return;
        };
        if inside_known_value_function_argument(file, call.callee, self.lookup) {
            return;
        }
        if let Some((declaration, property)) = self.exact_props_callee(file, call.callee) {
            if self.discover_props_call(file, call, &declaration, &property, allowed, result) {
                return;
            }
        } else if !self.setters.contains_key(symbol)
            && !self.actions.contains_key(symbol)
            && !self.accessors.contains_key(symbol)
            // A standard-library method (`props.label.trim()`) runs no
            // setter, action or read.
            && self
                .lookup
                .standard_library_declaration(file, call, typefacts::CallKind::Call)
                .is_none()
            && self.handed_member_callee(file, call.callee)
        {
            let execution = semantic_execution_role(
                file,
                call.callee,
                allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            );
            // Its type proves it a Solid accessor, signal or store: it may
            // read, never write or act.
            let read_only = self.typed_sources.contains_key(symbol)
                || self.declared_read_type(file, call.callee);
            self.push_owned_dispatch_obligation_for(
                file,
                call,
                allowed,
                execution,
                UNRESOLVED_HANDED_MEMBER,
                !read_only,
                result,
            );
        }
        let inside_function = file.ast.any_function_body_containing(call.span);
        if inside_function && self.setters.contains_key(symbol) {
            result.write_action_obligations.insert((
                "write",
                callee.path.to_string(),
                callee.start_byte,
                callee.end_byte,
            ));
        }
        if inside_function && self.actions.contains_key(symbol) {
            result.write_action_obligations.insert((
                "action",
                callee.path.to_string(),
                callee.start_byte,
                callee.end_byte,
            ));
        }
        // ADR 0463: a mention of a local alias that may run before the
        // alias's declarator throws there instead of reaching the value, so
        // nothing places what it does: every claim about it is unplaced.
        let alias_withheld = self.lookup.local_alias_mention_withheld(file, call.callee);
        let execution = if alias_withheld {
            ExecutionRole::Unknown
        } else {
            semantic_execution_role(
                file,
                call.callee,
                allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            )
        };
        if self.async_sources.contains(symbol)
            && !alias_withheld
            && self.accessors.contains_key(symbol)
            && read_in_named_local_function(file, call.callee, self.lookup)
            && named_callback_execution_role(file, call.callee, self.lookup).is_none()
            && !pending_accessor_probe(file, call.callee, self.lookup)
        {
            self.push_call_attributed_async_reads(file, call, symbol, allowed, result);
        }
        // A universal callback-context proof can certify an accessor
        // read even when invocation is optional or resource-triggered.
        // Keep write/action and owner classification on their existing
        // execution proofs; this override applies only to known accessors.
        // ADR 0294: a typed candidate whose implementation is not visible
        // is located like an accessor read and reported as an obligation.
        let typed_unknown = !self.accessors.contains_key(symbol)
            && self
                .lookup
                .object_member_function(file, call.callee)
                .is_none()
            && self
                .typed_sources
                .get(symbol)
                .is_some_and(|typed| !typed.implementation_visible);
        let typed_effect_accessor = execution == ExecutionRole::EffectApply
            && self
                .lookup
                .object_member_function(file, call.callee)
                .is_none()
            && call.arguments.is_empty()
            && !crate::callback_return::binding_is_callback_result(self.lookup, file, call.callee)
            && typed_accessor_descriptor_at(self.lookup, file.path.as_str(), call.callee).is_some();
        let Some(multiplicity) = self.reachable_calls.get(&callee).copied().or_else(|| {
            (typed_effect_accessor
                || ((self.accessors.contains_key(symbol) || typed_unknown)
                    && (execution == ExecutionRole::EffectApply
                        || control_flow_execution_role(
                            file,
                            call.callee,
                            self.entities,
                            self.symbol_names,
                            self.lookup.dialect,
                        )
                        .is_some()
                        || named_callback_execution_role(file, call.callee, self.lookup)
                            .is_some()
                        || enclosing_render_function(file, call.callee, self.lookup))))
            .then_some(1)
        }) else {
            return;
        };
        let key = (callee.path.clone(), callee.start_byte, callee.end_byte);
        if let Some(accessor) = self.accessors.get(symbol)
            && (!inside_non_component_function(file, call.callee, self.lookup)
                || named_callback_execution_role(file, call.callee, self.lookup).is_some())
            && seen.insert(key.clone())
        {
            self.push_accessor_read(file, call, symbol, accessor, execution, result);
        }
        // ADR 0294: a typed candidate read where an accessor read would be
        // reported. Its type is the only evidence that it reads reactive
        // state, so this is an obligation, never a read.
        if typed_unknown
            && execution.reports_untracked_read()
            && (!inside_non_component_function(file, call.callee, self.lookup)
                || named_callback_execution_role(file, call.callee, self.lookup).is_some())
            && seen.insert(key.clone())
        {
            result
                .dispatch_obligations
                .push(typed_source_obligation(file, call.callee));
        }
        // The effect-apply shortcut: a no-argument call whose callee Type
        // Facts types as the Accessor alias, with no symbol entry. The
        // same premise as above, so the same obligation, unless the
        // callee's own implementation is visible.
        if !self.accessors.contains_key(symbol)
            && !self
                .typed_sources
                .get(symbol)
                .is_some_and(|typed| typed.implementation_visible)
            && execution == ExecutionRole::EffectApply
            && call.arguments.is_empty()
            && !crate::callback_return::binding_is_callback_result(self.lookup, file, call.callee)
            && typed_accessor_descriptor_at(self.lookup, file.path.as_str(), call.callee).is_some()
            && seen.insert(key.clone())
        {
            result
                .dispatch_obligations
                .push(typed_source_obligation(file, call.callee));
        }
        self.push_setter_write(
            file,
            call,
            symbol,
            allowed,
            alias_withheld,
            multiplicity,
            result,
        );
        self.push_action_invocation(
            file,
            call,
            symbol,
            allowed,
            alias_withheld,
            multiplicity,
            result,
        );
    }

    /// ADR 0509: the props declaration and prop name of a callee that is a
    /// prop of a component's exact props parameter: `props.name` on the
    /// current, unaliased parameter (the identity `discover_member`'s exact
    /// root requires), or a binding of the parameter's object pattern
    /// (`function C({ name })`).
    fn exact_props_callee(
        &self,
        file: &solid_facts::FileFacts,
        callee: solid_facts::core::Span,
    ) -> Option<(Location, String)> {
        let callee = file.ast.peel_ts_sugar_span(callee);
        if let Some(member) = file.ast.member_at(callee) {
            if file
                .ast
                .computed_members
                .binary_search(&member.span)
                .is_ok()
                || file
                    .ast
                    .optional_members
                    .binary_search(&member.span)
                    .is_ok()
            {
                return None;
            }
            let object = file.ast.peel_ts_sugar_span(member.object);
            if !file.ast.identifiers.iter().any(|identifier| {
                identifier.span == object
                    && identifier.role == solid_facts::ast::IdentifierRole::Reference
            }) {
                return None;
            }
            let root = self.entities.get(&location(file.path.shared(), object))?;
            if self.source_kinds.get(root) == Some(&ReactiveSourceKind::Store) {
                return None;
            }
            let (_, declaration) = self.prop_sources.get(root)?;
            let exact = self.entities.get(declaration) == Some(root)
                && props_root_is_current(file, declaration, root, self.entities, self.lookup)
                && props_root_attachments_are_views(
                    file,
                    self.prop_sources,
                    declaration,
                    root,
                    self.entities,
                    self.lookup,
                );
            if !exact {
                return None;
            }
            let name = file.source_text(member.property)?.to_owned();
            return Some((declaration.clone(), name));
        }
        // A destructured props parameter: the classification is addressed by
        // the parameter's pattern.
        let declaration = file.ast.reference_declaration(callee)?;
        let function = containing_ast_function(&file.ast, callee)?;
        let mut owner = Some(function);
        while let Some(function) = owner {
            if let Some(parameter) = function.parameters.first()
                && parameter.shape == solid_facts::ast::BindingShape::Object
                && let Some(slot) = parameter
                    .object_slots
                    .iter()
                    .find(|slot| slot.local.span == declaration)
            {
                if crate::indexes::binding_written(file, declaration) {
                    return None;
                }
                let pattern = location(file.path.shared(), parameter.pattern);
                return self
                    .props_reactivity
                    .for_declaration(&pattern)
                    .is_some()
                    .then(|| (pattern, slot.property.to_string()));
            }
            owner = file
                .ast
                .functions
                .iter()
                .filter(|outer| outer.span != function.span && outer.span.contains(function.span))
                .min_by_key(|outer| outer.span.end - outer.span.start);
        }
        None
    }

    /// ADR 0509: a call of a prop's value, `props.name(…)`. Each exact source
    /// a visible call site passes for the prop (a primitive's setter, action
    /// or accessor) is called at that site's render: the call writes, acts or
    /// reads with that source's row. Where some call site passes anything
    /// else, or the component is not enumerated, the call is an obligation
    /// wherever it would report a write or a read. Returns whether the call
    /// is decided here.
    fn discover_props_call(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        declaration: &Location,
        property: &str,
        allowed: &[solid_facts::core::Span],
        result: &mut LocalAccessResult,
    ) -> bool {
        let (sources, complete) = self.props_reactivity.passed_sources(declaration, property);
        if complete && sources.is_empty() {
            // Every caller passes nothing for it, or a non-source value
            // `passed` classified: not decided here.
            return false;
        }
        let callee = location(file.path.shared(), call.callee);
        let alias_withheld = self.lookup.local_alias_mention_withheld(file, call.callee);
        let execution = if alias_withheld {
            ExecutionRole::Unknown
        } else {
            semantic_execution_role(
                file,
                call.callee,
                allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            )
        };
        // As `discover_call`: a write or an action needs a reached call; an
        // accessor read is also placed by its own role.
        let reached = self.reachable_calls.get(&callee).copied();
        let read_multiplicity = reached.or_else(|| {
            (execution == ExecutionRole::EffectApply
                || control_flow_execution_role(
                    file,
                    call.callee,
                    self.entities,
                    self.symbol_names,
                    self.lookup.dialect,
                )
                .is_some()
                || named_callback_execution_role(file, call.callee, self.lookup).is_some()
                || enclosing_render_function(file, call.callee, self.lookup))
            .then_some(1)
        });
        // As `discover_call`: a read in a non-component function is placed
        // by the summaries of its callers, which a prop's value has none of:
        // there the read is an obligation.
        let placed = self.props_call_placed(file, call);
        let mut unplaced_read = false;
        for source in &sources {
            if let Some(accessor) = self.accessors.get(source.as_str())
                && read_multiplicity.is_some()
            {
                if placed {
                    self.push_accessor_read(
                        file,
                        call,
                        source.as_str(),
                        accessor,
                        execution,
                        result,
                    );
                } else {
                    unplaced_read = true;
                }
            }
        }
        if let Some(multiplicity) = reached.filter(|_| placed) {
            for source in &sources {
                self.push_setter_write(
                    file,
                    call,
                    source.as_str(),
                    allowed,
                    alias_withheld,
                    multiplicity,
                    result,
                );
                self.push_action_invocation(
                    file,
                    call,
                    source.as_str(),
                    allowed,
                    alias_withheld,
                    multiplicity,
                    result,
                );
            }
        }
        if !complete || unplaced_read || (!placed && !sources.is_empty()) {
            self.push_owned_dispatch_obligation(
                file,
                call,
                allowed,
                execution,
                "closed-parameter-member-dispatch",
                result,
            );
        }
        true
    }

    /// ADR 0509: whether a call of a prop's value is placed where its roles
    /// alone decide it: written directly in the component's body, or directly
    /// in a function literal a dialect primitive's call receives (an effect's
    /// compute or apply, a memo). Anywhere deeper (a closure, a guard, an
    /// IIFE) its run is its callers', which nothing follows for a prop.
    fn props_call_placed(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
    ) -> bool {
        let Some(function) = containing_ast_function(&file.ast, call.span) else {
            return false;
        };
        if self.lookup.function_is_component(file, function) {
            return true;
        }
        file.ast
            .arguments_containing(function.span)
            .any(|(outer, index)| {
                !outer.arguments[index].spread
                    && file.ast.peel_ts_sugar_span(outer.arguments[index].span) == function.span
                    && self.lookup.primitive_at_call(file, outer.span).is_some()
                    && containing_ast_function(&file.ast, outer.span)
                        .is_some_and(|owner| self.lookup.function_is_component(file, owner))
            })
    }

    /// An obligation at a call whose callee may be a setter, an action or an
    /// accessor this cannot see, where the call's role would report a write,
    /// an action or an untracked read.
    fn push_owned_dispatch_obligation(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        allowed: &[solid_facts::core::Span],
        execution: ExecutionRole,
        context: &str,
        result: &mut LocalAccessResult,
    ) {
        self.push_owned_dispatch_obligation_for(
            file, call, allowed, execution, context, true, result,
        );
    }

    /// [`Self::push_owned_dispatch_obligation`], where `may_write` says
    /// whether the callee may be a setter or an action at all.
    #[allow(clippy::too_many_arguments)]
    fn push_owned_dispatch_obligation_for(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        allowed: &[solid_facts::core::Span],
        execution: ExecutionRole,
        context: &str,
        may_write_or_act: bool,
        result: &mut LocalAccessResult,
    ) {
        let write = semantic_write_execution_role(
            file,
            call.callee,
            allowed,
            self.entities,
            self.symbol_names,
            self.lookup,
            RootBodyGuard::Rejects,
        );
        // A write or an action call, with or without arguments (an action is
        // usually called with none): in tracked JSX too, where a read is
        // legal, this call may be either.
        // A function every reference of which is an event handler runs only
        // as one, outside any owned scope (the read path's proof).
        if containing_ast_function(&file.ast, call.span)
            .is_some_and(|function| self.exclusive_event_caller(file, function))
        {
            return;
        }
        let may_write =
            may_write_or_act && (write.reports_disallowed_write() || write.reports_unplaced());
        if may_write
            || execution.reports_untracked_read()
            || self.may_run_in_leaf_owner(file, call.callee)
        {
            result.dispatch_obligations.push(crate::StaticDefect {
                kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                    callee: file
                        .source_text(call.callee)
                        .unwrap_or("this callee")
                        .to_owned(),
                    member: None,
                },
                location: location(file.path.shared(), call.callee),
                analysis_context: if context == UNRESOLVED_HANDED_MEMBER && !may_write {
                    UNRESOLVED_HANDED_READ.to_owned()
                } else {
                    context.to_owned()
                },
                fixes: vec![],
                uncertain: true,
            });
        }
    }

    /// Whether `span` is inside a leaf owner's callback (`onSettled`,
    /// `createTrackedEffect`), where a pending async read is forbidden, or in
    /// a named local function a reference inside one names.
    fn may_run_in_leaf_owner(
        &self,
        file: &solid_facts::FileFacts,
        span: solid_facts::core::Span,
    ) -> bool {
        self.may_run_in_leaf_owner_within(file, span, 3)
    }

    fn may_run_in_leaf_owner_within(
        &self,
        file: &solid_facts::FileFacts,
        span: solid_facts::core::Span,
        depth: usize,
    ) -> bool {
        if containing_leaf_owner(file, span, self.entities, self.symbol_names, self.lookup)
            .is_some()
            || file.ast.functions.iter().any(|leaf| {
                leaf.span.contains(span)
                    && containing_leaf_owner(
                        file,
                        leaf.span,
                        self.entities,
                        self.symbol_names,
                        self.lookup,
                    )
                    .is_some()
            })
        {
            return true;
        }
        if depth == 0 {
            return false;
        }
        let Some(function) = containing_ast_function(&file.ast, span) else {
            return false;
        };
        let Some(name) = function_binding_name(file, function) else {
            return false;
        };
        file.ast.identifiers.iter().any(|identifier| {
            identifier.role == solid_facts::ast::IdentifierRole::Reference
                && file.ast.reference_declaration(identifier.span) == Some(name.span)
                && self.may_run_in_leaf_owner_within(file, identifier.span, depth - 1)
        })
    }

    /// Whether the callee, or the binding it names, is typed as a Solid
    /// accessor, signal or store (ADR 0294's type roles): a read, never a
    /// setter or an action.
    fn declared_read_type(
        &self,
        file: &solid_facts::FileFacts,
        callee: solid_facts::core::Span,
    ) -> bool {
        let read_role = |span| {
            self.lookup
                .smallest_contained_descriptor(file.path.as_str(), span)
                .is_some_and(|descriptor| {
                    descriptor.alias_declarations.iter().any(|declaration| {
                        matches!(
                            crate::symbols::descriptor_type_role(
                                self.lookup.dialect,
                                descriptor,
                                declaration
                            ),
                            Some(
                                solid_dialect::TypeRole::Accessor
                                    | solid_dialect::TypeRole::Signal
                                    | solid_dialect::TypeRole::Store
                            )
                        )
                    })
                })
        };
        let callee = file.ast.peel_ts_sugar_span(callee);
        read_role(callee)
            || file
                .ast
                .reference_declaration(callee)
                .is_some_and(read_role)
    }

    /// ADR 0509: a callee whose value a caller supplied: a member of, an
    /// alias of, or a slot destructured from a `useContext` result, a project
    /// function's result (a hook's), or a props object (a view of one too),
    /// through at most a few such steps. It may hold a setter, action or
    /// accessor this cannot see ([`UNRESOLVED_HANDED_MEMBER`]). Accessor
    /// transport may still resolve it; the obligation is dropped where it
    /// does (`reactive_analysis`).
    fn handed_member_callee(
        &self,
        file: &solid_facts::FileFacts,
        callee: solid_facts::core::Span,
    ) -> bool {
        // At most one member step from the value a caller supplied
        // (`ctx.save()`, not `props.label.trim()`).
        let peeled = file.ast.peel_ts_sugar_span(callee);
        if let Some(member) = file.ast.member_at(peeled)
            && file
                .ast
                .member_at(file.ast.peel_ts_sugar_span(member.object))
                .is_some()
        {
            return false;
        }
        handed_value(
            &HandedTables {
                lookup: self.lookup,
                entities: self.entities,
                prop_sources: self.prop_sources,
                source_kinds: self.source_kinds,
                accessors: self.accessors,
                setters: self.setters,
                actions: self.actions,
                props_reactivity: self.props_reactivity,
            },
            file,
            callee,
            HANDED_DEPTH,
        )
    }

    /// The write a call of the setter whose row is `symbol`'s makes: the
    /// callee's own row, or the row of the setter accessor transport carried
    /// to it ([`Self::transported_writes`]).
    #[allow(clippy::too_many_arguments)]
    fn push_setter_write(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        symbol: &str,
        allowed: &[solid_facts::core::Span],
        alias_withheld: bool,
        multiplicity: usize,
        result: &mut LocalAccessResult,
    ) {
        if let Some((name, declaration, owned_write, source_kind)) = self.setters.get(symbol) {
            // Under a root owner `setSignal`'s guard throws on every
            // release; a store setter's only where the resolved signals
            // dropped the root exemption (rc.9). In dev a component body
            // runs under a root too, so the same answer covers it.
            let root_body = match source_kind {
                ReactiveSourceKind::Accessor => RootBodyGuard::RejectsOutsideDisposal,
                ReactiveSourceKind::Store
                    if !self.lookup.dialect.store_setter_guard_exempts_roots() =>
                {
                    RootBodyGuard::RejectsOutsideDisposal
                }
                ReactiveSourceKind::Store => RootBodyGuard::Exempts,
            };
            let mut write_execution = if alias_withheld {
                ExecutionRole::Unknown
            } else {
                semantic_write_execution_role(
                    file,
                    call.callee,
                    allowed,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                    root_body,
                )
            };
            // Where the resolved release's optimistic-store setter meets no
            // owned-scope guard (rc.0), a store setter is reported only when
            // it is proven to be `createStore`'s, whose writes do reach the
            // guard. An optimistic-store setter is legal in every owned
            // scope, so it takes the role that reports nothing, as an
            // exempted root-body write does. A store setter found only by
            // its type could be either: the write is unplaced, an
            // obligation, never a silent clean.
            let primitive = self
                .source_primitives
                .get(symbol)
                .and_then(|primitive| self.lookup.dialect.primitive(primitive));
            if write_execution.reports_disallowed_write()
                && *source_kind == ReactiveSourceKind::Store
                && !self.lookup.dialect.optimistic_store_setter_guarded()
                && primitive != Some(solid_dialect::Primitive::CreateStore)
            {
                write_execution =
                    if primitive == Some(solid_dialect::Primitive::CreateOptimisticStore) {
                        ExecutionRole::UntrackedCallback
                    } else {
                        ExecutionRole::Unknown
                    };
            }
            // The same for a signal setter where `createOptimistic`'s setter
            // meets no owned-scope guard (rc.17's `optimisticWrite`): only a
            // setter proven to be `createSignal`'s is reported, and one of
            // unknown source is an obligation.
            if write_execution.reports_disallowed_write()
                && *source_kind == ReactiveSourceKind::Accessor
                && !self.lookup.dialect.optimistic_signal_setter_guarded()
                && primitive != Some(solid_dialect::Primitive::CreateSignal)
            {
                write_execution = if primitive == Some(solid_dialect::Primitive::CreateOptimistic) {
                    ExecutionRole::UntrackedCallback
                } else {
                    ExecutionRole::Unknown
                };
            }
            for _ in 0..multiplicity {
                result.writes.push(Arc::new(ReactiveWrite {
                    setter: name.to_string().into(),
                    operation: crate::ReactiveWriteOperation::Setter,
                    source_kind: *source_kind,
                    location: location(file.path.shared(), call.span),
                    declaration: declaration.clone(),
                    execution: write_execution,
                    owned_write: *owned_write,
                    context: analysis_context(
                        file,
                        call.span,
                        self.entities,
                        self.symbol_names,
                        self.lookup.dialect,
                        self.lookup,
                    )
                    .into(),
                }));
            }
        }
    }

    /// The action invocation a call of the action whose row is `symbol`'s
    /// makes ([`Self::push_setter_write`]).
    #[allow(clippy::too_many_arguments)]
    fn push_action_invocation(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        symbol: &str,
        allowed: &[solid_facts::core::Span],
        alias_withheld: bool,
        multiplicity: usize,
        result: &mut LocalAccessResult,
    ) {
        if let Some((name, declaration)) = self.actions.get(symbol) {
            // An action call's guard has no root exemption on any release:
            // it throws `ACTION_CALLED_IN_OWNED_SCOPE` directly in a
            // `createRoot` body, and in a memo nested in one (probed
            // rc.0-rc.9, dev).
            let action_execution = if alias_withheld {
                ExecutionRole::Unknown
            } else {
                semantic_write_execution_role(
                    file,
                    call.callee,
                    allowed,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                    RootBodyGuard::Rejects,
                )
            };
            for _ in 0..multiplicity {
                result.action_invocations.push(Arc::new(ActionInvocation {
                    action: name.to_string().into(),
                    location: location(file.path.shared(), call.span),
                    declaration: declaration.clone(),
                    execution: action_execution,
                    context: analysis_context(
                        file,
                        call.span,
                        self.entities,
                        self.symbol_names,
                        self.lookup.dialect,
                        self.lookup,
                    )
                    .into(),
                }));
            }
        }
    }

    /// Calls whose callee accessor transport resolves to a setter or an
    /// action a primitive created ([`Callable::Setter`], [`Callable::Action`]):
    /// a hook's returned tuple or object, a context value. The call writes or
    /// acts as a call of the source's own binding does, at the call site's
    /// role, with the source's row (its primitive and options). Its callee
    /// has no row of its own, or only the one its type gave it (ADR 0294),
    /// which the exact source replaces: the caller drops the call's other
    /// writes ([`LocalAccessResult::writes`] at the same location).
    ///
    /// [`Callable::Setter`]: crate::interproc::accessor_provenance::Callable::Setter
    /// [`Callable::Action`]: crate::interproc::accessor_provenance::Callable::Action
    pub(crate) fn transported_writes(
        &self,
        targets: &HashMap<Location, crate::interproc::accessor_provenance::Callable>,
        reads: &HashMap<Location, crate::interproc::accessor_provenance::Callable>,
    ) -> LocalAccessResult {
        use crate::interproc::accessor_provenance::Callable;
        let mut sites = targets
            .iter()
            .filter_map(|(callee, target)| match target {
                Callable::Setter(symbol) | Callable::Action(symbol) => Some((callee, symbol)),
                _ => None,
            })
            .chain(reads.iter().filter_map(|(callee, target)| match target {
                // The read row is the summaries'; the async read is made here.
                Callable::Accessor(symbol) if self.async_sources.contains(symbol.as_str()) => {
                    Some((callee, symbol))
                }
                _ => None,
            }))
            .collect::<Vec<_>>();
        sites.sort_by(|(left, _), (right, _)| {
            (left.path.as_ref(), left.start_byte, left.end_byte).cmp(&(
                right.path.as_ref(),
                right.start_byte,
                right.end_byte,
            ))
        });
        let mut result = LocalAccessResult::default();
        for (callee, symbol) in sites {
            let Some(file) = self.lookup.file_by_path(callee.path.as_ref()) else {
                continue;
            };
            let Some(call) = file
                .ast
                .calls
                .iter()
                .find(|call| location(file.path.shared(), call.callee) == *callee)
            else {
                continue;
            };
            if inside_known_value_function_argument(file, call.callee, self.lookup) {
                continue;
            }
            if let Some(accessor) = self.accessors.get(symbol.as_str())
                && self.async_sources.contains(symbol.as_str())
                && !self.setters.contains_key(symbol.as_str())
            {
                let allowed = allowed_callback_spans(file, self.lookup);
                let execution = if self.lookup.local_alias_mention_withheld(file, call.callee) {
                    ExecutionRole::Unknown
                } else {
                    semantic_execution_role(
                        file,
                        call.callee,
                        &allowed,
                        self.entities,
                        self.symbol_names,
                        self.lookup,
                    )
                };
                // As `discover_call`: a read in a non-component function is
                // placed by its callers, so only an obligation here.
                if !inside_non_component_function(file, call.callee, self.lookup)
                    || named_callback_execution_role(file, call.callee, self.lookup).is_some()
                {
                    self.push_async_read(
                        file,
                        call,
                        symbol.as_str(),
                        accessor,
                        execution,
                        &mut result,
                    );
                } else {
                    self.push_owned_dispatch_obligation(
                        file,
                        call,
                        &allowed,
                        execution,
                        UNRESOLVED_HANDED_MEMBER,
                        &mut result,
                    );
                }
                continue;
            }
            if file.ast.any_function_body_containing(call.span) {
                let kind = if self.setters.contains_key(symbol.as_str()) {
                    "write"
                } else {
                    "action"
                };
                result.write_action_obligations.insert((
                    kind,
                    callee.path.to_string(),
                    callee.start_byte,
                    callee.end_byte,
                ));
            }
            let Some(multiplicity) = self.reachable_calls.get(callee).copied() else {
                continue;
            };
            let allowed = allowed_callback_spans(file, self.lookup);
            let alias_withheld = self.lookup.local_alias_mention_withheld(file, call.callee);
            self.push_setter_write(
                file,
                call,
                symbol.as_str(),
                &allowed,
                alias_withheld,
                multiplicity,
                &mut result,
            );
            self.push_action_invocation(
                file,
                call,
                symbol.as_str(),
                &allowed,
                alias_withheld,
                multiplicity,
                &mut result,
            );
        }
        result
    }

    /// ADR 0462: an async source read written directly in a named local
    /// function (`const wrapUse = () => { a1(); }`) is rendered where a call
    /// of that function sits in rendered JSX, through at most
    /// [`CALL_ATTRIBUTED_DEPTH`] such functions. Each row sits at the
    /// rendering call and is `call_attributed`: nothing proves the call reads
    /// the source pending while it renders, so the missing-boundary finding is
    /// uncertifiable, never proven. A reference that is not a direct call, a
    /// function whose binding is written, a generator, or a site that may not
    /// run during its function's call stops the walk (no row, as before).
    fn push_call_attributed_async_reads(
        &self,
        file: &solid_facts::FileFacts,
        read: &solid_facts::ast::CallFact,
        symbol: &str,
        allowed: &[solid_facts::core::Span],
        result: &mut LocalAccessResult,
    ) {
        let Some((name, declaration)) = self.accessors.get(symbol) else {
            return;
        };
        let options = self.effective_async_options(symbol);
        // The flag: the read runs on every completion of each function from
        // its own to the current call's (ADR 0504 review: a guard in a
        // called closure skips it on the apply's first run).
        let mut sites = vec![(read.span, 0_usize, true)];
        let mut visited = HashSet::<solid_facts::core::Span>::default();
        while let Some((site, depth, every)) = sites.pop() {
            let Some(function) = containing_ast_function(&file.ast, site) else {
                continue;
            };
            if depth >= CALL_ATTRIBUTED_DEPTH
                || !visited.insert(function.span)
                || function.generator
                || self.lookup.function_may_be_component(file, function)
                || !site_runs_during_local_call(file, function, site)
            {
                continue;
            }
            let every = every
                && crate::interproc::site_on_every_completion(
                    self.lookup,
                    file,
                    function.span,
                    site,
                );
            let Some(binding) = function_binding_name(file, function) else {
                continue;
            };
            if self
                .entities
                .at(file.path.as_str(), binding.span)
                .is_none_or(|binding| {
                    crate::value_identity::binding_has_write(file, self.entities, binding)
                })
            {
                continue;
            }
            for reference in file.ast.identifiers.iter().filter(|identifier| {
                identifier.role == solid_facts::ast::IdentifierRole::Reference
                    && file.ast.reference_declaration(identifier.span) == Some(binding.span)
            }) {
                let Some(call) = file.ast.calls.iter().find(|call| {
                    call.direct_callee
                        && !call.construct
                        && file.ast.peel_ts_sugar_span(call.callee) == reference.span
                }) else {
                    continue;
                };
                if crate::execution_role::discarded_region_contains(file, call.span)
                    || pending_accessor_probe(file, call.span, self.lookup)
                {
                    continue;
                }
                let execution = semantic_execution_role(
                    file,
                    call.callee,
                    allowed,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                );
                let execution = async_read_role(
                    file,
                    call.callee,
                    async_execution_role(file, call.callee, execution),
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                );
                let leaf_owner = containing_leaf_owner(
                    file,
                    call.callee,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                );
                // ADR 0490: a call that runs where a direct read is
                // unsuspendable (a component body's strict-read window,
                // module evaluation, a leaf owner) reads there, as the
                // direct read does.
                let unsuspendable = leaf_owner.is_some()
                    || execution == ExecutionRole::ModuleInitialization
                    || (execution == ExecutionRole::UntrackedRendering
                        && nested_literal_runs_during_body(
                            file,
                            call.callee,
                            self.entities,
                            self.symbol_names,
                            self.lookup,
                        ));
                // ADR 0504: a call in an effect apply whose first run is
                // proven to meet the source pending reads it there.
                if every
                    && !options.declared_loading
                    && !options.opaque
                    && let Some(apply) = crate::execution_role::pending_apply_first_read(
                        file,
                        call.span,
                        declaration,
                        self.lookup,
                    )
                {
                    let call_location = location(file.path.shared(), call.span);
                    if !result
                        .async_reads
                        .iter()
                        .any(|existing| existing.location == call_location)
                    {
                        result.async_reads.push(Arc::new(AsyncRead {
                            accessor: format!("{name}()").into(),
                            location: call_location,
                            declaration: declaration.clone(),
                            execution,
                            leaf_owner: None,
                            under_loading: false,
                            mount_unresolved: false,
                            call_attributed: true,
                            apply_first_run_pending: true,
                            pending_origin: Some(location(file.path.shared(), read.span)),
                            pending_apply: Some(location(file.path.shared(), apply)),
                            async_provenance: true,
                            declared_loading: false,
                            options_opaque: false,
                            ssr_client_hole: options.ssr_client_bare,
                            server_rendering_unresolved: options.server_rendering_unresolved,
                            host_callback_timing: false,
                            callee_callback_timing: false,
                            invocation_context_unproven: false,
                        }));
                    }
                    continue;
                }
                if execution != ExecutionRole::TrackedJsx && !unsuspendable {
                    // The call may itself sit in a named local function a
                    // rendered call runs.
                    sites.push((call.span, depth + 1, every));
                    continue;
                }
                let call_location = location(file.path.shared(), call.span);
                if result
                    .async_reads
                    .iter()
                    .any(|existing| existing.location == call_location)
                {
                    continue;
                }
                let cover = read_loading_cover(self.lookup, file, call.callee, self.symbol_names);
                result.async_reads.push(Arc::new(AsyncRead {
                    accessor: format!("{name}()").into(),
                    location: call_location,
                    declaration: declaration.clone(),
                    execution,
                    leaf_owner: leaf_owner.map(Into::into),
                    under_loading: cover == LoadingCover::Covered,
                    mount_unresolved: cover == LoadingCover::Unresolved,
                    call_attributed: true,
                    apply_first_run_pending: false,
                    pending_origin: None,
                    pending_apply: None,
                    async_provenance: true,
                    declared_loading: options.declared_loading,
                    options_opaque: options.opaque,
                    ssr_client_hole: options.ssr_client_bare,
                    server_rendering_unresolved: options.server_rendering_unresolved,
                    host_callback_timing: host_callback_timing(
                        file,
                        call.callee,
                        execution,
                        self.lookup,
                    ),
                    callee_callback_timing: callee_callback_timing(
                        file,
                        call.callee,
                        execution,
                        self.lookup,
                    ),
                    invocation_context_unproven: false,
                }));
            }
        }
    }

    fn push_accessor_read(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        symbol: &str,
        accessor: &(SymbolId, Location),
        execution: ExecutionRole,
        result: &mut LocalAccessResult,
    ) {
        let (name, declaration) = accessor;
        let origin = self.accessor_origins.get(symbol);
        let display_name = call.static_callee(&file.source).unwrap_or(name);
        result.reads.push(Arc::new(ReactiveRead {
            summary_attributed: false,
            transported: false,
            kind: "accessor".into(),
            accessor: origin
                .map_or_else(|| display_name.to_string(), |origin| origin.0.to_string())
                .into(),
            location: location(file.path.shared(), call.span),
            declaration: origin.map_or_else(|| declaration.clone(), |origin| origin.2.clone()),
            execution,
            context: read_analysis_context(file, call.span, execution, self.lookup).into(),
            via: origin.map_or_else(String::new, |_| name.to_string()).into(),
            origin: origin.map(|origin| origin.2.clone()),
            origin_context: origin
                .map_or_else(String::new, |origin| origin.1.to_string())
                .into(),
            uncertain: self.lookup.inside_possible_component(file, call.span),
            missing_jsx_census: missing_jsx_census(file, call.span, execution),
            host_callback_timing: host_callback_timing(file, call.span, execution, self.lookup),
            project_consumer_non_strict: false,
            callback_invocation_unproven: false,
            callee_callback_timing: callee_callback_timing(file, call.span, execution, self.lookup),
        }));
        if counts_as_strict_read_root(file, call.span, execution, self.lookup) {
            result.strict_read_obligations += 1;
        }
        self.push_async_read(file, call, symbol, accessor, execution, result);
    }

    /// The pending-async half of [`Self::push_accessor_read`]: the async read
    /// a call of an async source makes. A read accessor transport carries
    /// (`ctx.data()`, ADR 0509) has its read row from the interprocedural
    /// summaries, and only this half here.
    fn push_async_read(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        symbol: &str,
        accessor: &(SymbolId, Location),
        execution: ExecutionRole,
        result: &mut LocalAccessResult,
    ) {
        let (name, declaration) = accessor;
        let async_provenance = self.async_sources.contains(symbol);
        let async_options = self.effective_async_options(symbol);
        if (async_provenance
            || async_options.ssr_client_bare
            || async_options.server_rendering_unresolved)
            && !pending_accessor_probe(file, call.callee, self.lookup)
        {
            let async_execution = async_execution_role(file, call.callee, execution);
            let async_execution =
                if async_options.ssr_client_bare || async_options.server_rendering_unresolved {
                    async_execution
                } else {
                    async_read_role(
                        file,
                        call.callee,
                        async_execution,
                        self.entities,
                        self.symbol_names,
                        self.lookup,
                    )
                };
            let cover = read_loading_cover(self.lookup, file, call.callee, self.symbol_names);
            // ADR 0504: the proof climbs from the read to the apply itself, so a
            // read in a closure the apply calls (its own role the body's) is
            // placed there too.
            let pending_apply = (async_provenance
                && async_execution != ExecutionRole::DiscardedRendering
                && !async_options.declared_loading
                && !async_options.opaque)
                .then(|| {
                    crate::execution_role::pending_apply_first_read(
                        file,
                        call.span,
                        declaration,
                        self.lookup,
                    )
                })
                .flatten();
            result.async_reads.push(Arc::new(AsyncRead {
                accessor: format!("{name}()").into(),
                location: location(file.path.shared(), call.span),
                declaration: declaration.clone(),
                execution: async_execution,
                leaf_owner: containing_leaf_owner(
                    file,
                    call.callee,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                )
                .map(Into::into),
                under_loading: cover == LoadingCover::Covered,
                mount_unresolved: cover == LoadingCover::Unresolved,
                call_attributed: false,
                // The proof climbs from the read to the apply itself, so a
                // read in a closure the apply calls (its own role the body's)
                // is placed there too.
                apply_first_run_pending: pending_apply.is_some(),
                pending_origin: None,
                pending_apply: pending_apply.map(|apply| location(file.path.shared(), apply)),
                async_provenance,
                declared_loading: async_options.declared_loading,
                options_opaque: async_options.opaque,
                ssr_client_hole: async_options.ssr_client_bare,
                server_rendering_unresolved: async_options.server_rendering_unresolved,
                host_callback_timing: host_callback_timing(
                    file,
                    call.callee,
                    async_execution,
                    self.lookup,
                ),
                callee_callback_timing: callee_callback_timing(
                    file,
                    call.callee,
                    async_execution,
                    self.lookup,
                ),
                invocation_context_unproven: async_execution == ExecutionRole::UntrackedRendering
                    && !nested_literal_runs_during_body(
                        file,
                        call.callee,
                        self.entities,
                        self.symbol_names,
                        self.lookup,
                    ),
            }));
        }
    }

    fn discover_member(
        &self,
        file: &solid_facts::FileFacts,
        member: &solid_facts::ast::MemberFact,
        allowed: &[solid_facts::core::Span],
        result: &mut LocalAccessResult,
        seen: &mut HashSet<(Arc<str>, u64, u64)>,
    ) {
        // A member that is another member's object is read as part of the
        // longer path, which is reported instead -- except a prop read at
        // the head of a chain (`props.snapshot` in
        // `props.snapshot?.messages[i]`): the getter that runs is that
        // prop's, and the longer path's object resolves to no source.
        let receiver = file.ast.peel_ts_sugar_span(member.object);
        let receiver_is_identifier = file.ast.identifiers.iter().any(|identifier| {
            identifier.span == receiver
                && identifier.role == solid_facts::ast::IdentifierRole::Reference
        });
        let prop_head = receiver_is_identifier
            && self
                .entities
                .get(&location(
                    file.path.shared(),
                    file.ast.peel_ts_sugar_span(member.object),
                ))
                .is_some_and(|symbol| {
                    self.prop_sources.contains_key(symbol)
                        && self.source_kinds.get(symbol) != Some(&ReactiveSourceKind::Store)
                });
        // ADR 0498: a member that is the object of a written, deleted or
        // iterated member (`store.a` in `store.a.v = 1`) is read on its own:
        // the longer path is no read, so the Get of this one is the chain's.
        let read_as_part = |candidate: &solid_facts::ast::MemberFact| {
            file.ast.peel_ts_sugar_span(candidate.object) == member.span
                && !(file.ast.is_plain_assignment_target(candidate.span)
                    || file
                        .ast
                        .deleted_targets
                        .iter()
                        .any(|target| file.ast.peel_ts_sugar_span(*target) == candidate.span)
                    || file
                        .ast
                        .iteration_targets
                        .iter()
                        .any(|target| file.ast.peel_ts_sugar_span(*target) == candidate.span))
        };
        if !prop_head && file.ast.members.iter().any(read_as_part) {
            return;
        }
        // A plain assignment writes this exact member without reading its
        // old value. Compound assignments, updates, and members nested
        // inside a target (a computed key, a destructuring default) are
        // retained as reads.
        if file.ast.is_plain_assignment_target(member.span)
            || file
                .ast
                .deleted_targets
                .iter()
                .any(|target| file.ast.peel_ts_sugar_span(*target) == member.span)
            || file
                .ast
                .iteration_targets
                .iter()
                .any(|target| file.ast.peel_ts_sugar_span(*target) == member.span)
        {
            return;
        }
        let object_span = if receiver_is_identifier {
            receiver
        } else {
            member.object
        };
        let object = location(file.path.shared(), object_span);
        // ADR 0323: an inline view (`omit(props, "y").x`) is the same
        // expression as a bound one; its root is the source, and a read
        // through a view is never an exact root read.
        // Type Facts keeps no entity at an identifier under a TypeScript
        // wrapper (`(props as P).x`, `props!.x`); the binder resolves it.
        let binder_symbol = if receiver_is_identifier && self.entities.get(&object).is_none() {
            crate::source_discovery::reference_symbol(file, receiver, self.entities, self.lookup)
        } else {
            None
        };
        // An entity may sit at the view call itself (its result), which
        // is no source: the view's root is.
        let known_source = |symbol: &SymbolId| {
            self.prop_sources.contains_key(symbol)
                || self.accessors.contains_key(symbol)
                || self.typed_sources.contains_key(symbol)
        };
        let inline_root =
            if binder_symbol.is_none() && !self.entities.get(&object).is_some_and(known_source) {
                self.inline_view_root(file, member.object)
            } else {
                None
            };
        // ADR 0498: a receiver that is a member read holding exactly a
        // `const` (ADR 0481, `thing.useIt`) is that `const`.
        // The receiver may be the alias itself or a static path below it
        // (`thing.useIt.items` in `thing.useIt.items.push(1)`).
        let member_alias_symbol = {
            let mut at = file.ast.peel_ts_sugar_span(member.object);
            let mut found = None;
            for _ in 0..8 {
                if let Some(alias) = file.ast.local_alias_member(at) {
                    found = self.entities.at(file.path.as_str(), alias.source).cloned();
                    break;
                }
                let Some(inner) = file
                    .ast
                    .member_at(at)
                    .filter(|_| file.ast.computed_members.binary_search(&at).is_err())
                else {
                    break;
                };
                at = file.ast.peel_ts_sugar_span(inner.object);
            }
            found
        };
        // The alias fact is exact, so it comes before the entity at the
        // member (the property's own symbol, no source).
        let Some(symbol) = member_alias_symbol
            .as_ref()
            .or(inline_root.as_ref())
            .or(self.entities.get(&object))
            .or(binder_symbol.as_ref())
        else {
            return;
        };
        // ADR 0463: as for a call, an alias mention before its declarator.
        let execution = if self.lookup.local_alias_mention_withheld(file, object_span) {
            ExecutionRole::Unknown
        } else {
            semantic_execution_role(
                file,
                member.span,
                allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            )
        };
        let helper = self
            .prop_sources
            .get(symbol)
            .filter(|_| self.source_kinds.get(symbol) != Some(&ReactiveSourceKind::Store))
            .and_then(|_| self.component_prop_helper(file, member.span));
        let gate_refuses = inside_non_component_function(file, member.span, self.lookup)
                && named_callback_execution_role(file, member.span, self.lookup).is_none()
                // This literal is the exact dialect-owned children callback,
                // and this site is in its own body (nested defaults excluded).
                && direct_control_flow_body_role(
                    file,
                    member.span,
                    self.entities,
                    self.symbol_names,
                    self.lookup.dialect,
                ).is_none()
                && !matches!(
                    execution,
                    ExecutionRole::EffectApply
                        | ExecutionRole::UntrackedCallback
                        | ExecutionRole::UntrackedUnwindowed
                );
        if helper.is_none() && gate_refuses {
            return;
        }
        // ADR 0294: a Get of a typed store candidate. `Store<T>` is `T`,
        // so the type says nothing about a proxy.
        if !self.accessors.contains_key(symbol)
            && !self.prop_sources.contains_key(symbol)
            && let Some(typed) = self.typed_sources.get(symbol)
            && typed.kind == ReactiveSourceKind::Store
        {
            let key = (object.path.clone(), object.start_byte, object.end_byte);
            if !typed.implementation_visible
                && execution.reports_untracked_read()
                && seen.insert(key)
            {
                result
                    .dispatch_obligations
                    .push(typed_source_obligation(file, member.span));
            }
            return;
        }
        let source = if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
            self.accessors.get(symbol)
        } else {
            self.prop_sources.get(symbol)
        };
        let Some((name, declaration)) = source else {
            return;
        };
        // A dynamic first key is not the prop bearing that key's source
        // spelling. Literal keys use the parser's cooked property name.
        let property = if file.ast.computed_members.contains(&member.span) {
            file.ast
                .literal_computed_members
                .iter()
                .find(|key| key.span == member.span)
                .map(|key| key.key.as_str())
        } else {
            file.source_text(member.property)
        };
        if property.is_some_and(|key| self.view_hides_key(file, member.object, key, 0)) {
            return;
        }
        // A component's `ref` prop is an imperative output channel: the
        // child calls it once to publish its handle. This is not a
        // reactive read, but only when the complete member expression is
        // the direct callee. Reading or aliasing `props.ref` still flows
        // through strict-read analysis like every other prop.
        if self.prop_sources.contains_key(symbol)
            && property == Some("ref")
            && file
                .ast
                .calls
                .iter()
                .any(|call| file.ast.peel_ts_sugar_span(call.callee) == member.span)
        {
            return;
        }
        // Caller-proven props: a prop every call site passes statically
        // compiles to a plain property — reading it is not a reactive
        // read at all. Unprovable backing stays a proof obligation.
        let mut uncertain = self.uncertain_prop_sources.contains(symbol)
            || self.lookup.inside_possible_component(file, member.span);
        if self.source_kinds.get(symbol) != Some(&ReactiveSourceKind::Store)
            && let Some((_, prop_declaration)) = self.prop_sources.get(symbol)
        {
            // A view/alias can have different keys from the original
            // parameter. ADR 0216's classification is for that parameter,
            // not permission to certify a merge view's local head Get.
            let exact_root = inline_root.is_none()
                && self.entities.get(prop_declaration) == Some(symbol)
                && props_root_is_current(
                    file,
                    prop_declaration,
                    symbol,
                    self.entities,
                    self.lookup,
                )
                && props_root_attachments_are_views(
                    file,
                    self.prop_sources,
                    prop_declaration,
                    symbol,
                    self.entities,
                    self.lookup,
                );
            let backing = if exact_root {
                property.map_or(PropUse::Unknown, |property| {
                    self.props_reactivity.prop_use(prop_declaration, property)
                })
            } else {
                PropUse::Unknown
            };
            match backing {
                PropUse::Static => return,
                PropUse::Reactive => {}
                PropUse::Unknown => uncertain = true,
            }
            // ADR 0509: the getter a caller passed reads an async source, so
            // this read may read it pending: the strict read is not proven
            // (rc.17 may throw the pending read first), and outside tracked
            // JSX the pending read is an obligation.
            if exact_root
                && backing == PropUse::Reactive
                && property.is_some_and(|property| {
                    self.props_reactivity
                        .async_getter(prop_declaration, property)
                })
            {
                uncertain = true;
            }
            if exact_root
                && backing == PropUse::Reactive
                && (execution != ExecutionRole::TrackedJsx
                    || self.may_run_in_leaf_owner(file, member.span))
                && property.is_some_and(|property| {
                    self.props_reactivity
                        .async_getter(prop_declaration, property)
                })
            {
                result.dispatch_obligations.push(crate::StaticDefect {
                    kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                        callee: file
                            .source_text(member.span)
                            .unwrap_or("this prop")
                            .to_owned(),
                        member: None,
                    },
                    location: location(file.path.shared(), member.span),
                    analysis_context: "async-prop-getter".to_owned(),
                    fixes: vec![],
                    uncertain: true,
                });
            }
        }
        // ADR 0293: the store's strict-read warning is its head Get's.
        // A `then` head does not warn; a key solid-checker cannot spell
        // (a symbol, a dynamic expression) may not either. Review round
        // 1: a `then` Get is withheld only when its backing runs nothing.
        if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
            match store_get_decision(file, member.span, declaration, self.lookup) {
                StoreHeadGet::Warns => {}
                StoreHeadGet::Exempt => return,
                StoreHeadGet::Undecided(reason) => {
                    let key = (object.path.clone(), object.start_byte, object.end_byte);
                    if execution.reports_untracked_read() && seen.insert(key) {
                        result.dispatch_obligations.push(crate::StaticDefect {
                            kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                                callee: file
                                    .source_text(member.span)
                                    .unwrap_or("this store Get")
                                    .to_owned(),
                                member: None,
                            },
                            location: location(file.path.shared(), member.span),
                            analysis_context: format!("{}{reason}", crate::STORE_KEY_UNDECIDED),
                            fixes: vec![],
                            uncertain: true,
                        });
                    }
                    return;
                }
            }
        }
        let key = (object.path.clone(), object.start_byte, object.end_byte);
        if !seen.insert(key) {
            return;
        }
        let read = self.member_read(
            file,
            member,
            symbol,
            (name, declaration),
            (execution, uncertain, prop_head),
        );
        if let Some(helper) = helper {
            self.project_helper_prop_read(
                file,
                helper,
                member.span,
                allowed,
                read,
                !gate_refuses,
                result,
            );
            return;
        }
        result.reads.push(Arc::new(read));
        if counts_as_strict_read_root(file, member.span, execution, self.lookup) {
            result.strict_read_obligations += 1;
        }
        self.push_member_async_read(file, member, symbol, (name, declaration), execution, result);
    }

    fn member_read(
        &self,
        file: &solid_facts::FileFacts,
        member: &solid_facts::ast::MemberFact,
        symbol: &SymbolId,
        (name, declaration): (&SymbolId, &Location),
        (execution, uncertain, prop_head): (ExecutionRole, bool, bool),
    ) -> ReactiveRead {
        let accessor = if prop_head {
            // Name the Get we proved, including brackets/optional syntax,
            // rather than a suffix's property or an invented dotted key.
            file.source_text(member.span)
                .unwrap_or(name.as_str())
                .to_owned()
        } else {
            usize::try_from(member.span.start)
                .ok()
                .zip(usize::try_from(member.span.end).ok())
                .and_then(|(start, end)| file.source.get(start..end))
                .and_then(|path| {
                    path.find('.')
                        .map(|index| format!("{name}{}", &path[index..]))
                })
                .unwrap_or_else(|| {
                    format!(
                        "{name}.{}",
                        file.source_text(member.property).unwrap_or_default()
                    )
                })
        };
        ReactiveRead {
            summary_attributed: false,
            transported: false,
            kind: if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
                "store-path".into()
            } else {
                "component-props".into()
            },
            accessor: accessor.into(),
            location: location(file.path.shared(), member.span),
            declaration: declaration.clone(),
            execution,
            context: read_analysis_context(file, member.span, execution, self.lookup).into(),
            via: Arc::from(""),
            origin: None,
            origin_context: Arc::from(""),
            uncertain,
            missing_jsx_census: missing_jsx_census(file, member.span, execution),
            host_callback_timing: host_callback_timing(file, member.span, execution, self.lookup),
            project_consumer_non_strict: false,
            callback_invocation_unproven: false,
            callee_callback_timing: callee_callback_timing(
                file,
                member.span,
                execution,
                self.lookup,
            ),
        }
    }

    fn push_member_async_read(
        &self,
        file: &solid_facts::FileFacts,
        member: &solid_facts::ast::MemberFact,
        symbol: &SymbolId,
        (name, declaration): (&SymbolId, &Location),
        execution: ExecutionRole,
        result: &mut LocalAccessResult,
    ) {
        let member_async = self.async_sources.contains(symbol);
        let member_options = self.effective_async_options(symbol);
        if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store)
            && (member_async
                || member_options.ssr_client_bare
                || member_options.server_rendering_unresolved)
        {
            let async_execution = async_execution_role(file, member.span, execution);
            let async_execution =
                if member_options.ssr_client_bare || member_options.server_rendering_unresolved {
                    async_execution
                } else {
                    async_read_role(
                        file,
                        member.span,
                        async_execution,
                        self.entities,
                        self.symbol_names,
                        self.lookup,
                    )
                };
            let cover = read_loading_cover(self.lookup, file, member.span, self.symbol_names);
            result.async_reads.push(Arc::new(AsyncRead {
                accessor: format!(
                    "{name}.{}",
                    file.source_text(member.property).unwrap_or_default()
                )
                .into(),
                location: location(file.path.shared(), member.span),
                declaration: declaration.clone(),
                execution: async_execution,
                leaf_owner: containing_leaf_owner(
                    file,
                    member.span,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                )
                .map(Into::into),
                under_loading: cover == LoadingCover::Covered,
                mount_unresolved: cover == LoadingCover::Unresolved,
                call_attributed: false,
                apply_first_run_pending: false,
                pending_origin: None,
                pending_apply: None,
                async_provenance: member_async,
                declared_loading: member_options.declared_loading,
                options_opaque: member_options.opaque,
                ssr_client_hole: member_options.ssr_client_bare,
                server_rendering_unresolved: member_options.server_rendering_unresolved,
                host_callback_timing: host_callback_timing(
                    file,
                    member.span,
                    async_execution,
                    self.lookup,
                ),
                callee_callback_timing: callee_callback_timing(
                    file,
                    member.span,
                    async_execution,
                    self.lookup,
                ),
                invocation_context_unproven: async_execution == ExecutionRole::UntrackedRendering
                    && !nested_literal_runs_during_body(
                        file,
                        member.span,
                        self.entities,
                        self.symbol_names,
                        self.lookup,
                    ),
            }));
        }
    }

    fn discover_spreads(
        &self,
        file: &solid_facts::FileFacts,
        allowed: &[solid_facts::core::Span],
        result: &mut LocalAccessResult,
    ) {
        for spread in &file.ast.spreads {
            // The value the argument denotes, not the symbol at its first
            // byte: `...(props.q ? {} : {})` spreads no props object.
            let argument = self.entities.value_symbol(file, spread.argument);
            let inline_root = if !argument.is_some_and(|symbol| {
                self.prop_sources.contains_key(symbol) || self.accessors.contains_key(symbol)
            }) {
                self.inline_view_root(file, spread.argument)
            } else {
                None
            };
            let Some(symbol) = inline_root.as_ref().or(argument) else {
                continue;
            };
            let execution = semantic_execution_role(
                file,
                spread.span,
                allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            );
            if inside_non_component_function(file, spread.span, self.lookup)
                && named_callback_execution_role(file, spread.span, self.lookup).is_none()
                && !matches!(
                    execution,
                    ExecutionRole::EffectApply | ExecutionRole::UntrackedCallback
                )
            {
                continue;
            }
            let source = if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
                self.accessors.get(symbol)
            } else {
                self.prop_sources.get(symbol)
            };
            let Some((name, declaration)) = source else {
                continue;
            };
            // A spread unwraps every prop, so it follows the whole-object
            // caller classification.
            let mut uncertain = self.uncertain_prop_sources.contains(symbol)
                || self.lookup.inside_possible_component(file, spread.span);
            if self.source_kinds.get(symbol) != Some(&ReactiveSourceKind::Store)
                && self.prop_sources.contains_key(symbol)
            {
                match self.props_reactivity.object_use(declaration) {
                    PropUse::Static => continue,
                    // ADR 0323: a spread of a view (an `omit` or a merge)
                    // reads the view's keys, not the parameter's: the keys a
                    // view hides may be the only reactive ones, so the
                    // parameter's classification proves nothing for it.
                    PropUse::Reactive => {
                        if inline_root.is_some()
                            || self.entities.get(declaration) != Some(symbol)
                            || !props_root_is_current(
                                file,
                                declaration,
                                symbol,
                                self.entities,
                                self.lookup,
                            )
                        {
                            uncertain = true;
                        }
                    }
                    PropUse::Unknown => uncertain = true,
                }
            }
            result.reads.push(Arc::new(ReactiveRead {
                summary_attributed: false,
                transported: false,
                kind: if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
                    "store-path".into()
                } else {
                    "component-props".into()
                },
                accessor: format!("{name} spread").into(),
                location: location(file.path.shared(), spread.span),
                declaration: declaration.clone(),
                execution,
                context: read_analysis_context(file, spread.span, execution, self.lookup).into(),
                via: Arc::from(""),
                origin: None,
                origin_context: Arc::from(""),
                uncertain,
                missing_jsx_census: missing_jsx_census(file, spread.span, execution),
                host_callback_timing: host_callback_timing(
                    file,
                    spread.span,
                    execution,
                    self.lookup,
                ),
                project_consumer_non_strict: false,
                callback_invocation_unproven: false,
                callee_callback_timing: callee_callback_timing(
                    file,
                    spread.span,
                    execution,
                    self.lookup,
                ),
            }));
            if counts_as_strict_read_root(file, spread.span, execution, self.lookup) {
                result.strict_read_obligations += 1;
            }
        }
    }

    fn discover_dynamic_jsx(&self, file: &solid_facts::FileFacts, result: &mut LocalAccessResult) {
        for element in &file.ast.jsx_elements {
            let name_location = location(file.path.shared(), element.name.span);
            let Some(symbol) = self.entities.get(&name_location) else {
                continue;
            };
            if !self.async_sources.contains(symbol)
                || self.source_primitives.get(symbol).map(SymbolId::as_str) != Some("dynamic")
            {
                continue;
            }
            let execution = ExecutionRole::TrackedJsx;
            let cover = read_loading_cover(self.lookup, file, element.name.span, self.symbol_names);
            result.async_reads.push(Arc::new(AsyncRead {
                accessor: format!(
                    "<{}>",
                    file.source_text(element.name.span).unwrap_or_default()
                )
                .into(),
                location: location(file.path.shared(), element.span),
                declaration: self.source_declarations.get(symbol).map_or_else(
                    || name_location.clone(),
                    |declaration| declaration.location.clone(),
                ),
                execution,
                leaf_owner: containing_leaf_owner(
                    file,
                    element.name.span,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                )
                .map(Into::into),
                under_loading: cover == LoadingCover::Covered,
                mount_unresolved: cover == LoadingCover::Unresolved,
                call_attributed: false,
                apply_first_run_pending: false,
                pending_origin: None,
                pending_apply: None,
                async_provenance: true,
                declared_loading: false,
                options_opaque: false,
                ssr_client_hole: false,
                server_rendering_unresolved: false,
                host_callback_timing: false,
                callee_callback_timing: false,
                invocation_context_unproven: false,
            }));
        }
    }
}

pub(crate) fn append_local_access_result(
    target: &mut LocalAccessResult,
    source: &LocalAccessResult,
) {
    target.reads.extend(source.reads.iter().cloned());
    target.writes.extend(source.writes.iter().cloned());
    target
        .action_invocations
        .extend(source.action_invocations.iter().cloned());
    target
        .async_reads
        .extend(source.async_reads.iter().cloned());
    target.strict_read_obligations += source.strict_read_obligations;
    target
        .write_action_obligations
        .extend(source.write_action_obligations.iter().cloned());
    target
        .dispatch_obligations
        .extend(source.dispatch_obligations.iter().cloned());
}

pub(crate) fn append_local_access_result_owned(
    target: &mut LocalAccessResult,
    source: LocalAccessResult,
) {
    target.reads.extend(source.reads);
    target.writes.extend(source.writes);
    target.action_invocations.extend(source.action_invocations);
    target.async_reads.extend(source.async_reads);
    target.strict_read_obligations += source.strict_read_obligations;
    target
        .write_action_obligations
        .extend(source.write_action_obligations);
    target
        .dispatch_obligations
        .extend(source.dispatch_obligations);
}

/// ADR 0294: the obligation at a read whose only reactive evidence is a type.
fn typed_source_obligation(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
) -> crate::StaticDefect {
    crate::StaticDefect {
        kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
            callee: file.source_text(span).unwrap_or("this value").to_owned(),
            member: None,
        },
        location: location(file.path.shared(), span),
        analysis_context: crate::TYPE_ONLY_SOURCE.to_owned(),
        fixes: vec![],
        uncertain: true,
    }
}

/// What a store member chain's strict-read warning is (ADR 0293).
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum StoreHeadGet {
    /// The head Get's known string key warns in this dialect.
    Warns,
    /// The chain is one Get whose known string key is exempt (`then`).
    Exempt,
    /// The head key is not a known string (a symbol or dynamic key), or an
    /// exempt head is followed by a Get on its value, which may warn if the
    /// value is a store proxy. The reason words the obligation.
    Undecided(&'static str),
}

/// The chain's head Get decides: it runs first, on the store proxy itself.
/// A warning head warns whatever follows it; an exempt one decides only a
/// chain of one Get. `span` is any member of the chain.
pub(crate) fn store_head_get(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    dialect: &dyn solid_dialect::Dialect,
) -> StoreHeadGet {
    let Some(mut head) = file.ast.member_at(span) else {
        return StoreHeadGet::Undecided("the store Get's key is not a known string");
    };
    while let Some(inner) = file.ast.member_at(file.ast.peel_ts_sugar_span(head.object)) {
        head = inner;
    }
    let chained = file
        .ast
        .members
        .iter()
        .any(|member| file.ast.peel_ts_sugar_span(member.object) == head.span);
    let key = if file.ast.computed_members.binary_search(&head.span).is_ok() {
        file.ast
            .literal_computed_members
            .iter()
            .find(|literal| literal.span == head.span)
            .map(|literal| literal.key.as_ref())
    } else {
        file.source_text(head.property)
    };
    match key {
        None => StoreHeadGet::Undecided(
            "the store Get's key is not a known string; a symbol key or \"then\" does not warn",
        ),
        Some(key) if dialect.store_key_may_skip_strict_read(key) => {
            if chained {
                StoreHeadGet::Undecided(
                    "the store Get's head key does not warn, and whether its value is a store proxy whose next Get does is not proven",
                )
            } else {
                StoreHeadGet::Exempt
            }
        }
        Some(_) => StoreHeadGet::Warns,
    }
}

/// Review round 1 of ADR 0293: [`store_head_get`] for a Get on the store
/// declared at `store`. The `then` exemption is the trap's own warning only:
/// the trap then evaluates the backing property, and a getter there (or a
/// function a call of the Get runs) performs its own reads. An exempt Get
/// stays exempt only where [`exempt_store_get_runs_nothing`] proves the
/// backing property runs no code; otherwise it is undecided.
pub(crate) fn store_get_decision(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    store: &Location,
    lookup: &SemanticLookup<'_>,
) -> StoreHeadGet {
    match store_head_get(file, span, lookup.dialect) {
        StoreHeadGet::Exempt if !exempt_store_get_runs_nothing(file, span, store, lookup) => {
            StoreHeadGet::Undecided(
                "the store Get's key \"then\" does not warn, but the property it reads is not proven to be data that runs nothing (a getter there reads on access)",
            )
        }
        decision => decision,
    }
}

/// Whether the one-Get chain at `span`, whose key the dialect exempts, reads
/// a backing property that runs no code. Holds only when all of these do:
///
/// - the store is a `const [store] = createStore(literal)` (or another
///   dialect store primitive) binding in the same file, whose setter slot is
///   absent or never
///   referenced, so nothing writes the backing after it is built;
/// - the first argument is a closed object literal
///   ([`crate::interproc::accessor_provenance::closed_properties`]: data
///   properties only, no spread, computed key or `__proto__`);
/// - the key is absent from it (the Get falls through to `Object.prototype`),
///   or its value is a primitive, object or array literal, or a function
///   literal [`function_literal_runs_nothing`] proves inert;
/// - no assignment anywhere writes a member of that name or through a
///   `prototype`, and no analyzed file defines a property reflectively.
fn exempt_store_get_runs_nothing(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    store: &Location,
    lookup: &SemanticLookup<'_>,
) -> bool {
    use solid_facts::ast::RuntimeValueKind;
    let Some(member) = file.ast.member_at(span) else {
        return false;
    };
    let key = if file
        .ast
        .computed_members
        .binary_search(&member.span)
        .is_ok()
    {
        file.ast
            .literal_computed_members
            .iter()
            .find(|literal| literal.span == member.span)
            .map(|literal| literal.key.as_ref())
    } else {
        file.source_text(member.property)
    };
    let Some(key) = key else {
        return false;
    };
    if lookup.member_name_may_be_reassigned(key)
        || crate::interproc::accessor_provenance::reflective_prototype_write(lookup)
        || lookup.property_may_be_defined_reflectively()
    {
        return false;
    }
    // The store is declared in this file: the file's own bytes then bind the
    // literal for the local-access cache, and the chunk records the
    // project-wide vetoes above ([`crate::base::reads`]).
    if store.path.as_ref() != file.path.as_str() {
        return false;
    }
    let store_file = file;
    let (Ok(start), Ok(end)) = (
        u32::try_from(store.start_byte),
        u32::try_from(store.end_byte),
    ) else {
        return false;
    };
    let declaration = solid_facts::core::Span::new(start, end);
    let Some(binding) = store_file.ast.bindings.iter().find(|binding| {
        binding.shape == solid_facts::ast::BindingShape::Array
            && binding
                .array_slots
                .first()
                .and_then(Option::as_ref)
                .is_some_and(|slot| slot.span == declaration)
    }) else {
        return false;
    };
    if !binding.immutable
        || binding.array_slots.len() > 2
        || binding.names.len() != binding.array_slots.iter().flatten().count()
        || binding
            .array_slots
            .get(1)
            .and_then(Option::as_ref)
            .is_some_and(|setter| {
                store_file
                    .ast
                    .reference_declarations
                    .iter()
                    .any(|(_, target)| *target == setter.span)
            })
    {
        return false;
    }
    let Some(initializer) = binding.initializer else {
        return false;
    };
    let initializer = store_file.ast.peel_ts_sugar_span(initializer);
    let Some(call) = store_file
        .ast
        .calls
        .iter()
        .find(|call| call.span == initializer && !call.construct)
    else {
        return false;
    };
    if !lookup
        .primitive_at_call(store_file, call.span)
        .is_some_and(|primitive| lookup.dialect.returns_store(primitive))
        || call.arguments.iter().any(|argument| argument.spread)
    {
        return false;
    }
    let Some(argument) = call.arguments.first() else {
        return false;
    };
    let literal = store_file.ast.peel_ts_sugar_span(argument.span);
    let Some(properties) =
        crate::interproc::accessor_provenance::closed_properties(store_file, literal)
    else {
        return false;
    };
    let Some(property) = properties.into_iter().find(|property| {
        store_file
            .source_text(property.key)
            .is_some_and(|name| name.trim_matches(['\'', '"']) == key)
    }) else {
        return true;
    };
    if property.shorthand_binding.is_some() {
        return false;
    }
    match property.value_kind {
        RuntimeValueKind::Primitive
        | RuntimeValueKind::Nullish
        | RuntimeValueKind::Object
        | RuntimeValueKind::Array => true,
        RuntimeValueKind::Function => function_literal_runs_nothing(
            store_file,
            store_file.ast.peel_ts_sugar_span(property.value),
        ),
        RuntimeValueKind::Unknown => false,
    }
}

/// Whether calling the function literal at `span` provably runs no code but
/// its own: neither it nor its parameter defaults hold a call, a member
/// access, a reference to any binding (a global included), a spread, a
/// tagged template, JSX, or a `this`, `super` or `import` keyword. Such a
/// body builds only literals, so it reads no reactive state (under the
/// unpatched built-in prototype premise every other proof here keeps).
pub(crate) fn function_literal_runs_nothing(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
) -> bool {
    let Some(function) = file.ast.function_at(span) else {
        return false;
    };
    let within = |inner: solid_facts::core::Span| function.span.contains(inner);
    if function.generator
        || file.ast.calls.iter().any(|call| within(call.span))
        || file.ast.members.iter().any(|member| within(member.span))
        || file.ast.identifiers.iter().any(|identifier| {
            identifier.role == solid_facts::ast::IdentifierRole::Reference
                && within(identifier.span)
        })
        || file.ast.spreads.iter().any(|spread| within(spread.span))
        || file.ast.tagged_template_tags.iter().any(|tag| within(*tag))
        || file
            .ast
            .jsx_elements
            .iter()
            .any(|element| within(element.span))
    {
        return false;
    }
    // Keywords no fact records. Matching a word inside a string literal only
    // refuses, which is the safe direction.
    let Some(text) = file.source_text(function.span) else {
        return false;
    };
    !text
        .split(|character: char| {
            !(character.is_alphanumeric() || character == '_' || character == '$')
        })
        .any(|word| {
            matches!(
                word,
                "this" | "super" | "import" | "arguments" | "await" | "yield"
            )
        })
}

// Insert at the end of the existing local_access.rs module.

/// ADR 0496: the read sits directly in a function that is no component, no
/// compiler callback role and no primitive's callback argument, so the call-attributed
/// walk ([`LocalAccessAnalyzer::push_call_attributed_async_reads`]) may follow
/// its calls; that walk judges every call by its own role. A read in a named
/// closure inside a `createRoot` callback is such a read, which
/// `inside_non_component_function` (any primitive callback around it) refused.
fn read_in_named_local_function(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(function) = containing_ast_function(&file.ast, span) else {
        return false;
    };
    !file
        .compiler
        .callback_roles
        .iter()
        .any(|callback| callback.span == function.span)
        && !file
            .ast
            .arguments_containing(function.span)
            .any(|(call, index)| {
                file.ast.peel_ts_sugar_span(call.arguments[index].span) == function.span
                    && lookup.primitive_at_call(file, call.span).is_some()
            })
        && !lookup.function_may_be_component(file, function)
}
