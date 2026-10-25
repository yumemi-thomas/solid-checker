//! Callbacks registered with a reviewed test runner (ADR 0396).

use super::*;

impl SemanticLookup<'_> {
    /// Whether `call` registers a test, a suite body or a hook with a reviewed
    /// test runner ([`crate::runtime_semantics::TEST_RUNNER_DECLARATIONS`]): its
    /// callee is a registration function reached through reviewed modifier
    /// members (`it`, `it.only`, `test.describe.serial`, `test.each(rows)`),
    /// whose root identifier resolves, through its imports, to declarations that
    /// all lie in a runner's installed declaration files and are named for a
    /// registration function. A project function named `it` resolves to a
    /// project declaration and is not one; a function the call merely hands the
    /// callback to (`wrap(fn)`) is not the runner, so the caller reads only a
    /// function literal written directly as the argument.
    pub(crate) fn is_test_runner_registration(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
    ) -> bool {
        if call.construct {
            return false;
        }
        let Some(root) = Self::registration_root(file, call.callee) else {
            return false;
        };
        self.registration_root_is_declared_by_runner(file, root)
    }

    /// The root identifier of a registration callee chain
    /// (`test.describe.serial`, `it.each(rows)`), or `None` when a member or a
    /// call in the chain is not a reviewed modifier.
    fn registration_root(file: &solid_facts::FileFacts, callee: Span) -> Option<Span> {
        let mut span = file.ast.peel_ts_sugar_span(callee);
        loop {
            if file.ast.computed_members.binary_search(&span).is_ok() {
                return None;
            }
            if let Some(member) = file.ast.member_at(span) {
                let property = file.source_text(member.property)?;
                if !crate::runtime_semantics::test_runner_modifier(property) {
                    return None;
                }
                span = file.ast.peel_ts_sugar_span(member.object);
            } else if let Some(inner) = file.ast.call_at(span) {
                // `test.each(rows)(name, fn)`: the call before the
                // registration is one of the curried modifiers.
                let inner_callee = file.ast.peel_ts_sugar_span(inner.callee);
                let member = file.ast.member_at(inner_callee)?;
                let property = file.source_text(member.property)?;
                if inner.construct
                    || file
                        .ast
                        .computed_members
                        .binary_search(&inner_callee)
                        .is_ok()
                    || !crate::runtime_semantics::test_runner_curried_modifier(property)
                {
                    return None;
                }
                span = inner_callee;
            } else {
                break;
            }
        }
        file.ast
            .identifiers
            .binary_search_by_key(&span, |identifier| identifier.span)
            .is_ok()
            .then_some(span)
    }

    /// Whether the identifier at `root` is a runner's registration function:
    /// the symbol, followed through its alias chain, is declared only in a
    /// reviewed runner's declaration files outside the analyzed program, under
    /// a registration function's name.
    fn registration_root_is_declared_by_runner(
        &self,
        file: &solid_facts::FileFacts,
        root: Span,
    ) -> bool {
        let Some(mut symbol) = self.symbol_at(file.path.as_str(), root) else {
            return false;
        };
        for _ in 0..MAX_ALIAS_HOPS {
            if symbol.alias_target().is_empty() {
                break;
            }
            let Some(target) = self.symbols_by_id().get(symbol.alias_target()).copied() else {
                return false;
            };
            symbol = target;
        }
        if !symbol.alias_target().is_empty() {
            return false;
        }
        let declarations = symbol.declarations();
        !declarations.is_empty()
            && declarations.iter().all(|declaration| {
                let path = declaration.location.path.as_ref();
                self.file_by_path(path).is_none()
                    && crate::runtime_semantics::test_runner_function_declaration(
                        path,
                        declaration.name.as_ref(),
                    )
            })
    }
}

/// Re-export hops followed from an import to the declaration.
const MAX_ALIAS_HOPS: usize = 8;
