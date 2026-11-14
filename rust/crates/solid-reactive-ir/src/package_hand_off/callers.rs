//! ADR 0980: the complete list of the calls that run a project function, for a store through a parameter (`function put(t) { t.m = f; }`).
//!
//! The list is complete or there is none (`None`: the callers are not known, and the store is handed). A function no binding names (an object-literal or class method, an
//! IIFE's argument), a binding some assignment reassigns, a missing symbol, reference list or file, a use that is no direct call (a copy, an argument, a tag, a re-export,
//! an export specifier of another module), an export a program may enter, and a namespace that hands the function out by name all end in `None`. Never `Vec::new()`: an
//! empty list says "no caller", which only a complete census can say, and the census of `foreign_reference_hand_offs` (which other rules read) skips a call the call graph
//! holds, a missing name or a missing file, and so is no census of callers.

use super::*;

impl<'l, 'a> UnreadPackageHandOff<'l, 'a> {
    /// The calls that run `function`, when every reference of its binding (in every file of the program) is a direct call of it; `None` when anything else may call it.
    pub(super) fn exact_callers(
        &self,
        file: &'a FileFacts,
        function: &'a FunctionFact,
    ) -> Option<Vec<(&'a FileFacts, &'a CallFact)>> {
        let lookup = self.lookup;
        // A method is entered by member access through any receiver holding it; a function nothing names is a value handed wherever it is written.
        if function.method_name.is_some()
            || crate::owners::own_method_name(file, function).is_some()
            || file
                .ast
                .classes
                .iter()
                .any(|class| class.span.contains(function.span))
        {
            return None;
        }
        // The names the function is entered through: a named function expression through its own name and the binding holding it.
        let mut names = Vec::new();
        if function.kind == FunctionKind::Expression && function.name.is_some() {
            let own = function.name.as_ref()?;
            let outer = crate::execution_role::named_expression_binding(file, function, lookup)?;
            names.extend([own.span, outer.span]);
        } else {
            names.push(crate::owners::function_binding_name(file, function)?.span);
        }
        if crate::execution_role::entered_by_unseen_code(file, function, lookup) {
            return None;
        }
        let call_graph = lookup.function_call_site_references(file.path.as_str(), function.span);
        let mut found: Vec<(&'a FileFacts, &'a CallFact)> = Vec::new();
        let mut seen_names: HashSet<Key> = HashSet::new();
        let mut pending: Vec<(&'a FileFacts, Span)> =
            names.into_iter().map(|name| (file, name)).collect();
        while let Some((holder, name)) = pending.pop() {
            if !seen_names.insert((holder.path.shared(), name)) {
                continue;
            }
            let symbol = lookup
                .entities()
                .exact(&name.location(holder.path.shared()))?;
            lookup.symbol_references_if_present(symbol.as_str())?;
            // A binding something reassigns names another function after that.
            if holder.ast.references_to(name).iter().any(|reference| {
                holder
                    .ast
                    .write_targets
                    .iter()
                    .any(|target| holder.ast.peel_ts_sugar_span(*target) == *reference)
            }) {
                return None;
            }
            for reference in lookup.symbol_references_through_redirects(symbol.as_str()) {
                let at = Span::new(
                    u32::try_from(reference.start_byte).ok()?,
                    u32::try_from(reference.end_byte).ok()?,
                );
                let in_file = self.files.get(reference.path.as_ref()).copied()?;
                if in_file.path == holder.path && at == name {
                    continue;
                }
                if in_file
                    .ast
                    .type_queries
                    .iter()
                    .any(|query| query.contains(at))
                {
                    continue;
                }
                // A call or a render the call graph holds, or a direct call in any case: the call written there.
                let accounted = call_graph
                    .iter()
                    .any(|(site, span)| site.path == in_file.path && *span == at);
                let call = in_file
                    .ast
                    .calls_with_peeled_callee(at)
                    .find(|call| !call.construct);
                if let Some(call) = call
                    && (accounted || call.direct_callee)
                {
                    if !found.iter().any(|(held, held_call)| {
                        held.path == in_file.path && held_call.span == call.span
                    }) {
                        found.push((in_file, call));
                    }
                    continue;
                }
                if accounted {
                    // A render (`<Put />`) is no call written there.
                    return None;
                }
                // The module's own export of the function: its importers are references of the symbol too, and the export itself is no use.
                if in_file.path == file.path
                    && holder.path == file.path
                    && in_file
                        .ast
                        .exports
                        .iter()
                        .filter(|export| export.module.is_none())
                        .flat_map(|export| {
                            export.specifiers.iter().chain(export.declarations.iter())
                        })
                        .any(|specifier| specifier.local.span == at)
                    && names_declare_function(file, function, at)
                {
                    continue;
                }
                // An import of the function names it again under the local name; its uses are the references of that name.
                if in_file
                    .ast
                    .imports
                    .iter()
                    .flat_map(|import| &import.bindings)
                    .any(|binding| binding.local.span == at)
                {
                    pending.push((in_file, at));
                    continue;
                }
                return None;
            }
        }
        Some(found)
    }
}

/// Whether the span of an export specifier's local names the function's own binding (`export { put }` in the function's own module).
fn names_declare_function(file: &FileFacts, function: &FunctionFact, at: Span) -> bool {
    let declared = file.ast.reference_declaration(at).unwrap_or(at);
    crate::owners::function_binding_name(file, function).is_some_and(|name| name.span == declared)
        || function
            .name
            .as_ref()
            .is_some_and(|name| name.span == declared)
}
