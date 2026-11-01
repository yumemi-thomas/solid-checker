//! Member resolution as L2 memos (study S5b), every read recorded.
//!
//! - [`ObjectMemberTargets`], per file slot: the function each call or event
//!   handler member read invokes (ADR 0299, 0358);
//! - [`SpreadTargetsAt`], per file slot: what each intrinsic element's spread
//!   hands the element (ADR 0358);
//! - [`LiteralReceivers`], per file slot: the member expressions whose
//!   receiver is bound to an exact object literal;
//! - [`ReceiverClosed`], per receiver (owner file, literal, declaration,
//!   symbol): whether every use of the receiver is an immediate call of an
//!   inert member. It reads the receiver's uses from the base `symbol_uses`
//!   index and TypeScript's references of its symbols, never every file.
//!
//! The lookup's `Targets`, `SpreadTargets` and literal-receiver set are
//! assemblies of these answers in file order (the order the old builders
//! walked), so a later slot of a path still overwrites an earlier one.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use solid_facts::FileFacts;
use solid_facts::ast;
use solid_facts::compiler::ExecutionMap;
use solid_facts::core::Span;

use super::{binding_written, enclosing_function, intrinsic_element, receiver_inert, references};
use crate::SymbolId;
use crate::base::memo::build::{
    BuildSchema, BuildTables, Input, LITERAL_RECEIVERS, OBJECT_MEMBER_TARGETS, RECEIVER_CLOSED,
    SPREAD_TARGETS_AT, Section,
};
use crate::base::memo::{Ctx, Cycle, Memo, Table, TableId};
use crate::base::{FileId, Hazard, SymbolUse};
use crate::{HashMap, HashSet};

/// One slot of one analyzed file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Slot {
    pub(super) id: FileId,
    pub(super) rank: u32,
}

type Cx<'a, 'c, 'w> = &'a Ctx<'c, 'w, BuildSchema>;

/// The file's facts, recorded as a read of its source.
pub(super) fn facts<'w>(cx: Cx<'_, '_, 'w>, slot: Slot) -> Option<&'w FileFacts> {
    Some(cx.file(slot.id, slot.rank as usize)?.facts())
}

fn symbol_at<'w>(cx: Cx<'_, '_, 'w>, slot: Slot, span: Span) -> Option<&'w SymbolId> {
    cx.file(slot.id, slot.rank as usize)?.symbol_at(span)
}

fn compiler<'w>(cx: Cx<'_, '_, 'w>, slot: Slot) -> Option<&'w ExecutionMap> {
    Some(cx.file(slot.id, slot.rank as usize)?.compiler())
}

fn ordinal(index: usize) -> u32 {
    u32::try_from(index).expect("fewer than 2^32 items in one file")
}

/// The inputs a per-slot key names itself.
fn own_inputs(id: FileId) -> Vec<Input> {
    vec![
        Input::File(id, Section::Source),
        Input::File(id, Section::Compiler),
        Input::File(id, Section::Entities),
    ]
}

/// An exact object literal a receiver is bound to.
#[derive(Clone, Copy)]
pub(super) struct Literal {
    pub(super) owner: Slot,
    pub(super) literal: Span,
    pub(super) declaration: Span,
    pub(super) frozen: bool,
}

/// `{ … }` or `Object.freeze({ … })` at `span` of `owner`: the literal's span
/// and whether it is frozen.
fn object_literal(cx: Cx<'_, '_, '_>, owner: Slot, span: Span) -> Option<(Span, bool)> {
    let file = facts(cx, owner)?;
    let span = file.ast.peel_ts_sugar_span(span);
    if file
        .ast
        .object_get_shapes
        .iter()
        .any(|shape| shape.span == span)
    {
        return Some((span, false));
    }
    let call = file.ast.call_at(span)?;
    let call_ordinal = file
        .ast
        .calls
        .iter()
        .position(|candidate| std::ptr::eq(candidate, call))?;
    let (name, source_file) = cx.standard_call((owner.id, owner.rank, ordinal(call_ordinal)))?;
    if name.as_ref() != "ObjectConstructor.freeze"
        || call.arguments.len() != 1
        || call.arguments[0].spread
    {
        return None;
    }
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    let member = file
        .ast
        .members
        .iter()
        .find(|m| m.span == callee && !file.ast.computed_members.contains(&callee))?;
    let receiver = file.ast.peel_ts_sugar_span(member.object);
    if file.source_text(member.property) != Some("freeze")
        || file.source_text(receiver) != Some("Object")
        || file.ast.reference_declaration(receiver).is_some()
    {
        return None;
    }
    let symbol = cx.symbol_at(owner.id, receiver)?;
    if symbol.alias {
        return None;
    }
    let declarations = symbol
        .declarations
        .iter()
        .filter(|declaration| {
            declaration.kind.as_ref() == "variable" && declaration.name.as_ref() == "Object"
        })
        .collect::<Vec<_>>();
    if declarations.len() != 1
        || declarations[0].path != source_file
        || !source_file.ends_with("/lib.es5.d.ts")
    {
        return None;
    }
    let literal = file.ast.peel_ts_sugar_span(call.arguments[0].span);
    file.ast
        .object_get_shapes
        .iter()
        .any(|shape| shape.span == literal)
        .then_some((literal, true))
}

/// The exact object literal the identifier `receiver` of `from` is bound to.
pub(super) fn receiver_literal(
    cx: Cx<'_, '_, '_>,
    from: Slot,
    receiver: Span,
    symbol: &str,
) -> Option<Literal> {
    if let Some((id, rank, binding)) = cx.binding_at(from.id, receiver) {
        let owner = Slot { id, rank };
        let file = facts(cx, owner)?;
        let binding = file.ast.bindings.get(binding as usize)?;
        if !binding.immutable
            || binding.shape != ast::BindingShape::Identifier
            || binding.names.len() != 1
        {
            return None;
        }
        let declaration = binding.names[0].span;
        if binding_written(file, declaration) {
            return None;
        }
        let (literal, frozen) = object_literal(cx, owner, binding.initializer?)?;
        return Some(Literal {
            owner,
            literal,
            declaration,
            frozen,
        });
    }
    // Anonymous `export default { … }` has an exact export declaration,
    // not a variable binding. Require that very expression location.
    let info = cx.symbol_info(symbol)?;
    let [declaration] = info.declarations.as_slice() else {
        return None;
    };
    let (id, rank) = cx.path_file(&declaration.path)?;
    let owner = Slot { id, rank };
    let file = facts(cx, owner)?;
    let export = file.ast.exports.iter().find(|export| {
        export.kind == ast::ExportKind::Default
            && ((u64::from(export.span.start) == declaration.start
                && u64::from(export.span.end) == declaration.end)
                || export.declarations.iter().any(|value| {
                    u64::from(value.local.span.start) == declaration.start
                        && u64::from(value.local.span.end) == declaration.end
                }))
    })?;
    let value = export.declarations.first()?.local.span;
    let (literal, frozen) = object_literal(cx, owner, value)?;
    Some(Literal {
        owner,
        literal,
        declaration: value,
        frozen,
    })
}

/// The function an object literal holds under `name`, when it is exactly a
/// literal function or one declared function.
pub(super) fn literal_function<'w>(
    cx: Cx<'_, '_, 'w>,
    owner: Slot,
    literal: Span,
    name: &str,
) -> Option<(&'w FileFacts, &'w ast::FunctionFact)> {
    let file = facts(cx, owner)?;
    let shape = file.ast.object_get_shapes.iter().find(|shape| {
        shape.span == literal && shape.closed && shape.properties.iter().all(|p| p.getter.is_none())
    })?;
    let own = |property: &&ast::ObjectPropertyFact| {
        shape.span.contains(property.span)
            && !file.ast.object_get_shapes.iter().any(|nested| {
                nested.span != literal
                    && literal.contains(nested.span)
                    && nested.span.contains(property.span)
            })
    };
    let properties = file
        .ast
        .object_properties
        .iter()
        .filter(own)
        .collect::<Vec<_>>();
    // Methods can expose their receiver through `this` or `arguments` to
    // opaque code. This conservative spelling veto only withholds a target;
    // it never supplies identity (comments/strings can over-veto).
    if properties
        .iter()
        .any(|p| !p.data || p.value_kind == ast::RuntimeValueKind::Function)
        && file.source_text(literal).is_some_and(|text| {
            text.contains("this")
                || text.contains("arguments")
                || text.contains("eval")
                || text.contains("super")
        })
    {
        return None;
    }
    if properties.len() != shape.properties.len() {
        return None;
    }
    let property = properties
        .into_iter()
        .zip(&shape.properties)
        .find(|(property, key)| !property.computed && key.key == name)?
        .0;
    let value = file.ast.peel_ts_sugar_span(property.value);
    if let Some(function) = file.ast.function_at(value) {
        return Some((file, function));
    }
    // Shorthand is one declared function, not a callable type or an alias.
    let declaration = property.shorthand_binding?;
    if let Some(function) = file.ast.functions.iter().find(|function| {
        function.kind == ast::FunctionKind::Declaration
            && function
                .name
                .as_ref()
                .is_some_and(|name| name.span == declaration)
    }) {
        return crate::indexes::function_value_is_current_in(file, function)
            .then_some((file, function));
    }
    let symbol = symbol_at(cx, owner, declaration)?;
    let (id, rank, function) = cx.function_for_symbol(symbol.as_str())?;
    let target = facts(cx, Slot { id, rank })?;
    let function = target.ast.functions.get(function as usize)?;
    (function.kind == ast::FunctionKind::Declaration
        && crate::indexes::function_value_is_current_in(target, function))
    .then_some((target, function))
}

/// Whether the module-level code before the literal enters no function the
/// analysis cannot see.
fn module_initialization_closed(cx: Cx<'_, '_, '_>, owner: Slot, literal: Span) -> bool {
    let Some(file) = facts(cx, owner) else {
        return false;
    };
    // A hoisted function can run before a module-scope const too, without an
    // import cycle. Refuse an executable prefix whose callbacks/entry order
    // are unmodeled. The existing dialect axiom for a signal with only literal
    // data, and the exact standard-library freeze of a literal, enter no user
    // function. Function bodies written in the prefix are deferred.
    file.ast.calls.iter().enumerate().all(|(index, call)| {
        if call.span.start >= literal.start || enclosing_function(file, call.span).is_some() {
            return true;
        }
        let data_signal = !call.construct
            && cx.create_signal_call((owner.id, owner.rank, ordinal(index)))
            && call.arguments.len() <= 1
            && call.arguments.iter().all(|argument| {
                !argument.spread
                    && matches!(
                        argument.literal_value,
                        ast::ArgumentLiteralFact::Null
                            | ast::ArgumentLiteralFact::Boolean(_)
                            | ast::ArgumentLiteralFact::Integer(_)
                            | ast::ArgumentLiteralFact::String(_)
                    )
            });
        data_signal || (!call.construct && object_literal(cx, owner, call.span).is_some())
    })
}

/// Whether the use of the receiver at `reference` of `file` runs after the
/// receiver is initialized.
fn initialized_use(
    cx: Cx<'_, '_, '_>,
    owner: Slot,
    owner_file: &FileFacts,
    (literal, declaration): (Span, Span),
    file: &FileFacts,
    file_id: FileId,
    reference: Span,
) -> bool {
    let declared_function = enclosing_function(owner_file, declaration);
    let used_function = enclosing_function(file, reference);
    if let Some(function) = declared_function {
        // A captured binding has no entry-order proof. A switch's shared case
        // scope has no lexical dominance proof either; source text only vetoes.
        file_id == owner.id
            && used_function == Some(function)
            && reference.start >= literal.end
            && owner_file
                .source_text(function)
                .is_some_and(|text| !text.contains("switch"))
    } else {
        used_function.is_some() && !cx.cyclic(owner.id)
    }
}

/// Whether every use of the receiver is an immediate call of an inert member
/// of its literal (ADR 0299, 0358).
fn receiver_closed(
    cx: Cx<'_, '_, '_>,
    owner: Slot,
    literal: Span,
    declaration: Span,
    symbol: &str,
) -> bool {
    let Some(owner_file) = facts(cx, owner) else {
        return false;
    };
    let symbols = cx.redirects(symbol);
    if enclosing_function(owner_file, declaration).is_none()
        && (cx.unknown_load()
            || cx.exposed(owner.id)
            || !module_initialization_closed(cx, owner, literal))
    {
        return false;
    }
    // The uses of the receiver's symbols, by file slot. The owner's slots are
    // always read: they hold the declaration.
    let mut uses = BTreeMap::<(FileId, u32), Vec<SymbolUse>>::new();
    for symbol in &symbols {
        for (id, rank, used) in cx.symbol_uses(symbol) {
            uses.entry((id, rank)).or_default().push(used);
        }
    }
    for rank in 0..cx.slots(owner.id).len() {
        uses.entry((owner.id, ordinal(rank))).or_default();
    }
    let mut reached = BTreeSet::<(FileId, Span)>::new();
    // Binder references remain essential when a dependency import's Type
    // Facts references still name its .d.ts symbol before runtime redirects.
    for ((id, rank), used) in uses {
        let Some(file) = facts(cx, Slot { id, rank }) else {
            return false;
        };
        let mut bindings = HashSet::default();
        if id == owner.id {
            bindings.insert(declaration);
        }
        for each in &used {
            match each {
                SymbolUse::ImportLocal { local, imported } => {
                    if !imported {
                        return false;
                    }
                    bindings.insert(*local);
                }
                SymbolUse::BoundReference { bound } => {
                    bindings.insert(*bound);
                }
                SymbolUse::Reference(_) => {}
            }
        }
        for (reference, bound) in &file.ast.reference_declarations {
            if bindings.contains(bound) {
                reached.insert((id, *reference));
            }
        }
        for each in &used {
            if let SymbolUse::Reference(span) = each {
                reached.insert((id, *span));
            }
        }
    }
    for symbol in &symbols {
        let Some(semantic) = cx.symbol_references(symbol) else {
            continue;
        };
        for (path, start, end) in semantic {
            let Some((id, _)) = cx.path_file(&path) else {
                return false;
            };
            let (Ok(start), Ok(end)) = (u32::try_from(start), u32::try_from(end)) else {
                return false;
            };
            reached.insert((id, Span::new(start, end)));
        }
    }
    for (id, span) in reached {
        // A path listed twice is read at its last slot.
        let Some(rank) = cx.slots(id).len().checked_sub(1) else {
            return false;
        };
        let slot = Slot {
            id,
            rank: ordinal(rank),
        };
        let Some(file) = facts(cx, slot) else {
            return false;
        };
        if file
            .ast
            .type_queries
            .iter()
            .any(|query| query.contains(span))
        {
            continue;
        }
        // Import/export name tokens are the exact module channel. Namespace
        // accesses and any ordinary value use are deliberately refused.
        if file.ast.imports.iter().any(|import| {
            !import.type_only
                && import
                    .bindings
                    .iter()
                    .any(|binding| binding.imported.is_some() && binding.local.span == span)
        }) || file.ast.exports.iter().any(|export| {
            export.namespace.is_none()
                && export
                    .specifiers
                    .iter()
                    .any(|specifier| specifier.local.span == span)
        }) {
            continue;
        }
        if id == owner.id && span == declaration {
            continue;
        }
        if !initialized_use(
            cx,
            owner,
            owner_file,
            (literal, declaration),
            file,
            id,
            span,
        ) {
            return false;
        }
        // A spread onto an intrinsic element copies the object's own handlers
        // onto the element (ADR 0358): a read that neither escapes the object nor
        // hands it to a callee. A spread into a component hands it on, so it is
        // not this case.
        if file.ast.jsx_elements.iter().any(|element| {
            intrinsic_element(file, element)
                && element
                    .spreads
                    .iter()
                    .any(|spread| file.ast.peel_ts_sugar_span(spread.argument) == span)
        }) {
            continue;
        }
        let Some(member) = file.ast.members.iter().find(|member| {
            file.ast.peel_ts_sugar_span(member.object) == span
                && !file.ast.computed_members.contains(&member.span)
        }) else {
            return false;
        };
        // Merely extracting a member lets a method receive an opaque `this`.
        // Require immediate calls, rather than treating all member reads as
        // harmless receiver uses.
        // An event attribute's value is read by the host with no receiver of the
        // program's, the same way an immediate call of a function literal
        // without `this` is (ADR 0358).
        let invoked =
            file.ast.calls.iter().any(|call| {
                !call.construct && file.ast.peel_ts_sugar_span(call.callee) == member.span
            }) || compiler(cx, slot).is_some_and(|map| {
                map.callback_roles.iter().any(|callback| {
                    callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
                        && crate::owners::handler_value_span(file, callback.span) == member.span
                })
            });
        // ADR 0468: so is a member a comma, conditional or logical callee may
        // evaluate to (`(0, o.m)()`): the call may invoke it, with no receiver.
        let invoked = invoked
            || file.ast.calls.iter().any(|call| {
                !call.construct
                    && file
                        .ast
                        .composite_callee_operands(call.callee)
                        .contains(&member.span)
            });
        // ADR 0516: so is a member handed whole to a call (`setTimeout(o.cb)`):
        // the callee gets the function, never the object, and calls it with
        // no receiver of the program's.
        let invoked = invoked || handed_whole(file, member.span);
        if !invoked {
            return false;
        }
        let Some((function_file, function)) = file
            .source_text(member.property)
            .and_then(|name| literal_function(cx, owner, literal, name))
        else {
            return false;
        };
        if !receiver_inert(function_file, function) {
            return false;
        }
    }
    true
}

/// Whether the member expression at `member` is a call's whole, non-spread
/// argument (under transparent TypeScript wrappers).
fn handed_whole(file: &FileFacts, member: Span) -> bool {
    file.ast.arguments_containing(member).any(|(call, index)| {
        let argument = &call.arguments[index];
        !call.construct && !argument.spread && file.ast.peel_ts_sugar_span(argument.span) == member
    })
}

/// A receiver's closure, keyed by the owner's slot, the literal and the
/// declaration spans, and the receiver's symbol.
pub(crate) type ClosedKey = ((FileId, u32), (Span, Span), Arc<str>);

/// Whether every use of one receiver is an immediate call of an inert member
/// of its exact object literal.
pub(crate) struct ReceiverClosed;

impl Memo<BuildSchema> for ReceiverClosed {
    type Key = ClosedKey;
    type Value = bool;
    const NAME: &'static str = "receiver_closed";
    const ID: TableId = RECEIVER_CLOSED;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        tables.receiver_closed()
    }

    fn compute(
        cx: &Ctx<'_, '_, BuildSchema>,
        ((id, rank), (literal, declaration), symbol): &ClosedKey,
    ) -> Result<bool, Cycle> {
        Ok(receiver_closed(
            cx,
            Slot {
                id: *id,
                rank: *rank,
            },
            *literal,
            *declaration,
            symbol,
        ))
    }

    fn on_cycle(_: &ClosedKey) -> bool {
        false
    }

    fn implied(((id, _), _, _): &ClosedKey) -> Vec<Input> {
        own_inputs(*id)
    }
}

/// The key of `literal`'s receiver closure.
fn closed_key(literal: &Literal, symbol: &SymbolId) -> ClosedKey {
    (
        (literal.owner.id, literal.owner.rank),
        (literal.literal, literal.declaration),
        symbol.shared(),
    )
}

/// The target of each call and event-handler member read of one file slot:
/// `(read span, target path, target function span)`, in production order.
pub(crate) struct ObjectMemberTargets;

type SlotKey = (FileId, u32);

impl Memo<BuildSchema> for ObjectMemberTargets {
    type Key = SlotKey;
    type Value = Vec<(Span, Arc<str>, Span)>;
    const NAME: &'static str = "object_member_targets";
    const ID: TableId = OBJECT_MEMBER_TARGETS;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        tables.object_member_targets()
    }

    fn compute(cx: &Ctx<'_, '_, BuildSchema>, (id, rank): &SlotKey) -> Result<Self::Value, Cycle> {
        let slot = Slot {
            id: *id,
            rank: *rank,
        };
        let mut targets = Vec::new();
        if cx.hazard(Hazard::Eval) {
            return Ok(targets);
        }
        let Some(read) = cx.file(slot.id, slot.rank as usize) else {
            return Ok(targets);
        };
        let file = read.facts();
        let members = file
            .ast
            .members
            .iter()
            .map(|member| (member.span, member))
            .collect::<HashMap<_, _>>();
        let references = references(file);
        // The member a call invokes, and the member a JSX event handler attribute
        // takes as its value: both name the function the member holds (ADR 0358).
        let reads = file
            .ast
            .calls
            .iter()
            .filter(|call| !call.construct)
            .map(|call| (call.callee, call.callee))
            // ADR 0468: a member a comma, conditional or logical callee may
            // evaluate to (`(0, o.m)()`), which the call invokes without its
            // receiver: the owner walk asks which function it holds.
            .chain(
                file.ast
                    .calls
                    .iter()
                    .filter(|call| !call.construct)
                    .flat_map(|call| file.ast.composite_callee_operands(call.callee))
                    .map(|operand| (operand, operand)),
            )
            // ADR 0516: a member handed whole to a call (`setTimeout(o.cb)`).
            .chain(
                file.ast
                    .members
                    .iter()
                    .filter(|member| handed_whole(file, member.span))
                    .map(|member| (member.span, member.span)),
            )
            .chain(
                read.compiler()
                    .callback_roles
                    .iter()
                    .filter(|callback| {
                        callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
                    })
                    .map(|callback| {
                        (
                            callback.span,
                            crate::owners::handler_value_span(file, callback.span),
                        )
                    }),
            )
            .collect::<Vec<_>>();
        for (read_span, value) in reads {
            let callee = file.ast.peel_ts_sugar_span(value);
            let Some(member) = members.get(&callee) else {
                continue;
            };
            if file.ast.computed_members.contains(&callee) {
                continue;
            }
            let receiver = file.ast.peel_ts_sugar_span(member.object);
            if !references.contains(&receiver) {
                continue;
            }
            let Some(symbol) = read.symbol_at(receiver) else {
                continue;
            };
            let Some(literal) = receiver_literal(cx, slot, receiver, symbol.as_str()) else {
                continue;
            };
            if !cx.memo::<ReceiverClosed>(&closed_key(&literal, symbol))? {
                continue;
            }
            let Some(name) = file.source_text(member.property) else {
                continue;
            };
            if !cx.member_stable(literal.owner.id, name) {
                continue;
            }
            if literal.frozen && !cx.member_stable(literal.owner.id, "freeze") {
                continue;
            }
            let Some((function_file, function)) =
                literal_function(cx, literal.owner, literal.literal, name)
            else {
                continue;
            };
            if !receiver_inert(function_file, function) {
                continue;
            }
            // Both receiver and own callable are now proven non-nullish;
            // optional links therefore select the same function.
            targets.push((read_span, function_file.path.shared(), function.span));
        }
        Ok(targets)
    }

    fn on_cycle(_: &SlotKey) -> Self::Value {
        Vec::new()
    }

    fn implied((id, _): &SlotKey) -> Vec<Input> {
        own_inputs(*id)
    }
}

/// The exact handlers one spread argument supplies: `None` when they cannot
/// be resolved exactly.
type Handlers = Option<Vec<(Arc<str>, Span)>>;

/// What each intrinsic element's spread argument of one file slot supplies,
/// by the argument's span, in production order.
pub(crate) struct SpreadTargetsAt;

impl Memo<BuildSchema> for SpreadTargetsAt {
    type Key = SlotKey;
    type Value = Vec<(Span, Handlers)>;
    const NAME: &'static str = "spread_handler_targets";
    const ID: TableId = SPREAD_TARGETS_AT;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        tables.spread_targets_at()
    }

    fn compute(cx: &Ctx<'_, '_, BuildSchema>, (id, rank): &SlotKey) -> Result<Self::Value, Cycle> {
        let slot = Slot {
            id: *id,
            rank: *rank,
        };
        let mut spreads = Vec::new();
        let Some(file) = facts(cx, slot) else {
            return Ok(spreads);
        };
        let references = references(file);
        for element in &file.ast.jsx_elements {
            if !intrinsic_element(file, element) {
                continue;
            }
            for spread in &element.spreads {
                let argument = file.ast.peel_ts_sugar_span(spread.argument);
                // A direct `eval` anywhere can rewrite any object, so every
                // spread is unresolved then.
                let targets = if cx.hazard(Hazard::Eval) {
                    None
                } else {
                    spread_handlers(cx, slot, file, argument, &references)?.or_else(|| {
                        declared_keys_hold_no_listener(cx, slot, spread.argument).then(Vec::new)
                    })
                };
                spreads.push((spread.argument, targets));
            }
        }
        Ok(spreads)
    }

    fn on_cycle(_: &SlotKey) -> Self::Value {
        Vec::new()
    }

    fn implied((id, _): &SlotKey) -> Vec<Input> {
        own_inputs(*id)
    }
}

/// ADR 0436: a spread argument that no exact object resolves still holds no
/// handler when the compiler gives its declared type a complete key set and the
/// dialect lowers none of those keys as a listener. A key set the producer did
/// not prove complete (an index signature, `any`, a union with an open member,
/// a generic type) is no fact, and an unaudited dialect answers `None` for
/// every key; both leave the spread unresolved.
fn declared_keys_hold_no_listener(cx: Cx<'_, '_, '_>, slot: Slot, argument: Span) -> bool {
    cx.file(slot.id, slot.rank as usize)
        .and_then(|read| read.entity_at(argument))
        .and_then(|entity| entity.declared_keys.as_deref())
        .is_some_and(|keys| {
            keys.iter()
                .all(|key| cx.dialect().spread_key_is_event(key) == Some(false))
        })
}

/// The handlers one spread argument supplies. An object literal written in
/// the spread is exact on its own; a const binding must pass the same
/// receiver premises a call through it does (ADR 0299).
fn spread_handlers(
    cx: Cx<'_, '_, '_>,
    slot: Slot,
    file: &FileFacts,
    argument: Span,
    references: &HashSet<Span>,
) -> Result<Handlers, Cycle> {
    let Some(text) = file.source_text(argument) else {
        return Ok(None);
    };
    if text.trim_start().starts_with('{') {
        return Ok(on_keys(cx, slot, argument, false));
    }
    if !references.contains(&argument) {
        return Ok(None);
    }
    let Some(symbol) = symbol_at(cx, slot, argument) else {
        return Ok(None);
    };
    let Some(literal) = receiver_literal(cx, slot, argument, symbol.as_str()) else {
        return Ok(None);
    };
    if !cx.memo::<ReceiverClosed>(&closed_key(&literal, symbol))? {
        return Ok(None);
    }
    if literal.frozen && !cx.member_stable(literal.owner.id, "freeze") {
        return Ok(None);
    }
    Ok(on_keys(cx, literal.owner, literal.literal, true))
}

/// The `on…` functions a closed object literal holds, each resolved exactly. A
/// key that is not exactly a literal function, an accessor, or a member the
/// receiver may have replaced (`stability`) makes the whole object unresolved.
fn on_keys(cx: Cx<'_, '_, '_>, owner: Slot, literal: Span, stability: bool) -> Handlers {
    let file = facts(cx, owner)?;
    let shape = file
        .ast
        .object_get_shapes
        .iter()
        .find(|shape| shape.span == literal)?;
    if !shape.closed {
        return None;
    }
    let mut targets = Vec::new();
    for property in &shape.properties {
        let name = property.key.as_str();
        if !cx.dialect().spread_key_is_event(name)? {
            continue;
        }
        if property.getter.is_some() {
            return None;
        }
        if stability && !cx.member_stable(owner.id, name) {
            return None;
        }
        let (target, function) = literal_function(cx, owner, literal, name)?;
        if !receiver_inert(target, function) {
            return None;
        }
        targets.push((target.path.shared(), function.span));
    }
    Some(targets)
}

/// The member expressions of one file slot whose receiver is an identifier
/// bound to an exact object literal, by member span, in member order, each
/// with whether the literal is declared in another file (only those can
/// change while this file does not).
pub(crate) struct LiteralReceivers;

impl Memo<BuildSchema> for LiteralReceivers {
    type Key = SlotKey;
    type Value = Vec<(Span, bool)>;
    const NAME: &'static str = "literal_receiver_members";
    const ID: TableId = LITERAL_RECEIVERS;

    fn table(tables: &BuildTables) -> &Table<BuildSchema, Self> {
        tables.literal_receivers()
    }

    fn compute(cx: &Ctx<'_, '_, BuildSchema>, (id, rank): &SlotKey) -> Result<Self::Value, Cycle> {
        let slot = Slot {
            id: *id,
            rank: *rank,
        };
        let Some(read) = cx.file(slot.id, slot.rank as usize) else {
            return Ok(Vec::new());
        };
        let file = read.facts();
        let references = references(file);
        let mut found = Vec::new();
        for member in &file.ast.members {
            let receiver = file.ast.peel_ts_sugar_span(member.object);
            if !references.contains(&receiver) {
                continue;
            }
            if let Some(literal) = read
                .symbol_at(receiver)
                .and_then(|symbol| receiver_literal(cx, slot, receiver, symbol))
            {
                found.push((member.span, literal.owner.id != slot.id));
            }
        }
        Ok(found)
    }

    fn on_cycle(_: &SlotKey) -> Self::Value {
        Vec::new()
    }

    fn implied((id, _): &SlotKey) -> Vec<Input> {
        own_inputs(*id)
    }
}
