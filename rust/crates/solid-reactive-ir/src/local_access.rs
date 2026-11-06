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
    pending_accessor_probe, read_analysis_context, read_execution_role, semantic_execution_role,
    semantic_write_execution_role, unclassified_callback_reaches_modelled_consumer,
};
use crate::identity::SymbolId;
use crate::indexes::{EntitySymbols, SemanticLookup};
use solid_facts::ProjectFacts;
use typefacts::{Declaration, Location};

mod open_compute;
pub(crate) use open_compute::OpenComputes;

/// The obligation context of a member call on a caller-supplied value
/// (ADR 0509), dropped where accessor transport resolves the call.
pub(crate) const UNRESOLVED_HANDED_MEMBER: &str = "handed-member-dispatch";

/// The one value a fresh object literal holds under `key`: its own data
/// property, a quoted computed key, or the same in a literal it spreads. A
/// second candidate, or any other spread, answers nothing.
fn literal_slot(
    file: &solid_facts::FileFacts,
    literal: solid_facts::core::Span,
    key: &str,
) -> Option<solid_facts::core::Span> {
    let ast = &file.ast;
    let inside = |span: solid_facts::core::Span| {
        literal.contains(span)
            && literal != span
            && !ast.object_properties.iter().any(|outer| {
                outer.span != span
                    && literal.contains(outer.span)
                    && outer.span.contains(span)
                    && outer.span != literal
            })
    };
    let mut values = Vec::new();
    for property in ast
        .object_properties
        .iter()
        .filter(|property| inside(property.span))
    {
        if !property.data {
            return None;
        }
        let text = file.source_text(property.key)?;
        let quoted = text.len() >= 2
            && matches!(text.as_bytes()[0], b'"' | b'\'')
            && text.as_bytes()[text.len() - 1] == text.as_bytes()[0];
        if property.computed && !quoted {
            return None;
        }
        if text.trim_matches(['\'', '"']) == key {
            values.push(property.shorthand_binding.unwrap_or(property.value));
        }
    }
    for spread in ast.spreads.iter().filter(|spread| inside(spread.span)) {
        values.push(literal_slot(
            file,
            ast.peel_ts_sugar_span(spread.argument),
            key,
        )?);
    }
    match values.as_slice() {
        [value] => Some(*value),
        _ => None,
    }
}

/// The nodes [`LocalAccessContext::calls_reach`] walks before it hands over.
const CALL_WALK_BUDGET: usize = 512;

/// What `function` may return: each `return`'s value (`None` for a bare one), or
/// the expression of an arrow with no block.
fn returned_values(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> Vec<Option<solid_facts::core::Span>> {
    if function.expression_body {
        return vec![Some(function.body)];
    }
    file.ast
        .returns
        .iter()
        .filter(|returned| {
            containing_ast_function(&file.ast, returned.span)
                .is_some_and(|owner| owner.span == function.span)
        })
        .map(|returned| returned.argument)
        .collect()
}

/// The `catch` at `handler` holds a single `throw` statement and nothing else.
fn handler_only_throws(file: &solid_facts::FileFacts, handler: solid_facts::core::Span) -> bool {
    file.ast
        .throw_statements
        .iter()
        .filter(|thrown| handler.contains(**thrown))
        .any(|thrown| {
            let around = |start: u32, end: u32| {
                file.source_text(solid_facts::core::Span::new(start, end))
                    .unwrap_or("?")
            };
            let before = around(handler.start, thrown.start);
            let after = around(thrown.end, handler.end);
            before.trim_end().ends_with('{')
                && before.matches('{').count() == 1
                && !before.contains(';')
                && after.trim_start_matches([';', ' ', '\t', '\r', '\n']) == "}"
        })
}

/// A source no pending state can reach: a signal or store of a data literal, or
/// a memo whose sync literal compute reads nothing. `createSignal(fn)` computes,
/// and an identifier may be a function.
fn source_call_holds_a_value(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: solid_dialect::Primitive,
) -> bool {
    use solid_dialect::Primitive;
    use solid_facts::ast::ArgumentValueKind;
    let first = call.arguments.first();
    match primitive {
        Primitive::CreateSignal | Primitive::CreateStore => first.is_none_or(|argument| {
            matches!(
                argument.value,
                ArgumentValueKind::Null | ArgumentValueKind::Undefined
            ) || argument.runtime_value_kind.is_data_literal()
        }),
        Primitive::CreateMemo => first.is_some_and(|argument| {
            argument.value == ArgumentValueKind::Function
                && !file
                    .ast
                    .calls
                    .iter()
                    .any(|inner| argument.span.contains(inner.span))
                && !file
                    .ast
                    .members
                    .iter()
                    .any(|member| argument.span.contains(member.span))
        }),
        _ => false,
    }
}

/// ADR 0800: how many values one [`LocalAccessContext::value_reaches`] visits.
const VALUE_WALK_BUDGET: usize = 256;

/// ADR 0800: what a value's walk answers. Every caller opens an obligation on
/// [`Self::may`]: a walk that ran out of budget decided nothing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ValueReach {
    Reaches,
    Clear,
    Undecided,
}

impl ValueReach {
    pub(crate) fn may(self) -> bool {
        self != Self::Clear
    }
}

/// The walk's state: what it has visited, and how much it may still visit.
struct ValueWalk {
    seen: HashSet<(Arc<str>, solid_facts::core::Span)>,
    left: usize,
    /// How many steps deep it stands, and how deep it may go, if it is cut.
    depth: usize,
    cut_at: Option<usize>,
}

/// ADR 0802: a tracked call of a value that holds an accessor, kept only if
/// [`LocalAccessContext::value_holds_open_source`] says so; then it is
/// [`UNRESOLVED_HANDED_READ`].
pub(crate) const HELD_OPEN_SOURCE: &str = "held-open-source";

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
    /// ADR 0771: at a tracked position a read of a settled accessor is legal,
    /// so the accessors that count are these. `None`: every one.
    pub(crate) accessors_that_count: Option<&'a dyn Fn(&str) -> bool>,
}

/// Whether `member`'s object is a local literal that declares an accessor.
pub(crate) fn literal_member_has_getter(
    file: &solid_facts::FileFacts,
    member: &solid_facts::ast::MemberFact,
) -> bool {
    file.ast
        .reference_declaration(file.ast.peel_ts_sugar_span(member.object))
        .and_then(|declaration| {
            file.ast
                .bindings
                .iter()
                .find(|binding| binding.names.iter().any(|name| name.span == declaration))
        })
        .and_then(|binding| binding.initializer)
        .is_some_and(|initializer| {
            let initializer = file.ast.peel_ts_sugar_span(initializer);
            file.ast.object_get_shapes.iter().any(|shape| {
                shape.span == initializer
                    && shape
                        .properties
                        .iter()
                        .any(|property| property.getter.is_some())
            })
        })
}

/// ADR 0509: whether the value at `span` is one a caller supplied
/// ([`UNRESOLVED_HANDED_MEMBER`]), through at most `depth` steps. ADR 0751:
/// past them it may be, so it answers true (this only opens obligations).
pub(crate) fn handed_value(
    t: &HandedTables<'_, '_>,
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    depth: usize,
) -> bool {
    // Past the bound the value is unknown, and this only opens obligations.
    if depth == 0 {
        return true;
    }
    let span = file.ast.peel_ts_sugar_span(span);
    if let Some(member) = file.ast.member_at(span) {
        // A computed member too (`pair[1]`): this only opens obligations.
        return handed_value(t, file, member.object, depth - 1);
    }
    // F-B: an operand's value, whichever operand runs.
    if let Some(conditional) = file
        .ast
        .conditional_expressions
        .iter()
        .find(|conditional| conditional.span == span)
    {
        return handed_value(t, file, conditional.consequent, depth - 1)
            || handed_value(t, file, conditional.alternate, depth - 1);
    }
    if let Some(logical) = file
        .ast
        .logical_expressions
        .iter()
        .find(|logical| logical.span == span)
    {
        return handed_value(t, file, logical.left, depth - 1)
            || handed_value(t, file, logical.right, depth - 1);
    }
    if let Some(sequence) = file
        .ast
        .sequences
        .iter()
        .find(|sequence| sequence.span == span)
    {
        return handed_value(t, file, sequence.tail, depth - 1);
    }
    // ADR 0777: an assignment's value, as a callee (`(slot ||= r1)()`).
    let operands = file.ast.composite_callee_references(span);
    if operands.iter().any(|operand| *operand != span) {
        return operands
            .iter()
            .any(|operand| handed_value(t, file, *operand, depth - 1));
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
    // F0: a hook's result used on the spot (`useT(p.n).read()`, `useT(n)()()`),
    // which no binding holds.
    if let Some(call) = file.ast.call_at(span) {
        return handed_call(t, file, call, depth);
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
        (t.accessors.contains_key(symbol)
            && t.accessors_that_count.is_none_or(|counts| counts(symbol)))
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
    // F0: a callback's parameter holds an element of the standard iteration's
    // receiver (`useT(n).forEach((f) => f())`).
    if let Some(receiver) = iteration_receiver_of_parameter(t, file, declaration) {
        return handed_value(t, file, receiver, depth - 1);
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
        && handed_call(t, file, call, depth)
    {
        return true;
    }
    if binding
        .initializer
        .is_some_and(|initializer| handed_value(t, file, initializer, depth - 1))
    {
        return true;
    }
    // F-B: what a rebound binding was given.
    if !binding.immutable
        && file.ast.assignments.iter().any(|assignment| {
            file.ast
                .reference_declaration(file.ast.peel_ts_sugar_span(assignment.target))
                == Some(declaration)
                && handed_value(t, file, assignment.value_span, depth - 1)
        })
    {
        return true;
    }
    // F-B: what a standard container was filled with (`m.set("f", p.f)`).
    let container = binding.initializer.is_some_and(|initializer| {
        let initializer = file.ast.peel_ts_sugar_span(initializer);
        file.ast.array_literal_at(initializer).is_some()
            || file.ast.call_at(initializer).is_some_and(|call| {
                let constructor = file.ast.peel_ts_sugar_span(call.callee);
                call.construct
                    && file.ast.reference_declaration(constructor).is_none()
                    && matches!(
                        file.source_text(constructor),
                        Some("Map" | "Set" | "WeakMap" | "WeakSet" | "Array")
                    )
            })
    });
    container
        && file.ast.calls.iter().any(|call| {
            file.ast
                .member_at(file.ast.peel_ts_sugar_span(call.callee))
                .is_some_and(|member| {
                    file.ast
                        .reference_declaration(file.ast.peel_ts_sugar_span(member.object))
                        == Some(declaration)
                })
                && call
                    .arguments
                    .iter()
                    .any(|argument| handed_value(t, file, argument.span, depth - 1))
        })
}

/// The receiver of the standard-library call `declaration`'s function is an
/// argument of, when `declaration` is one of that function's parameters.
fn iteration_receiver_of_parameter(
    t: &HandedTables<'_, '_>,
    file: &solid_facts::FileFacts,
    declaration: solid_facts::core::Span,
) -> Option<solid_facts::core::Span> {
    let callback = file.ast.functions.iter().find(|function| {
        function
            .parameters
            .iter()
            .any(|parameter| parameter.names.iter().any(|name| name.span == declaration))
    })?;
    let call = file.ast.calls.iter().find(|call| {
        call.arguments
            .iter()
            .any(|argument| file.ast.peel_ts_sugar_span(argument.span) == callback.span)
    })?;
    t.lookup
        .standard_library_declaration(file, call, typefacts::CallKind::Call)?;
    file.ast
        .member_at(file.ast.peel_ts_sugar_span(call.callee))
        .map(|member| member.object)
}

/// Whether the value `call` answers is one a caller supplied: `useContext`'s, or
/// a project function's. A call of a call is the inner call's answer.
fn handed_call(
    t: &HandedTables<'_, '_>,
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
    depth: usize,
) -> bool {
    let depth = depth.saturating_sub(1);
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    let handed_argument = || {
        call.arguments
            .iter()
            .any(|argument| handed_value(t, file, argument.span, depth))
    };
    // F-B: an object built around a handed value (`new Map([["f", p.f]])`), or a
    // class that holds one.
    if call.construct {
        return handed_argument() || class_holds_handed_value(t, file, callee, depth);
    }
    if let Some(primitive) = t.lookup.primitive_at_call(file, call.span) {
        return t
            .lookup
            .dialect
            .name_of(primitive)
            .and_then(|name| t.lookup.dialect.context_role(name))
            == Some(solid_dialect::ContextRole::Read);
    }
    if t.lookup
        .function_called_at(file.path.as_str(), callee)
        .is_some()
    {
        return true;
    }
    if let Some(inner) = file.ast.call_at(callee) {
        return handed_call(t, file, inner, depth);
    }
    // F-B: an IIFE's answer.
    if let Some(function) = file.ast.function_at(callee) {
        let returned = function
            .expression_body
            .then_some(function.body)
            .into_iter()
            .chain(
                file.ast
                    .returns
                    .iter()
                    .filter(|returned| {
                        crate::owners::containing_ast_function(&file.ast, returned.span)
                            .is_some_and(|holder| holder.span == function.span)
                    })
                    .filter_map(|returned| returned.argument),
            )
            .collect::<Vec<_>>();
        return returned
            .into_iter()
            .any(|value| handed_value(t, file, value, depth));
    }
    let callee_member = file.ast.member_at(callee);
    // ADR 0772: which function a computed member holds is unknown (`hooks[0]()`).
    if callee_member.is_some_and(|member| {
        file.ast
            .computed_members
            .binary_search(&member.span)
            .is_ok()
    }) {
        return true;
    }
    // ADR 0772: `useA.call(null)` and `useA.bind(null)` answer as `useA`, a
    // standard method's (`Array.prototype.slice.call(p.xs)`) below.
    if callee_member.is_some_and(|member| {
        function_prototype_dispatch(t.lookup, file, call) && !rooted_at_global(file, member.object)
    }) {
        return true;
    }
    let callee_binding = file
        .ast
        .reference_declaration(callee)
        .and_then(|declaration| {
            file.ast
                .bindings
                .iter()
                .find(|binding| binding.names.iter().any(|name| name.span == declaration))
        });
    // ADR 0772: what a rebound binding holds at the call is unknown.
    if callee_binding.is_some_and(|binding| !binding.immutable) {
        return true;
    }
    // F-B: a function chosen at run time (`const hook = c ? useA : useB`).
    let chosen = callee_binding
        .and_then(|binding| binding.initializer)
        .unwrap_or(callee);
    if !file.ast.composite_callee_operands(chosen).is_empty() {
        return true;
    }
    // ADR 0772: a method of a handed value (`props.api.use()`), which a
    // standard method's own arm below also asks.
    if callee_member.is_some_and(|member| handed_value(t, file, member.object, depth)) {
        return true;
    }
    // F-B: a standard call hands on what it was given (`m.get("f")`).
    t.lookup
        .standard_library_declaration(file, call, typefacts::CallKind::Call)
        .is_some()
        && (handed_argument()
            || file
                .ast
                .member_at(callee)
                .is_some_and(|member| handed_value(t, file, member.object, depth)))
}

/// ADR 0772: whether `call` is `Function.prototype`'s `call`, `apply` or `bind`.
pub(crate) fn function_prototype_dispatch(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
) -> bool {
    lookup
        .standard_library_declaration(file, call, typefacts::CallKind::Call)
        .is_some_and(|declaration| {
            matches!(
                declaration.qualified_name.as_ref(),
                "Function.call"
                    | "Function.apply"
                    | "Function.bind"
                    | "CallableFunction.call"
                    | "CallableFunction.apply"
                    | "CallableFunction.bind"
            )
        })
}

/// Whether the member chain at `span` starts at a name no declaration in the
/// file binds (`Array.prototype.sort`).
fn rooted_at_global(file: &solid_facts::FileFacts, span: solid_facts::core::Span) -> bool {
    let mut root = file.ast.peel_ts_sugar_span(span);
    while let Some(member) = file.ast.member_at(root) {
        root = file.ast.peel_ts_sugar_span(member.object);
    }
    file.ast.reference_declaration(root).is_none()
}

/// Whether the class `callee` names has a field holding a handed value.
fn class_holds_handed_value(
    t: &HandedTables<'_, '_>,
    file: &solid_facts::FileFacts,
    callee: solid_facts::core::Span,
    depth: usize,
) -> bool {
    let Some(declaration) = file.ast.reference_declaration(callee) else {
        return false;
    };
    let holds = |value: solid_facts::core::Span| {
        // ADR 0777: a function an instance holds runs where it is called.
        file.ast
            .function_at(file.ast.peel_ts_sugar_span(value))
            .is_some()
            || handed_value(t, file, value, depth)
    };
    file.ast
        .classes
        .iter()
        .filter(|class| {
            class
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration)
        })
        .any(|class| {
            class
                .elements
                .iter()
                .filter(|element| element.kind == solid_facts::ast::ClassElementKind::Field)
                .filter_map(|element| element.value)
                .any(holds)
                // `this.m = () => …` in the class.
                || file.ast.assignments.iter().any(|assignment| {
                    class.span.contains(assignment.target)
                        && file.ast.member_at(file.ast.peel_ts_sugar_span(assignment.target)).is_some_and(|member| file.source_text(member.object) == Some("this"))
                        && holds(assignment.value_span)
                })
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

/// What a store's literal initial value says about its first reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StoreShape {
    /// An array literal: `length` is there to read.
    Array,
    /// An object literal with a key of its own.
    Keyed,
    /// `{}`: no key to read yet.
    Empty,
    /// The `[store, setter]` pair itself, bound whole: no store is read.
    Tuple,
    /// Anything else, a derive's seed included.
    Unknown,
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
    pub(crate) reachable_calls: &'a HashMap<Location, u64>,
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
    /// ADR 0803: slots source discovery withdrew (`SourceDiscovery`).
    pub(crate) withdrawn_array_slots: &'a HashSet<SymbolId>,
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

impl<'facts> LocalAccessContext<'_, 'facts> {
    pub(crate) fn handed_tables<'t>(
        &'t self,
        accessors_that_count: Option<&'t dyn Fn(&str) -> bool>,
    ) -> HandedTables<'t, 'facts> {
        HandedTables {
            lookup: self.lookup,
            entities: self.entities,
            prop_sources: self.prop_sources,
            source_kinds: self.source_kinds,
            accessors: self.accessors,
            setters: self.setters,
            actions: self.actions,
            props_reactivity: self.props_reactivity,
            accessors_that_count,
        }
    }
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
            // The symbols whose source-discovery entries moved since the
            // files were last checked, by value: whichever file's discovery
            // or project pass moved them (ADR 0397).
            let sources = self.sources();
            // ADR 0803: a moved withdrawal reaches references no dependency
            // names (`{ k: set }`): every chunk is recomputed.
            let withdrawal_unchanged = cache.sources.as_ref().is_none_or(|previous| {
                previous.withdrawn_array_slots == sources.withdrawn_array_slots
            });
            let exact_typescript_delta =
                (typescript_unchanged || source_discovery_delta.is_some()) && withdrawal_unchanged;
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
            // Each kept chunk's proof is checked on the workers: a check
            // reads only its own entry and this build's lookup, and one at a
            // time it was most of the stage on a large project.
            let kept = &cache.files;
            let files = &self.facts.files;
            let reusable = parallel_slice_results(files, |file| {
                exact_typescript_delta
                    && kept.get(file.path.as_str()).is_some_and(|cached| {
                        self.cached_matches(
                            file,
                            cached,
                            unchanged_paths.contains(file.path.as_str()),
                            &changed_dependencies,
                        )
                    })
            });
            let mut recompute = Vec::new();
            for (file, reusable) in files.iter().zip(reusable) {
                if reusable {
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
            withdrawn_array_slots: self.withdrawn_array_slots.clone(),
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
    ) -> Vec<(Location, Option<u64>)> {
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
        // A `const` view name: continue at its initializer.
        let Some(call) = self.view_call(file, expression) else {
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

    /// ADR 0630: where a Get of `key` on the `merge` view `expression` lands.
    /// rc.21's `merge` walks its sources last to first, one `in` each
    /// (`@solidjs/signals@2.0.0-rc.21` `dist/dev.js`), and the Get lands on the
    /// last source that has the key, so a key an object literal holds after
    /// the store is that literal's data: no read. (Several sources return the
    /// last one as it is only in the no-Proxy fallback.)
    fn merge_view_get(
        &self,
        file: &solid_facts::FileFacts,
        expression: solid_facts::core::Span,
        key: Option<&str>,
    ) -> MergeGet {
        let call = self.view_call(file, expression);
        let Some(call) = call.filter(|call| {
            self.lookup.primitive_at_call(file, call.span) == Some(solid_dialect::Primitive::Merge)
        }) else {
            return MergeGet::NoView;
        };
        let Some(key) = key else {
            return MergeGet::Unproven;
        };
        for argument in call.arguments.iter().rev() {
            let value = file.ast.peel_ts_sugar_span(argument.span);
            if argument.spread {
                return MergeGet::Unproven;
            }
            if file
                .ast
                .object_get_shapes
                .iter()
                .any(|shape| shape.span == value && shape.closed)
            {
                if crate::execution_role::closed_literal_slot_value(file, value, key).is_some() {
                    return MergeGet::Plain;
                }
                continue;
            }
            let store = self
                .entities
                .value_symbol(file, value)
                .filter(|symbol| self.source_kinds.get(*symbol) == Some(&ReactiveSourceKind::Store))
                .and_then(|symbol| self.accessors.get(symbol));
            return match store {
                Some((_, declaration)) if self.store_holds_key(file, declaration, key) => {
                    MergeGet::Store
                }
                _ => MergeGet::Unproven,
            };
        }
        MergeGet::Plain
    }

    /// The store's own literal initial value holds `key`.
    fn store_holds_key(
        &self,
        file: &solid_facts::FileFacts,
        declaration: &Location,
        key: &str,
    ) -> bool {
        self.store_initial_value(file, declaration)
            .is_some_and(|value| {
                crate::execution_role::closed_literal_slot_value(file, value, key).is_some()
            })
    }

    /// The call a `merge` or `omit` view expression is: itself, or a `const`'s
    /// initializer.
    fn view_call<'f>(
        &self,
        file: &'f solid_facts::FileFacts,
        expression: solid_facts::core::Span,
    ) -> Option<&'f solid_facts::ast::CallFact> {
        let expression = file.ast.peel_ts_sugar_span(expression);
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
        file.ast.call_at(named.map_or(expression, |initializer| {
            file.ast.peel_ts_sugar_span(initializer)
        }))
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
                && !crate::execution_role::handler_event_denied(file, callback.span)
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
                && !crate::execution_role::handler_event_denied(file, callback.span)
                && crate::owners::yielded_value_contains(file, callback.span, caller.span)
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
            let execution = read_execution_role(
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
        self.push_withdrawn_slot_obligations(file, &mut result);
        for call in &file.ast.calls {
            self.push_function_call_target_obligation(file, call, &mut result);
            self.discover_call(file, call, &allowed, &mut result, &mut seen);
            if self.lookup.primitive_at_call(file, call.span)
                == Some(solid_dialect::Primitive::Untrack)
            {
                self.discover_untrack_reference(file, call, &allowed, &mut result);
            } else {
                if self
                    .lookup
                    .callee_symbol(file, call.callee)
                    .is_none_or(|symbol| {
                        !self.accessors.contains_key(symbol)
                            || (!self.async_sources.contains(symbol)
                                && self.value_reaches_async_source(file, call.callee).may())
                    })
                {
                    // ADR 0640: a value closed forms name exactly is that source.
                    match self.closed_source(file, call.callee, 6) {
                        // Under an `untrack` that keeps the pending throw: a read in
                        // a computation or a leaf is its own rule's, by its role.
                        Some(symbol)
                            if self.async_sources.contains(&symbol)
                                && crate::execution_role::untrack_layers(
                                    file,
                                    call.callee,
                                    None,
                                    self.entities,
                                    self.symbol_names,
                                    self.lookup,
                                )
                                .is_some_and(|layers| layers.keeps_pending_throw()) =>
                        {
                            if let Some(accessor) = self.accessors.get(&symbol) {
                                let execution = read_execution_role(
                                    file,
                                    call.callee,
                                    &allowed,
                                    self.entities,
                                    self.symbol_names,
                                    self.lookup,
                                );
                                self.push_async_read(
                                    file,
                                    call,
                                    symbol.as_str(),
                                    accessor,
                                    execution,
                                    None,
                                    &mut result,
                                );
                            }
                        }
                        Some(symbol) if !self.async_sources.contains(&symbol) => {}
                        _ => self.push_untrack_hook_value_obligation(
                            file,
                            call.callee,
                            None,
                            &mut result,
                        ),
                    }
                }
                self.discover_handed_to_untrack(file, call, &allowed, &mut result);
            }
        }
        for member in &file.ast.members {
            self.discover_member(file, member, &allowed, &mut result, &mut seen);
        }
        self.discover_spreads(file, &allowed, &mut result);
        self.discover_whole_store_reads(file, &allowed, &mut result);
        self.discover_caught_reads(file, &allowed, &mut result);
        self.discover_optimistic_reach(file, &mut result);
        self.discover_dynamic_jsx(file, &mut result);
        // ADR 0646: a read in a function only a carrier that may run it zero
        // times runs proves nothing, on whichever path made its row.
        for read in &mut result.reads {
            if read.callback_invocation_unproven
                || read.location.path.as_ref() != file.path.as_str()
            {
                continue;
            }
            let (Ok(start), Ok(end)) = (
                u32::try_from(read.location.start_byte),
                u32::try_from(read.location.end_byte),
            ) else {
                continue;
            };
            if named_carrier_may_not_run(file, solid_facts::core::Span { start, end }, self.lookup)
            {
                Arc::make_mut(read).callback_invocation_unproven = true;
            }
        }
        result
    }

    /// Whether `span` names a known setter, accessor or action.
    fn reactive_symbol(
        &self,
        file: &solid_facts::FileFacts,
        span: solid_facts::core::Span,
    ) -> bool {
        self.lookup.callee_symbol(file, span).is_some_and(|symbol| {
            self.setters.contains_key(symbol)
                || self.actions.contains_key(symbol)
                || self.accessors.contains_key(symbol)
        })
    }

    /// ADR 0516: the reference `f` of a `Function.prototype` dispatch callee
    /// (`f.call`, `f.apply`, or a call `f.bind(…)`, non-computed members)
    /// when `f` is a known setter, accessor or action.
    fn dispatch_target(
        &self,
        file: &solid_facts::FileFacts,
        callee: solid_facts::core::Span,
    ) -> Option<solid_facts::core::Span> {
        let ast = &file.ast;
        let callee = ast.peel_ts_sugar_span(callee);
        let member_of = |span: solid_facts::core::Span, names: &[&str]| {
            ast.member_at(span)
                .filter(|member| {
                    ast.computed_members.binary_search(&member.span).is_err()
                        && file
                            .source_text(member.property)
                            .is_some_and(|property| names.contains(&property))
                })
                .map(|member| ast.peel_ts_sugar_span(member.object))
        };
        member_of(callee, &["call", "apply"])
            .or_else(|| {
                ast.call_at(callee)
                    .and_then(|inner| member_of(ast.peel_ts_sugar_span(inner.callee), &["bind"]))
            })
            .filter(|object| self.reactive_symbol(file, *object))
    }

    /// ADR 0516: whether a dispatch invokes its function whenever the call
    /// runs. `f.call(…)` with no spread at the receiver's place, and
    /// `f.bind(…)(…)`, do; `f.apply(t, x)` does where `x` is absent, `null`,
    /// `undefined` or an array literal (another value may throw first).
    fn dispatch_invokes(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
    ) -> bool {
        dispatch_invokes(file, call)
    }

    /// ADR 0688: `Array.prototype.sort.call(store)`. The callee resolves to
    /// `Function.call`, whose target no fact gives, so a reactive value handed
    /// to it reaches code nothing here reads: an obligation, never silence.
    /// ADR 0773: so do `.bind(store)()` and `Reflect.apply(sort, store, [])`,
    /// and an `.apply` whose argument list no fact shows.
    /// ADR 0803: each reference of a slot withdrawn because the program
    /// writes the array iterator: what it holds is not the primitive's own.
    fn push_withdrawn_slot_obligations(
        &self,
        file: &solid_facts::FileFacts,
        result: &mut LocalAccessResult,
    ) {
        if self.withdrawn_array_slots.is_empty() {
            return;
        }
        // TypeFacts' references of each withdrawn slot in this file, which an
        // import or a non-callee position (`{ k: set }`) does not resolve to.
        let referenced = self
            .withdrawn_array_slots
            .iter()
            .flat_map(|symbol| self.lookup.symbol_references(symbol.as_str()))
            .filter(|reference| reference.path.as_ref() == file.path.as_str())
            .filter_map(|reference| {
                Some(solid_facts::core::Span::new(
                    u32::try_from(reference.start_byte).ok()?,
                    u32::try_from(reference.end_byte).ok()?,
                ))
            })
            .collect::<HashSet<_>>();
        for identifier in &file.ast.identifiers {
            // A reference outside callee position (`{ k: set }`, `[0, set]`)
            // has no callee symbol: its declaration's entity names it.
            if identifier.role != solid_facts::ast::IdentifierRole::Reference
                || !(referenced.contains(&identifier.span)
                    || self
                        .lookup
                        .callee_symbol(file, identifier.span)
                        .is_some_and(|symbol| self.withdrawn_array_slots.contains(symbol))
                    || file
                        .ast
                        .reference_declaration(identifier.span)
                        .and_then(|declaration| {
                            self.lookup.entity_at(file.path.as_str(), declaration)
                        })
                        .is_some_and(|entity| self.withdrawn_array_slots.contains(&*entity.symbol)))
            {
                continue;
            }
            result
                .dispatch_obligations
                .push(withdrawn_slot_obligation(file, identifier.span));
        }
    }

    fn push_function_call_target_obligation(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        result: &mut LocalAccessResult,
    ) {
        let ast = &file.ast;
        let reflected = self
            .lookup
            .standard_library_declaration(file, call, typefacts::CallKind::Call)
            .is_some_and(|declaration| declaration.qualified_name.as_ref() == "Reflect.apply");
        if !reflected {
            let Some(member) = ast.member_at(ast.peel_ts_sugar_span(call.callee)) else {
                return;
            };
            if !function_prototype_dispatch(self.lookup, file, call)
                || self
                    .lookup
                    .function_called_at(file.path.as_str(), member.object)
                    .is_some()
                || self
                    .lookup
                    .callee_symbol(file, member.object)
                    .is_some_and(|symbol| {
                        self.setters.contains_key(symbol)
                            || self.actions.contains_key(symbol)
                            || self.accessors.contains_key(symbol)
                    })
            {
                return;
            }
        }
        let reactive = call.arguments.iter().any(|argument| {
            let mut root = ast.peel_ts_sugar_span(argument.span);
            while let Some(member) = ast.member_at(root) {
                root = ast.peel_ts_sugar_span(member.object);
            }
            self.lookup.callee_symbol(file, root).is_some_and(|symbol| {
                self.setters.contains_key(symbol)
                    || self.actions.contains_key(symbol)
                    || self.accessors.contains_key(symbol)
            })
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
            analysis_context: crate::blockers::FUNCTION_CALL_TARGET_UNRESOLVED.to_owned(),
            fixes: vec![],
            uncertain: true,
        });
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
        // ADR 0516: `f.call(…)`, `f.apply(…)` and `f.bind(…)(…)` invoke `f`
        // through `Function.prototype`. With `f` a known setter, accessor or
        // action the call is `f`'s, judged as `f(…)` at this call
        // ([`Self::discover_call_as`]: `f` names the symbol, the written call
        // places it); where `f` may not be invoked (`dispatch_invokes`), an
        // obligation, never silence. In an event handler a write, an action
        // call and a read are all legal, so there is nothing to owe.
        if let Some(target) = self.dispatch_target(file, call.callee) {
            if self.dispatch_invokes(file, call) {
                self.discover_call_as(file, call, target, allowed, result, seen);
                return;
            }
            if semantic_execution_role(
                file,
                call.callee,
                allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            ) == ExecutionRole::EventCallback
            {
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
                analysis_context: "function-prototype-callee-unresolved".to_owned(),
                fixes: vec![],
                uncertain: true,
            });
            return;
        }
        if self.lookup.callee_symbol(file, call.callee).is_none() {
            if let Some(created) = file
                .ast
                .call_at(file.ast.peel_ts_sugar_span(call.callee))
                .filter(|inner| self.creates_whole_accessor(file, inner))
            {
                self.discover_created_accessor_call(file, call, created, allowed, result);
                return;
            }
            // F-B: a call of what a call handed back (`m.get("f")!()`, `make()()`).
            // ADR 0770: or a member no type names (`props.o.go()`, `o: any`).
            let peeled = file.ast.peel_ts_sugar_span(call.callee);
            if (file.ast.call_at(peeled).is_some()
                || (file.ast.member_at(peeled).is_some()
                    && self
                        .lookup
                        .standard_library_declaration(file, call, typefacts::CallKind::Call)
                        .is_none()))
                && self.handed_callee(file, call.callee)
            {
                let execution = read_execution_role(
                    file,
                    call.callee,
                    allowed,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                );
                self.push_owned_dispatch_obligation(
                    file,
                    call,
                    allowed,
                    execution,
                    UNRESOLVED_HANDED_MEMBER,
                    result,
                );
                return;
            }
            let reactive = file
                .ast
                .composite_callee_references(call.callee)
                .into_iter()
                .filter(|operand| self.reactive_symbol(file, *operand))
                .collect::<Vec<_>>();
            let operands = file.ast.composite_callee_operands(call.callee);
            let one_symbol = reactive.iter().all(|operand| {
                self.lookup.callee_symbol(file, *operand)
                    == reactive
                        .first()
                        .and_then(|first| self.lookup.callee_symbol(file, *first))
            });
            // ADR 0516: a composite callee that always yields one of its
            // operands, all naming one reactive function, is that function's
            // call here.
            if !reactive.is_empty()
                && reactive.len() == operands.len()
                && one_symbol
                && file.ast.composite_callee_total(call.callee)
            {
                self.discover_call_as(file, call, reactive[0], allowed, result, seen);
                return;
            }
            // Otherwise no finding is proven, but a clean is: every reactive
            // operand's call here reports nothing. In an async function, also
            // through layers inside it, the after-await rule, which this probe
            // does not ask, may report the direct read.
            if !reactive.is_empty()
                && !file
                    .ast
                    .functions
                    .iter()
                    .any(|function| function.r#async && function.body.contains(call.span))
            {
                let mut scratch = LocalAccessResult::default();
                let mut scratch_seen = seen.clone();
                for operand in &reactive {
                    self.discover_call_as(
                        file,
                        call,
                        *operand,
                        allowed,
                        &mut scratch,
                        &mut scratch_seen,
                    );
                }
                if finding_free(&scratch) {
                    seen.extend(scratch_seen);
                    append_local_access_result_owned(result, scratch);
                    return;
                }
            }
            self.push_composite_callee_obligation(file, call, result);
            return;
        }
        self.discover_call_as(file, call, call.callee, allowed, result, seen);
    }

    /// Whether `call` is a primitive whose whole result is an accessor.
    fn creates_whole_accessor(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
    ) -> bool {
        self.lookup
            .primitive_at_call(file, call.span)
            .is_some_and(|primitive| {
                self.lookup
                    .dialect
                    .reactive_result_slot(primitive, solid_dialect::ResultSlot::Whole)
                    == Some(solid_dialect::ReactiveRole::Accessor)
            })
    }

    /// ADR 0690: `createMemo(() => c())()`, the accessor a primitive just made,
    /// called where it is made. A plain read, unless its compute may suspend.
    fn discover_created_accessor_call(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        created: &solid_facts::ast::CallFact,
        allowed: &[solid_facts::core::Span],
        result: &mut LocalAccessResult,
    ) {
        let execution = read_execution_role(
            file,
            call.callee,
            allowed,
            self.entities,
            self.symbol_names,
            self.lookup,
        );
        // In an apply the creation throws first: SC6002's, and the read never runs.
        if execution == ExecutionRole::EffectApply
            || (!execution.reports_untracked_read() && !execution.reports_unplaced())
        {
            return;
        }
        let compute = created.arguments.first();
        let settled = compute.is_some_and(|argument| {
            !crate::source_discovery::source_may_be_async(self.lookup, file, argument)
                && !file
                    .ast
                    .identifiers
                    .iter()
                    .filter(|id| {
                        id.role == solid_facts::ast::IdentifierRole::Reference
                            && argument.span.contains(id.span)
                    })
                    .filter_map(|id| self.lookup.callee_symbol(file, id.span))
                    .any(|symbol| self.async_sources.contains(symbol))
        });
        let callee = location(file.path.shared(), call.callee);
        if settled
            && (self.reachable_calls.contains_key(&callee)
                || execution == ExecutionRole::EffectApply
                || enclosing_render_function(file, call.callee, self.lookup))
        {
            let name = file
                .source_text(created.callee)
                .unwrap_or("createMemo")
                .to_owned();
            let accessor = (
                crate::symbol_id(&name),
                location(file.path.shared(), created.span),
            );
            self.push_accessor_read(file, call, "", &accessor, execution, result);
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
            location: callee,
            analysis_context: crate::blockers::CREATED_ACCESSOR_READ_UNPROVEN.to_owned(),
            fixes: vec![],
            uncertain: true,
        });
    }

    /// ADR 0516: a known accessor read in a function that reaches a place the
    /// analysis does not follow (a primitive position the role index does not
    /// admit, an alias, an object literal slot) may run there: an obligation,
    /// never silence. The reachable-call index and the role proofs answer only
    /// for the calls and positions they model; "no row" is not "never runs". A
    /// class member's body (`class K { constructor() { r1(); } } new K();` in a
    /// component body) is such a place: the construction or the member call
    /// that runs it is followed by neither.
    #[allow(clippy::too_many_arguments)]
    fn unreached_read(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        symbol: &str,
        mention: &Location,
        allowed: &[solid_facts::core::Span],
        result: &mut LocalAccessResult,
        seen: &mut HashSet<(Arc<str>, u64, u64)>,
    ) {
        let Some(accessor) = self.accessors.get(symbol) else {
            return;
        };
        // ADR 0674: a named function no call edge enters, handed by name to a
        // carrier (`[1].map(h)`): a carrier that provably runs it places the
        // read, and one that may not leaves it unplaced. SC1001's zero-run
        // trade is not extended to this arm.
        // ADR 0776: a pending source is also a claim at a tracked carrier.
        let pending = self.async_sources.contains(symbol);
        let handed = (if pending {
            handed_pending_read_role
        } else {
            handed_async_read_role
        })(
            file,
            call.callee,
            allowed,
            self.entities,
            self.symbol_names,
            self.lookup,
        );
        let key = (mention.path.clone(), mention.start_byte, mention.end_byte);
        if pending
            && self.place_pending_read(file, call, symbol, accessor, handed, &key, result, seen)
        {
            return;
        }
        if crate::execution_role::named_at_tracked_position(file, call.callee, self.lookup) {
            return;
        }
        if let Some((role, Some(carrier))) = handed
            && role.reports_untracked_read()
            && seen.insert(key.clone())
        {
            self.push_accessor_read_placed(
                file,
                call,
                symbol,
                accessor,
                role,
                Some(carrier),
                result,
            );
        } else if (placed_nowhere(file, call.callee, self.lookup)
            || handed.is_some_and(|(role, _)| role == ExecutionRole::Unknown))
            && seen.insert(key)
        {
            self.push_accessor_read(file, call, symbol, accessor, ExecutionRole::Unknown, result);
        }
    }

    /// ADR 0776, ADR 0778: the row of a pending source's read in a named
    /// function, where [`handed_pending_read_role`] decides it. True when it did.
    #[allow(clippy::too_many_arguments)]
    fn place_pending_read(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        symbol: &str,
        accessor: &(SymbolId, Location),
        handed: Option<(ExecutionRole, Option<solid_facts::core::Span>)>,
        key: &(Arc<str>, u64, u64),
        result: &mut LocalAccessResult,
        seen: &mut HashSet<(Arc<str>, u64, u64)>,
    ) -> bool {
        let (execution, carrier) = match handed {
            Some((ExecutionRole::TrackedJsx, Some(carrier))) => {
                (ExecutionRole::TrackedJsx, Some(carrier))
            }
            Some((ExecutionRole::DeferredCallback, None)) => {
                (ExecutionRole::DeferredCallback, None)
            }
            Some((ExecutionRole::Unknown, _)) => {
                if seen.insert(key.clone()) {
                    self.push_accessor_read(
                        file,
                        call,
                        symbol,
                        accessor,
                        ExecutionRole::Unknown,
                        result,
                    );
                }
                return true;
            }
            _ => return false,
        };
        if seen.insert(key.clone()) {
            self.push_async_read(file, call, symbol, accessor, execution, carrier, result);
        }
        true
    }

    /// ADR 0676: the role of a read in a callback an exact setter runs on its
    /// stack: argument 0 (`setA((p) => …)`, `setSt((d) => …)`) or the key of a
    /// `reconcile` that is itself that argument. The setter's call places it:
    /// a window at the call is open in the callback. Where the write guard
    /// throws first the callback never runs. A key on a `reconcile` held
    /// anywhere else runs where nothing places it. A setter's own store is
    /// its draft in the callback, whose Gets never warn (`own`, the read's
    /// root). `None`: no setter holds the site.
    fn setter_callback_role(
        &self,
        file: &solid_facts::FileFacts,
        span: solid_facts::core::Span,
        allowed: &[solid_facts::core::Span],
        own: Option<&SymbolId>,
    ) -> Option<ExecutionRole> {
        let ast = &file.ast;
        let function = crate::owners::owning_function(ast, span)?;
        let (holder, index) = ast
            .arguments_containing(function.span)
            .find(|(call, index)| {
                ast.peel_ts_sugar_span(call.arguments[*index].span) == function.span
            })?;
        let setter_call = if index == 1
            && self.lookup.primitive_at_call(file, holder.span)
                == Some(solid_dialect::Primitive::Reconcile)
        {
            let around = ast
                .arguments_containing(holder.span)
                .find(|(call, index)| {
                    *index == 0 && ast.peel_ts_sugar_span(call.arguments[0].span) == holder.span
                })
                .map(|(call, _)| call);
            match around {
                Some(call) if self.exact_setter_row(file, call).is_some() => call,
                // `setSt((d) => { reconcile(v, key)(d); })`: applied to the
                // draft, on the setter's stack.
                _ if self.applied_to_setters_draft(file, holder) => {
                    return self.setter_callback_role(file, holder.span, allowed, None);
                }
                _ => return Some(ExecutionRole::Unknown),
            }
        } else if index == 0 && self.exact_setter_row(file, holder).is_some() {
            holder
        } else if self.lookup.primitive_at_call(file, holder.span).is_none()
            && self
                .lookup
                .standard_library_declaration(file, holder, typefacts::CallKind::Call)
                .is_none()
            && ast.arguments_containing(holder.span).any(|(call, index)| {
                index == 0
                    && ast.peel_ts_sugar_span(call.arguments[0].span) == holder.span
                    && self.exact_setter_row(file, call).is_some()
            })
        {
            // Anything else built for the setter (`setSt(storePath("a", (p) =>
            // …))`): when it runs the callback is not this code's to prove.
            // `setA(untrack(() => …))` and `setA(xs.map((x) => …))` run while
            // the argument is built.
            return Some(ExecutionRole::Unknown);
        } else {
            return None;
        };
        if let Some(root) = own {
            match self.is_setters_store(root, self.exact_setter_row(file, setter_call)?) {
                Some(true) => return Some(ExecutionRole::DeferredCallback),
                Some(false) => {}
                None => return Some(ExecutionRole::Unknown),
            }
        }
        let role = read_execution_role(
            file,
            setter_call.callee,
            allowed,
            self.entities,
            self.symbol_names,
            self.lookup,
        );
        Some(if role.reports_disallowed_write() {
            ExecutionRole::Unreported
        } else {
            role
        })
    }

    /// Whether `reconcile` is called on the spot with the first parameter of
    /// a setter's callback around it.
    fn applied_to_setters_draft(
        &self,
        file: &solid_facts::FileFacts,
        reconcile: &solid_facts::ast::CallFact,
    ) -> bool {
        let ast = &file.ast;
        let Some(function) = crate::owners::owning_function(ast, reconcile.span) else {
            return false;
        };
        let setters_callback = ast
            .arguments_containing(function.span)
            .any(|(call, index)| {
                index == 0
                    && ast.peel_ts_sugar_span(call.arguments[0].span) == function.span
                    && self.exact_setter_row(file, call).is_some()
            });
        setters_callback
            && ast.calls_with_peeled_callee(reconcile.span).any(|applied| {
                applied.arguments.first().is_some_and(|argument| {
                    !argument.spread
                        && ast
                            .reference_declaration(ast.peel_ts_sugar_span(argument.span))
                            .is_some_and(|declared| {
                                function.span.contains(declared)
                                    && !function.body.contains(declared)
                            })
                })
            })
    }

    /// The setter call [`Self::setter_callback_role`] placed `span` at: the
    /// read's timing is that call's.
    fn running_setter_call(
        &self,
        file: &solid_facts::FileFacts,
        span: solid_facts::core::Span,
    ) -> Option<solid_facts::core::Span> {
        let ast = &file.ast;
        let function = crate::owners::owning_function(ast, span)?;
        ast.arguments_containing(function.span)
            .flat_map(|(call, _)| {
                std::iter::once(call).chain(
                    ast.arguments_containing(call.span)
                        .map(|(around, _)| around),
                )
            })
            .find(|call| self.exact_setter_row(file, call).is_some())
            .map(|call| call.span)
    }

    /// Whether the store at `root` is the one the setter's tuple destructures.
    /// `None` when `root` has no source row here.
    fn is_setters_store(
        &self,
        root: &SymbolId,
        setter: &(
            SymbolId,
            Location,
            crate::OwnedWriteOption,
            ReactiveSourceKind,
        ),
    ) -> Option<bool> {
        let (_, store) = self
            .prop_sources
            .get(root)
            .or_else(|| self.accessors.get(root))?;
        let declared = &setter.1;
        if store.path != declared.path {
            return Some(false);
        }
        let file = self.lookup.file_by_path(store.path.as_ref())?;
        let holds = |binding: &solid_facts::ast::BindingFact, at: &Location| {
            binding.names.iter().any(|name| {
                u64::from(name.span.start) == at.start_byte
                    && u64::from(name.span.end) == at.end_byte
            })
        };
        Some(
            file.ast
                .bindings
                .iter()
                .any(|binding| holds(binding, store) && holds(binding, declared)),
        )
    }

    /// The row of the setter `call` calls, by the callee's exact symbol.
    fn exact_setter_row(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
    ) -> Option<&(
        SymbolId,
        Location,
        crate::OwnedWriteOption,
        ReactiveSourceKind,
    )> {
        self.setters
            .get(self.lookup.callee_symbol(file, call.callee)?)
    }

    /// The write half of "no row is not never runs" (ADRs 0715, 0721): no
    /// call edge reaches the function holding the call, yet it may run (a
    /// getter, `toString`, an iterator, a tag, a slot, a method, a
    /// constructor), so a setter or action call there is uncertain, never
    /// certified. Module initialization (outside every function, a static
    /// field or block) has no owner and is silent; a non-static field
    /// initializer runs at `new` and asks.
    fn unreached_write(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        symbol: &str,
        result: &mut LocalAccessResult,
    ) {
        if !(self.setters.contains_key(symbol) || self.actions.contains_key(symbol))
            || !unreached_write_may_run(file, call.span, self.lookup)
        {
            return;
        }
        let allowed = allowed_callback_spans(file, self.lookup);
        self.push_setter_write(file, call, symbol, &allowed, true, 1, result);
        self.push_action_invocation(file, call, symbol, &allowed, true, 1, result);
    }

    /// [`Self::discover_call`] for the call `call` invoking the function the
    /// reference `target` names: `call.callee` itself, a dispatch's `f`, a
    /// composite callee's operand (ADR 0516). `target` decides the symbol and
    /// its mention; the written call decides reachability and placement.
    fn discover_call_as(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        target: solid_facts::core::Span,
        allowed: &[solid_facts::core::Span],
        result: &mut LocalAccessResult,
        seen: &mut HashSet<(Arc<str>, u64, u64)>,
    ) {
        let callee = location(file.path.shared(), call.callee);
        let mention = location(file.path.shared(), target);
        let Some(symbol) = self.lookup.callee_symbol(file, target) else {
            return;
        };
        if inside_known_value_function_argument(file, call.callee, self.lookup) {
            return;
        }
        // ADR 0511: or a closed alias of the prop (`const d = props.k`,
        // `const { d } = { d: props.k }`).
        let props_callee = self.exact_props_callee(file, call.callee).or_else(|| {
            crate::execution_role::handed_member_alias(file, call.callee)
                .filter(|member| *member != file.ast.peel_ts_sugar_span(call.callee))
                .and_then(|member| self.exact_props_callee(file, member))
        });
        if let Some((declaration, property)) = props_callee {
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
            && self.handed_callee(file, call.callee)
        {
            let execution = read_execution_role(
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
        let alias_withheld = self.lookup.local_alias_mention_withheld(file, target);
        let execution = if alias_withheld {
            ExecutionRole::Unknown
        } else {
            read_execution_role(
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
        // ADR 0628: a function a hand-off runs, written inside an `untrack` in a
        // strict window: the read may meet the pending throw there, or run later.
        if self.async_sources.contains(symbol)
            && !alias_withheld
            && !pending_accessor_probe(file, call.callee, self.lookup)
            && crate::execution_role::untrack_layers(
                file,
                call.callee,
                None,
                self.entities,
                self.symbol_names,
                self.lookup,
            )
            .is_some_and(|layers| layers.keeps_pending_throw())
            && let Some(accessor) = self.accessors.get(symbol)
        {
            self.push_async_read(
                file,
                call,
                symbol,
                accessor,
                ExecutionRole::Unknown,
                None,
                result,
            );
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
        // ADR 0676: a setter runs this callback on its own stack.
        if self.accessors.contains_key(symbol)
            && let Some(role) = self.setter_callback_role(file, call.callee, allowed, None)
        {
            if (role.reports_untracked_read() || role.reports_unplaced())
                && seen.insert((mention.path.clone(), mention.start_byte, mention.end_byte))
                && let Some(accessor) = self.accessors.get(symbol)
            {
                self.push_accessor_read_placed(
                    file,
                    call,
                    symbol,
                    accessor,
                    role,
                    self.running_setter_call(file, call.callee),
                    result,
                );
            }
            return;
        }
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
            self.unreached_read(file, call, symbol, &mention, allowed, result, seen);
            self.unreached_write(file, call, symbol, result);
            return;
        };
        let key = (mention.path.clone(), mention.start_byte, mention.end_byte);
        // ADR 0776: a pending source is also a claim at a tracked carrier.
        if let Some(accessor) = self.accessors.get(symbol)
            && self.async_sources.contains(symbol)
            && (inside_non_component_function(file, call.callee, self.lookup)
                || nested_named_function(file, call.callee, self.lookup))
        {
            let handed = handed_pending_read_role(
                file,
                call.callee,
                allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            );
            if self.place_pending_read(file, call, symbol, accessor, handed, &key, result, seen) {
                return;
            }
        }
        // ADR 0531: a read written in a named function the program hands by
        // name to a carrier that runs it before returning (`[1].forEach(f)`,
        // `` tag`x` ``, `f.call(null)`, `Reflect.apply(f, …)`) runs at the
        // carrier's position; where that position reports untracked reads the
        // read is proven there, whatever else names the function. Asked inside
        // a primitive's callback too (ADR 0535): `inside_non_component_function`
        // answers for the callback, not for a named function nested in it.
        let handed = (self.accessors.contains_key(symbol)
            && (inside_non_component_function(file, call.callee, self.lookup)
                || nested_named_function(file, call.callee, self.lookup)))
        .then(|| {
            handed_read_role(
                file,
                call.callee,
                allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            )
        })
        .flatten();
        if let Some(accessor) = self.accessors.get(symbol)
            && let Some((role, Some(carrier))) = handed
            && role.reports_untracked_read()
            && seen.insert(key.clone())
        {
            self.push_accessor_read_placed(
                file,
                call,
                symbol,
                accessor,
                role,
                Some(carrier),
                result,
            );
        } else if let Some(accessor) = self.accessors.get(symbol)
            && (!inside_non_component_function(file, call.callee, self.lookup)
                || named_callback_execution_role(file, call.callee, self.lookup).is_some())
            // ADR 0535: in a function no named-callback role places, an
            // unplaced hand-off or an escape is an obligation below, never
            // this role. ADR 0539: so is a hand-off whose carrier is unplaced
            // in a function a named callback nests (its default role is no
            // placement).
            && !handed.is_some_and(|(role, _)| role == ExecutionRole::Unknown)
            && !(nested_named_function(file, call.callee, self.lookup)
                && handed_to_unknown_code(file, call.callee, self.lookup))
            && !escapes_nested(file, call.callee, self.lookup)
            && seen.insert(key.clone())
        {
            self.push_accessor_read(file, call, symbol, accessor, execution, result);
        } else if let Some(accessor) = self.accessors.get(symbol)
            && !crate::execution_role::named_at_tracked_position(file, call.callee, self.lookup)
            && (placed_nowhere(file, call.callee, self.lookup)
                || handed.is_some_and(|(role, _)| role == ExecutionRole::Unknown))
            && seen.insert(key.clone())
        {
            // ADR 0516: a function no named role places, handed where the
            // analysis does not follow, or a class member's body: the read is
            // placed nowhere, so it is an obligation (as at the unreached arm
            // above).
            self.push_accessor_read(file, call, symbol, accessor, ExecutionRole::Unknown, result);
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
            read_execution_role(
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
        let placed = self.props_call_placed(file, call)
            || self.props_call_pending_in_layers(file, call, &sources);
        let mut unplaced_read = false;
        // ADR 0511: as an own source's (`discover_call`), a read in a named
        // local function outside the component's own code is read where the
        // calls of that function run (`push_call_attributed_async_reads`
        // below), not at its own site.
        let read_by_callers = inside_non_component_function(file, call.callee, self.lookup)
            && named_callback_execution_role(file, call.callee, self.lookup).is_none()
            && read_in_named_local_function(file, call.callee, self.lookup);
        for source in &sources {
            if let Some(accessor) = self.accessors.get(source.as_str())
                && read_multiplicity.is_some()
            {
                if placed && read_by_callers && self.async_sources.contains(source.as_str()) {
                    continue;
                }
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
        // ADR 0511: a read in a named local function the body defines
        // (`const wrapUse = () => { props.k(); }`) is read where a call of it
        // runs, as an own source's is.
        if placed
            && !alias_withheld
            && read_in_named_local_function(file, call.callee, self.lookup)
            && named_callback_execution_role(file, call.callee, self.lookup).is_none()
            && !pending_accessor_probe(file, call.callee, self.lookup)
        {
            for source in &sources {
                if self.async_sources.contains(source.as_str())
                    && self.accessors.contains_key(source.as_str())
                {
                    self.push_call_attributed_async_reads(
                        file,
                        call,
                        source.as_str(),
                        allowed,
                        result,
                    );
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

    /// ADR 0511: a call of a prop's value deeper than
    /// [`Self::props_call_placed`] admits (an IIFE, a closure or a helper's
    /// callback inside an effect's apply) is placed by the first-run proof
    /// itself: every source a visible call site passes is an async accessor,
    /// and ADR 0504/0510's proof climbs from the call through those in-place
    /// layers to an apply whose first run meets each pending
    /// ([`crate::execution_role::pending_apply_first_read`], which walks the
    /// layers with `runs_nothing_before_in_layers`). The read then throws
    /// `PENDING_ASYNC_UNTRACKED_READ` there; anything else stays the
    /// obligation.
    fn props_call_pending_in_layers(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        sources: &[SymbolId],
    ) -> bool {
        !sources.is_empty()
            && sources.iter().all(|source| {
                self.accessors
                    .get(source.as_str())
                    .is_some_and(|(_, declaration)| {
                        let options = self.effective_async_options(source.as_str());
                        self.async_sources.contains(source.as_str())
                            && !options.declared_loading
                            && !options.opaque
                            && crate::execution_role::pending_apply_first_read(
                                file,
                                call.span,
                                declaration,
                                self.lookup,
                            )
                            .is_some()
                    })
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
        // ADR 0551: a callee proven, through `const` bindings only, to be the
        // function `createReaction` returned writes nothing, calls no action and
        // reads nothing itself: its argument runs as its own effect's compute.
        if self
            .lookup
            .returned_factory_at(file, call)
            .is_some_and(|(_, answer)| {
                answer.exact
                    && answer.slot.is_none()
                    && self
                        .lookup
                        .dialect
                        .returned_invocation_owner_requirement_role(answer.primitive)
                        .is_some()
            })
        {
            return;
        }
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
        // A read no fact places (an instance field initializer, which runs
        // at construction) is as unproven as one in a reporting role: the
        // callee cannot be selected exactly and nothing rules the window out.
        if may_write
            || execution.reports_untracked_read()
            || execution.reports_unplaced()
            || self.may_run_in_leaf_owner(file, call.callee)
            || self.tracked_read_may_suspend(file, call, execution)
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
        } else if self.tracked_read_may_hold_open_source(file, call, execution) {
            // ADR 0802: decided once every table is built.
            result.dispatch_obligations.push(crate::StaticDefect {
                kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                    callee: file
                        .source_text(call.callee)
                        .unwrap_or("this callee")
                        .to_owned(),
                    member: None,
                },
                location: location(file.path.shared(), call.callee),
                analysis_context: HELD_OPEN_SOURCE.to_owned(),
                fixes: vec![],
                uncertain: true,
            });
        }
    }

    /// ADR 0775: a read is legal at a tracked position, and there SC5003 asks
    /// about a pending source: whether this call may reach one, past no
    /// boundary. A value that reaches only settled sources does not (`g1c`).
    fn tracked_read_may_suspend(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        execution: ExecutionRole,
    ) -> bool {
        async_execution_role(file, call.callee, execution) == ExecutionRole::TrackedJsx
            && read_loading_cover(self.lookup, file, call.span, self.symbol_names)
                != crate::owners::LoadingCover::Covered
            && self.pending_value(file, call.callee)
    }

    /// ADR 0802: a tracked call that holds a settled-looking accessor may still
    /// hold one whose compute is open, and only the built tables can say so.
    fn tracked_read_may_hold_open_source(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        execution: ExecutionRole,
    ) -> bool {
        async_execution_role(file, call.callee, execution) == ExecutionRole::TrackedJsx
            && read_loading_cover(self.lookup, file, call.span, self.symbol_names)
                != crate::owners::LoadingCover::Covered
            && self.handed_callee(file, call.callee)
    }

    /// [`handed_value`] with an accessor counted only if it may be pending.
    fn pending_value(&self, file: &solid_facts::FileFacts, span: solid_facts::core::Span) -> bool {
        handed_value(
            &self.handed_tables(Some(&|symbol| self.async_sources.contains(symbol))),
            file,
            span,
            HANDED_DEPTH - 1,
        )
    }

    /// Whether `span` is inside a leaf owner's callback (`onSettled`,
    /// `createTrackedEffect`), where a pending async read is forbidden, or in
    /// a named local function a reference inside one names. ADR 0800: each
    /// reference is visited once, and a walk out of budget may.
    fn may_run_in_leaf_owner(
        &self,
        file: &solid_facts::FileFacts,
        span: solid_facts::core::Span,
    ) -> bool {
        may_run_in_leaf_owner(file, span, self.entities, self.symbol_names, self.lookup)
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
        self.handed_callee(file, callee)
    }

    /// ADR 0770: [`Self::handed_member_callee`] at any member depth, for a
    /// call whose standard-library method is already ruled out: what
    /// `props.o.go()` runs is as unknown as what `props.go()` does.
    fn handed_callee(
        &self,
        file: &solid_facts::FileFacts,
        callee: solid_facts::core::Span,
    ) -> bool {
        handed_value(&self.handed_tables(None), file, callee, HANDED_DEPTH)
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
        multiplicity: u64,
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
            write_execution = handed_write_execution(
                file,
                call.callee,
                allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
                root_body,
                write_execution,
            );
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
            // ADR 0636: on rc.21 an optimistic setter of an async source throws
            // `OPTIMISTIC_WRITE_BEFORE_LOAD` before its first answer, ahead of
            // any owned-scope guard: SC2009, not SC2001. Where a write meets
            // the first answer is proven in the pass that creates the source,
            // and open anywhere else: a handler may run before it lands.
            if self.optimistic_async_setter(symbol) {
                self.push_optimistic_write(file, call, name, declaration, result);
                return;
            }
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
            // One record standing for every reachable path.
            if multiplicity > 0 {
                let record = Arc::new(ReactiveWrite {
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
                    multiplicity,
                });
                result.writes.push(record);
            }
        }
    }

    /// The action invocation a call of the action whose row is `symbol`'s
    /// makes ([`Self::push_setter_write`]).
    /// ADR 0636: `symbol` is, or aliases, the setter of an optimistic creation
    /// that may not have answered. An alias holds the creation's declaration.
    fn optimistic_async_setter(&self, symbol: &str) -> bool {
        let optimistic = |symbol: &str| {
            self.async_sources.contains(symbol)
                && matches!(
                    self.source_primitives
                        .get(symbol)
                        .and_then(|primitive| self.lookup.dialect.primitive(primitive)),
                    Some(
                        solid_dialect::Primitive::CreateOptimistic
                            | solid_dialect::Primitive::CreateOptimisticStore
                    )
                )
        };
        self.lookup.dialect.refuses_optimistic_write_before_load()
            && self.setters.get(symbol).is_some_and(|row| {
                optimistic(symbol)
                    || self.setters.iter().any(|(other, candidate)| {
                        candidate.1 == row.1 && optimistic(other.as_str())
                    })
            })
    }

    /// ADR 0636: a call nothing placed that may run such a setter (a binding
    /// no alias resolves, a function that writes it, a hand-off the walk does
    /// not reach): an obligation, in any role, since a click may land first.
    fn discover_optimistic_reach(
        &self,
        file: &solid_facts::FileFacts,
        result: &mut LocalAccessResult,
    ) {
        use solid_dialect::Primitive;
        if !self
            .lookup
            .project_names_primitive(Primitive::CreateOptimistic)
            && !self
                .lookup
                .project_names_primitive(Primitive::CreateOptimisticStore)
        {
            return;
        }
        // ADR 0752: with no async optimistic source in the program, no setter
        // is [`Self::optimistic_async_setter`] and no call reaches one (a
        // reach walk past its budget is otherwise undecided).
        if !self.source_primitives.iter().any(|(symbol, primitive)| {
            self.async_sources.contains(symbol.as_str())
                && matches!(
                    self.lookup.dialect.primitive(primitive),
                    Some(Primitive::CreateOptimistic | Primitive::CreateOptimisticStore)
                )
        }) {
            return;
        }
        for call in &file.ast.calls {
            let here = location(file.path.shared(), call.span);
            if result
                .static_violations
                .iter()
                .any(|violation| violation.location == here && violation.id == "SC2009")
            {
                continue;
            }
            // A setter no row placed (a call at module scope has no reachable
            // row) is its own proof.
            if let Some(symbol) = self.lookup.callee_symbol(file, call.callee)
                && self.optimistic_async_setter(symbol)
                && let Some((name, declaration, ..)) = self.setters.get(symbol)
            {
                self.push_optimistic_write(file, call, name, declaration, result);
                continue;
            }
            if !self.callee_reaches(file, call.callee, &|symbol| {
                self.optimistic_async_setter(symbol.as_str())
            }) {
                continue;
            }
            result.static_violations.push(crate::StaticViolation {
                id: "SC2009".into(),
                rule: "optimistic-write-before-load".into(),
                message: "this call may run an optimistic setter before its async source's first \
                          answer; Solid throws OPTIMISTIC_WRITE_BEFORE_LOAD"
                    .into(),
                hint: "Keep pending items in a separate list until the source loads, or declare a \
                       loadingValue."
                    .into(),
                location: here,
                analysis_context: String::new(),
                fixes: vec![],
                uncertain: true,
            });
        }
        // ADR 0712: a component's prop (or spread) handed such a setter as a
        // value: the component's `props.k(…)` is a call no row places (prop
        // value flow is not followed), so the hand-off is the obligation.
        // An intrinsic element's handler is placed by the handler pass.
        for element in &file.ast.jsx_elements {
            if crate::execution_role::intrinsic_element(file, element) {
                continue;
            }
            let values = element
                .attributes
                .iter()
                .filter(|attribute| attribute.namespace.is_none())
                .filter_map(|attribute| attribute.expression)
                .chain(element.spreads.iter().map(|spread| spread.argument));
            // The values the prop may hold: a branch of `?:`, `&&`, `||`,
            // `??`, then a name, a member or a call. A function written here
            // is walked where it calls, and any other form (`!x`, a literal,
            // a template) holds no function.
            let mut leaves = Vec::new();
            let mut pending = values.collect::<Vec<_>>();
            while let Some(value) = pending.pop() {
                let value = file.ast.peel_ts_sugar_span(value);
                if let Some(conditional) = file.ast.conditional_at(value) {
                    pending.extend([conditional.consequent, conditional.alternate]);
                } else if let Some(logical) = file.ast.logical_at(value) {
                    pending.extend([logical.left, logical.right]);
                } else if file.ast.member_at(value).is_some()
                    || file.ast.call_at(value).is_some()
                    || file.ast.reference_declaration(value).is_some()
                {
                    leaves.push(value);
                }
            }
            for value in leaves {
                let here = location(file.path.shared(), value);
                if result
                    .static_violations
                    .iter()
                    .any(|violation| violation.location == here && violation.id == "SC2009")
                    || !self.callee_reaches(file, value, &|symbol| {
                        self.optimistic_async_setter(symbol.as_str())
                    })
                {
                    continue;
                }
                result.static_violations.push(crate::StaticViolation {
                    id: "SC2009".into(),
                    rule: "optimistic-write-before-load".into(),
                    message: "this prop hands a component an optimistic setter; whether the \
                              component runs it before its async source's first answer is not \
                              proven, and Solid throws OPTIMISTIC_WRITE_BEFORE_LOAD if it does"
                        .into(),
                    hint: "Keep pending items in a separate list until the source loads, or \
                           declare a loadingValue."
                        .into(),
                    location: here,
                    analysis_context: crate::blockers::OPTIMISTIC_REACH_UNRESOLVED.into(),
                    fixes: vec![],
                    uncertain: true,
                });
            }
        }
    }

    /// ADR 0640: the reactive source `value` names through forms that hold one
    /// value: an unwritten `let` or `var`, a pattern or member of a fresh object
    /// literal (a spread of one, a quoted computed key), or the slot of a local
    /// hook's returned literal. Anything else answers nothing.
    fn closed_source(
        &self,
        file: &solid_facts::FileFacts,
        value: solid_facts::core::Span,
        depth: usize,
    ) -> Option<SymbolId> {
        let depth = depth.checked_sub(1)?;
        let ast = &file.ast;
        let value = ast.peel_ts_sugar_span(value);
        // A name in a literal has no demanded symbol: its declaration has one.
        if let Some(symbol) = self.entities.value_symbol(file, value).or_else(|| {
            ast.reference_declaration(value)
                .and_then(|declared| self.entities.at(file.path.as_str(), declared))
        }) && self.accessors.contains_key(symbol)
        {
            return Some(symbol.clone());
        }
        let mut path = Vec::new();
        let mut head = value;
        while let Some(member) = ast.member_at(head) {
            if ast.computed_members.contains(&member.span)
                || ast.optional_members.binary_search(&member.span).is_ok()
            {
                return None;
            }
            path.push(file.source_text(member.property)?.to_owned());
            head = ast.peel_ts_sugar_span(member.object);
        }
        let declared = ast.reference_declaration(head)?;
        let binding = ast
            .bindings
            .iter()
            .find(|binding| binding.names.iter().any(|name| name.span == declared))?;
        if crate::indexes::binding_written(file, declared) {
            return None;
        }
        if binding.shape == solid_facts::ast::BindingShape::Object {
            let slot = binding
                .object_slots
                .iter()
                .find(|slot| slot.local.span == declared)?;
            path.push(slot.property.to_string());
        } else if binding.shape != solid_facts::ast::BindingShape::Identifier {
            return None;
        }
        path.reverse();
        let initializer = ast.peel_ts_sugar_span(binding.initializer?);
        if let Some(call) = ast.call_at(initializer) {
            let (hook_file, hook, _) =
                crate::execution_role::project_callee_at_parameter(file, call, 0, self.lookup)?;
            let [Some(literal)] = returned_values(hook_file, hook)[..] else {
                return None;
            };
            return self.literal_path_source(hook_file, literal, &path, depth);
        }
        self.literal_path_source(file, initializer, &path, depth)
    }

    /// [`Self::closed_source`] at `path` inside the fresh literal `literal`.
    fn literal_path_source(
        &self,
        file: &solid_facts::FileFacts,
        literal: solid_facts::core::Span,
        path: &[String],
        depth: usize,
    ) -> Option<SymbolId> {
        let Some((key, rest)) = path.split_first() else {
            return self.closed_source(file, literal, depth);
        };
        let value = literal_slot(file, file.ast.peel_ts_sugar_span(literal), key)?;
        self.literal_path_source(file, value, rest, depth)
    }

    /// What the callee `value` runs may hit: the slot of a hook's closed result
    /// it takes, not all the hook's text, and no argument of a call in the chain
    /// (`expect(a).toBe` runs nothing of `a`).
    fn callee_reaches(
        &self,
        file: &solid_facts::FileFacts,
        value: solid_facts::core::Span,
        hit: &dyn Fn(&SymbolId) -> bool,
    ) -> bool {
        let ast = &file.ast;
        let mut head = ast.peel_ts_sugar_span(value);
        let mut key = None;
        loop {
            if let Some(member) = ast.member_at(head) {
                key = (!ast.computed_members.contains(&member.span))
                    .then(|| file.source_text(member.property))
                    .flatten();
                head = ast.peel_ts_sugar_span(member.object);
            } else if let Some(call) = ast.call_at(head) {
                key = None;
                head = ast.peel_ts_sugar_span(call.callee);
            } else {
                break;
            }
        }
        let declared = ast.reference_declaration(head);
        let holder = declared.and_then(|declared| {
            ast.bindings
                .iter()
                .find(|binding| binding.names.iter().any(|name| name.span == declared))
        });
        // The slot taken of a hook's result: `{ a } =`, `h.a`, or `[, a] =`.
        let slot = holder.and_then(|holder| match holder.shape {
            solid_facts::ast::BindingShape::Object => holder
                .object_slots
                .iter()
                .find(|slot| Some(slot.local.span) == declared && slot.plain_identifier_key)
                .map(|slot| (Some(slot.property.as_ref()), None)),
            solid_facts::ast::BindingShape::Array => holder
                .array_slots
                .iter()
                .position(|slot| slot.as_ref().map(|name| name.span) == declared)
                .map(|index| (None, Some(index))),
            _ => key.map(|key| (Some(key), None)),
        });
        if let Some((slot_key, slot_index)) = slot
            && let Some(call) = holder
                .and_then(|holder| holder.call_initializer)
                .and_then(|initializer| ast.call_at(initializer))
            && let Some((hook_file, hook, _)) =
                crate::execution_role::project_callee_at_parameter(file, call, 0, self.lookup)
        {
            let values = returned_values(hook_file, hook)
                .into_iter()
                .map(|returned| {
                    let literal = hook_file.ast.peel_ts_sugar_span(returned?);
                    match (slot_key, slot_index) {
                        (Some(key), _) => crate::execution_role::closed_literal_slot_value(
                            hook_file, literal, key,
                        ),
                        (_, Some(index)) => {
                            let elements = &hook_file.ast.array_literal_at(literal)?.elements;
                            let value = elements.get(index)?;
                            (!elements.iter().take(index + 1).any(|element| {
                                element.kind == solid_facts::ast::ArrayElementKind::Spread
                            }) && value.kind == solid_facts::ast::ArrayElementKind::Value)
                                .then_some(value.span)
                        }
                        _ => None,
                    }
                })
                .collect::<Option<Vec<_>>>();
            if let Some(values) = values.filter(|values| !values.is_empty()) {
                return values
                    .into_iter()
                    .any(|value| self.calls_reach(hook_file, value, hit));
            }
        }
        self.calls_reach(file, head, hit)
    }

    /// What running `value` may call: the value itself, a function's callees and
    /// the identifiers it hands on, what a hook returns, what a `let` is
    /// assigned. A name a function only mentions is no call. Each node is walked
    /// once; at an import nothing here follows, the answer is
    /// [`Self::value_reaches`]'s, the mention walk. ADR 0752: a walk that ran
    /// past the budget did not decide, so it may reach.
    fn calls_reach(
        &self,
        file: &solid_facts::FileFacts,
        value: solid_facts::core::Span,
        hit: &dyn Fn(&SymbolId) -> bool,
    ) -> bool {
        let mut seen = HashSet::default();
        self.calls_reach_within(file, value, hit, &mut seen) || seen.len() > CALL_WALK_BUDGET
    }

    fn function_calls_reach(
        &self,
        file: &solid_facts::FileFacts,
        function: &solid_facts::ast::FunctionFact,
        hit: &dyn Fn(&SymbolId) -> bool,
        seen: &mut HashSet<(String, solid_facts::core::Span)>,
    ) -> bool {
        let ast = &file.ast;
        ast.calls
            .iter()
            .filter(|call| function.span.contains(call.span))
            .any(|call| {
                let callee =
                    crate::execution_role::project_callee_at_parameter(file, call, 0, self.lookup);
                let runs = match callee {
                    Some((callee_file, callee, _)) => {
                        seen.insert((callee_file.path.to_string(), callee.span))
                            && self.function_calls_reach(callee_file, callee, hit, seen)
                    }
                    None => self.calls_reach_within(file, call.callee, hit, seen),
                };
                runs || call.arguments.iter().any(|argument| {
                    argument.value == solid_facts::ast::ArgumentValueKind::Identifier
                        && self.calls_reach_within(file, argument.span, hit, seen)
                })
            })
    }

    fn calls_reach_within(
        &self,
        file: &solid_facts::FileFacts,
        value: solid_facts::core::Span,
        hit: &dyn Fn(&SymbolId) -> bool,
        seen: &mut HashSet<(String, solid_facts::core::Span)>,
    ) -> bool {
        if seen.len() > CALL_WALK_BUDGET {
            return false;
        }
        let ast = &file.ast;
        let value = ast.peel_ts_sugar_span(value);
        if !seen.insert((file.path.to_string(), value)) {
            return false;
        }
        if let Some(function) = ast.function_at(value) {
            return self.function_calls_reach(file, function, hit, seen);
        }
        if let Some(call) = ast.call_at(value) {
            if let Some((hook_file, hook, _)) =
                crate::execution_role::project_callee_at_parameter(file, call, 0, self.lookup)
            {
                return returned_values(hook_file, hook).iter().any(|returned| {
                    returned.is_some_and(|returned| {
                        self.calls_reach_within(hook_file, returned, hit, seen)
                    })
                });
            }
            return self.calls_reach_within(file, call.callee, hit, seen);
        }
        let mut head = value;
        while let Some(member) = ast.member_at(head) {
            head = ast.peel_ts_sugar_span(member.object);
        }
        if head != value && ast.call_at(head).is_some() {
            return self.calls_reach_within(file, head, hit, seen);
        }
        if !ast.identifiers.iter().any(|name| name.span == head) {
            return self.value_reaches(file, value, hit).may();
        }
        if crate::source_discovery::reference_symbol(file, head, self.entities, self.lookup)
            .is_some_and(|symbol| hit(&symbol))
        {
            return true;
        }
        let binding = ast.reference_declaration(head).and_then(|declaration| {
            ast.bindings
                .iter()
                .find(|binding| binding.names.iter().any(|name| name.span == declaration))
                .map(|binding| (declaration, binding))
        });
        let named = ast.reference_declaration(head).and_then(|declaration| {
            ast.functions.iter().find(|function| {
                function
                    .name
                    .as_ref()
                    .is_some_and(|name| name.span == declaration)
            })
        });
        if let Some(function) = named {
            return self.function_calls_reach(file, function, hit, seen);
        }
        // An import, a parameter: nothing here follows it.
        let Some((declaration, binding)) = binding else {
            return self.value_reaches(file, value, hit).may();
        };
        ast.assignments.iter().any(|assignment| {
            ast.identifiers.iter().any(|name| {
                assignment.target.contains(name.span)
                    && ast.reference_declaration(name.span) == Some(declaration)
            }) && self.calls_reach_within(file, assignment.value_span, hit, seen)
        }) || binding
            .initializer
            .is_some_and(|initializer| self.calls_reach_within(file, initializer, hit, seen))
    }

    /// ADR 0636: a write to an optimistic setter of an async source.
    fn push_optimistic_write(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        name: &SymbolId,
        declaration: &Location,
        result: &mut LocalAccessResult,
    ) {
        result.static_violations.push(crate::StaticViolation {
            id: "SC2009".into(),
            rule: "optimistic-write-before-load".into(),
            message: format!(
                "optimistic setter {name} is written before its async source's first answer; \
                 Solid throws OPTIMISTIC_WRITE_BEFORE_LOAD"
            ),
            hint: "Keep pending items in a separate list until the source loads, disable the \
                   action until it has, or declare a loadingValue."
                .into(),
            location: location(file.path.shared(), call.span),
            analysis_context: String::new(),
            fixes: vec![],
            uncertain: !self.written_in_creating_pass(file, call, declaration),
        });
    }

    /// ADR 0636: rc.21 throws whatever the role, on the first answer alone, so
    /// the proof is temporal: the call of the setter runs on one stack with a
    /// `const` creation with a literal `async` compute, entered after it, past
    /// no await. A `let` or `var` may not hold it there.
    fn written_in_creating_pass(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        declaration: &Location,
    ) -> bool {
        let (Ok(start), Ok(end)) = (
            u32::try_from(declaration.start_byte),
            u32::try_from(declaration.end_byte),
        ) else {
            return false;
        };
        if declaration.path.as_ref() != file.path.as_str() {
            return false;
        }
        let declared = solid_facts::core::Span::new(start, end);
        let creating = containing_ast_function(&file.ast, declared);
        let Some(binding) = file
            .ast
            .bindings
            .iter()
            .find(|binding| binding.names.iter().any(|name| name.span == declared))
        else {
            return false;
        };
        let literal_async = binding
            .call_initializer
            .and_then(|initializer| file.ast.call_at(initializer))
            .and_then(|created| created.arguments.first())
            .and_then(|compute| {
                file.ast
                    .function_at(file.ast.peel_ts_sugar_span(compute.span))
            })
            .is_some_and(|compute| compute.r#async);
        let Some(entry) =
            crate::execution_role::entry_on_stack_of(file, call.span, creating, self.lookup)
        else {
            return false;
        };
        binding.immutable
            && literal_async
            && declared.end <= entry.start
            && !creating.is_some_and(|function| function.r#async)
            && !crate::execution_role::may_follow_suspension(file, call.span)
            && !file.ast.awaits.iter().any(|awaited| {
                awaited.end <= entry.start && containing_ast_function(&file.ast, *awaited).is_none()
            })
    }

    /// ADR 0637: a reactive read in a `try` with a `catch`, in a computation. rc.21
    /// reports `CAUGHT_REACTIVE_READ` when the read's throw lands in the pass and
    /// the body completes. Proven for an async source the same body created,
    /// with a literal async compute, read first in the block and never rethrown:
    /// the compute runs where it is created, while the source is pending. A memo
    /// may throw its error, and anything else may be pending: open.
    fn discover_caught_reads(
        &self,
        file: &solid_facts::FileFacts,
        allowed: &[solid_facts::core::Span],
        result: &mut LocalAccessResult,
    ) {
        for caught in &file.ast.try_catches {
            let Some(function) = containing_ast_function(&file.ast, caught.block) else {
                continue;
            };
            let role = read_execution_role(
                file,
                caught.block,
                allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            );
            // A pass the throw can land in, or may: a body's read throws its own
            // code, and a handler or an apply has no pass.
            // A callback a component receives runs where that component calls it.
            let handed_to_component = file
                .ast
                .jsx_containing(function.span)
                .any(|element| crate::owners::is_component_element(file, element));
            if !handed_to_component
                && matches!(
                    role,
                    ExecutionRole::UntrackedRendering
                        | ExecutionRole::ModuleInitialization
                        | ExecutionRole::EventCallback
                        | ExecutionRole::EffectApply
                )
            {
                continue;
            }
            // A handler that is one `throw` leaves the pass on every path, and
            // rc.21 only checks a pass that returned.
            if handler_only_throws(file, caught.handler) {
                continue;
            }
            let rethrows = file
                .ast
                .throw_statements
                .iter()
                .any(|thrown| caught.handler.contains(*thrown));
            // The reads in the block: the sites, and whether the form is one the
            // proof reads (an accessor's call, a Get or a probe of an async store).
            let mut sites = Vec::new();
            for call in file
                .ast
                .calls
                .iter()
                .filter(|call| caught.block.contains(call.span))
            {
                let symbol = self.lookup.callee_symbol(file, call.callee);
                // A setter performs no read: what a function argument reads is
                // a call of its own in this block (ADR 0640).
                if symbol.is_some_and(|symbol| self.setters.contains_key(symbol)) {
                    continue;
                }
                let closed = symbol
                    .is_none_or(|symbol| !self.accessors.contains_key(symbol))
                    .then(|| self.closed_source(file, call.callee, 6))
                    .flatten();
                let source = symbol
                    .and_then(|symbol| self.accessors.get_key_value(symbol))
                    .or_else(|| closed.and_then(|symbol| self.accessors.get_key_value(&symbol)));
                if source.is_some_and(|(symbol, _)| self.caught_read_silent(symbol))
                    || (source.is_none()
                        && crate::execution_role::hook_result_root(file, call.callee, self.lookup)
                            .is_none_or(|hook| {
                                crate::execution_role::project_callee_at_parameter(
                                    file,
                                    hook,
                                    0,
                                    self.lookup,
                                )
                                .is_none()
                                    || !self.span_may_make_computation(file, hook.span, 4)
                            })
                        && !self.value_reaches_async_source(file, call.callee).may()
                        // ADR 0658: a value a caller handed over may read.
                        && !self.handed_member_callee(file, call.callee))
                {
                    continue;
                }
                sites.push((
                    call.span,
                    call.callee,
                    source.map(|(symbol, _)| symbol.clone()),
                ));
            }
            for identifier in file.ast.identifiers.iter().filter(|identifier| {
                caught.block.contains(identifier.span)
                    && file.ast.reference_declaration(identifier.span).is_some()
            }) {
                let Some(symbol) = crate::source_discovery::reference_symbol(
                    file,
                    identifier.span,
                    self.entities,
                    self.lookup,
                ) else {
                    continue;
                };
                if self.async_sources.contains(&symbol)
                    && self.accessors.contains_key(&symbol)
                    && !self.caught_read_silent(&symbol)
                    && !sites
                        .iter()
                        .any(|(site, _, _)| site.contains(identifier.span))
                {
                    sites.push((identifier.span, identifier.span, Some(symbol)));
                }
            }
            // ADR 0658: a Get runs code a call list never shows: a prop's getter
            // reads, and so may a handed root's member or a literal's accessor.
            for member in &file.ast.members {
                if !caught.block.contains(member.span)
                    || sites.iter().any(|(site, _, _)| site.contains(member.span))
                    || file
                        .ast
                        .assignments
                        .iter()
                        .any(|assignment| assignment.target.contains(member.span))
                    || file
                        .ast
                        .functions_containing(member.span)
                        .any(|nested| caught.block.contains(nested.span))
                {
                    continue;
                }
                if self.member_get_may_read(file, member) {
                    sites.push((member.span, member.span, None));
                }
            }
            for (site, read, source) in sites {
                let proven = !rethrows
                    && source.as_ref().is_some_and(|symbol| {
                        self.caught_read_proven(file, read, symbol, &file.ast.try_catches)
                    });
                let name = source
                    .as_ref()
                    .and_then(|symbol| self.accessors.get(symbol))
                    .map_or_else(
                        || {
                            file.source_text(read)
                                .unwrap_or("a reactive value")
                                .to_owned()
                        },
                        |(name, _)| name.to_string(),
                    );
                result.static_violations.push(crate::StaticViolation {
                    id: "SC5008".into(),
                    rule: "caught-reactive-read".into(),
                    message: format!(
                        "this computation catches the throw of {name}; a reactive read is not an \
                         await: the catch swallows its suspension or error, and when the source \
                         changes later the computation is marked pending and not re-run"
                    ),
                    hint: "Branch on readiness with isPending(), show ahead with latest(), or let \
                           the read suspend; handle errors with <Errored>."
                        .into(),
                    location: location(file.path.shared(), site),
                    analysis_context: if proven {
                        String::new()
                    } else {
                        crate::blockers::CAUGHT_READ_UNPROVEN.to_owned()
                    },
                    fixes: vec![],
                    uncertain: !proven,
                });
            }
        }
    }

    /// ADR 0658: whether a member Get may run a read no call shows: a member of
    /// a value a caller handed over (a prop is a getter), or of a local literal
    /// that declares an accessor.
    pub(crate) fn member_get_may_read(
        &self,
        file: &solid_facts::FileFacts,
        member: &solid_facts::ast::MemberFact,
    ) -> bool {
        self.handed_member_callee(file, member.span) || literal_member_has_getter(file, member)
    }

    /// A read that cannot throw into a pass: a plain signal, a source served
    /// its declared value.
    fn caught_read_silent(&self, symbol: &SymbolId) -> bool {
        (!self.async_sources.contains(symbol) && self.lookup_primitive_is_signal(symbol))
            || self.effective_async_options(symbol).declared_loading
    }

    /// ADR 0637: the pass of a compute that runs where it is written holds this
    /// read of a pending source, on its stack, and every other read that might
    /// throw sits in a `try` that keeps it: an uncaught throw leaves the pass,
    /// which then never completes with a value.
    fn caught_read_proven(
        &self,
        file: &solid_facts::FileFacts,
        read: solid_facts::core::Span,
        symbol: &SymbolId,
        tries: &[solid_facts::ast::TryCatchFact],
    ) -> bool {
        let keeps = |span: solid_facts::core::Span| {
            tries.iter().any(|caught| {
                caught.block.contains(span)
                    && !file
                        .ast
                        .throw_statements
                        .iter()
                        .any(|thrown| caught.handler.contains(*thrown))
            })
        };
        let Some((_, declaration)) = self.accessors.get(symbol) else {
            return false;
        };
        let options = self.effective_async_options(symbol);
        !options.declared_loading
            && !options.opaque
            && file.ast.functions_owning(read).any(|compute| {
                !compute.r#async
                    && self.computation_runs_where_written(file, compute)
                    && crate::execution_role::runs_on_stack_of(file, read, compute, self.lookup)
                    && self.created_by_async_literal_around(file, compute, declaration)
                    && file.ast.identifiers.iter().all(|identifier| {
                        !compute.body.contains(identifier.span)
                            || keeps(identifier.span)
                            || file.ast.reference_declaration(identifier.span).is_none()
                            || crate::source_discovery::reference_symbol(
                                file,
                                identifier.span,
                                self.entities,
                                self.lookup,
                            )
                            .is_none_or(|other| {
                                !self.accessors.contains_key(&other)
                                    || self.caught_read_silent(&other)
                            })
                    })
                    && file.ast.calls.iter().all(|call| {
                        !compute.body.contains(call.span)
                            || keeps(call.span)
                            || !(crate::execution_role::hook_result_root(
                                file,
                                call.callee,
                                self.lookup,
                            )
                            .is_some()
                                || (self
                                    .lookup
                                    .callee_symbol(file, call.callee)
                                    .is_none_or(|symbol| !self.accessors.contains_key(symbol))
                                    && self.value_reaches_async_source(file, call.callee).may()))
                    })
            })
    }

    /// `function` is the compute of a memo or an effect with no options, so it
    /// runs where it is written: a `lazy` memo waits for its first read.
    fn computation_runs_where_written(
        &self,
        file: &solid_facts::FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> bool {
        use solid_dialect::Primitive;
        file.ast
            .arguments_containing(function.span)
            .any(|(call, index)| {
                index == 0
                    && file.ast.peel_ts_sugar_span(call.arguments[0].span) == function.span
                    && match self.lookup.primitive_at_call(file, call.span) {
                        Some(Primitive::CreateMemo) => call.arguments.len() == 1,
                        Some(Primitive::CreateEffect | Primitive::CreateRenderEffect) => true,
                        _ => false,
                    }
            })
    }

    fn lookup_primitive_is_signal(&self, symbol: &SymbolId) -> bool {
        self.source_primitives
            .get(symbol)
            .and_then(|primitive| self.lookup.dialect.primitive(primitive))
            == Some(solid_dialect::Primitive::CreateSignal)
    }

    /// The source declared at `declaration` is a binding of a literal `async`
    /// compute written in the body holding the computation `function`, before it.
    fn created_by_async_literal_around(
        &self,
        file: &solid_facts::FileFacts,
        function: &solid_facts::ast::FunctionFact,
        declaration: &Location,
    ) -> bool {
        let (Ok(start), Ok(end)) = (
            u32::try_from(declaration.start_byte),
            u32::try_from(declaration.end_byte),
        ) else {
            return false;
        };
        let declared = solid_facts::core::Span::new(start, end);
        declaration.path.as_ref() == file.path.as_str()
            && declared.end <= function.span.start
            && containing_ast_function(&file.ast, declared).is_some_and(|owner| {
                owner.body.contains(function.span)
                    && containing_ast_function(&file.ast, function.span)
                        .is_some_and(|outer| outer.span == owner.span)
            })
            && file
                .ast
                .bindings
                .iter()
                .find(|binding| binding.names.iter().any(|name| name.span == declared))
                .and_then(|binding| binding.call_initializer)
                .and_then(|initializer| file.ast.call_at(initializer))
                .and_then(|created| created.arguments.first())
                .and_then(|compute| {
                    file.ast
                        .function_at(file.ast.peel_ts_sugar_span(compute.span))
                })
                .is_some_and(|compute| compute.r#async)
    }

    #[allow(clippy::too_many_arguments)]
    fn push_action_invocation(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        symbol: &str,
        allowed: &[solid_facts::core::Span],
        alias_withheld: bool,
        multiplicity: u64,
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
                let role =
                    crate::role_log::decide("action", file.path.as_str(), call.callee, || {
                        semantic_write_execution_role(
                            file,
                            call.callee,
                            allowed,
                            self.entities,
                            self.symbol_names,
                            self.lookup,
                            RootBodyGuard::Rejects,
                        )
                    });
                handed_write_execution(
                    file,
                    call.callee,
                    allowed,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                    RootBodyGuard::Rejects,
                    role,
                )
            };
            // One record standing for every reachable path.
            if multiplicity > 0 {
                let record = Arc::new(ActionInvocation {
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
                    multiplicity,
                });
                result.action_invocations.push(record);
            }
        }
    }

    /// F0: the obligation of each call that runs a transported array's members
    /// whose summaries are not complete, where a run there could report.
    pub(crate) fn open_array_member_obligations(
        &self,
        callees: &[(&solid_facts::FileFacts, &Location)],
    ) -> Vec<crate::StaticDefect> {
        let mut result = LocalAccessResult::default();
        for (file, callee) in callees {
            let Some(call) = file
                .ast
                .calls
                .iter()
                .find(|call| location(file.path.shared(), call.callee) == **callee)
            else {
                continue;
            };
            let allowed = allowed_callback_spans(file, self.lookup);
            let execution = read_execution_role(
                file,
                call.callee,
                &allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            );
            self.push_owned_dispatch_obligation(
                file,
                call,
                &allowed,
                execution,
                UNRESOLVED_HANDED_MEMBER,
                &mut result,
            );
        }
        result.dispatch_obligations
    }

    /// ADR 0771: whether an argument of the call at `call`, at a tracked
    /// position, holds what a helper may write with, act on or suspend on
    /// through its parameter: a setter, an action, an async source, or a value
    /// a caller supplied.
    pub(crate) fn tracked_call_hands_effect(
        &self,
        file: &solid_facts::FileFacts,
        call: solid_facts::core::Span,
        open: &OpenComputes<'_>,
    ) -> bool {
        let Some(call) = file.ast.call_at(call) else {
            return true;
        };
        // One step down: at the full depth `handed_value` leaves a bare
        // source name to the row its own call has.
        // ADR 0775: a withheld literal member's callee holds it too.
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        let holds = |value| self.value_holds_open_source(file, value, open);
        call.arguments.iter().any(|argument| holds(argument.span))
            || (file.ast.member_at(callee).is_some() && holds(callee))
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
                // ADR 0803: a withdrawn slot's read is held here too.
                Callable::Accessor(symbol)
                    if self.async_sources.contains(symbol.as_str())
                        || self.withdrawn_array_slots.contains(symbol.as_str()) =>
                {
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
            // ADR 0803: transport carried a slot pass 0 withdrew (a hook's
            // object, a member, a destructure, a default export): what the
            // call reaches is not the primitive's own, so the call is held.
            if self.withdrawn_array_slots.contains(symbol.as_str()) {
                result
                    .dispatch_obligations
                    .push(withdrawn_slot_obligation(file, call.callee));
                continue;
            }
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
                // ADR 0537: as `discover_call` (ADR 0531/0535), a read in a
                // named function handed to a carrier is placed there, and an
                // unplaced hand-off, an escape or a member's function value
                // places it nowhere: the lexical role of the callback around
                // the function (a control-flow child's) is no proof for it.
                let handed = (inside_non_component_function(file, call.callee, self.lookup)
                    || nested_named_function(file, call.callee, self.lookup))
                .then(|| {
                    handed_pending_read_role(
                        file,
                        call.callee,
                        &allowed,
                        self.entities,
                        self.symbol_names,
                        self.lookup,
                    )
                })
                .flatten();
                if let Some((role, Some(carrier))) = handed
                    && (role.reports_untracked_read() || role == ExecutionRole::TrackedJsx)
                {
                    self.push_async_read(
                        file,
                        call,
                        symbol.as_str(),
                        accessor,
                        role,
                        Some(carrier),
                        &mut result,
                    );
                } else if !crate::execution_role::named_at_tracked_position(
                    file,
                    call.callee,
                    self.lookup,
                ) && ((nested_named_function(file, call.callee, self.lookup)
                    && (handed.is_some_and(|(role, _)| role == ExecutionRole::Unknown)
                        || handed_to_unknown_code(file, call.callee, self.lookup)))
                    || escapes_nested(file, call.callee, self.lookup))
                {
                    self.push_async_read(
                        file,
                        call,
                        symbol.as_str(),
                        accessor,
                        ExecutionRole::Unknown,
                        None,
                        &mut result,
                    );
                // As `discover_call`: a read in a non-component function is
                // placed by its callers, so only an obligation here.
                } else if !inside_non_component_function(file, call.callee, self.lookup)
                    || named_callback_execution_role(file, call.callee, self.lookup).is_some()
                {
                    self.push_async_read(
                        file,
                        call,
                        symbol.as_str(),
                        accessor,
                        execution,
                        None,
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
            // ADR 0712: transport carried an async optimistic setter to this
            // call (a prop, a hook's slot, a context value); whether it runs
            // before the source's first answer is not placed here.
            if self.optimistic_async_setter(symbol.as_str()) {
                let here = location(file.path.shared(), call.span);
                if !result
                    .static_violations
                    .iter()
                    .any(|violation| violation.location == here && violation.id == "SC2009")
                {
                    result.static_violations.push(crate::StaticViolation {
                        id: "SC2009".into(),
                        rule: "optimistic-write-before-load".into(),
                        message: "this call runs an optimistic setter transport carried here (a prop, a \
                                  hook's result, a context value); whether it runs before its async \
                                  source's first answer is not proven, and Solid throws \
                                  OPTIMISTIC_WRITE_BEFORE_LOAD if it does"
                            .into(),
                        hint: "Keep pending items in a separate list until the source loads, or declare a \
                               loadingValue."
                            .into(),
                        location: here,
                        analysis_context: crate::blockers::OPTIMISTIC_REACH_UNRESOLVED.into(),
                        fixes: vec![],
                        uncertain: true,
                    });
                }
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
                // ADRs 0715, 0721: a setter or action a hook or context
                // carries, called where no edge reaches, as
                // [`Self::unreached_write`].
                if unreached_write_may_run(file, call.span, self.lookup) {
                    let allowed = allowed_callback_spans(file, self.lookup);
                    self.push_setter_write(
                        file,
                        call,
                        symbol.as_str(),
                        &allowed,
                        true,
                        1,
                        &mut result,
                    );
                    self.push_action_invocation(
                        file,
                        call,
                        symbol.as_str(),
                        &allowed,
                        true,
                        1,
                        &mut result,
                    );
                }
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
    /// ADR 0564: a call in a named nested function no named-callback role
    /// places climbs into it (its lexical role is no proof), and a function
    /// holding a call of the reader is also run at each inline carrier it is
    /// handed to by name, whose run ADR 0563's predicate decides.
    /// A function a primitive holds in an options bundle (an effect's `error`
    /// arm), or a tag this walk follows to its template: both have a place.
    fn placed_elsewhere(
        &self,
        file: &solid_facts::FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> bool {
        let ast = &file.ast;
        ast.arguments_containing(function.span)
            .any(|(call, _)| self.lookup.primitive_at_call(file, call.span).is_some())
            || function_binding_name(file, function).is_some_and(|name| {
                ast.tagged_template_tags
                    .iter()
                    .any(|tag| ast.reference_declaration(*tag) == Some(name.span))
            })
    }

    /// ADR 0667: a read of a possibly async source in a function nothing places:
    /// a window would make it SC5001's, a render SC5003's, and neither is proven.
    fn push_unplaced_async_reads(
        &self,
        file: &solid_facts::FileFacts,
        read: &solid_facts::ast::CallFact,
        name: &str,
        declaration: &Location,
        options: &AsyncSourceOptions,
        result: &mut LocalAccessResult,
    ) {
        let here = location(file.path.shared(), read.span);
        if result.async_reads.iter().any(|row| {
            row.location == here && row.async_provenance && row.execution != ExecutionRole::Unknown
        }) {
            return;
        }
        for (execution, invocation_context_unproven) in [
            (ExecutionRole::Unknown, true),
            (ExecutionRole::TrackedJsx, false),
        ] {
            result.async_reads.push(Arc::new(AsyncRead {
                accessor: format!("{name}()").into(),
                location: here.clone(),
                declaration: declaration.clone(),
                execution,
                leaf_call_site_gates: Vec::new(),
                leaf_owner: None,
                under_loading: false,
                mount_unresolved: false,
                settle_unproven: false,
                call_attributed: true,
                creation_pass_unproven: false,
                root_pass_unproven: false,
                carrier_run_unproven: false,
                apply_first_run_pending: false,
                pending_origin: None,
                pending_apply: None,
                async_provenance: true,
                declared_loading: options.declared_loading,
                options_opaque: options.opaque,
                provenance_open: options.provenance_open,
                caught: false,
                component_prop: false,
                ssr_client_hole: false,
                server_rendering_unresolved: false,
                host_callback_timing: false,
                callee_callback_timing: false,
                invocation_context_unproven,
            }));
        }
    }

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
        // The second flag (ADR 0510): no call of each function's own body is
        // written before the site, so nothing there throws first.
        let mut sites = vec![(read.span, 0_usize, true, true)];
        let mut visited = HashSet::<solid_facts::core::Span>::default();
        while let Some((site, depth, every, first)) = sites.pop() {
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
            let first = first
                && !file.ast.calls.iter().any(|other| {
                    function.body.contains(other.span)
                        && other.span.end <= site.start
                        && containing_ast_function(&file.ast, other.span)
                            .is_some_and(|owner| owner.span == function.span)
                });
            // ADR 0667: an object method, a class member or a tag function has
            // no binding to follow to its calls: what runs it, and where, is
            // not known here.
            if depth == 0
                && crate::execution_role::unplaced_run(file, site, self.lookup)
                && !self.placed_elsewhere(file, function)
            {
                self.push_unplaced_async_reads(file, read, name, declaration, &options, result);
                continue;
            }
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
            // The positions that run the function: each direct call of its
            // binding (the role and anchor are the call's), and (ADR 0563)
            // each inline carrier it is handed to by name (`[1].forEach(f)`,
            // `f.call(null)`, `` f`x` ``), which runs it before returning:
            // the role and anchor are the carrier's. The read's own function
            // is placed at its carriers by the read's row (ADR 0531), so only
            // a function holding a call of the reader is followed there.
            let calls = file
                .ast
                .identifiers
                .iter()
                .filter(|identifier| {
                    identifier.role == solid_facts::ast::IdentifierRole::Reference
                        && file.ast.reference_declaration(identifier.span) == Some(binding.span)
                })
                .filter_map(|reference| {
                    file.ast.calls.iter().find(|call| {
                        call.direct_callee
                            && !call.construct
                            && file.ast.peel_ts_sugar_span(call.callee) == reference.span
                    })
                })
                .map(|call| (call.callee, call.span, false));
            let carriers = if depth > 0 {
                inline_carriers(file, function, self.lookup)
            } else {
                Vec::new()
            }
            .into_iter()
            .map(|carrier| (carrier, carrier, true));
            for (position, anchor, carrier) in calls.chain(carriers).collect::<Vec<_>>() {
                if crate::execution_role::discarded_region_contains(file, anchor)
                    || pending_accessor_probe(file, anchor, self.lookup)
                {
                    continue;
                }
                // ADR 0632, as for a direct read: the call reads the served value.
                if options.declared_loading
                    && crate::execution_role::served_in_creating_pass(
                        file,
                        anchor,
                        declaration,
                        self.lookup,
                    )
                {
                    continue;
                }
                // ADR 0563: a position in a named function no named-callback
                // role places (or in a function written as a member's value)
                // runs where that function runs, never at its lexical role
                // (a flow component's child around it): climb into it, as
                // the direct read's placement does (ADR 0531/0535).
                if nested_named_function(file, position, self.lookup)
                    || escapes_nested(file, position, self.lookup)
                {
                    sites.push((anchor, depth + 1, every && !carrier, first && !carrier));
                    continue;
                }
                // ADR 0563: a carrier that may run the function zero times,
                // or one around the position, proves no read.
                let run_unproven = (carrier
                    && !crate::carrier_runs::carrier_provably_runs(file, anchor, self.lookup))
                    || crate::carrier_runs::carrier_run_unproven(file, position, self.lookup);
                let execution = semantic_execution_role(
                    file,
                    position,
                    allowed,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                );
                let lexical = async_execution_role(file, position, execution);
                let execution = async_read_role(
                    file,
                    position,
                    lexical,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                );
                // ADR 0580: a position in a named function inline carriers
                // run is under the leaf owner of the carrier that runs it.
                let (leaf_owner, leaf_carrier_unproven) = carried_leaf_owner(
                    file,
                    position,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                    CARRIED_LEAF_DEPTH,
                );
                let run_unproven = run_unproven || leaf_carrier_unproven;
                // ADR 0628: a call in an exact `untrack` reads as the read
                // under it does.
                let under_untrack = self.untrack_pending_role(
                    file,
                    position,
                    None,
                    declaration,
                    ExecutionRole::DeferredCallback,
                );
                let execution = if under_untrack == ExecutionRole::DeferredCallback {
                    execution
                } else {
                    under_untrack
                };
                // ADR 0490: a call that runs where a direct read is
                // unsuspendable (a component body's strict-read window,
                // module evaluation, a leaf owner) reads there, as the
                // direct read does.
                let unsuspendable = leaf_owner.is_some()
                    || execution == ExecutionRole::ModuleInitialization
                    || under_untrack != ExecutionRole::DeferredCallback
                    || (execution == ExecutionRole::UntrackedRendering
                        && nested_literal_runs_during_body(
                            file,
                            position,
                            self.entities,
                            self.symbol_names,
                            self.lookup,
                        ));
                // ADR 0504: a call in an effect apply whose first run is
                // proven to meet the source pending reads it there.
                if every
                    && first
                    && !carrier
                    && !options.declared_loading
                    && !options.opaque
                    && let Some(apply) = crate::execution_role::pending_apply_first_read_from(
                        file,
                        anchor,
                        read.span,
                        declaration,
                        self.lookup,
                    )
                {
                    let call_location = location(file.path.shared(), anchor);
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
                            leaf_call_site_gates: Vec::new(),
                            leaf_owner: None,
                            under_loading: false,
                            mount_unresolved: false,
                            settle_unproven: false,
                            call_attributed: true,
                            creation_pass_unproven: false,
                            root_pass_unproven: false,
                            carrier_run_unproven: run_unproven,
                            apply_first_run_pending: true,
                            pending_origin: Some(location(file.path.shared(), read.span)),
                            pending_apply: Some(location(file.path.shared(), apply)),
                            async_provenance: true,
                            declared_loading: false,
                            options_opaque: options.opaque,
                            provenance_open: options.provenance_open,
                            caught: false,
                            component_prop: false,
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
                    // ADR 0779: an anonymous function in a tracked position,
                    // which no binding climbs out of (`<K run={() => h()} />`):
                    // it may run there, past no boundary.
                    if lexical == ExecutionRole::TrackedJsx
                        && read_loading_cover(self.lookup, file, position, self.symbol_names)
                            != LoadingCover::Covered
                        && function_binding_name(
                            file,
                            containing_ast_function(&file.ast, position).unwrap_or(function),
                        )
                        .is_none()
                    {
                        self.push_unplaced_async_reads(
                            file,
                            read,
                            name,
                            declaration,
                            &options,
                            result,
                        );
                        continue;
                    }
                    // The call may itself sit in a named local function a
                    // rendered call runs.
                    sites.push((anchor, depth + 1, every && !carrier, first && !carrier));
                    continue;
                }
                let call_location = location(file.path.shared(), anchor);
                if result
                    .async_reads
                    .iter()
                    .any(|existing| existing.location == call_location)
                {
                    continue;
                }
                let cover = read_loading_cover(self.lookup, file, position, self.symbol_names);
                result.async_reads.push(Arc::new(AsyncRead {
                    accessor: format!("{name}()").into(),
                    location: call_location,
                    declaration: declaration.clone(),
                    execution,
                    leaf_call_site_gates: gated_leaf_call_locations(file, position, self.lookup),
                    leaf_owner: leaf_owner.map(Into::into),
                    under_loading: cover == LoadingCover::Covered,
                    mount_unresolved: cover == LoadingCover::Unresolved,
                    settle_unproven: self.lookup.module_census().may_settle_before_mount(),
                    call_attributed: true,
                    creation_pass_unproven: false,
                    root_pass_unproven: false,
                    carrier_run_unproven: run_unproven,
                    apply_first_run_pending: false,
                    pending_origin: None,
                    pending_apply: None,
                    async_provenance: true,
                    declared_loading: options.declared_loading,
                    options_opaque: options.opaque,
                    provenance_open: options.provenance_open,
                    caught: false,
                    component_prop: false,
                    ssr_client_hole: options.ssr_client_bare,
                    server_rendering_unresolved: options.server_rendering_unresolved,
                    host_callback_timing: host_callback_timing(
                        file,
                        position,
                        execution,
                        self.lookup,
                    ),
                    callee_callback_timing: callee_callback_timing(
                        file,
                        position,
                        execution,
                        self.lookup,
                    ),
                    invocation_context_unproven: under_untrack == ExecutionRole::Unknown,
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
        self.push_accessor_read_placed(file, call, symbol, accessor, execution, None, result);
    }

    /// [`Self::push_accessor_read`] for a read whose run `placed_at` proves
    /// (ADR 0531: the carrier call that runs the read's named function before
    /// returning): the timing questions asked of a read written in place --
    /// a host-retained or project callback around it, a missing JSX census --
    /// are asked of that position, since the read's own function is placed by
    /// the carrier, not by where it is written.
    #[allow(clippy::too_many_arguments)]
    fn push_accessor_read_placed(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        symbol: &str,
        accessor: &(SymbolId, Location),
        execution: ExecutionRole,
        placed_at: Option<solid_facts::core::Span>,
        result: &mut LocalAccessResult,
    ) {
        let (name, declaration) = accessor;
        let timing_at = placed_at.unwrap_or(call.span);
        let origin = self.accessor_origins.get(symbol);
        let display_name = call.static_callee(&file.source).unwrap_or(name);
        result.reads.push(Arc::new(ReactiveRead {
            withdrawn_slot: false,
            pending_open: false,
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
            // ADR 0628: a memo over an async source is pending with it, and a
            // pending read throws instead of warning.
            // ADR 0800: this question alone stops at a depth. A strict read is
            // wrong settled and wrong pending, so a walk cut here proves no less.
            uncertain: self.lookup.inside_possible_component(file, call.span)
                || (!self.async_sources.contains(symbol)
                    && self.value_reaches_near_async_source(file, call.callee)),
            missing_jsx_census: missing_jsx_census(file, timing_at, execution),
            host_callback_timing: host_callback_timing(file, timing_at, execution, self.lookup),
            project_consumer_non_strict: false,
            callback_invocation_unproven: false,
            callee_callback_timing: callee_callback_timing(file, timing_at, execution, self.lookup),
        }));
        if counts_as_strict_read_root(file, call.span, execution, self.lookup) {
            result.strict_read_obligations += 1;
        }
        self.push_async_read(file, call, symbol, accessor, execution, placed_at, result);
    }

    /// The pending-async half of [`Self::push_accessor_read`]: the async read
    /// a call of an async source makes. A read accessor transport carries
    /// (`ctx.data()`, ADR 0509) has its read row from the interprocedural
    /// summaries, and only this half here.
    /// ADR 0629: what the value `span` names holds, where its head (`o` in
    /// `o.a.b`, or the call in `untrack(() => s.o).a`) is the result of an
    /// exact `untrack`, or of a project function returning one. `untrack`
    /// scopes the wrapped read only: a store proxy it hands back warns on the
    /// next Get.
    fn escaped_from_untrack(
        &self,
        file: &solid_facts::FileFacts,
        span: solid_facts::core::Span,
    ) -> Option<EscapedValue> {
        let ast = &file.ast;
        let mut head = ast.peel_ts_sugar_span(span);
        while let Some(member) = ast.member_at(head) {
            head = ast.peel_ts_sugar_span(member.object);
        }
        let call = if let Some(call) = ast.call_at(head) {
            call
        } else {
            let declaration = ast.reference_declaration(head)?;
            let binding = ast.bindings.iter().find(|binding| {
                binding.immutable
                    && binding.shape == solid_facts::ast::BindingShape::Identifier
                    && binding.names.iter().any(|name| name.span == declaration)
            })?;
            ast.call_at(ast.peel_ts_sugar_span(binding.initializer?))?
        };
        if self.lookup.primitive_at_call(file, call.span) != Some(solid_dialect::Primitive::Untrack)
        {
            // A helper that hands one back is not followed: an obligation.
            let (callee_file, callee, _) =
                crate::execution_role::project_callee_at_parameter(file, call, 0, self.lookup)?;
            let returns_untrack = callee_file.ast.returns.iter().any(|returned| {
                containing_ast_function(&callee_file.ast, returned.span)
                    .is_some_and(|function| function.span == callee.span)
                    && returned
                        .argument
                        .and_then(|value| {
                            callee_file
                                .ast
                                .call_at(callee_file.ast.peel_ts_sugar_span(value))
                        })
                        .is_some_and(|inner| {
                            self.lookup.primitive_at_call(callee_file, inner.span)
                                == Some(solid_dialect::Primitive::Untrack)
                        })
            });
            return returns_untrack.then_some(EscapedValue::Unknown);
        }
        if self
            .lookup
            .entity_at(file.path.as_str(), call.span)
            .and_then(|entity| entity.primitive_value_domain.present())
            .is_some_and(|domain| !domain.unknown() && !domain.may_be_object())
        {
            return Some(EscapedValue::Plain);
        }
        let Some(returned) = untrack_returned_value(file, call) else {
            return Some(EscapedValue::Unknown);
        };
        // A prop every call site fills with no source holds no proxy.
        if let Some((declaration, property)) = self.exact_props_callee(file, returned)
            && self
                .props_reactivity
                .holds_no_proxy(&declaration, &property)
        {
            return Some(EscapedValue::Plain);
        }
        if self.lookup.proven_primitive(file, returned, 8)
            || self.lookup.primitive_at_call(file, returned)
                == Some(solid_dialect::Primitive::Snapshot)
        {
            return Some(EscapedValue::Plain);
        }
        let mut keys = Vec::new();
        let mut root = returned;
        while let Some(member) = ast.member_at(root) {
            if ast.computed_members.contains(&member.span)
                || ast.optional_members.binary_search(&member.span).is_ok()
            {
                return Some(EscapedValue::Unknown);
            }
            keys.push(file.source_text(member.property)?);
            root = ast.peel_ts_sugar_span(member.object);
        }
        keys.reverse();
        let symbol = self.entities.value_symbol(file, root);
        let Some(symbol) = symbol.filter(|symbol| {
            self.source_kinds.get(*symbol) == Some(&ReactiveSourceKind::Store)
                && self.accessors.contains_key(*symbol)
        }) else {
            return Some(EscapedValue::Unknown);
        };
        Some(match self.store_path_literal(file, symbol, &keys) {
            StorePathValue::Wrapped => EscapedValue::Store(symbol.clone(), true),
            StorePathValue::Primitive => EscapedValue::Plain,
            StorePathValue::Unknown => EscapedValue::Store(symbol.clone(), false),
        })
    }

    /// What the literal initial value of the store `symbol` holds at `keys`.
    /// The store's root is a proxy whatever it holds; below it only a value
    /// written as an object or array literal in a store this file declares is
    /// proven, since anything else may be a function, a `Date`, a class
    /// instance or a later write.
    fn store_path_literal(
        &self,
        file: &solid_facts::FileFacts,
        symbol: &SymbolId,
        keys: &[&str],
    ) -> StorePathValue {
        if keys.is_empty() {
            return StorePathValue::Wrapped;
        }
        let Some((_, declaration)) = self.accessors.get(symbol) else {
            return StorePathValue::Unknown;
        };
        if self.async_sources.contains(symbol)
            || !matches!(
                self.store_initial_shape(file, declaration),
                StoreShape::Keyed
            )
        {
            return StorePathValue::Unknown;
        }
        let Some(mut value) = self.store_initial_value(file, declaration) else {
            return StorePathValue::Unknown;
        };
        for key in keys {
            let Some(next) = crate::execution_role::closed_literal_slot_value(file, value, key)
            else {
                return StorePathValue::Unknown;
            };
            value = file.ast.peel_ts_sugar_span(next);
        }
        if file.ast.array_literals.contains(&value)
            || file
                .ast
                .object_get_shapes
                .iter()
                .any(|shape| shape.span == value && shape.closed)
        {
            StorePathValue::Wrapped
        } else if self.lookup.proven_primitive(file, value, 8) {
            StorePathValue::Primitive
        } else {
            StorePathValue::Unknown
        }
    }

    /// ADR 0629: the Get at `span` of a value escaped from `untrack`: the
    /// read of a proxy where the store's shape proves one, an obligation where
    /// nothing says what the value holds, and nothing on a plain value.
    /// Answers whether the value escaped, so the caller stops.
    fn discover_escaped_get(
        &self,
        file: &solid_facts::FileFacts,
        span: solid_facts::core::Span,
        allowed: &[solid_facts::core::Span],
        result: &mut LocalAccessResult,
    ) -> bool {
        let Some(escaped) = self.escaped_from_untrack(file, span) else {
            return false;
        };
        // A pattern's reads have no head member; it reads what it names.
        let warns = file.ast.member_at(span).is_none()
            || matches!(
                store_head_get(file, span, self.lookup.dialect),
                StoreHeadGet::Warns
            );
        match escaped {
            EscapedValue::Plain => {}
            EscapedValue::Store(symbol, proven) => {
                self.push_whole_store_read(
                    file,
                    span,
                    &symbol,
                    "value escaped from untrack",
                    proven && warns,
                    allowed,
                    result,
                );
            }
            EscapedValue::Unknown => {
                if read_execution_role(
                    file,
                    span,
                    allowed,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                )
                .reports_untracked_read()
                {
                    result.dispatch_obligations.push(crate::StaticDefect {
                        kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                            callee: file.source_text(span).unwrap_or("this value").to_owned(),
                            member: None,
                        },
                        location: location(file.path.shared(), span),
                        analysis_context: "value-escaped-from-untrack".to_owned(),
                        fixes: vec![],
                        uncertain: true,
                    });
                }
            }
        }
        true
    }

    /// ADR 0629: a whole-value use (a spread, `JSON.stringify(o)`) at `site`
    /// of an escaped value named exactly at `value`: its shape is not the
    /// store's, so it is an obligation at most.
    fn discover_escaped_whole(
        &self,
        file: &solid_facts::FileFacts,
        value: solid_facts::core::Span,
        site: solid_facts::core::Span,
        allowed: &[solid_facts::core::Span],
        result: &mut LocalAccessResult,
    ) -> bool {
        match self.escaped_from_untrack(file, value) {
            Some(EscapedValue::Store(symbol, _)) => {
                self.push_whole_store_read(
                    file,
                    site,
                    &symbol,
                    "value escaped from untrack",
                    false,
                    allowed,
                    result,
                );
                true
            }
            Some(EscapedValue::Plain) => true,
            Some(EscapedValue::Unknown) | None => false,
        }
    }

    /// The role of a read of an async source under an exact `untrack`
    /// (ADR 0628): it throws there as a direct read does, or no proof places
    /// its source and it is an obligation.
    fn untrack_pending_role(
        &self,
        file: &solid_facts::FileFacts,
        read: solid_facts::core::Span,
        placed_at: Option<solid_facts::core::Span>,
        declaration: &Location,
        execution: ExecutionRole,
    ) -> ExecutionRole {
        use crate::execution_role::UntrackPendingRead;
        // `untrack(m)` places the read at the call, and names it in its argument.
        let carrier = placed_at
            .and_then(|at| file.ast.call_at(at))
            .filter(|call| {
                self.lookup.primitive_at_call(file, call.span)
                    == Some(solid_dialect::Primitive::Untrack)
            });
        let read = carrier
            .and_then(|call| call.arguments.first())
            .map_or(read, |argument| argument.span);
        let layers = crate::execution_role::untrack_layers(
            file,
            read,
            carrier,
            self.entities,
            self.symbol_names,
            self.lookup,
        );
        match crate::execution_role::untrack_pending_read(
            file,
            layers.as_ref(),
            read,
            declaration,
            self.lookup,
        ) {
            UntrackPendingRead::Throws => ExecutionRole::UntrackedRendering,
            UntrackPendingRead::Unproven => ExecutionRole::Unknown,
            UntrackPendingRead::NotUnderUntrack | UntrackPendingRead::OtherOwner => execution,
        }
    }

    /// ADR 0628: the value `value` a body reads under `untrack` is the result
    /// of a hook, and nothing resolves what it holds: a pending memo the hook
    /// created throws there. The obligation yields to a transport that does.
    fn push_untrack_hook_value_obligation(
        &self,
        file: &solid_facts::FileFacts,
        value: solid_facts::core::Span,
        carrier: Option<&solid_facts::ast::CallFact>,
        result: &mut LocalAccessResult,
    ) {
        if !self.lookup.dialect.untrack_keeps_own_pending_throw() {
            return;
        }
        let hook = crate::execution_role::hook_result_root(file, value, self.lookup);
        if hook.is_none() && !self.value_reaches_async_source(file, value).may() {
            return;
        }
        // A module's source has no owner of a body.
        if crate::execution_role::read_of_module_hook_result(file, value)
            || (hook.is_none()
                && file
                    .ast
                    .reference_declaration(file.ast.peel_ts_sugar_span(value))
                    .is_some_and(|declared| crate::indexes::at_module_scope(&file.ast, declared)))
        {
            return;
        }
        // rc.21 throws on a pending source of this very owner: a computation
        // this body or the hook made. A signal of a value is never pending, and
        // a prop or a context value was made by another owner.
        if hook.is_some_and(|hook| {
            !self.span_may_make_computation(file, hook.span, 4)
                && !self
                    .value_reaches(file, value, &|symbol| {
                        self.accessors.contains_key(symbol) && !self.accessor_holds_a_value(symbol)
                    })
                    .may()
                && !self.value_reaches_open_prop(file, value)
        }) {
            return;
        }
        let holds = crate::execution_role::untrack_layers(
            file,
            value,
            carrier,
            self.entities,
            self.symbol_names,
            self.lookup,
        )
        .is_some_and(|layers| {
            layers.keeps_pending_throw()
                && hook.is_none_or(|hook| layers.holds_call(file, hook.span))
        });
        if holds {
            result.dispatch_obligations.push(crate::StaticDefect {
                kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                    callee: file.source_text(value).unwrap_or("this value").to_owned(),
                    member: None,
                },
                location: location(file.path.shared(), value),
                // No transport follows a value handed to `untrack` by name, and
                // one that names a sync memo says nothing of the memo's sources.
                analysis_context: if carrier.is_some() {
                    "value-read-under-untrack".to_owned()
                } else {
                    match self.value_reaches_async_source(file, value) {
                        ValueReach::Reaches => "derived-read-under-untrack".to_owned(),
                        ValueReach::Undecided => {
                            crate::blockers::SEARCH_BUDGET_EXHAUSTED.to_owned()
                        }
                        ValueReach::Clear => UNRESOLVED_HANDED_READ.to_owned(),
                    }
                },
                fixes: vec![],
                uncertain: true,
            });
        }
    }

    /// ADR 0628: a call in `span`, or in the project functions those calls run,
    /// may make a computation: a source that can be pending. A signal or store
    /// of a data literal is none. A call nothing resolves may make one.
    fn span_may_make_computation(
        &self,
        file: &solid_facts::FileFacts,
        span: solid_facts::core::Span,
        depth: usize,
    ) -> bool {
        let Some(depth) = depth.checked_sub(1) else {
            return true;
        };
        file.ast
            .calls
            .iter()
            .filter(|call| span.contains(call.span))
            .any(|call| {
                if let Some(primitive) = self.lookup.primitive_at_call(file, call.span) {
                    return self.lookup.dialect.creates_reactive_source(primitive)
                        && !source_call_holds_a_value(file, call, primitive);
                }
                if self
                    .lookup
                    .callee_symbol(file, call.callee)
                    .is_some_and(|symbol| self.accessors.contains_key(symbol))
                {
                    return false;
                }
                if crate::indexes::standard_library_declaration_of(
                    self.lookup.resolved_callee_call(file, call.callee),
                    typefacts::CallKind::Call,
                )
                .is_some()
                {
                    return false;
                }
                crate::execution_role::project_callee_at_parameter(file, call, 0, self.lookup)
                    .is_none_or(|(callee_file, callee, _)| {
                        self.span_may_make_computation(callee_file, callee.span, depth)
                    })
            })
    }

    /// `value` reaches a parameter of a function around it that is no props
    /// object every call site fills with signals of values. A prop's getter runs
    /// here, so a memo written inline in the parent's JSX is this body's own.
    fn value_reaches_open_prop(
        &self,
        file: &solid_facts::FileFacts,
        value: solid_facts::core::Span,
    ) -> bool {
        let ast = &file.ast;
        ast.functions_owning(value)
            .flat_map(|function| &function.parameters)
            .flat_map(|parameter| parameter.names.iter().map(move |name| (parameter, name)))
            .any(|(parameter, name)| {
                let Some(symbol) = self.entities.at(file.path.as_str(), name.span) else {
                    return true;
                };
                if !self
                    .value_reaches(file, value, &|reached| reached == symbol)
                    .may()
                {
                    return false;
                }
                let open = |declared: &Location, property: &str| {
                    let (sources, complete) =
                        self.props_reactivity.passed_sources(declared, property);
                    !complete
                        || sources.is_empty()
                        || sources
                            .iter()
                            .any(|source| !self.accessor_holds_a_value(source))
                };
                // `({ r1 })`: the slot names its prop.
                if parameter.shape != solid_facts::ast::BindingShape::Identifier {
                    return parameter
                        .object_slots
                        .iter()
                        .find(|slot| slot.local.span == name.span && slot.plain_identifier_key)
                        .is_none_or(|slot| {
                            open(
                                &location(file.path.shared(), parameter.pattern),
                                &slot.property,
                            )
                        });
                }
                let declared = location(file.path.shared(), name.span);
                ast.identifiers
                    .iter()
                    .filter(|identifier| {
                        ast.reference_declaration(identifier.span) == Some(name.span)
                    })
                    .any(|identifier| {
                        let member = ast.members.iter().find(|member| {
                            ast.peel_ts_sugar_span(member.object) == identifier.span
                                && !ast.computed_members.contains(&member.span)
                        });
                        member
                            .and_then(|member| file.source_text(member.property))
                            .is_none_or(|property| open(&declared, property))
                    })
            })
    }

    /// The accessor `symbol` belongs to a signal of a data literal.
    fn accessor_holds_a_value(&self, symbol: &SymbolId) -> bool {
        let Some((_, declared)) = self.accessors.get(symbol) else {
            return false;
        };
        let Some(file) = self.lookup.file_by_path(&declared.path) else {
            return false;
        };
        file.ast
            .bindings
            .iter()
            .find(|binding| {
                binding.names.iter().any(|name| {
                    u64::from(name.span.start) == declared.start_byte
                        && u64::from(name.span.end) == declared.end_byte
                })
            })
            .and_then(|binding| binding.call_initializer)
            .and_then(|initializer| file.ast.call_at(initializer))
            .is_some_and(|call| {
                self.lookup
                    .primitive_at_call(file, call.span)
                    .is_some_and(|primitive| source_call_holds_a_value(file, call, primitive))
            })
    }

    /// ADR 0628: what `value` names may hold an async source or a function
    /// reading one: an identifier in it is one, or a `let` nothing states, or
    /// a binding or function whose text names one, through `depth` steps.
    /// Over-approximates: it only ever makes an obligation of a clean.
    fn value_reaches_async_source(
        &self,
        file: &solid_facts::FileFacts,
        value: solid_facts::core::Span,
    ) -> ValueReach {
        self.value_reaches(file, value, &|symbol| self.async_sources.contains(symbol))
    }

    /// ADR 0800: whether the value at `value` names a symbol `hit` takes,
    /// through bindings, functions and hooks, each visited once. A chain has no
    /// length limit: only the work budget stops the walk, and then it is
    /// undecided.
    fn value_reaches(
        &self,
        file: &solid_facts::FileFacts,
        value: solid_facts::core::Span,
        hit: &dyn Fn(&SymbolId) -> bool,
    ) -> ValueReach {
        let mut walk = ValueWalk {
            seen: HashSet::default(),
            left: VALUE_WALK_BUDGET,
            depth: 0,
            cut_at: None,
        };
        if self.value_walk(file, value, &mut walk, hit) {
            ValueReach::Reaches
        } else if walk.left == 0 {
            ValueReach::Undecided
        } else {
            ValueReach::Clear
        }
    }

    /// [`Self::value_reaches_async_source`] within four steps, for the one
    /// question whose answer stands either way.
    fn value_reaches_near_async_source(
        &self,
        file: &solid_facts::FileFacts,
        value: solid_facts::core::Span,
    ) -> bool {
        let mut walk = ValueWalk {
            seen: HashSet::default(),
            left: VALUE_WALK_BUDGET,
            depth: 0,
            cut_at: Some(4),
        };
        self.value_walk(file, value, &mut walk, &|symbol| {
            self.async_sources.contains(symbol)
        })
    }

    fn value_walk(
        &self,
        file: &solid_facts::FileFacts,
        value: solid_facts::core::Span,
        walk: &mut ValueWalk,
        hit: &dyn Fn(&SymbolId) -> bool,
    ) -> bool {
        // A cut walk may meet a value again nearer the root.
        if walk.left == 0
            || walk.cut_at.is_some_and(|cut| walk.depth >= cut)
            || (walk.cut_at.is_none() && !walk.seen.insert((file.path.shared(), value)))
        {
            return false;
        }
        walk.left -= 1;
        walk.depth += 1;
        let reaches = self.value_step(file, value, walk, hit);
        walk.depth -= 1;
        reaches
    }

    fn value_step(
        &self,
        file: &solid_facts::FileFacts,
        value: solid_facts::core::Span,
        walk: &mut ValueWalk,
        hit: &dyn Fn(&SymbolId) -> bool,
    ) -> bool {
        let ast = &file.ast;
        // What the text declares: a shorthand `return { setO }` is no reference.
        if ast.bindings.iter().any(|binding| {
            binding.names.iter().any(|name| {
                value.contains(name.span)
                    && self
                        .entities
                        .at(file.path.as_str(), name.span)
                        .is_some_and(hit)
            })
        }) {
            return true;
        }
        ast.identifiers
            .iter()
            .filter(|identifier| {
                value.contains(identifier.span)
                    && ast.reference_declaration(identifier.span).is_some()
            })
            .any(|identifier| {
                if crate::source_discovery::reference_symbol(
                    file,
                    identifier.span,
                    self.entities,
                    self.lookup,
                )
                .is_some_and(|symbol| hit(&symbol))
                {
                    return true;
                }
                let Some(declaration) = ast.reference_declaration(identifier.span) else {
                    return false;
                };
                if let Some(function) = ast.functions.iter().find(|function| {
                    function
                        .name
                        .as_ref()
                        .is_some_and(|name| name.span == declaration)
                }) {
                    return self.value_walk(file, function.span, walk, hit);
                }
                let binding = ast
                    .bindings
                    .iter()
                    .find(|binding| binding.names.iter().any(|name| name.span == declaration));
                // A hook's result holds what the hook's own text names, in
                // whichever module it is.
                if let Some(call) = binding
                    .and_then(|binding| binding.call_initializer)
                    .and_then(|initializer| ast.call_at(initializer))
                    && let Some((hook_file, hook, _)) =
                        crate::execution_role::project_callee_at_parameter(
                            file,
                            call,
                            0,
                            self.lookup,
                        )
                    && self.value_walk(hook_file, hook.span, walk, hit)
                {
                    return true;
                }
                binding.is_some_and(|binding| match binding.initializer {
                    Some(initializer) => self.value_walk(file, initializer, walk, hit),
                    // A `let` with no initializer holds what is assigned to it.
                    None => {
                        !binding.immutable
                            && ast.assignments.iter().any(|assignment| {
                                ast.identifiers.iter().any(|name| {
                                    assignment.target.contains(name.span)
                                        && ast.reference_declaration(name.span) == Some(declaration)
                                }) && self.value_walk(file, assignment.value_span, walk, hit)
                            })
                    }
                })
            })
    }

    /// ADR 0628: `untrack(m)` reads `m` where the call runs, and an async
    /// source a value names there is read as `untrack(() => m())` reads it.
    fn discover_untrack_reference(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        allowed: &[solid_facts::core::Span],
        result: &mut LocalAccessResult,
    ) {
        let Some(argument) = call.arguments.first().filter(|argument| !argument.spread) else {
            return;
        };
        let value = file.ast.peel_ts_sugar_span(argument.span);
        if file.ast.function_at(value).is_some() {
            return;
        }
        match self.entities.value_symbol(file, value) {
            Some(symbol) if self.accessors.contains_key(symbol) => {
                if self.async_sources.contains(symbol)
                    && let Some(accessor) = self.accessors.get(symbol)
                {
                    let execution = read_execution_role(
                        file,
                        call.span,
                        allowed,
                        self.entities,
                        self.symbol_names,
                        self.lookup,
                    );
                    self.push_async_read(
                        file,
                        call,
                        symbol.as_str(),
                        accessor,
                        execution,
                        Some(call.span),
                        result,
                    );
                }
            }
            _ => self.push_untrack_hook_value_obligation(file, value, Some(call), result),
        }
    }

    /// ADR 0628: a value handed to a project function that reads the
    /// parameter under `untrack`, in a body's strict-read window: an async
    /// source the body created, or a hook's unresolved result, may throw.
    fn discover_handed_to_untrack(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        allowed: &[solid_facts::core::Span],
        result: &mut LocalAccessResult,
    ) {
        if !self.lookup.dialect.untrack_keeps_own_pending_throw()
            || call.arguments.is_empty()
            || read_execution_role(
                file,
                call.span,
                allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            ) != ExecutionRole::UntrackedRendering
        {
            return;
        }
        for (index, argument) in call.arguments.iter().enumerate() {
            if argument.spread
                || !crate::execution_role::callee_reads_parameter_under_untrack(
                    file,
                    call,
                    index,
                    self.lookup,
                )
            {
                continue;
            }
            let value = file.ast.peel_ts_sugar_span(argument.span);
            let symbol = self.entities.value_symbol(file, value);
            let handed = match symbol {
                Some(symbol) if self.accessors.contains_key(symbol) => {
                    self.async_sources.contains(symbol)
                }
                _ => crate::execution_role::hook_result_root(file, value, self.lookup).is_some(),
            };
            if handed {
                result.dispatch_obligations.push(crate::StaticDefect {
                    kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                        callee: file.source_text(value).unwrap_or("this value").to_owned(),
                        member: None,
                    },
                    location: location(file.path.shared(), value),
                    analysis_context: "value-read-under-untrack".to_owned(),
                    fixes: vec![],
                    uncertain: true,
                });
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn push_async_read(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        symbol: &str,
        accessor: &(SymbolId, Location),
        execution: ExecutionRole,
        placed_at: Option<solid_facts::core::Span>,
        result: &mut LocalAccessResult,
    ) {
        let (name, declaration) = accessor;
        let here = location(file.path.shared(), call.span);
        if result.async_reads.iter().any(|read| read.location == here) {
            return;
        }
        // ADR 0531: the timing of a read a carrier places is the carrier's.
        let timing_at = placed_at.unwrap_or(call.callee);
        let async_provenance = self.async_sources.contains(symbol);
        let async_options = self.effective_async_options(symbol);
        // ADR 0632: the pass that creates a declared source reads the served
        // value: no pending throw to report, and the strict read stands.
        if async_options.declared_loading
            && crate::execution_role::served_in_creating_pass(
                file,
                timing_at,
                declaration,
                self.lookup,
            )
        {
            return;
        }
        if (async_provenance
            || async_options.ssr_client_bare
            || async_options.server_rendering_unresolved)
            && !pending_accessor_probe(file, call.callee, self.lookup)
        {
            let async_execution = async_execution_role(file, call.callee, execution);
            // ADR 0623: tracked, in a computation's own callback: it suspends.
            let computation_read = async_execution == ExecutionRole::TrackedJsx
                && crate::execution_role::computation_read_suspends(
                    file,
                    call.callee,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                );
            let async_execution =
                if async_options.ssr_client_bare || async_options.server_rendering_unresolved {
                    async_execution
                } else {
                    async_read_role(
                        file,
                        // ADR 0776: a carrier's role is the carrier's.
                        placed_at.unwrap_or(call.callee),
                        async_execution,
                        self.entities,
                        self.symbol_names,
                        self.lookup,
                    )
                };
            // ADR 0619, ADR 0628: rc.21 keeps the pending throw through an
            // unlabelled `untrack` for a source the strict body itself created.
            let async_execution = if async_provenance {
                self.untrack_pending_role(
                    file,
                    call.callee,
                    placed_at,
                    declaration,
                    async_execution,
                )
            } else {
                async_execution
            };
            // The walk proved the read runs on the `untrack`'s stack in the body.
            let throws_under_untrack = async_provenance
                && async_execution == ExecutionRole::UntrackedRendering
                && execution != ExecutionRole::UntrackedRendering;
            let cover = read_loading_cover(self.lookup, file, timing_at, self.symbol_names);
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
            // ADR 0580: a read in a named function run by inline carriers
            // is under the leaf owner of the carrier that runs it.
            let (leaf_owner, leaf_carrier_unproven) = carried_leaf_owner(
                file,
                call.callee,
                self.entities,
                self.symbol_names,
                self.lookup,
                CARRIED_LEAF_DEPTH,
            );
            // ADR 0778: a pending read on a later task throws with no code, and
            // whether the source has settled by then is unknown. The projector
            // takes no such role: the read is unplaced, as a handler's is. A
            // leaf owner's callback has its own rule.
            let async_execution = if async_provenance
                && async_execution == ExecutionRole::DeferredCallback
                && leaf_owner.is_none()
                && !async_options.declared_loading
                && !file.ast.any_jsx_containing(call.callee)
            {
                ExecutionRole::Unknown
            } else {
                async_execution
            };
            result.async_reads.push(Arc::new(AsyncRead {
                accessor: format!("{name}()").into(),
                location: location(file.path.shared(), call.span),
                declaration: declaration.clone(),
                execution: async_execution,
                leaf_call_site_gates: gated_leaf_call_locations(file, call.span, self.lookup),
                leaf_owner: leaf_owner.map(Into::into),
                under_loading: cover == LoadingCover::Covered,
                mount_unresolved: cover == LoadingCover::Unresolved,
                settle_unproven: self.lookup.module_census().may_settle_before_mount(),
                call_attributed: false,
                // ADR 0563: the carrier that places the read, and every
                // carrier around it, must run it.
                creation_pass_unproven: false,
                root_pass_unproven: false,
                carrier_run_unproven: leaf_carrier_unproven
                    || placed_at.is_some_and(|carrier| {
                        !crate::carrier_runs::carrier_provably_runs(file, carrier, self.lookup)
                    })
                    || crate::carrier_runs::carrier_run_unproven(file, timing_at, self.lookup),
                // The proof climbs from the read to the apply itself, so a
                // read in a closure the apply calls (its own role the body's)
                // is placed there too.
                apply_first_run_pending: pending_apply.is_some(),
                pending_origin: None,
                pending_apply: pending_apply.map(|apply| location(file.path.shared(), apply)),
                async_provenance,
                declared_loading: async_options.declared_loading,
                options_opaque: async_options.opaque,
                provenance_open: async_options.provenance_open,
                caught: caught_in_try(file, timing_at),
                component_prop: self.read_in_component_prop(file, timing_at),
                ssr_client_hole: async_options.ssr_client_bare,
                server_rendering_unresolved: async_options.server_rendering_unresolved,
                host_callback_timing: host_callback_timing(
                    file,
                    timing_at,
                    async_execution,
                    self.lookup,
                ),
                callee_callback_timing: callee_callback_timing(
                    file,
                    timing_at,
                    async_execution,
                    self.lookup,
                ),
                invocation_context_unproven: (async_execution == ExecutionRole::Unknown
                    && !computation_read)
                    || async_execution == ExecutionRole::UntrackedRendering
                        && !throws_under_untrack
                        && !nested_literal_runs_during_body(
                            file,
                            timing_at,
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
        if !prop_head && read_as_part_of_longer_path(file, member) {
            return;
        }
        // ADR 0629: the head of the path is a value `untrack` returned. A write
        // through it is dropped wherever it runs if it holds a store.
        if file.ast.is_no_get_target(member.span) {
            if matches!(
                self.escaped_from_untrack(file, member.span),
                Some(EscapedValue::Store(..) | EscapedValue::Unknown)
            ) {
                result.dispatch_obligations.push(crate::StaticDefect {
                    kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                        callee: file
                            .source_text(member.span)
                            .unwrap_or("this write")
                            .to_owned(),
                        member: None,
                    },
                    location: location(file.path.shared(), member.span),
                    analysis_context: "value-escaped-from-untrack".to_owned(),
                    fixes: vec![],
                    uncertain: true,
                });
            }
        } else if self.discover_escaped_get(file, member.span, allowed, result) {
            return;
        }
        // ADR 0628: a member of a hook's unresolved result in a body's
        // `untrack`: a pending store the hook made throws there.
        if self
            .entities
            .get(&location(file.path.shared(), receiver))
            .is_none_or(|symbol| {
                !self.accessors.contains_key(symbol) && !self.prop_sources.contains_key(symbol)
            })
            && file
                .ast
                .calls_with_peeled_callee(member.span)
                .next()
                .is_none()
        {
            self.push_untrack_hook_value_obligation(file, member.span, None, result);
        }
        // A plain assignment writes this exact member without reading its
        // old value, as does a plain destructuring assignment at each leaf of
        // its pattern (`({ v: store.v } = source)`, ADR 0526: the Get is the
        // source's). Compound assignments, updates, and members nested inside
        // a target (a computed key, a destructuring default) are retained as
        // reads.
        if file.ast.is_no_get_target(member.span)
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
        let member_alias_symbol = member_receiver_alias(file, member.object)
            .and_then(|alias| self.entities.at(file.path.as_str(), alias.source).cloned());
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
        let mut execution = if self.lookup.local_alias_mention_withheld(file, object_span) {
            ExecutionRole::Unknown
        } else {
            read_execution_role(
                file,
                member.span,
                allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            )
        };
        // ADR 0676: as for a call.
        let in_setter = self.setter_callback_role(file, member.span, allowed, Some(symbol));
        if let Some(role) = in_setter {
            execution = role;
        }
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
        // ADR 0614: where the gate refuses, the summaries see only a direct
        // call of the function. As for an accessor's call (ADR 0531, ADR
        // 0516), a carrier that runs the function places the read, and a
        // function handed where nothing follows it makes it an obligation.
        let mut placed_at = None;
        let nested = nested_named_function(file, member.span, self.lookup);
        let escapes = escapes_nested(file, member.span, self.lookup);
        if helper.is_none()
            && (gate_refuses
                || nested
                || escapes
                || inside_non_component_function(file, member.span, self.lookup))
        {
            let handed = handed_read_role(
                file,
                member.span,
                allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            );
            let unplaced = handed.is_some_and(|(role, _)| role == ExecutionRole::Unknown);
            if let Some((role, Some(carrier))) = handed
                && role.reports_untracked_read()
            {
                execution = role;
                placed_at = Some(carrier);
            } else if gate_refuses
                || unplaced
                || (nested && handed_to_unknown_code(file, member.span, self.lookup))
                || escapes
            {
                if crate::execution_role::named_at_tracked_position(file, member.span, self.lookup)
                    || !(unplaced || placed_nowhere(file, member.span, self.lookup))
                {
                    return;
                }
                execution = ExecutionRole::Unknown;
            }
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
                .calls_with_peeled_callee(member.span)
                .next()
                .is_some()
        {
            return;
        }
        // Caller-proven props: a prop every call site passes statically
        // compiles to a plain property — reading it is not a reactive
        // read at all. Unprovable backing stays a proof obligation.
        let mut pending_open = false;
        let mut uncertain = self.uncertain_prop_sources.contains(symbol)
            || self.lookup.inside_possible_component(file, member.span);
        uncertain |= self.store_may_be_raw(symbol);
        // ADR 0630: a key a `merge` view serves from a plain source.
        if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
            match self.merge_view_get(file, member.object, property) {
                MergeGet::Plain => return,
                MergeGet::Unproven => uncertain = true,
                MergeGet::NoView | MergeGet::Store => {}
            }
        }
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
                pending_open = !uncertain;
                uncertain = true;
            }
            // ADR 0803: an unknown backing keeps the obligation too (a
            // getter calling a withdrawn async slot is neither proven).
            let async_passed = exact_root
                && backing != PropUse::Static
                && property.is_some_and(|property| {
                    self.props_reactivity
                        .async_getter(prop_declaration, property)
                });
            // ADR 0823: in a leaf owner (createTrackedEffect, onSettled) a
            // pending read is the runtime's PENDING_ASYNC_FORBIDDEN_SCOPE.
            // Unless every caller is enumerated and none may pass a pending
            // getter, an unseen or unclassified caller may: an obligation.
            let context = if async_passed
                && (execution != ExecutionRole::TrackedJsx
                    || self.may_run_in_leaf_owner(file, member.span))
            {
                Some("async-prop-getter")
            } else if !async_passed
                && !(exact_root
                    && property.is_some_and(|property| {
                        self.props_reactivity
                            .pending_excluded(prop_declaration, property)
                    }))
                && self.may_run_in_leaf_owner(file, member.span)
            {
                Some(crate::blockers::PROP_PENDING_UNPROVEN)
            } else {
                None
            };
            if let Some(context) = context {
                result.dispatch_obligations.push(crate::StaticDefect {
                    kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                        callee: file
                            .source_text(member.span)
                            .unwrap_or("this prop")
                            .to_owned(),
                        member: None,
                    },
                    location: location(file.path.shared(), member.span),
                    analysis_context: context.to_owned(),
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
        let mut read = self.member_read(
            file,
            member,
            symbol,
            (name, declaration),
            (execution, uncertain, prop_head),
            placed_at,
        );
        read.pending_open = pending_open;
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
        self.push_member_async_read(
            file,
            member.span,
            file.source_text(member.property).unwrap_or_default(),
            symbol,
            (name, declaration),
            execution,
            result,
        );
    }

    fn member_read(
        &self,
        file: &solid_facts::FileFacts,
        member: &solid_facts::ast::MemberFact,
        symbol: &SymbolId,
        (name, declaration): (&SymbolId, &Location),
        (execution, uncertain, prop_head): (ExecutionRole, bool, bool),
        placed_at: Option<solid_facts::core::Span>,
    ) -> ReactiveRead {
        // ADR 0531: the timing of a read a carrier places is the carrier's.
        // ADR 0676: a read in a setter's callback runs when the setter does.
        let timing_at = placed_at
            .or_else(|| self.running_setter_call(file, member.span))
            .unwrap_or(member.span);
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
            withdrawn_slot: false,
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
            pending_open: false,
            missing_jsx_census: missing_jsx_census(file, timing_at, execution),
            host_callback_timing: host_callback_timing(file, timing_at, execution, self.lookup),
            project_consumer_non_strict: false,
            callback_invocation_unproven: false,
            callee_callback_timing: callee_callback_timing(file, timing_at, execution, self.lookup),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn push_member_async_read(
        &self,
        file: &solid_facts::FileFacts,
        member: solid_facts::core::Span,
        property: &str,
        symbol: &SymbolId,
        (name, declaration): (&SymbolId, &Location),
        execution: ExecutionRole,
        result: &mut LocalAccessResult,
    ) {
        let member_async = self.async_sources.contains(symbol);
        let member_options = self.effective_async_options(symbol);
        // ADR 0633: `isPending(() => p.a)` and `latest(() => p.a)` are probes:
        // the strict read warns and no pending throw fires.
        if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store)
            && (member_async
                || member_options.ssr_client_bare
                || member_options.server_rendering_unresolved)
            && !pending_accessor_probe(file, member, self.lookup)
        {
            let async_execution = async_execution_role(file, member, execution);
            let async_execution =
                if member_options.ssr_client_bare || member_options.server_rendering_unresolved {
                    async_execution
                } else {
                    async_read_role(
                        file,
                        member,
                        async_execution,
                        self.entities,
                        self.symbol_names,
                        self.lookup,
                    )
                };
            // ADR 0632, as for an accessor.
            if member_options.declared_loading
                && crate::execution_role::served_in_creating_pass(
                    file,
                    member,
                    declaration,
                    self.lookup,
                )
            {
                return;
            }
            // ADR 0619, as for an accessor: a derive the strict body created.
            let async_execution = if member_async {
                self.untrack_pending_role(file, member, None, declaration, async_execution)
            } else {
                async_execution
            };
            let cover = read_loading_cover(self.lookup, file, member, self.symbol_names);
            result.async_reads.push(Arc::new(AsyncRead {
                accessor: format!("{name}.{property}").into(),
                location: location(file.path.shared(), member),
                declaration: declaration.clone(),
                execution: async_execution,
                leaf_call_site_gates: gated_leaf_call_locations(file, member, self.lookup),
                leaf_owner: containing_leaf_owner(
                    file,
                    member,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                )
                .map(Into::into),
                under_loading: cover == LoadingCover::Covered,
                mount_unresolved: cover == LoadingCover::Unresolved,
                settle_unproven: self.lookup.module_census().may_settle_before_mount(),
                call_attributed: false,
                creation_pass_unproven: false,
                root_pass_unproven: false,
                carrier_run_unproven: crate::carrier_runs::carrier_run_unproven(
                    file,
                    member,
                    self.lookup,
                ),
                apply_first_run_pending: false,
                pending_origin: None,
                pending_apply: None,
                async_provenance: member_async,
                declared_loading: member_options.declared_loading,
                options_opaque: member_options.opaque,
                provenance_open: member_options.provenance_open,
                caught: caught_in_try(file, member),
                component_prop: self.read_in_component_prop(file, member),
                ssr_client_hole: member_options.ssr_client_bare,
                server_rendering_unresolved: member_options.server_rendering_unresolved,
                host_callback_timing: host_callback_timing(
                    file,
                    member,
                    async_execution,
                    self.lookup,
                ),
                callee_callback_timing: callee_callback_timing(
                    file,
                    member,
                    async_execution,
                    self.lookup,
                ),
                invocation_context_unproven: async_execution == ExecutionRole::UntrackedRendering
                    && !nested_literal_runs_during_body(
                        file,
                        member,
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
            // ADR 0629: a spread of a value escaped from `untrack`.
            if inline_root.is_none()
                && !argument.is_some_and(|symbol| {
                    self.prop_sources.contains_key(symbol) || self.accessors.contains_key(symbol)
                })
                && self.discover_escaped_whole(file, spread.argument, spread.span, allowed, result)
            {
                continue;
            }
            let Some(symbol) = inline_root.as_ref().or(argument) else {
                continue;
            };
            // ADR 0621: an object spread of a store reads the keys it holds, and
            // an array or call spread reads `length`: the store's own shape
            // proves either, and nothing else does. `{...s}` on `{}` is silent.
            if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
                if let Some((_, declaration)) = self.accessors.get(symbol) {
                    let shape = self.store_initial_shape(file, declaration);
                    if shape == StoreShape::Tuple {
                        continue;
                    }
                    let proven = !self.names_merge_view(file, spread.argument)
                        && if file.ast.iterated_operands.contains(&spread.argument) {
                            shape == StoreShape::Array
                        } else {
                            shape == StoreShape::Keyed
                        };
                    self.push_whole_store_read(
                        file,
                        spread.span,
                        symbol,
                        "spread",
                        proven,
                        allowed,
                        result,
                    );
                }
                continue;
            }
            let execution = read_execution_role(
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
                withdrawn_slot: false,
                pending_open: false,
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

    /// What a store's initial value proves about a whole-store read: a key
    /// read needs a key (rc.21 warns on `{...s}`, `Object.values(s)` only
    /// through a key the store holds), and an iteration needs an array to read
    /// `length`. The first argument of the exact store primitive in the
    /// declaring binding; anything else proves nothing.
    fn store_initial_shape(
        &self,
        file: &solid_facts::FileFacts,
        declaration: &Location,
    ) -> StoreShape {
        // A store of another module proves nothing here: an obligation.
        if declaration.path.as_ref() != file.path.as_str() {
            return StoreShape::Unknown;
        }
        let (Ok(start), Ok(end)) = (
            u32::try_from(declaration.start_byte),
            u32::try_from(declaration.end_byte),
        ) else {
            return StoreShape::Unknown;
        };
        let span = solid_facts::core::Span::new(start, end);
        let Some(binding) = file
            .ast
            .bindings
            .iter()
            .find(|binding| binding.names.iter().any(|name| name.span == span))
        else {
            return StoreShape::Unknown;
        };
        let Some(call) = binding
            .call_initializer
            .and_then(|initializer| file.ast.call_at(initializer))
        else {
            return StoreShape::Unknown;
        };
        if !matches!(
            self.lookup.primitive_at_call(file, call.span),
            Some(
                solid_dialect::Primitive::CreateStore
                    | solid_dialect::Primitive::CreateOptimisticStore
            )
        ) {
            return StoreShape::Unknown;
        }
        // `const pair = createStore(…)`: the `[store, setter]` tuple, no proxy.
        if binding.array_slots.is_empty() {
            return StoreShape::Tuple;
        }
        let Some(first) = call.arguments.first().filter(|argument| !argument.spread) else {
            return StoreShape::Unknown;
        };
        let value = file.ast.peel_ts_sugar_span(first.span);
        if file.ast.array_literals.contains(&value) {
            return StoreShape::Array;
        }
        file.ast
            .object_get_shapes
            .iter()
            .find(|shape| shape.span == value && shape.closed)
            .map_or(StoreShape::Unknown, |shape| {
                if shape.properties.is_empty() {
                    StoreShape::Empty
                } else {
                    StoreShape::Keyed
                }
            })
    }

    /// The first argument of the store primitive that creates the store
    /// declared at `declaration` in this file.
    fn store_initial_value(
        &self,
        file: &solid_facts::FileFacts,
        declaration: &Location,
    ) -> Option<solid_facts::core::Span> {
        if declaration.path.as_ref() != file.path.as_str() {
            return None;
        }
        let span = solid_facts::core::Span::new(
            u32::try_from(declaration.start_byte).ok()?,
            u32::try_from(declaration.end_byte).ok()?,
        );
        let call = file
            .ast
            .bindings
            .iter()
            .find(|binding| binding.names.iter().any(|name| name.span == span))?
            .call_initializer
            .and_then(|initializer| file.ast.call_at(initializer))?;
        call.arguments
            .first()
            .filter(|argument| !argument.spread)
            .map(|argument| file.ast.peel_ts_sugar_span(argument.span))
    }

    /// ADR 0634: the source's value may be raw, so a Get reads no proxy.
    fn store_may_be_raw(&self, symbol: &SymbolId) -> bool {
        self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store)
            && self.accessors.get(symbol).is_some_and(|(_, declaration)| {
                self.lookup
                    .file_by_path(declaration.path.as_ref())
                    .is_some_and(|file| {
                        file.ast
                            .bindings
                            .iter()
                            .find(|binding| {
                                binding.names.iter().any(|name| {
                                    u64::from(name.span.start) == declaration.start_byte
                                })
                            })
                            .is_some_and(|binding| {
                                self.lookup.store_value_may_be_raw(file, binding)
                            })
                    })
            })
    }

    /// ADR 0630: `value` is a `merge` view, whose keys and shape are its
    /// sources', and which may be its last source itself.
    fn names_merge_view(
        &self,
        file: &solid_facts::FileFacts,
        value: solid_facts::core::Span,
    ) -> bool {
        self.view_call(file, value).is_some_and(|call| {
            self.lookup.primitive_at_call(file, call.span) == Some(solid_dialect::Primitive::Merge)
        })
    }

    /// The store a whole-store use reads, when `span` denotes one exactly.
    fn whole_store_source(
        &self,
        file: &solid_facts::FileFacts,
        span: solid_facts::core::Span,
    ) -> Option<&SymbolId> {
        // The binder too: Type Facts keeps no entity at a `for…of` operand.
        let symbol =
            crate::source_discovery::reference_symbol(file, span, self.entities, self.lookup)?;
        let (symbol, _) = self.accessors.get_key_value(&symbol)?;
        (self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store)).then_some(symbol)
    }

    /// One read of a whole store at `span`, placed as a spread is.
    #[allow(clippy::too_many_arguments)]
    fn push_whole_store_read(
        &self,
        file: &solid_facts::FileFacts,
        span: solid_facts::core::Span,
        symbol: &SymbolId,
        form: &str,
        proven: bool,
        allowed: &[solid_facts::core::Span],
        result: &mut LocalAccessResult,
    ) {
        let Some((name, declaration)) = self.accessors.get(symbol) else {
            return;
        };
        let mut execution = read_execution_role(
            file,
            span,
            allowed,
            self.entities,
            self.symbol_names,
            self.lookup,
        );
        // ADR 0635: every trap of an uninitialized derive throws, so a read of
        // the whole store is the pending read.
        if self.async_sources.contains(symbol)
            && !self.effective_async_options(symbol).declared_loading
            && form != "value escaped from untrack"
        {
            self.push_member_async_read(
                file,
                span,
                form,
                symbol,
                (name, declaration),
                execution,
                result,
            );
            return;
        }
        // No summary describes these reads, so a closure no role places is
        // an obligation here, where a member's read is the summaries' (ADR
        // 0614), unless it is named at a tracked position.
        if inside_non_component_function(file, span, self.lookup)
            && named_callback_execution_role(file, span, self.lookup).is_none()
            && !matches!(
                execution,
                ExecutionRole::EffectApply
                    | ExecutionRole::UntrackedCallback
                    | ExecutionRole::UntrackedUnwindowed
            )
        {
            if crate::execution_role::named_at_tracked_position(file, span, self.lookup) {
                return;
            }
            execution = match handed_read_role(
                file,
                span,
                allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            ) {
                Some((role, Some(_))) if role.reports_untracked_read() => role,
                // ADR 0640: every run is in place, so the role where the layers
                // land is the read's: a handler or a continuation has no window.
                _ => self
                    .role_through_in_place_layers(file, span, allowed)
                    .filter(|role| !role.reports_untracked_read())
                    .unwrap_or(ExecutionRole::Unknown),
            };
        }
        result.reads.push(Arc::new(ReactiveRead {
            withdrawn_slot: false,
            pending_open: false,
            summary_attributed: false,
            transported: false,
            kind: "store-path".into(),
            accessor: format!("{name} {form}").into(),
            location: location(file.path.shared(), span),
            declaration: declaration.clone(),
            execution,
            context: read_analysis_context(file, span, execution, self.lookup).into(),
            via: Arc::from(""),
            origin: None,
            origin_context: Arc::from(""),
            // A pending derive throws here instead of warning.
            uncertain: !proven
                || self.store_may_be_raw(symbol)
                || self.async_sources.contains(symbol)
                || self.lookup.inside_possible_component(file, span),
            missing_jsx_census: missing_jsx_census(file, span, execution),
            host_callback_timing: host_callback_timing(file, span, execution, self.lookup),
            project_consumer_non_strict: false,
            callback_invocation_unproven: false,
            callee_callback_timing: callee_callback_timing(file, span, execution, self.lookup),
        }));
        if counts_as_strict_read_root(file, span, execution, self.lookup) {
            result.strict_read_obligations += 1;
        }
    }

    /// ADR 0641: `read` is written in what a component tag receives, its props or
    /// children, not a control-flow tag's, which the dialect runs in a render.
    fn read_in_component_prop(
        &self,
        file: &solid_facts::FileFacts,
        read: solid_facts::core::Span,
    ) -> bool {
        file.ast.jsx_containing(read).any(|element| {
            crate::owners::is_component_element(file, element)
                && crate::jsx_primitive_name(
                    file,
                    element,
                    self.entities,
                    self.symbol_names,
                    self.lookup.dialect,
                )
                .is_none()
        })
    }

    /// The role at the call that enters the layers around `span`, where each
    /// layer runs only in place: an IIFE, a helper's inline callback.
    fn role_through_in_place_layers(
        &self,
        file: &solid_facts::FileFacts,
        span: solid_facts::core::Span,
        allowed: &[solid_facts::core::Span],
    ) -> Option<ExecutionRole> {
        let mut at = span;
        for _ in 0..16 {
            if !inside_non_component_function(file, at, self.lookup) {
                break;
            }
            let layer = containing_ast_function(&file.ast, at)?;
            at = crate::execution_role::in_place_layer_call(file, layer, self.lookup)?;
        }
        (at != span).then(|| {
            read_execution_role(
                file,
                at,
                allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            )
        })
    }

    /// ADR 0635: `Object.keys(s)`, `k in s`, `for…in`, `hasOwn` and their kin
    /// read no key, so a settled store is silent (ADR 0621); an uninitialized
    /// derive's every trap holds its first answer, so they read pending.
    fn discover_key_probes(
        &self,
        file: &solid_facts::FileFacts,
        allowed: &[solid_facts::core::Span],
        result: &mut LocalAccessResult,
    ) {
        let mut sites = file
            .ast
            .key_probed_operands
            .iter()
            .map(|operand| (*operand, *operand))
            .collect::<Vec<_>>();
        for call in &file.ast.calls {
            let library = self
                .lookup
                .standard_library_declaration(file, call, typefacts::CallKind::Call)
                .map(|declaration| declaration.qualified_name.as_ref());
            if matches!(
                library,
                Some(
                    "ObjectConstructor.keys"
                        | "ObjectConstructor.hasOwn"
                        | "ObjectConstructor.getOwnPropertyNames"
                        | "Reflect.ownKeys"
                        | "Reflect.has"
                )
            ) && let Some(value) = call.arguments.first().filter(|value| !value.spread)
            {
                sites.push((value.span, call.span));
            }
        }
        for (value, site) in sites {
            let Some(symbol) = self.whole_store_source(file, value) else {
                continue;
            };
            if !self.async_sources.contains(symbol) {
                continue;
            }
            let Some((name, declaration)) = self.accessors.get(symbol) else {
                continue;
            };
            let execution = read_execution_role(
                file,
                site,
                allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            );
            self.push_member_async_read(
                file,
                site,
                "keys",
                symbol,
                (name, declaration),
                execution,
                result,
            );
        }
    }

    /// ADR 0633: `latest(m)` and `isPending(m)` call `m` in their own frame, a
    /// read like `latest(() => m())`, with no literal for a role to place.
    fn discover_named_probes(
        &self,
        file: &solid_facts::FileFacts,
        allowed: &[solid_facts::core::Span],
        result: &mut LocalAccessResult,
    ) {
        use solid_dialect::Primitive;
        for call in &file.ast.calls {
            if !matches!(
                self.lookup.primitive_at_call(file, call.span),
                Some(Primitive::Latest | Primitive::IsPending)
            ) {
                continue;
            }
            let Some(argument) = call.arguments.first().filter(|argument| !argument.spread) else {
                continue;
            };
            let value = file.ast.peel_ts_sugar_span(argument.span);
            if file.ast.function_at(value).is_some() {
                continue;
            }
            let execution = read_execution_role(
                file,
                call.span,
                allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            );
            if !execution.reports_untracked_read() {
                continue;
            }
            let symbol = self.entities.value_symbol(file, value);
            match symbol.and_then(|symbol| self.accessors.get(symbol).map(|row| (symbol, row))) {
                Some((symbol, (name, declaration)))
                    if self.source_kinds.get(symbol) != Some(&ReactiveSourceKind::Store) =>
                {
                    let call_location = location(file.path.shared(), call.span);
                    result.reads.push(Arc::new(ReactiveRead {
                        withdrawn_slot: false,
                        pending_open: false,
                        summary_attributed: false,
                        transported: false,
                        kind: "accessor".into(),
                        accessor: name.to_string().into(),
                        location: call_location,
                        declaration: declaration.clone(),
                        execution,
                        context: read_analysis_context(file, call.span, execution, self.lookup)
                            .into(),
                        via: Arc::from(""),
                        origin: None,
                        origin_context: Arc::from(""),
                        uncertain: self.lookup.inside_possible_component(file, call.span),
                        missing_jsx_census: missing_jsx_census(file, call.span, execution),
                        host_callback_timing: host_callback_timing(
                            file,
                            call.span,
                            execution,
                            self.lookup,
                        ),
                        project_consumer_non_strict: false,
                        callback_invocation_unproven: false,
                        callee_callback_timing: callee_callback_timing(
                            file,
                            call.span,
                            execution,
                            self.lookup,
                        ),
                    }));
                    if counts_as_strict_read_root(file, call.span, execution, self.lookup) {
                        result.strict_read_obligations += 1;
                    }
                }
                _ => result.dispatch_obligations.push(crate::StaticDefect {
                    kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                        callee: file.source_text(value).unwrap_or("this value").to_owned(),
                        member: None,
                    },
                    location: location(file.path.shared(), value),
                    analysis_context: UNRESOLVED_HANDED_READ.to_owned(),
                    fixes: vec![],
                    uncertain: true,
                }),
            }
        }
    }

    /// ADR 0620, 0621: the reads of a whole store no member or spread fact
    /// states, as rc.21 measures them in a strict-read window. `deep`, `JSON`'s
    /// `toJSON` probe, `Reflect.get` and `Array.from` read a key on any store;
    /// an iteration reads `length`, so it needs an array; `Object.values`,
    /// `entries` and `assign` read a key the store holds. `snapshot`,
    /// `Object.keys`, `in`, `hasOwn` and `for…in` read none.
    fn discover_whole_store_reads(
        &self,
        file: &solid_facts::FileFacts,
        allowed: &[solid_facts::core::Span],
        result: &mut LocalAccessResult,
    ) {
        self.discover_key_probes(file, allowed, result);
        self.discover_named_probes(file, allowed, result);
        // ADR 0629: `const { x } = untrack(() => s.o)` reads at the pattern, and
        // a value `untrack` returned handed to a function is read there.
        for binding in &file.ast.bindings {
            if matches!(
                binding.shape,
                solid_facts::ast::BindingShape::Object | solid_facts::ast::BindingShape::Array
            ) && !binding.names.is_empty()
                && let Some(initializer) = binding.initializer
            {
                self.discover_escaped_get(
                    file,
                    file.ast.peel_ts_sugar_span(initializer),
                    allowed,
                    result,
                );
            }
        }
        for call in &file.ast.calls {
            if call.arguments.is_empty() || self.lookup.primitive_at_call(file, call.span).is_some()
            {
                continue;
            }
            for argument in call.arguments.iter().filter(|argument| !argument.spread) {
                let value = file.ast.peel_ts_sugar_span(argument.span);
                if file
                    .ast
                    .identifiers
                    .iter()
                    .any(|identifier| identifier.span == value)
                    && matches!(
                        self.escaped_from_untrack(file, value),
                        Some(EscapedValue::Store(..) | EscapedValue::Unknown)
                    )
                    && read_execution_role(
                        file,
                        call.span,
                        allowed,
                        self.entities,
                        self.symbol_names,
                        self.lookup,
                    )
                    .reports_untracked_read()
                {
                    result.dispatch_obligations.push(crate::StaticDefect {
                        kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                            callee: file.source_text(value).unwrap_or("this value").to_owned(),
                            member: None,
                        },
                        location: location(file.path.shared(), value),
                        analysis_context: "value-escaped-from-untrack".to_owned(),
                        fixes: vec![],
                        uncertain: true,
                    });
                }
            }
        }
        for operand in &file.ast.iterated_operands {
            if file
                .ast
                .spreads
                .iter()
                .any(|spread| spread.argument == *operand)
            {
                continue;
            }
            let Some(symbol) = self.whole_store_source(file, *operand) else {
                self.discover_escaped_whole(file, *operand, *operand, allowed, result);
                continue;
            };
            let Some((_, declaration)) = self.accessors.get(symbol) else {
                continue;
            };
            let shape = self.store_initial_shape(file, declaration);
            if shape == StoreShape::Tuple {
                continue;
            }
            self.push_whole_store_read(
                file,
                *operand,
                symbol,
                "iterated",
                shape == StoreShape::Array && !self.names_merge_view(file, *operand),
                allowed,
                result,
            );
        }
        // A substitution reads `valueOf` or `toString` on any store; a tagged
        // template hands its substitutions to the tag instead.
        for template in &file.ast.template_literals {
            for operand in template
                .expressions
                .iter()
                .filter(|operand| file.ast.coercing_operands.binary_search(operand).is_ok())
            {
                if let Some(symbol) = self.whole_store_source(file, *operand) {
                    let proven = !self.names_merge_view(file, *operand);
                    self.push_whole_store_read(
                        file, *operand, symbol, "coerced", proven, allowed, result,
                    );
                } else {
                    self.discover_escaped_whole(file, *operand, *operand, allowed, result);
                }
            }
        }
        for call in &file.ast.calls {
            let library = self
                .lookup
                .standard_library_declaration(file, call, typefacts::CallKind::Call)
                .map(|declaration| declaration.qualified_name.as_ref());
            let (form, argument, needs_key) = if self.lookup.primitive_at_call(file, call.span)
                == Some(solid_dialect::Primitive::Deep)
            {
                ("deep", 0, false)
            } else {
                match library {
                    Some("JSON.stringify") => ("stringified", 0, false),
                    Some("Reflect.get") => ("read", 0, false),
                    Some("ArrayConstructor.from") => ("iterated", 0, false),
                    Some("ObjectConstructor.values") => ("values", 0, true),
                    Some("ObjectConstructor.entries") => ("entries", 0, true),
                    Some("ObjectConstructor.assign") => ("assigned", 1, true),
                    _ => continue,
                }
            };
            let Some(value) = call.arguments.get(argument).filter(|value| !value.spread) else {
                continue;
            };
            let Some(symbol) = self.whole_store_source(file, value.span) else {
                self.discover_escaped_whole(file, value.span, call.span, allowed, result);
                continue;
            };
            let Some((_, declaration)) = self.accessors.get(symbol) else {
                continue;
            };
            let shape = self.store_initial_shape(file, declaration);
            if shape == StoreShape::Tuple {
                continue;
            }
            let proven = !self.names_merge_view(file, value.span)
                && (!needs_key || matches!(shape, StoreShape::Keyed | StoreShape::Array));
            self.push_whole_store_read(file, call.span, symbol, form, proven, allowed, result);
        }
    }

    fn discover_dynamic_jsx(&self, file: &solid_facts::FileFacts, result: &mut LocalAccessResult) {
        for element in &file.ast.jsx_elements {
            let name_location = location(file.path.shared(), element.name.span);
            let Some(symbol) = self.entities.get(&name_location) else {
                continue;
            };
            let primitive = self.source_primitives.get(symbol).map(SymbolId::as_str);
            let lazy = primitive == Some("lazy");
            if !self.async_sources.contains(symbol) || (primitive != Some("dynamic") && !lazy) {
                continue;
            }
            let options = self.effective_async_options(symbol);
            let provenance_open = lazy || options.provenance_open;
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
                leaf_call_site_gates: gated_leaf_call_locations(
                    file,
                    element.name.span,
                    self.lookup,
                ),
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
                settle_unproven: self.lookup.module_census().may_settle_before_mount(),
                call_attributed: false,
                creation_pass_unproven: false,
                root_pass_unproven: false,
                carrier_run_unproven: false,
                apply_first_run_pending: false,
                pending_origin: None,
                pending_apply: None,
                async_provenance: true,
                declared_loading: false,
                options_opaque: provenance_open,
                provenance_open,
                caught: false,
                component_prop: false,
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
    target
        .static_violations
        .extend(source.static_violations.iter().cloned());
}

/// ADR 0516: whether a result reports nothing: no row a rule reports
/// (`projection.rs`'s predicates, without its exemptions) and no obligation
/// (the strict-read and write/action counts are metrics, not findings).
fn finding_free(result: &LocalAccessResult) -> bool {
    result
        .reads
        .iter()
        .all(|read| !read.execution.reports_untracked_read() && !read.execution.reports_unplaced())
        && result.writes.iter().all(|write| {
            !write.execution.reports_disallowed_write() && !write.execution.reports_unplaced()
        })
        && result.action_invocations.iter().all(|action| {
            !action.execution.reports_disallowed_write() && !action.execution.reports_unplaced()
        })
        && result.async_reads.is_empty()
        && result.dispatch_obligations.is_empty()
        && result.static_violations.is_empty()
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
    target.static_violations.extend(source.static_violations);
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

/// ADR 0516: whether a dispatch invokes its function whenever the call
/// runs. `f.call(…)` with no spread at the receiver's place, and
/// `f.bind(…)(…)`, do; `f.apply(t, x)` does where `x` is absent, `null`,
/// `undefined` or an array literal (another value may throw first).
pub(crate) fn dispatch_invokes(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
) -> bool {
    let ast = &file.ast;
    let Some(member) = ast.member_at(ast.peel_ts_sugar_span(call.callee)) else {
        return true;
    };
    match file.source_text(member.property) {
        Some("call") => !call
            .arguments
            .first()
            .is_some_and(|argument| argument.spread),
        Some("apply") => {
            !call
                .arguments
                .iter()
                .take(2)
                .any(|argument| argument.spread)
                && call.arguments.get(1).is_none_or(|argument| {
                    matches!(
                        argument.value,
                        solid_facts::ast::ArgumentValueKind::Null
                            | solid_facts::ast::ArgumentValueKind::Undefined
                    ) || ast
                        .array_literal_at(ast.peel_ts_sugar_span(argument.span))
                        .is_some()
                })
        }
        _ => false,
    }
}

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

/// A direct hand-off that no proof follows, including one around a nested
/// literal whose run has not been attributed through the handed function.
/// ADR 0535: whether `span` is in a named non-component function that no
/// named-callback role places, nested where `inside_non_component_function`
/// answers for the primitive's callback around it.
pub(crate) fn nested_named_function(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    named_callback_execution_role(file, span, lookup).is_none()
        && containing_ast_function(&file.ast, span).is_some_and(|function| {
            !lookup.function_may_be_component(file, function)
                && function_binding_name(file, function).is_some()
        })
}

/// ADR 0535: whether `span` is in a non-component function written as a
/// member's value (`this.m = () => …`), wherever
/// `inside_non_component_function` answers for a primitive's callback around
/// it.
pub(crate) fn escapes_nested(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    containing_ast_function(&file.ast, span).is_some_and(|function| {
        !lookup.function_may_be_component(file, function) && member_assigned(file, function)
    })
}

/// ADR 0580: how many named functions [`carried_leaf_owner`] follows out
/// through their inline carriers.
const CARRIED_LEAF_DEPTH: usize = 4;

/// ADR 0646: the function around `span` runs only through inline carriers, and
/// none is proven to run it (`const hf = () => count(); [].forEach(hf);`). A
/// read there may never run, so it proves nothing (ADR 0563's rule for a
/// literal, ADR 0566's for a read after an await).
pub(crate) fn named_carrier_may_not_run(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    containing_ast_function(&file.ast, span).is_some_and(|function| {
        let carriers = inline_carriers(file, function, lookup);
        !carriers.is_empty()
            && !carriers
                .iter()
                .any(|carrier| crate::carrier_runs::carrier_provably_runs(file, *carrier, lookup))
    })
}

/// ADR 0580: the leaf owner a read at `span` runs under, and whether a
/// carrier on the way may run it zero times. A read written in a named
/// function its binding hands to inline carriers (`[1].forEach(hf)`,
/// `hf.call(null)`, `` hf`x` ``) runs where those carriers run, so the
/// first carrier under a leaf owner answers, its run decided by ADR 0563's
/// [`crate::carrier_runs::carrier_provably_runs`]: a named callback the
/// function is nested in (`onSettled(cb)`, whose leaf owner the read's own
/// position never shows) is found, and a carrier that may run it zero
/// times (`[].forEach(hf)` inside an `onSettled` literal) proves nothing.
/// Otherwise the read's own position answers ([`containing_leaf_owner`]).
fn carried_leaf_owner(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    depth: usize,
) -> (Option<String>, bool) {
    if depth > 0
        && let Some(function) = containing_ast_function(&file.ast, span)
        && !function.generator
        && !lookup.function_may_be_component(file, function)
    {
        for carrier in inline_carriers(file, function, lookup) {
            if let (Some(owner), unproven) =
                carried_leaf_owner(file, carrier, entities, symbol_names, lookup, depth - 1)
            {
                return (
                    Some(owner),
                    unproven || !crate::carrier_runs::carrier_provably_runs(file, carrier, lookup),
                );
            }
        }
    }
    (
        containing_leaf_owner(file, span, entities, symbol_names, lookup),
        false,
    )
}

/// The calls a read at `span` owes its leaf owner to whose callback is a leaf
/// only under an owned call site (`onSettled`, the dialect's
/// `leaf_owner_requires_owned_call_site`), along the walk
/// [`carried_leaf_owner`] takes. Their spans key the owner fixed point's
/// call-site gates.
fn gated_leaf_calls(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    lookup: &SemanticLookup<'_>,
    depth: usize,
) -> Vec<solid_facts::core::Span> {
    let mut calls = crate::owners::gated_leaf_calls_at(file, span, lookup);
    if depth > 0
        && let Some(function) = containing_ast_function(&file.ast, span)
        && !function.generator
        && !lookup.function_may_be_component(file, function)
    {
        for carrier in inline_carriers(file, function, lookup) {
            calls.extend(gated_leaf_calls(file, carrier, lookup, depth - 1));
        }
    }
    calls
}

/// [`gated_leaf_calls`] as the locations a read carries to the gates.
fn gated_leaf_call_locations(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    lookup: &SemanticLookup<'_>,
) -> Vec<Location> {
    gated_leaf_calls(file, span, lookup, CARRIED_LEAF_DEPTH)
        .into_iter()
        .map(|call| location(file.path.shared(), call))
        .collect()
}

/// ADR 0537: the inline carriers `function`'s binding is handed to by name
/// (ADR 0531's `HandOff::Inline`): the calls that run it before returning.
pub(crate) fn inline_carriers(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Vec<solid_facts::core::Span> {
    let named_expression =
        function.kind == solid_facts::ast::FunctionKind::Expression && function.name.is_some();
    let name = if named_expression {
        crate::execution_role::named_expression_binding(file, function, lookup)
    } else {
        crate::owners::function_binding_name(file, function)
    };
    let Some(name) = name else {
        return Vec::new();
    };
    let mut hand_offs = Vec::new();
    binding_hand_offs(file, name.span, lookup, &mut hand_offs);
    hand_offs
        .into_iter()
        .filter_map(|hand_off| match hand_off {
            HandOff::Inline(at) => Some(at),
            _ => None,
        })
        .collect()
}

/// ADR 0585 (merge with ADR 0580): [`inline_carriers`] when they are the only
/// hand-offs of `function`'s binding (no copy, no project or unknown call, no
/// host that keeps it) and it is no member's or object slot's value; empty
/// otherwise.
pub(crate) fn only_inline_carriers(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Vec<solid_facts::core::Span> {
    if function.generator
        || function.r#async
        || member_assigned(file, function)
        || object_slot_value(file, function)
    {
        return Vec::new();
    }
    let named_expression =
        function.kind == solid_facts::ast::FunctionKind::Expression && function.name.is_some();
    let name = if named_expression {
        crate::execution_role::named_expression_binding(file, function, lookup)
    } else {
        crate::owners::function_binding_name(file, function)
    };
    let Some(name) = name else {
        return Vec::new();
    };
    let mut hand_offs = Vec::new();
    binding_hand_offs(file, name.span, lookup, &mut hand_offs);
    let mut carriers = Vec::new();
    for hand_off in hand_offs {
        match hand_off {
            HandOff::Inline(at) => carriers.push(at),
            _ => return Vec::new(),
        }
    }
    carriers
}

/// ADR 0535: whether the named function around `span` is handed whole to
/// project code or an unresolved call (`run(hf)`, `box.forEach(hf)`), which
/// may run it anywhere, or written as a member's value. A host that keeps it
/// (`addEventListener`) runs it later, as the lexical role already says.
pub(crate) fn handed_to_unknown_code(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(function) = containing_ast_function(&file.ast, span) else {
        return false;
    };
    if member_assigned(file, function) {
        return true;
    }
    let named_expression =
        function.kind == solid_facts::ast::FunctionKind::Expression && function.name.is_some();
    let name = if named_expression {
        crate::execution_role::named_expression_binding(file, function, lookup)
    } else {
        crate::owners::function_binding_name(file, function)
    };
    name.is_some_and(|name| {
        let mut hand_offs = Vec::new();
        binding_hand_offs(file, name.span, lookup, &mut hand_offs);
        hand_offs.contains(&HandOff::Unfollowed)
    })
}

/// ADR 0535: whether `function` is written as a member's value (`o.m = () =>
/// …`, `this.m = …`): it runs wherever that member is called.
pub(crate) fn member_assigned(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    file.ast.assignments.iter().any(|assignment| {
        assignment.value == solid_facts::ast::AssignmentValueKind::Function
            && file.ast.peel_ts_sugar_span(assignment.value_span) == function.span
            && file.ast.member_at(assignment.target).is_some()
    })
}

/// ADR 0538: whether `function` is the value of an object literal's data
/// property (`{ m: () => … }`): it runs wherever the object's member is
/// called, which the receiver-less walks do not follow.
pub(crate) fn object_slot_value(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> bool {
    file.ast.object_properties.iter().any(|property| {
        property.data && file.ast.peel_ts_sugar_span(property.value) == function.span
    })
}

/// ADR 0538: whether `function`'s run is one the step and effect-phase walks
/// cannot place: written as a member's value or an object slot, or its binding
/// handed whole to project code or an unresolved call
/// ([`handed_to_unknown_code`]). A hand-off to a fresh-stack scheduler or a
/// host that keeps the listener is placed (it runs elsewhere) and is not this.
pub(crate) fn run_unplaced(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    object_slot_value(file, function) || handed_to_unknown_code(file, function.body, lookup)
}

/// ADR 0673: a read no role and no call edge places. A hidden call site (a
/// getter, `toString`, an iterator, a tag, a field) has no call node, so this
/// is decided from the function, and from the functions an exact call of the
/// named helper around `span` sits in. `inside_class_member` stays beside
/// [`crate::static_api::runs_unplaced_exhaustive`]: it has a climb that
/// reports exhaustion.
fn placed_nowhere(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    function_escapes_unfollowed(file, span, lookup)
        || crate::execution_role::inside_class_member(file, span, lookup)
        || crate::static_api::runs_unplaced_exhaustive(file, span, lookup, false)
}

pub(crate) fn function_escapes_unfollowed(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let Some(inner) = containing_ast_function(&file.ast, span) else {
        return false;
    };
    if function_handed_unfollowed(file, inner, lookup) {
        return true;
    }
    file.ast.functions.iter().any(|outer| {
        if outer.span == inner.span
            || !outer.span.contains(inner.span)
            || lookup.function_may_be_component(file, outer)
        {
            return false;
        }
        let Some(name) = crate::owners::function_binding_name(file, outer) else {
            return false;
        };
        let mut hand_offs = Vec::new();
        binding_hand_offs(file, name.span, lookup, &mut hand_offs);
        hand_offs.iter().any(|hand_off| {
            matches!(
                hand_off,
                HandOff::Inline(_) | HandOff::Unfollowed | HandOff::Retained
            )
        })
    })
}

/// The escape test of [`function_escapes_unfollowed`] for `function` itself.
fn function_handed_unfollowed(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> bool {
    use solid_facts::core::Span;
    let ast = &file.ast;
    let path = file.path.as_str();
    // A call's whole argument (under transparent TypeScript wrappers).
    let whole_argument = |at: Span| {
        ast.arguments_containing(at)
            .any(|(call, index)| ast.peel_ts_sugar_span(call.arguments[index].span) == at)
    };
    let references = |name: Span| -> Vec<Span> {
        lookup
            .entities()
            .exact(&location(file.path.shared(), name))
            .map(|symbol| {
                lookup
                    .symbol_references(symbol.as_str())
                    .iter()
                    .filter(|reference| reference.path.as_ref() == path)
                    .filter_map(|reference| {
                        Some(Span::new(
                            u32::try_from(reference.start_byte).ok()?,
                            u32::try_from(reference.end_byte).ok()?,
                        ))
                    })
                    .filter(|at| *at != name)
                    .collect()
            })
            .unwrap_or_default()
    };
    // A value written in an object literal bound to one `const` (`const box =
    // { cb: f }`), whose slot is read and handed on as a call's whole
    // argument (`setTimeout(box.cb)`): the call runs it where nothing follows.
    let slot_handed = |value: Span| {
        let Some(property) = ast
            .object_properties
            .iter()
            .find(|property| ast.peel_ts_sugar_span(property.value) == value)
        else {
            return false;
        };
        let Some(key) = file.source_text(property.key) else {
            return false;
        };
        ast.bindings
            .iter()
            .filter(|binding| {
                binding.immutable
                    && binding
                        .initializer
                        .is_some_and(|initializer| initializer.contains(property.span))
            })
            .any(|binding| {
                let [holder] = binding.names.as_slice() else {
                    return false;
                };
                references(holder.span).into_iter().any(|object| {
                    ast.members.iter().any(|member| {
                        member.object == object
                            && file.source_text(member.property) == Some(key)
                            && whole_argument(member.span)
                    })
                })
            })
    };
    // ADR 0535: a function written as a member's value (`o.m = () => …`,
    // `this.m = …`) runs wherever that member is called, which no summary
    // follows: handed where nothing places it.
    if member_assigned(file, function) {
        return true;
    }
    if slot_handed(function.span) {
        return true;
    }
    // A named function expression is held by its `const` (its own name is
    // scoped inside it); any other function by its binding.
    let named_expression =
        function.kind == solid_facts::ast::FunctionKind::Expression && function.name.is_some();
    let name = if named_expression {
        crate::execution_role::named_expression_binding(file, function, lookup)
    } else {
        crate::owners::function_binding_name(file, function)
    };
    let Some(name) = name else {
        return false;
    };
    // ADR 0531: every reference of the binding that hands the function to a
    // call this does not follow (`hand_off_at`: a primitive position the role
    // index does not admit, a project helper's argument, a deferring or
    // retaining standard-library call, a slot of a literal argument) hands it
    // to code nothing places. A `const` copy is followed into its own
    // references; an object slot handed on through a `const` holder as before.
    let mut hand_offs = Vec::new();
    binding_hand_offs(file, name.span, lookup, &mut hand_offs);
    hand_offs.contains(&HandOff::Unfollowed)
        || hand_offs.contains(&HandOff::Retained)
        || references(name.span).into_iter().any(slot_handed)
}

/// ADR 0531: how one reference of a function's binding hands the function
/// over, read from the reference's own position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HandOff {
    /// A direct call of the binding (`f()`, `new F()`): the summaries
    /// attribute the body's reads at the call.
    Call,
    /// Run before the carrier returns, on the carrier's stack: the whole
    /// argument a standard-library call invokes inline
    /// ([`crate::runtime_semantics::argument_behavior`]: `forEach`, `sort`,
    /// `Array.from`'s map function, `JSON.parse`'s reviver, a `Promise`
    /// executor, `Reflect.apply`'s target), a tagged template's tag, or
    /// `Function.prototype.call`/`apply`'s receiver where the dispatch invokes
    /// it. Carries the carrier's span: the role of that position is the run's.
    Inline(solid_facts::core::Span),
    /// Handed to a reviewed fresh-stack scheduler (`setTimeout(f)`,
    /// `promise.then(f)`): runs after the call, with no strict-read window.
    FreshStack,
    /// A standard-library call the table proves does not invoke the argument
    /// (`removeEventListener(type, f)`).
    Value,
    /// The whole argument of a standard-library call that may keep or defer
    /// it (`addEventListener`, `Set.add`): unfollowed too, but run by the
    /// host later, never by project code (ADR 0535).
    Retained,
    /// Handed to a call nothing here follows: the whole argument of a project
    /// call or of an unresolved one, a part of a
    /// value an argument builds (`run([f])`, `defineProperty(o, "v", { get: f
    /// })`), `f.bind(…)`, a spread.
    Unfollowed,
}

/// The hand-offs of every same-file reference of the binding named at `name`
/// ([`HandOff`]); copies remain obligations.
pub(crate) fn binding_hand_offs(
    file: &solid_facts::FileFacts,
    name: solid_facts::core::Span,
    lookup: &SemanticLookup<'_>,
    out: &mut Vec<HandOff>,
) {
    let mut placed = Vec::new();
    binding_reference_hand_offs(file, name, lookup, &mut placed);
    out.extend(placed.into_iter().map(|(_, hand_off)| hand_off));
}

/// ADR 0565: the hand-off of every same-file reference of `function`'s
/// binding (a named function expression's `const`, any other function's own
/// binding), with the reference's span. `None` when it has no binding.
pub(crate) fn function_reference_hand_offs(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'_>,
) -> Option<Vec<(solid_facts::core::Span, HandOff)>> {
    let named_expression =
        function.kind == solid_facts::ast::FunctionKind::Expression && function.name.is_some();
    let name = if named_expression {
        crate::execution_role::named_expression_binding(file, function, lookup)
    } else {
        crate::owners::function_binding_name(file, function)
    }?;
    let mut placed = Vec::new();
    binding_reference_hand_offs(file, name.span, lookup, &mut placed);
    Some(placed)
}

/// ADR 0674: the hand-off of every reference of `function`'s binding in the
/// other files of the project, with its file: a reference there is no call
/// edge of this function when it hands the function on (`[1].map(h)`,
/// `h.call(null)`, a tag, a `const` copy). An import's own name is followed
/// to the uses of the local binding; a re-export hands nothing over.
pub(crate) fn foreign_reference_hand_offs<'s, 'f: 's>(
    file: &'s solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    lookup: &SemanticLookup<'f>,
) -> Vec<(&'f solid_facts::FileFacts, solid_facts::core::Span, HandOff)> {
    use solid_facts::core::Span;
    let named_expression =
        function.kind == solid_facts::ast::FunctionKind::Expression && function.name.is_some();
    let name = if named_expression {
        crate::execution_role::named_expression_binding(file, function, lookup)
    } else {
        crate::owners::function_binding_name(file, function)
    };
    let Some(name) = name else {
        return Vec::new();
    };
    let Some(symbol) = lookup
        .entities()
        .exact(&location(file.path.shared(), name.span))
    else {
        return Vec::new();
    };
    let accounted = lookup.function_call_site_references(file.path.as_str(), function.span);
    let mut out = Vec::new();
    for reference in lookup.symbol_references(symbol.as_str()).iter() {
        if reference.path.as_ref() == file.path.as_str() {
            continue;
        }
        let Some(holder) = lookup.file_by_path(reference.path.as_ref()) else {
            continue;
        };
        let (Ok(start), Ok(end)) = (
            u32::try_from(reference.start_byte),
            u32::try_from(reference.end_byte),
        ) else {
            continue;
        };
        let at = Span::new(start, end);
        // A call or a render the call graph holds.
        if accounted
            .iter()
            .any(|(site, span)| site.path == holder.path && *span == at)
        {
            continue;
        }
        let ast = &holder.ast;
        if ast
            .exports
            .iter()
            .flat_map(|export| &export.specifiers)
            .any(|specifier| specifier.local.span == at)
        {
            continue;
        }
        if ast
            .imports
            .iter()
            .flat_map(|import| &import.bindings)
            .any(|binding| binding.local.span == at)
        {
            let mut placed = Vec::new();
            binding_reference_hand_offs(holder, at, lookup, &mut placed);
            out.extend(
                placed
                    .into_iter()
                    .map(|(at, hand_off)| (holder, at, hand_off)),
            );
            continue;
        }
        if ast.bindings.iter().any(|binding| {
            binding
                .initializer_identifier
                .as_ref()
                .is_some_and(|identifier| identifier.span == at)
        }) {
            out.push((holder, at, HandOff::Unfollowed));
            continue;
        }
        out.push((holder, at, hand_off_at(holder, at, lookup)));
    }
    out
}

/// [`binding_hand_offs`] with each reference's span.
fn binding_reference_hand_offs(
    file: &solid_facts::FileFacts,
    name: solid_facts::core::Span,
    lookup: &SemanticLookup<'_>,
    out: &mut Vec<(solid_facts::core::Span, HandOff)>,
) {
    use solid_facts::core::Span;
    let path = file.path.as_str();
    let references = lookup
        .entities()
        .exact(&location(file.path.shared(), name))
        .map(|symbol| {
            lookup
                .symbol_references(symbol.as_str())
                .iter()
                .filter(|reference| reference.path.as_ref() == path)
                .filter_map(|reference| {
                    Some(Span::new(
                        u32::try_from(reference.start_byte).ok()?,
                        u32::try_from(reference.end_byte).ok()?,
                    ))
                })
                .filter(|at| *at != name)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for at in references {
        // Copies and other initializer positions store the function; the
        // direct-binding proof does not follow their invocations.
        if file.ast.bindings.iter().any(|binding| {
            binding
                .initializer_identifier
                .as_ref()
                .is_some_and(|identifier| identifier.span == at)
        }) {
            out.push((at, HandOff::Unfollowed));
            continue;
        }
        out.push((at, hand_off_at(file, at, lookup)));
    }
}

/// [`HandOff`] of the reference at `at`. Every carrier is the compiler-selected
/// standard-library declaration's, never a spelling: a local `forEach` or a
/// project helper's parameter is `Unfollowed`.
pub(crate) fn hand_off_at(
    file: &solid_facts::FileFacts,
    at: solid_facts::core::Span,
    lookup: &SemanticLookup<'_>,
) -> HandOff {
    use crate::runtime_semantics::{RuntimeArgumentBehavior, argument_behavior};
    let ast = &file.ast;
    // ADR 0674: `ReturnType<typeof useThing>` is erased: nothing runs there.
    if ast.type_queries.iter().any(|query| query.contains(at)) {
        return HandOff::Value;
    }
    let call_with_callee = |callee: solid_facts::core::Span| {
        lookup
            .call_by_callee(file, callee)
            .or_else(|| ast.calls_with_peeled_callee(callee).next())
    };
    if call_with_callee(at).is_some_and(|call| call.direct_callee) {
        return HandOff::Call;
    }
    if let Some(template) = ast
        .tagged_templates
        .iter()
        .find(|template| template.tag == at)
    {
        return HandOff::Inline(template.span);
    }
    if let Some((call, index)) = ast
        .arguments_containing(at)
        .find(|(call, index)| ast.peel_ts_sugar_span(call.arguments[*index].span) == at)
    {
        // A primitive position is the role index's (`named_callback_execution_role`).
        if call.arguments[index].spread || lookup.primitive_at_call(file, call.span).is_some() {
            return HandOff::Unfollowed;
        }
        let Some(resolved) = lookup.resolved_callee_call(file, call.callee) else {
            return HandOff::Unfollowed;
        };
        let callability = lookup
            .entity_at(file.path.as_str(), call.arguments[index].span)
            .and_then(|entity| entity.callability);
        return match argument_behavior(resolved, callability, index) {
            Some(RuntimeArgumentBehavior::InlineCallback) => HandOff::Inline(call.span),
            Some(RuntimeArgumentBehavior::FreshStackCallback) => HandOff::FreshStack,
            Some(RuntimeArgumentBehavior::ValueOnly) => HandOff::Value,
            Some(
                RuntimeArgumentBehavior::DeferredCallback | RuntimeArgumentBehavior::RetainedValue,
            ) => HandOff::Retained,
            // Every DOM redeclaration of `addEventListener` registers its
            // listener the same way (`runs_on_invoker_stack`).
            None if crate::runtime_semantics::runs_on_invoker_stack(
                resolved,
                callability,
                index,
            ) =>
            {
                HandOff::Retained
            }
            // ...and every redeclaration of `removeEventListener` only removes
            // it: argument 1 of a default-library declaration of that name.
            None if index == 1
                && resolved.declaration.as_ref().is_some_and(|declaration| {
                    declaration.standard_library
                        && declaration.qualified_name.rsplit_once('.').is_some_and(
                            |(owner, member)| !owner.is_empty() && member == "removeEventListener",
                        )
                }) =>
            {
                HandOff::Value
            }
            None => HandOff::Unfollowed,
        };
    }
    // `f.call(…)`, `f.apply(…)`: `Function.prototype`'s run the receiver
    // before they return (as `call_edge_symbol` reads them for owners). Any
    // other member of the function (`f.bind(…)`, `f.length`) is not followed.
    if let Some(member) = ast
        .members
        .iter()
        .find(|member| ast.peel_ts_sugar_span(member.object) == at)
    {
        let invoking_call = call_with_callee(member.span).filter(|call| {
            crate::indexes::standard_library_declaration_of(
                lookup.resolved_callee_call(file, call.callee),
                typefacts::CallKind::Call,
            )
            .is_some_and(|declaration| {
                matches!(
                    declaration.qualified_name.as_ref(),
                    "Function.call"
                        | "Function.apply"
                        | "CallableFunction.call"
                        | "CallableFunction.apply"
                        | "NewableFunction.call"
                        | "NewableFunction.apply"
                )
            }) && dispatch_invokes(file, call)
        });
        return match invoking_call {
            Some(call) => HandOff::Inline(call.span),
            None => HandOff::Unfollowed,
        };
    }
    // A part of a value an argument builds (`run([f])`, `run(cond ? f : g)`,
    // `defineProperty(o, "v", { get: f })`): handed to the call. A reference
    // inside a function literal written in an argument is that literal's
    // business, not the call's.
    let function_around = |span: solid_facts::core::Span| {
        crate::owners::containing_ast_function(ast, span).map(|function| function.span)
    };
    if ast
        .arguments_containing(at)
        .any(|(call, _)| function_around(call.span) == function_around(at))
    {
        return HandOff::Unfollowed;
    }
    HandOff::Unfollowed
}

/// ADR 0531: the role of a read written directly in a named function, placed
/// by the carriers its binding is handed to ([`binding_hand_offs`]). `None`
/// when no reference is a carrier (the other arms decide, as before). An
/// inline carrier at a position that reports untracked reads proves the read
/// runs there: that role. Otherwise an unfollowed reference, or a carrier at
/// an unplaced position, leaves the read unplaced (`Unknown`); a function only
/// scheduled on fresh stacks or run at non-reporting positions takes the first
/// such role, which reports nothing. A proven role comes with the carrier's
/// span: the read's timing is the carrier's ([`push_accessor_read_placed`]).
pub(crate) fn handed_read_role(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    allowed: &[solid_facts::core::Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> Option<(ExecutionRole, Option<solid_facts::core::Span>)> {
    handed_role(
        file,
        span,
        lookup,
        false,
        |at| read_execution_role(file, at, allowed, entities, symbol_names, lookup),
        ExecutionRole::reports_untracked_read,
    )
}

/// ADR 0563: [`handed_read_role`] for an async source read. Each finding about
/// it (a pending read that throws, a missing boundary) is a claim about a read
/// that happens, so only a carrier that provably runs the function
/// ([`crate::carrier_runs::carrier_provably_runs`]) proves its role; one that
/// may run it zero times at a reporting position leaves the read unplaced (an
/// obligation). SC1001's strict reads keep [`handed_read_role`], whose
/// zero-run trade `docs/open-work.md` records.
pub(crate) fn handed_async_read_role(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    allowed: &[solid_facts::core::Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> Option<(ExecutionRole, Option<solid_facts::core::Span>)> {
    handed_role(
        file,
        span,
        lookup,
        true,
        |at| read_execution_role(file, at, allowed, entities, symbol_names, lookup),
        ExecutionRole::reports_untracked_read,
    )
}

/// ADR 0776: [`handed_async_read_role`] for a source that may be pending: a
/// tracked carrier places it too, where SC5003 asks about it.
pub(crate) fn handed_pending_read_role(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    allowed: &[solid_facts::core::Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> Option<(ExecutionRole, Option<solid_facts::core::Span>)> {
    handed_role(
        file,
        span,
        lookup,
        true,
        |at| {
            if empty_literal_carrier(file, at) {
                return ExecutionRole::DiscardedRendering;
            }
            async_execution_role(
                file,
                at,
                read_execution_role(file, at, allowed, entities, symbol_names, lookup),
            )
        },
        |role| role.reports_untracked_read() || role == ExecutionRole::TrackedJsx,
    )
}

/// `[].map(f)`: the carrier at `at` is a method of an empty array literal.
fn empty_literal_carrier(file: &solid_facts::FileFacts, at: solid_facts::core::Span) -> bool {
    let ast = &file.ast;
    ast.call_at(at)
        .and_then(|call| ast.member_at(ast.peel_ts_sugar_span(call.callee)))
        .and_then(|member| ast.array_literal_at(ast.peel_ts_sugar_span(member.object)))
        .is_some_and(|literal| literal.elements.is_empty())
}

/// ADR 0535: a write or an action call in a named function handed by name
/// to an inline carrier runs at the carrier; an unplaced hand-off, or a
/// function written as a member's value, is an obligation. A role already
/// proving the write stands.
#[allow(clippy::too_many_arguments)]
pub(crate) fn handed_write_execution(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    allowed: &[solid_facts::core::Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
    execution: ExecutionRole,
) -> ExecutionRole {
    let mut walk = HelperWriteWalk {
        visiting: HashSet::default(),
        depth: HELPER_WRITE_DEPTH,
        memo: (!crate::execution_role::every_path()).then(HelperWalkMemo::default),
        steps: 0,
    };
    let role = handed_write_execution_within(
        file,
        span,
        allowed,
        entities,
        symbol_names,
        lookup,
        root_body,
        execution,
        &mut walk,
    );
    lookup.note_bounded_walk(walk.steps, walk.exhausted());
    // ADR 0560: a climb that followed its paths past the bound answers
    // nothing it found, and no placed hand-off replaces that.
    if walk.exhausted() {
        ExecutionRole::Unknown
    } else {
        role
    }
}

/// ADR 0537: how many helper calls [`helper_call_write_execution`] follows
/// outward from a write before the climb is exhausted (ADR 0532: unplaced).
const HELPER_WRITE_DEPTH: usize = 8;

/// The state of one [`helper_call_write_execution`] climb: the helpers being
/// placed, and the calls left to follow.
struct HelperWriteWalk {
    visiting: HashSet<(String, solid_facts::core::Span)>,
    depth: usize,
    /// ADR 0546: what the climb keeps (none with the role log on, under a
    /// probe or a proof's deferred check: the climb then follows every path
    /// as it is written).
    memo: Option<HelperWalkMemo>,
    /// ADR 0560: the calls the climb followed along its paths
    /// ([`Self::exhausted`]).
    steps: u64,
}

impl HelperWriteWalk {
    /// ADR 0560: whether the climb followed more calls along its paths than
    /// the bound ([`crate::execution_role::walk_bound`]): it then ends, and
    /// its root answers `Unknown`. A climb that rejects descends by
    /// [`first_rejection`], whose work is polynomial; one that follows its
    /// paths as written (the role log on, a probe, a proof's deferred
    /// check) or finds no rejection follows calls one by one.
    fn exhausted(&self) -> bool {
        self.steps > crate::execution_role::walk_bound()
    }
}

/// ADR 0546: a call's own answer ([`handed_tail`]) is the same on every path
/// that reaches it, so the climb computes it once; and where no call the
/// climb can reach has a rejecting one, no fold can reject, so a fold is
/// unplaced as soon as one entry is and stops there (`quick`).
#[derive(Default)]
struct HelperWalkMemo {
    tails: HashMap<(String, solid_facts::core::Span), Option<ExecutionRole>>,
    /// Decided when the climb's first helper is entered.
    quick: Option<bool>,
}

/// What [`climb_rejects_nowhere`] found of the calls a climb can reach.
enum ClimbBall {
    /// A call's own answer rejects the write.
    Rejects,
    /// None rejects: whether one is unplaced, and how many helpers with a
    /// call the climb can enter.
    Quiet { unplaced: bool, called: usize },
}

#[allow(clippy::too_many_arguments)]
fn handed_write_execution_within(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    allowed: &[solid_facts::core::Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
    execution: ExecutionRole,
    walk: &mut HelperWriteWalk,
) -> ExecutionRole {
    if execution.reports_disallowed_write() {
        return execution;
    }
    let execution = match helper_call_write_execution(
        file,
        span,
        entities,
        symbol_names,
        lookup,
        root_body,
        walk,
    ) {
        Some(role) if role.reports_disallowed_write() => return role,
        _ if walk.exhausted() => return ExecutionRole::Unknown,
        Some(_) => ExecutionRole::Unknown,
        None => execution,
    };
    handed_tail(
        file,
        span,
        allowed,
        entities,
        symbol_names,
        lookup,
        root_body,
    )
    .unwrap_or(execution)
}

/// What [`handed_write_execution_within`] answers at `span` whatever its
/// helpers' climb found: a placed hand-off's role, `Unknown` for an escape,
/// none where the climb's answer stands.
#[allow(clippy::too_many_arguments)]
fn handed_tail(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    allowed: &[solid_facts::core::Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
) -> Option<ExecutionRole> {
    if (inside_non_component_function(file, span, lookup)
        || nested_named_function(file, span, lookup))
        && let Some((role, carrier)) = handed_write_role(
            file,
            span,
            allowed,
            entities,
            symbol_names,
            lookup,
            root_body,
        )
        // Outside the blind spot the write walk follows the other hand-offs
        // itself: only a placed carrier adds to it there.
        && (carrier.is_some()
            || (role == ExecutionRole::Unknown
                && !inside_non_component_function(file, span, lookup)))
    {
        return Some(role);
    }
    if escapes_nested(file, span, lookup)
        // Only where `inside_non_component_function` answers for a primitive's
        // callback around the function (the summaries follow the rest).
        || (!inside_non_component_function(file, span, lookup)
            && nested_named_function(file, span, lookup)
            && handed_to_unknown_code(file, span, lookup))
    {
        return Some(ExecutionRole::Unknown);
    }
    None
}

/// [`handed_write_execution_within`] at a helper's call site `callee`, as
/// the climb asks it (the caller's own allowed callbacks, no role yet); the
/// call's [`handed_tail`] is computed once per climb.
fn handed_call_role(
    caller_file: &solid_facts::FileFacts,
    callee: solid_facts::core::Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
    walk: &mut HelperWriteWalk,
) -> ExecutionRole {
    let execution = match helper_call_write_execution(
        caller_file,
        callee,
        entities,
        symbol_names,
        lookup,
        root_body,
        walk,
    ) {
        Some(role) if role.reports_disallowed_write() => return role,
        Some(_) => ExecutionRole::Unknown,
        None => ExecutionRole::Unreported,
    };
    climb_tail(
        caller_file,
        callee,
        entities,
        symbol_names,
        lookup,
        root_body,
        walk,
    )
    .unwrap_or(execution)
}

/// The [`handed_tail`] of a call the climb reaches, kept for the climb.
fn climb_tail(
    caller_file: &solid_facts::FileFacts,
    callee: solid_facts::core::Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
    walk: &mut HelperWriteWalk,
) -> Option<ExecutionRole> {
    let key = (caller_file.path.to_string(), callee);
    if let Some(tail) = walk.memo.as_ref().and_then(|memo| memo.tails.get(&key)) {
        return *tail;
    }
    let caller_allowed = allowed_callback_spans(caller_file, lookup);
    let tail = handed_tail(
        caller_file,
        callee,
        &caller_allowed,
        entities,
        symbol_names,
        lookup,
        root_body,
    );
    if let Some(memo) = walk.memo.as_mut() {
        memo.tails.insert(key, tail);
    }
    tail
}

/// The helper whose body runs `span` during its call, if a climb enters it
/// there: not a possible component, and `span` runs while it is called.
fn climbed_helper<'f>(
    file: &'f solid_facts::FileFacts,
    span: solid_facts::core::Span,
    lookup: &SemanticLookup<'_>,
) -> Option<&'f solid_facts::ast::FunctionFact> {
    let function = containing_ast_function(&file.ast, span)?;
    (!lookup.function_may_be_component(file, function)
        && crate::interproc::body_site_runs_during_call(file, function, span))
    .then_some(function)
}

/// ADR 0546: whether no call a climb from `function` with `depth` calls left
/// can reach has a rejecting [`handed_tail`] (the only rejection a climb
/// finds). A call's tail is asked wherever a path of at most `depth` calls
/// reaches it, which a breadth-first search over the helpers finds by its
/// shortest such path; the search stops at the first rejecting tail. A
/// quiet ball also says whether a tail is unplaced and how many helpers it
/// holds that have a call (a climb runs out of calls only on a path through
/// `depth + 1` of them).
#[allow(clippy::too_many_arguments)]
fn climb_rejects_nowhere(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    depth: usize,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
    walk: &mut HelperWriteWalk,
) -> ClimbBall {
    let mut seen = HashSet::from_iter([(file.path.to_string(), function.span)]);
    let mut frontier = vec![(file.path.to_string(), function.span)];
    let mut unplaced = false;
    let mut called = 0;
    for level in 0..=depth {
        let mut next = Vec::new();
        for (path, span) in frontier {
            let sites = lookup.function_call_sites(&path, span);
            if !sites.is_empty() {
                called += 1;
            }
            // The calls of a helper entered with no call left are not asked.
            if level == depth {
                continue;
            }
            for (caller_file, callee) in sites {
                lookup.count_write_walk_steps(1);
                let tail = climb_tail(
                    caller_file,
                    callee,
                    entities,
                    symbol_names,
                    lookup,
                    root_body,
                    walk,
                );
                if tail.is_some_and(ExecutionRole::reports_disallowed_write) {
                    return ClimbBall::Rejects;
                }
                unplaced |= tail == Some(ExecutionRole::Unknown);
                if let Some(helper) = climbed_helper(caller_file, callee, lookup) {
                    let key = (caller_file.path.to_string(), helper.span);
                    if seen.insert(key.clone()) {
                        next.push(key);
                    }
                }
            }
        }
        frontier = next;
    }
    ClimbBall::Quiet { unplaced, called }
}

/// ADR 0560: the rejection the climb's paths meet first from `function`
/// (in progress, with `walk.depth` calls left), found without following
/// them. The climb tries each call of a helper in order: the calls of the
/// helper it is written in first (a rejection there ends the fold), then
/// its own answer ([`handed_tail`]). A helper's climb rejects exactly when
/// a path of at most its calls left, through helpers not in progress,
/// reaches a call whose own answer rejects ([`rejection_within`]: the
/// shortest such path is such a path), so the first call whose helper
/// rejects is where the climb goes, and the first rejection there is the
/// fold's. `None` where no call is found to lead to one (the climb then
/// follows its paths).
#[allow(clippy::too_many_arguments)]
fn first_rejection(
    file: &solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
    walk: &mut HelperWriteWalk,
) -> Option<ExecutionRole> {
    let (depth, mut entered) = (walk.depth, Vec::new());
    let mut at = (file.path.to_string(), function.span);
    let found = 'descent: loop {
        let mut next = None;
        for (caller_file, callee) in lookup.function_call_sites(&at.0, at.1) {
            lookup.count_write_walk_steps(1);
            if walk.depth == 0 {
                break 'descent None;
            }
            if let Some(helper) = climbed_helper(caller_file, callee, lookup) {
                let key = (caller_file.path.to_string(), helper.span);
                if !walk.visiting.contains(&key)
                    && rejection_within(
                        key.clone(),
                        walk.depth - 1,
                        entities,
                        symbol_names,
                        lookup,
                        root_body,
                        walk,
                    )
                {
                    next = Some(key);
                    break;
                }
            }
            let tail = climb_tail(
                caller_file,
                callee,
                entities,
                symbol_names,
                lookup,
                root_body,
                walk,
            );
            if let Some(role) = tail.filter(|role| role.reports_disallowed_write()) {
                break 'descent Some(role);
            }
        }
        let Some(key) = next else {
            break None;
        };
        walk.visiting.insert(key.clone());
        entered.push(key.clone());
        walk.depth -= 1;
        at = key;
    };
    for key in entered {
        walk.visiting.remove(&key);
    }
    walk.depth = depth;
    found
}

/// ADR 0560: whether the climb of `helper` with `calls` calls left rejects:
/// some call of a helper at most `calls - 1` calls away, over helpers not
/// in progress, has a rejecting [`handed_tail`] (breadth-first, so each
/// helper is reached by a shortest path, which repeats none).
#[allow(clippy::too_many_arguments)]
fn rejection_within(
    helper: (String, solid_facts::core::Span),
    calls: usize,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
    walk: &mut HelperWriteWalk,
) -> bool {
    let mut seen = walk.visiting.clone();
    seen.insert(helper.clone());
    let mut frontier = vec![helper];
    for _ in 0..calls {
        let mut next = Vec::new();
        for (path, span) in frontier {
            for (caller_file, callee) in lookup.function_call_sites(&path, span) {
                lookup.count_write_walk_steps(1);
                let tail = climb_tail(
                    caller_file,
                    callee,
                    entities,
                    symbol_names,
                    lookup,
                    root_body,
                    walk,
                );
                if tail.is_some_and(ExecutionRole::reports_disallowed_write) {
                    return true;
                }
                if let Some(helper) = climbed_helper(caller_file, callee, lookup) {
                    let key = (caller_file.path.to_string(), helper.span);
                    if seen.insert(key.clone()) {
                        next.push(key);
                    }
                }
            }
        }
        frontier = next;
    }
    false
}

/// ADR 0537: ADR 0536's summary placement for a write. A write that runs
/// during its helper's call (`const wrapUse = () => { setX(1); }`) runs
/// wherever that call runs, so a call written in a named function handed to
/// an inline carrier (`const hf = () => { wrapUse(); }; [1].forEach(hf)`) or
/// left unplaced takes ADR 0535's answer there, as a write written at the
/// call would. `Some` is a proven disallowed write or `Unknown`; `None` says
/// nothing, and the write's own role stands. The compiler places a helper's
/// body by where it is called (a handler) and knows no carrier between.
#[allow(clippy::too_many_arguments)]
fn helper_call_write_execution(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
    walk: &mut HelperWriteWalk,
) -> Option<ExecutionRole> {
    let function = climbed_helper(file, span, lookup)?;
    let key = (file.path.to_string(), function.span);
    if !walk.visiting.insert(key.clone()) {
        return None;
    }
    if walk.memo.as_ref().is_some_and(|memo| memo.quick.is_none()) {
        let ball = climb_rejects_nowhere(
            file,
            function,
            walk.depth,
            entities,
            symbol_names,
            lookup,
            root_body,
            walk,
        );
        if let Some(memo) = walk.memo.as_mut() {
            memo.quick = Some(!matches!(ball, ClimbBall::Rejects));
        }
        // No call rejects, none is unplaced and no path runs out of calls:
        // every call answers its own placed role or none, so no fold is
        // unplaced, this one included.
        if let ClimbBall::Quiet {
            unplaced: false,
            called,
        } = ball
            && called <= walk.depth
        {
            walk.visiting.remove(&key);
            return None;
        }
    }
    let quick = walk.memo.as_ref().and_then(|memo| memo.quick) == Some(true);
    // ADR 0560: a call rejects somewhere the climb reaches; the kept climb
    // descends straight to the first rejection its paths meet.
    if walk.memo.is_some()
        && !quick
        && let Some(role) = first_rejection(
            file,
            function,
            entities,
            symbol_names,
            lookup,
            root_body,
            walk,
        )
    {
        walk.visiting.remove(&key);
        return Some(role);
    }
    let mut unplaced = false;
    let mut proven = None;
    for (caller_file, callee) in lookup.function_call_sites(file.path.as_str(), function.span) {
        if walk.depth == 0 {
            unplaced = true;
            break;
        }
        // ADR 0560: past the bound the climb ends; its root answers
        // `Unknown` ([`handed_write_execution`]).
        walk.steps += 1;
        if walk.exhausted() {
            break;
        }
        lookup.count_write_walk_steps(1);
        walk.depth -= 1;
        let role = handed_call_role(
            caller_file,
            callee,
            entities,
            symbol_names,
            lookup,
            root_body,
            walk,
        );
        walk.depth += 1;
        if walk.exhausted() {
            break;
        }
        if role.reports_disallowed_write() {
            proven = Some(role);
            break;
        }
        if role == ExecutionRole::Unknown {
            unplaced = true;
            // Nothing this climb reaches rejects: the fold is unplaced now.
            if quick {
                break;
            }
        }
    }
    walk.visiting.remove(&key);
    proven.or(unplaced.then_some(ExecutionRole::Unknown))
}

/// ADR 0535: [`handed_read_role`] for a write: the role of a setter call
/// written directly in a named function, placed by its inline carriers.
#[allow(clippy::too_many_arguments)]
fn handed_write_role(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    allowed: &[solid_facts::core::Span],
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    root_body: RootBodyGuard,
) -> Option<(ExecutionRole, Option<solid_facts::core::Span>)> {
    handed_role(
        file,
        span,
        lookup,
        false,
        |at| {
            semantic_write_execution_role(
                file,
                at,
                allowed,
                entities,
                symbol_names,
                lookup,
                root_body,
            )
        },
        ExecutionRole::reports_disallowed_write,
    )
}

/// The placement shared by [`handed_read_role`] and [`handed_write_role`]:
/// `role_at` is the role of an operation written at a carrier, `reports`
/// whether that role is a proven defect. With `require_run`, a carrier that
/// may run the function zero times proves nothing at a reporting position
/// (ADR 0563).
fn handed_role(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    lookup: &SemanticLookup<'_>,
    require_run: bool,
    role_at: impl Fn(solid_facts::core::Span) -> ExecutionRole,
    reports: impl Fn(ExecutionRole) -> bool,
) -> Option<(ExecutionRole, Option<solid_facts::core::Span>)> {
    let function = containing_ast_function(&file.ast, span)?;
    if lookup.function_may_be_component(file, function) {
        return None;
    }
    let named_expression =
        function.kind == solid_facts::ast::FunctionKind::Expression && function.name.is_some();
    let name = if named_expression {
        crate::execution_role::named_expression_binding(file, function, lookup)
    } else {
        crate::owners::function_binding_name(file, function)
    }?;
    if crate::indexes::binding_written(file, name.span) {
        return Some((ExecutionRole::Unknown, None));
    }
    let mut hand_offs = Vec::new();
    binding_hand_offs(file, name.span, lookup, &mut hand_offs);
    if !hand_offs
        .iter()
        .any(|hand_off| matches!(hand_off, HandOff::Inline(_) | HandOff::FreshStack))
    {
        return None;
    }
    let mut unplaced = false;
    let mut silent = None;
    for hand_off in hand_offs {
        match hand_off {
            HandOff::Call | HandOff::Value => {}
            HandOff::FreshStack => silent = silent.or(Some(ExecutionRole::DeferredCallback)),
            HandOff::Unfollowed | HandOff::Retained => unplaced = true,
            HandOff::Inline(at) => {
                // A carrier inside another unplaced named function has no
                // proven execution at its lexical component position either.
                let role = if inside_non_component_function(file, at, lookup)
                    && named_callback_execution_role(file, at, lookup).is_none()
                {
                    ExecutionRole::Unknown
                } else {
                    role_at(at)
                };
                // ADR 0563: with `require_run`, a carrier that may run the
                // function zero times proves nothing where the role reports.
                if role == ExecutionRole::Unknown
                    || (reports(role)
                        && require_run
                        && !crate::carrier_runs::carrier_provably_runs(file, at, lookup))
                {
                    unplaced = true;
                } else if reports(role) {
                    return Some((role, Some(at)));
                } else {
                    silent = silent.or(Some(role));
                }
            }
        }
    }
    if unplaced {
        Some((ExecutionRole::Unknown, None))
    } else {
        silent.map(|role| (role, None))
    }
}

/// ADR 0630: where a Get on a store-rooted value lands if it is a `merge` view.
enum MergeGet {
    /// Not a `merge` view.
    NoView,
    /// The store holds the key, and no later source does.
    Store,
    /// A plain literal source serves it.
    Plain,
    Unproven,
}

/// ADR 0637: a `catch` around the read swallows the throw that would report it,
/// so no read there is proven pending at the boundary or in a body.
fn caught_in_try(file: &solid_facts::FileFacts, read: solid_facts::core::Span) -> bool {
    file.ast
        .try_catches
        .iter()
        .any(|caught| caught.block.contains(read))
}

/// ADR 0629: what a value escaped from `untrack` holds.
enum EscapedValue {
    /// A store proxy of this store, proven (or not, with `false`) to be one.
    Store(SymbolId, bool),
    /// No proxy: a Get reads nothing.
    Plain,
    /// Nothing says.
    Unknown,
}

/// What a store's literal initial value holds at a path.
enum StorePathValue {
    /// An object or array literal, wrapped by the proxy.
    Wrapped,
    Primitive,
    Unknown,
}

/// The expression the function an exact `untrack` call runs returns on its one
/// straight-line completion.
fn untrack_returned_value(
    file: &solid_facts::FileFacts,
    call: &solid_facts::ast::CallFact,
) -> Option<solid_facts::core::Span> {
    let ast = &file.ast;
    let literal = ast.function_at(ast.peel_ts_sugar_span(call.arguments.first()?.span))?;
    if literal.expression_body {
        return Some(ast.peel_ts_sugar_span(literal.body));
    }
    let mut returns = ast.returns.iter().filter(|returned| {
        containing_ast_function(ast, returned.span)
            .is_some_and(|function| function.span == literal.span)
    });
    let returned = returns.next()?;
    (returns.next().is_none() && returned.straight_line)
        .then(|| returned.argument.map(|value| ast.peel_ts_sugar_span(value)))
        .flatten()
}

/// ADR 0498: the `const` alias a member receiver holds exactly (ADR 0481,
/// `thing.useIt`), the receiver being the alias's own member or a static path
/// below it (`thing.useIt.items` in `thing.useIt.items.push(1)`).
pub(crate) fn member_receiver_alias(
    file: &solid_facts::FileFacts,
    receiver: solid_facts::core::Span,
) -> Option<&solid_facts::ast::LocalAliasMemberFact> {
    let mut at = file.ast.peel_ts_sugar_span(receiver);
    for _ in 0..8 {
        if let Some(alias) = file.ast.local_alias_member(at) {
            return Some(alias);
        }
        let inner = file
            .ast
            .member_at(at)
            .filter(|_| file.ast.computed_members.binary_search(&at).is_err())?;
        at = file.ast.peel_ts_sugar_span(inner.object);
    }
    None
}

/// Whether `member` is read as part of a longer path, which is reported
/// instead. ADR 0498: a member that is the object of a written, deleted or
/// iterated member (`store.a` in `store.a.v = 1`) is read on its own: the
/// longer path is no read, so the Get of this one is the chain's.
pub(crate) fn read_as_part_of_longer_path(
    file: &solid_facts::FileFacts,
    member: &solid_facts::ast::MemberFact,
) -> bool {
    let target = |targets: &[solid_facts::core::Span], candidate: solid_facts::core::Span| {
        targets
            .iter()
            .any(|target| file.ast.peel_ts_sugar_span(*target) == candidate)
    };
    file.ast.members.iter().any(|candidate| {
        file.ast.peel_ts_sugar_span(candidate.object) == member.span
            && !(file.ast.is_no_get_target(candidate.span)
                || target(&file.ast.iteration_targets, candidate.span))
    })
}

/// ADRs 0715, 0721: whether a setter, action or refresh call no reachable
/// row places may run: written in a function, or in a non-static field
/// initializer (it runs at `new`), and not proven unreached (ADR 0458).
/// Outside both it is module initialization: no owner, proven silent.
pub(crate) fn unreached_write_may_run(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let in_instance_field = file
        .ast
        .classes
        .iter()
        .flat_map(|class| &class.elements)
        .any(|element| {
            element.kind == solid_facts::ast::ClassElementKind::Field
                && !element.r#static
                && element.value.is_some_and(|value| value.contains(span))
        });
    (crate::owners::owning_function(&file.ast, span).is_some() || in_instance_field)
        && !matches!(
            crate::execution_role::SiteReach::new(lookup).site(file, span),
            crate::execution_role::Reach::Unreached(_)
        )
}

/// Whether `span` is inside a leaf owner's callback (`onSettled`,
/// `createTrackedEffect`), where a pending async read is forbidden, or in a
/// named local function a reference inside one names. ADR 0800: each
/// reference is visited once, and a walk out of budget may.
pub(crate) fn may_run_in_leaf_owner(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
) -> bool {
    let mut seen = HashSet::default();
    may_run_in_leaf_owner_within(file, span, entities, symbol_names, lookup, &mut seen)
}

fn may_run_in_leaf_owner_within(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
    entities: &EntitySymbols,
    symbol_names: &HashMap<SymbolId, SymbolId>,
    lookup: &SemanticLookup<'_>,
    seen: &mut HashSet<solid_facts::core::Span>,
) -> bool {
    if containing_leaf_owner(file, span, entities, symbol_names, lookup).is_some()
        || file.ast.functions.iter().any(|leaf| {
            leaf.span.contains(span)
                && containing_leaf_owner(file, leaf.span, entities, symbol_names, lookup).is_some()
        })
    {
        return true;
    }
    // The innermost function a name holds, through any IIFE written around
    // the span (it runs where it is written); any other anonymous function
    // runs where it is handed, which this does not follow.
    let mut enclosing = file.ast.functions_containing(span).collect::<Vec<_>>();
    enclosing.sort_by_key(|function| function.span.end - function.span.start);
    for function in enclosing {
        if let Some(name) = function_binding_name(file, function) {
            return file.ast.identifiers.iter().any(|identifier| {
                identifier.role == solid_facts::ast::IdentifierRole::Reference
                    && file.ast.reference_declaration(identifier.span) == Some(name.span)
                    && (seen.len() >= VALUE_WALK_BUDGET
                        || (seen.insert(identifier.span)
                            && may_run_in_leaf_owner_within(
                                file,
                                identifier.span,
                                entities,
                                symbol_names,
                                lookup,
                                seen,
                            )))
            });
        }
        let invoked = file
            .ast
            .calls
            .iter()
            .any(|call| file.ast.peel_ts_sugar_span(call.callee) == function.span);
        if !invoked {
            return false;
        }
    }
    false
}

/// ADR 0803: the obligation at `span`, a reference or a transported call of a
/// slot withdrawn because the program writes the array iterator.
fn withdrawn_slot_obligation(
    file: &solid_facts::FileFacts,
    span: solid_facts::core::Span,
) -> crate::StaticDefect {
    crate::StaticDefect {
        kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
            callee: file.source_text(span).unwrap_or("this value").to_owned(),
            member: None,
        },
        location: location(file.path.shared(), span),
        analysis_context: crate::blockers::TUPLE_ITERATION_WRITTEN.to_owned(),
        fixes: vec![],
        uncertain: true,
    }
}
