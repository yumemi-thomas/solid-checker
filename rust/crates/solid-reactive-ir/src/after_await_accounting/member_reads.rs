//! ADR 0976, S4: a member read written after a suspension whose object is not proven to be plain data.
//!
//! The read universe ([`crate::proof_universe`]) lists the reads of the accessors, the stores and the props the program names: an accessor call, and a root's chain of members at its
//! first Get. A store value reached through a binding is another proxy and its members are read the same way: `const local = store.n; await pause(); return local.x` raises
//! UNTRACKED_READ_AFTER_AWAIT in Chrome rc.21 at `local.x`, and `local` is no symbol of the tables (the review of #233, S4: a module-scope alias, a destructuring, an element of a store
//! array, a function's return, a cell of a literal, a props alias; Chrome-confirmed, silent on main in a recognized computation too). The pass lists what the universe does not:
//!
//! - **a member read**: a member expression written after a suspension of its own function (or in a promise handler, [`super::handlers`]) that is no call's callee (the call is its
//!   own site), no write or `delete` target, and no object of a longer member (the longest chain is the site, the inner members its alternates); the object of a callee's member
//!   counts, and so does an operand whose properties are read without a member (a destructured value, an iterated or spread operand);
//! - **unless its object is proven plain**: the chain's root is a primitive proven from its own expression, a fresh built-in value ([`crate::indexes::SemanticLookup::value_origin`]),
//!   or an unshadowed ECMAScript global in a program that leaves every built-in alone. A type proves nothing: a `{ x: number }` annotation is what the store proxy has too.
//!
//! The root of a chain that is a store or props root itself is the universe's read, not listed twice.
//!
//! The owner ruled the full rule (2026-11-15): every member read whose object is not proven plain is a site, a parameter, a package's return, an `any` and an unresolved call
//! included, so a value reached through a binding the program makes (an alias, a destructuring, an element, a cell, a return) needs no census to be found. Only uncertifiable
//! rows are added; a census of the store and props values the program binds (+196 rows against the full rule's +9,980) would leave an object of unknown origin silent, which
//! is not fail-closed, and is not built.

use solid_facts::FileFacts;
use solid_facts::core::Span;

use crate::pipeline::AnalysisContext;
use crate::{HashMap, HashSet, ReactiveSourceKind};

/// One listed read: where a row would stand, the inner members of the chain that a row at the site also covers, and the chain's root expression.
pub(super) struct Chain {
    pub(super) at: Span,
    pub(super) alternates: Vec<Span>,
    pub(super) root: Span,
}

/// The reads of a file: its longest member chains that are no callee, no write and no object of a longer one, and the operands whose properties are read without a member.
pub(super) fn chains(file: &FileFacts) -> Vec<Chain> {
    let ast = &file.ast;
    let peel = |span: Span| ast.peel_ts_sugar_span(span);
    let callees: HashSet<Span> = ast.calls.iter().map(|call| peel(call.callee)).collect();
    let written: HashSet<Span> = ast
        .write_targets
        .iter()
        .chain(&ast.deleted_targets)
        .chain(&ast.iteration_targets)
        .map(|target| peel(*target))
        .collect();
    let mut users: HashMap<Span, Vec<Span>> = HashMap::default();
    for member in &ast.members {
        users
            .entry(peel(member.object))
            .or_default()
            .push(member.span);
    }
    let root_of = |start: Span, alternates: &mut Vec<Span>| {
        let mut root = start;
        while let Some(inner) = ast.member_at(root) {
            alternates.push(root);
            root = peel(inner.object);
        }
        root
    };
    let mut found = Vec::new();
    for member in &ast.members {
        if callees.contains(&member.span) || written.contains(&member.span) {
            continue;
        }
        // The object of a longer member that is itself read covers it; one that is a callee or a write target leaves this the read.
        if users.get(&member.span).is_some_and(|longer| {
            longer
                .iter()
                .any(|user| !callees.contains(user) && !written.contains(user))
        }) {
            continue;
        }
        let mut alternates = Vec::new();
        let root = root_of(peel(member.object), &mut alternates);
        found.push(Chain {
            at: member.span,
            alternates,
            root,
        });
    }
    // The operands whose properties are read with no member: a destructured value, an iterated operand, a spread's argument. A destructuring is reported from its pattern (the
    // position a stack frame of the iteration or the property read has), the operand being the alternate; the others from the operand.
    let operands = ast
        .pattern_values
        .iter()
        .map(|(pattern, value)| (Some(*pattern), *value))
        .chain(ast.iterated_operands.iter().map(|operand| (None, *operand)))
        .chain(ast.spreads.iter().map(|spread| (None, spread.argument)));
    // An operand the file also destructures is reported once, from its pattern (an array pattern iterates it).
    let destructured: HashSet<Span> = ast
        .pattern_values
        .iter()
        .map(|(_, value)| peel(*value))
        .collect();
    for (pattern, operand) in operands {
        let operand = peel(operand);
        if ast.member_at(operand).is_some()
            || ast.call_at(operand).is_some()
            || (pattern.is_none() && destructured.contains(&operand))
        {
            continue;
        }
        match pattern.filter(|pattern| pattern.start <= operand.start) {
            Some(pattern) => found.push(Chain {
                at: Span::new(pattern.start, operand.end),
                alternates: vec![operand],
                root: operand,
            }),
            None => found.push(Chain {
                at: operand,
                alternates: Vec::new(),
                root: operand,
            }),
        }
    }
    found
}

/// Whether the chain's root is a store or props root of the program, whose reads the universe lists.
pub(super) fn root_is_a_source(ctx: &AnalysisContext<'_>, file: &FileFacts, root: Span) -> bool {
    ctx.entities.value_symbol(file, root).is_some_and(|symbol| {
        ctx.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store)
            || ctx.prop_sources.contains_key(symbol)
    })
}

/// Whether the chain's root is proven plain data: a primitive from its own expression, a fresh built-in value, or an ECMAScript global of a program that leaves every built-in alone.
pub(super) fn root_is_plain(ctx: &AnalysisContext<'_>, file: &FileFacts, root: Span) -> bool {
    use solid_facts::ast::RuntimeValueKind;
    let lookup = ctx.semantic_lookup;
    lookup.proven_primitive(file, root, 4)
        || matches!(
            lookup.value_origin(file, root, RuntimeValueKind::Unknown, 4),
            Some(crate::indexes::ValueOrigin::Builtin)
        )
        || super::library_call::is_unpatched_ecmascript_global(ctx, file, root)
}
