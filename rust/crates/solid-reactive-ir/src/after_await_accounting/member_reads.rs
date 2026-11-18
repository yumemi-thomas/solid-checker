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
    // ADR 1000: a write that reads its target first is a read of it: a compound assignment (`local.x += 1`), a logical one (`local.x ||= v`) and an
    // update (`local.x++`) each Get the member before they Set it, and the runtime reports that Get after an await as it does any other.
    let read_first: HashSet<Span> = ast
        .assignments
        .iter()
        .filter(|assignment| assignment.reads_target)
        .map(|assignment| peel(assignment.target))
        .collect();
    let written: HashSet<Span> = ast
        .write_targets
        .iter()
        .chain(&ast.deleted_targets)
        .chain(&ast.iteration_targets)
        .map(|target| peel(*target))
        .filter(|target| !read_first.contains(target))
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

/// ADR 1084 (census only): why [`root_is_plain`] answered no, as one sub-fact. Each disjunct failed; this names the one a proof would have
/// to change: for a root no declaration binds, the global test (`global:not-ecmascript`, `global:builtins-touched`); else what
/// `value_origin` stopped on, followed through the root's binding to its initializer (`origin:project-class`, `binding:parameter`,
/// `binding:mutable`, `binding:destructured`, `binding:import`, `binding:none` (a declaration the binding table does not hold: a catch
/// parameter, a function or class name), `init:none` (a `for … of`/`in` head), `init:builtin-shared` where the initializer
/// is a fresh built-in the binding does not keep closed, `init:call:<callee>`, `init:await:<shape>`, `init:object-literal`, `init:member`,
/// `init:other`), and for a root that is itself a call or a member, `call:<callee>` or `member`. A primitive proof that failed is
/// implied by each: the first disjunct needs a primitive-valued expression, which none of these shapes is.
pub(super) fn plain_failure(ctx: &AnalysisContext<'_>, file: &FileFacts, root: Span) -> String {
    use solid_facts::ast::RuntimeValueKind;
    let lookup = ctx.semantic_lookup;
    let ast = &file.ast;
    let root = ast.peel_ts_sugar_span(root);
    if let Some(call) = ast.call_at(root) {
        return format!("call:{}", callee_class(ctx, file, call));
    }
    if ast.member_at(root).is_some() {
        return "member".to_owned();
    }
    if ast.reference_declaration(root).is_none() {
        return if !super::library_call::is_ecmascript_global(ctx, file, root) {
            "global:not-ecmascript".to_owned()
        } else if !super::library_call::builtins_untouched(ctx) {
            "global:builtins-touched".to_owned()
        } else {
            "global:other".to_owned()
        };
    }
    if let Some(crate::indexes::ValueOrigin::ProjectClass(..)) =
        lookup.value_origin(file, root, RuntimeValueKind::Unknown, 4)
    {
        return "origin:project-class".to_owned();
    }
    let declaration = ast.reference_declaration(root);
    let is_parameter = |span: Span| {
        ast.functions.iter().any(|function| {
            function
                .parameters
                .iter()
                .any(|parameter| parameter.names.iter().any(|name| name.span == span))
        })
    };
    if declaration.is_some_and(is_parameter) {
        return "binding:parameter".to_owned();
    }
    if declaration.is_some_and(|declaration| {
        ast.imports.iter().any(|import| {
            import
                .bindings
                .iter()
                .any(|binding| binding.local.span == declaration)
        })
    }) {
        return "binding:import".to_owned();
    }
    let Some((binding_file, binding, _)) = lookup.binding_at_reference(file.path.as_str(), root)
    else {
        return "binding:none".to_owned();
    };
    let declared = |span: Span| {
        binding_file.ast.functions.iter().any(|function| {
            function
                .parameters
                .iter()
                .any(|parameter| parameter.names.iter().any(|name| name.span == span))
        })
    };
    if binding.names.iter().any(|name| declared(name.span)) {
        return "binding:parameter".to_owned();
    }
    if binding_file.path != file.path {
        return "binding:import".to_owned();
    }
    if !binding.immutable {
        return "binding:mutable".to_owned();
    }
    if binding.shape != solid_facts::ast::BindingShape::Identifier {
        return "binding:destructured".to_owned();
    }
    let Some(initializer) = binding.initializer else {
        return "init:none".to_owned();
    };
    let initializer = ast.peel_ts_sugar_span(initializer);
    if matches!(
        lookup.value_origin(file, initializer, binding.initializer_value_kind, 4),
        Some(crate::indexes::ValueOrigin::Builtin)
    ) {
        return "init:builtin-shared".to_owned();
    }
    if let Some(call) = ast.call_at(initializer) {
        return format!("init:call:{}", callee_class(ctx, file, call));
    }
    let text = file
        .source_text(initializer)
        .unwrap_or_default()
        .trim_start();
    if let Some(awaited) = text.strip_prefix("await") {
        let operand = ast
            .calls
            .iter()
            .filter(|call| initializer.contains(call.span) && call.span.start > initializer.start)
            .min_by_key(|call| call.span.start)
            .filter(|call| {
                file.source_text(call.span)
                    .is_some_and(|call_text| awaited.trim_start().starts_with(call_text))
            });
        return match operand {
            Some(call) => format!("init:await:call:{}", callee_class(ctx, file, call)),
            None => "init:await:other".to_owned(),
        };
    }
    if text.starts_with('{') {
        return "init:object-literal".to_owned();
    }
    if ast.member_at(initializer).is_some() {
        return "init:member".to_owned();
    }
    "init:other".to_owned()
}

/// ADR 1084 (census only): what a call's callee resolves to: `construct:` before a constructor; then `std:<qualified name>` (a standard
/// library declaration), `package:<package>`, `project-function`, `project-method`, `project-other`, or `unresolved`.
fn callee_class(
    ctx: &AnalysisContext<'_>,
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
) -> String {
    let lookup = ctx.semantic_lookup;
    let prefix = if call.construct { "construct:" } else { "" };
    let kind = if call.construct {
        typefacts::CallKind::Construct
    } else {
        typefacts::CallKind::Call
    };
    if let Some(declaration) = lookup.standard_library_declaration(file, call, kind) {
        return format!("{prefix}std:{}", declaration.qualified_name);
    }
    let Some(declaration) = lookup
        .resolved_callee_call(file, call.callee)
        .and_then(|resolved| resolved.declaration.as_ref())
    else {
        return format!("{prefix}unresolved");
    };
    let path = declaration.location.path.as_ref();
    if let Some(root) = solid_facts::package_reach::package_root_of(path) {
        let package = root.rsplit("/node_modules/").next().unwrap_or(root);
        return format!("{prefix}package:{package}");
    }
    let callee = file.ast.peel_ts_sugar_span(call.callee);
    if file.ast.member_at(callee).is_some() {
        return format!("{prefix}project-method");
    }
    match declaration.kind.as_ref() {
        "function" | "arrow-function" | "function-expression" | "variable" => {
            format!("{prefix}project-function")
        }
        other => format!("{prefix}project-other:{other}"),
    }
}
