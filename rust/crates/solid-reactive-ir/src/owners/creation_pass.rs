//! ADR 0570: whether a read of an async source runs in the synchronous pass
//! that created the source.
//!
//! `createMemo(async () => …)` is pending until a microtask settles it
//! (`@solidjs/signals@2.0.0-rc.17` `dist/dev-shared.js:3640-3690`). A read in
//! the run that created it, or in a component that run mounts on its own
//! stack, meets it pending; a component a `<Show when={on()}>` mounts on a
//! later click meets it settled, and rc.17 then warns only the strict read.

use super::*;

/// The run that creates an async source: a function's run (a component's
/// body, or the function that calls the hook holding the declaration), or
/// module evaluation.
enum Creator<'a> {
    Function(&'a solid_facts::FileFacts, Span),
    Module,
}

/// How many functions a climb may pass (a hook chain, a mount chain).
const CREATION_DEPTH: usize = 6;

/// ADR 0570: the read at `read` in `file` is proven to run in the synchronous
/// pass that created the source declared at `declaration`:
/// - the creator is the function whose body holds the declaration, or, for a
///   hook whose every reference is a plain call written in one function's own
///   code (or only at module scope), that function (module evaluation);
/// - from the read, the climb passes callbacks no call names (their role is
///   the read's own, already placed) to the function that holds them, then
///   follows call sites: a plain call runs in the function whose own code
///   holds it; a tag runs there only through holders that build their child
///   when they are created ([`super::synchronous_tag_position`]); it ends at
///   the creator's function, or, for a module-scope creator, at module scope
///   or a function a module-scope `render` mounts (ADR 0503's
///   `direct_render_sites`).
///
/// One such chain proves a run in that pass. Anything else (a later mount, a
/// project component's children, an unrendered export, an unresolved
/// declaration) is not a proof.
pub(crate) fn read_in_creation_pass(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    read: Span,
    declaration: &crate::Location,
) -> bool {
    let Some(source_file) = lookup.file_by_path(declaration.path.as_ref()) else {
        return false;
    };
    let (Ok(start), Ok(end)) = (
        u32::try_from(declaration.start_byte),
        u32::try_from(declaration.end_byte),
    ) else {
        return false;
    };
    let Some(creator) = creator_of(lookup, source_file, Span::new(start, end)) else {
        return false;
    };
    runs_in_pass(lookup, file, read, &creator, CREATION_DEPTH, true)
}

/// ADR 0661: the read's first run is inside a `render()` call: its position in
/// each function is unconditional, and the chain reaches a function a
/// module-scope `render` mounts. `ASYNC_OUTSIDE_LOADING_BOUNDARY` is enabled only
/// for that call (`web.dev.js:536-557`), so a later mount is quiet even when
/// the source is really pending.
pub(crate) fn read_in_root_pass(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    read: Span,
) -> bool {
    let mut at = read;
    for _ in 0..CREATION_DEPTH {
        if !super::async_context::unconditional_position(lookup, file, at) {
            return false;
        }
        let Some(function) = containing_ast_function(&file.ast, at) else {
            break;
        };
        // A callback no call names runs where its own position says.
        if !lookup
            .function_call_sites(file.path.as_str(), function.span)
            .is_empty()
        {
            break;
        }
        at = function.span;
    }
    runs_in_pass(lookup, file, read, &Creator::Module, CREATION_DEPTH, true)
}

fn creator_of<'a>(
    lookup: &SemanticLookup<'a>,
    file: &'a solid_facts::FileFacts,
    declaration: Span,
) -> Option<Creator<'a>> {
    let mut file = file;
    let mut at = declaration;
    for _ in 0..CREATION_DEPTH {
        let Some(function) = containing_ast_function(&file.ast, at) else {
            return Some(Creator::Module);
        };
        match hook_call(lookup, file, function) {
            Some(HookCall::In(caller, call)) => {
                file = caller;
                at = call;
            }
            Some(HookCall::Module) => return Some(Creator::Module),
            None => return Some(Creator::Function(file, function.span)),
        }
    }
    None
}

enum HookCall<'a> {
    /// Every call is written in one function's own code; one of them.
    In(&'a solid_facts::FileFacts, Span),
    /// Every call is at module scope.
    Module,
}

/// `function` runs only where a plain call names it: it has a name, every
/// reference to it is a call site the call graph accounts for, none is a JSX
/// tag, and they all sit in one function's own code, or all at module scope.
fn hook_call<'a>(
    lookup: &SemanticLookup<'a>,
    file: &'a solid_facts::FileFacts,
    function: &solid_facts::ast::FunctionFact,
) -> Option<HookCall<'a>> {
    let path = file.path.as_str();
    if lookup.program_has_eval() || !lookup.function_value_is_current(file, function) {
        return None;
    }
    let name = function_binding_name(file, function)?;
    let symbol = lookup.entities().at(path, name.span)?;
    let references = lookup.symbol_references_if_present(symbol.as_str())?;
    let accounted = lookup
        .function_call_site_references(path, function.span)
        .into_iter()
        .map(|(caller, callee)| (caller.path.as_str().to_owned(), callee.start, callee.end))
        .collect::<HashSet<_>>();
    for reference in &references {
        let (Ok(start), Ok(end)) = (
            u32::try_from(reference.start_byte),
            u32::try_from(reference.end_byte),
        ) else {
            return None;
        };
        if reference.path.as_ref() == path && start == name.span.start && end == name.span.end {
            continue;
        }
        if !accounted.contains(&(reference.path.as_ref().to_owned(), start, end)) {
            return None;
        }
    }
    let sites = lookup.function_call_sites(path, function.span);
    let mut owners = Vec::new();
    for (caller, callee) in &sites {
        let ast = &caller.ast;
        if ast
            .jsx_elements
            .iter()
            .any(|element| element.name.span == *callee)
            || !ast
                .calls_with_peeled_callee(*callee)
                .any(|call| !call.construct)
        {
            return None;
        }
        let owner = containing_ast_function(ast, *callee).map(|owner| owner.span);
        if !owners.contains(&(caller.path.as_str(), owner)) {
            owners.push((caller.path.as_str(), owner));
        }
    }
    match (owners.as_slice(), sites.first()) {
        ([(_, Some(_))], Some((caller, callee))) => Some(HookCall::In(caller, *callee)),
        (all, Some(_)) if all.iter().all(|(_, owner)| owner.is_none()) => Some(HookCall::Module),
        _ => None,
    }
}

fn runs_in_pass(
    lookup: &SemanticLookup<'_>,
    file: &solid_facts::FileFacts,
    at: Span,
    creator: &Creator<'_>,
    depth: usize,
    lexical: bool,
) -> bool {
    let Some(function) = containing_ast_function(&file.ast, at) else {
        return matches!(creator, Creator::Module);
    };
    if let Creator::Function(creator_file, creator_function) = creator
        && creator_file.path == file.path
        && *creator_function == function.span
    {
        return true;
    }
    if depth == 0 {
        return false;
    }
    let sites = lookup.function_call_sites(file.path.as_str(), function.span);
    if sites.is_empty() {
        // A callback no call names: the read's own placement decided its
        // role, which is the code around it; any other position is not run.
        return lexical && runs_in_pass(lookup, file, function.span, creator, depth, true);
    }
    if !lookup.function_value_is_current(file, function) {
        return false;
    }
    // A local closure the read sits in, called only by plain calls written
    // inside the function that declares it (`const f1 = () => …; f1();`,
    // also inside a `createRoot` or `onSettled` callback there): the read's
    // own placement followed those calls, so the climb stays lexical.
    let outer = containing_ast_function(&file.ast, function.span);
    if lexical
        && outer.is_some_and(|outer| {
            sites.iter().all(|(caller, callee)| {
                caller.path == file.path
                    && outer.body.contains(*callee)
                    && !caller
                        .ast
                        .jsx_elements
                        .iter()
                        .any(|element| element.name.span == *callee)
            })
        })
        && runs_in_pass(lookup, file, function.span, creator, depth - 1, true)
    {
        return true;
    }
    if matches!(creator, Creator::Module)
        && (!super::async_context::direct_render_sites(lookup, file, function).is_empty()
            || super::async_context::mounted_by_root(lookup, file, function))
    {
        return true;
    }
    sites.into_iter().any(|(caller, callee)| {
        let tag = caller
            .ast
            .jsx_elements
            .iter()
            .any(|element| element.name.span == callee);
        let position = if tag {
            synchronous_tag_position(lookup, caller, callee)
        } else {
            Some(callee)
        };
        position.is_some_and(|position| {
            runs_in_pass(lookup, caller, position, creator, depth - 1, false)
        })
    })
}
