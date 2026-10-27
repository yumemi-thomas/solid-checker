//! Callbacks registered with a reviewed test runner (ADR 0438).

use super::*;

/// How a reviewed runner runs the function literal handed to a registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TestRunnerCallback {
    /// From the runner's own scheduler, later: a test, a hook, or the body of
    /// a suite the runner collects after the file loads.
    Scheduled,
    /// Inside the registration call (a suite body of a runner that runs it
    /// there): the body takes the role of the call's own position.
    InlineSuiteBody,
}

impl SemanticLookup<'_> {
    /// How a reviewed test runner runs the callback `call` hands it, when
    /// `call` registers a test, a suite body or a hook
    /// ([`crate::runtime_semantics::TEST_RUNNER_DECLARATIONS`]): its callee is
    /// a registration function reached through reviewed modifier members
    /// (`it`, `it.only`, `test.describe.serial`, `test.each(rows)(…)`), whose
    /// root identifier resolves, through its imports, to declarations that all
    /// lie in a runner's installed declaration files and are named for a
    /// registration function. A project function named `it` resolves to a
    /// project declaration and is not one; a function the call merely hands the
    /// callback to (`wrap(fn)`) is not the runner, so the caller reads only a
    /// function literal written directly as the argument. The call that
    /// registers is the outermost one: the preliminary call of a curried
    /// registration (`describe.each(rows)`, `it.skipIf(cond)`) takes data, not
    /// a callback. A suite body is placed only for a runner whose bytes show
    /// when it runs the body ([`crate::runtime_semantics::SuiteBodyRun`]);
    /// `None` also answers a suite of a runner whose bytes show nothing.
    pub(crate) fn test_runner_callback(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
    ) -> Option<TestRunnerCallback> {
        if call.construct || Self::is_curried_preliminary_call(file, call.callee) {
            return None;
        }
        let (root, suite_modifier) = Self::registration_root(file, call.callee)?;
        let (suite_function, suite_bodies) = self.registration_root_runner(file, root)?;
        if !(suite_modifier || suite_function) {
            return Some(TestRunnerCallback::Scheduled);
        }
        use crate::runtime_semantics::SuiteBodyRun;
        match suite_bodies {
            SuiteBodyRun::Scheduled => Some(TestRunnerCallback::Scheduled),
            SuiteBodyRun::Inline => Some(TestRunnerCallback::InlineSuiteBody),
            SuiteBodyRun::Unproven => None,
        }
    }

    /// Whether `callee` ends in a member that takes data and returns the
    /// registration function (`describe.each`, `it.skipIf`): a call of it is
    /// the preliminary call of a curried registration, never the registration.
    fn is_curried_preliminary_call(file: &solid_facts::FileFacts, callee: Span) -> bool {
        let span = file.ast.peel_ts_sugar_span(callee);
        file.ast.member_at(span).is_some_and(|member| {
            file.source_text(member.property)
                .is_some_and(crate::runtime_semantics::test_runner_curried_modifier)
        })
    }

    /// The root identifier of a registration callee chain
    /// (`test.describe.serial`, `it.each(rows)`) and whether a `describe`
    /// member makes it a suite registration, or `None` when a member or a call
    /// in the chain is not a reviewed modifier.
    fn registration_root(file: &solid_facts::FileFacts, callee: Span) -> Option<(Span, bool)> {
        let mut span = file.ast.peel_ts_sugar_span(callee);
        let mut suite = false;
        loop {
            if file.ast.computed_members.binary_search(&span).is_ok() {
                return None;
            }
            if let Some(member) = file.ast.member_at(span) {
                let property = file.source_text(member.property)?;
                if !crate::runtime_semantics::test_runner_modifier(property) {
                    return None;
                }
                suite |= crate::runtime_semantics::test_runner_suite_modifier(property);
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
            .then_some((span, suite))
    }

    /// Whether the identifier at `root` is a runner's registration function,
    /// and what it registers: the symbol, followed through its alias chain, is
    /// declared only in a reviewed runner's declaration files outside the
    /// analyzed program, under a registration function's name. Answers whether
    /// every declaration is named for a suite, and the one suite-body timing
    /// all declarations agree on (declarations in different runners give the
    /// unproven one).
    fn registration_root_runner(
        &self,
        file: &solid_facts::FileFacts,
        root: Span,
    ) -> Option<(bool, crate::runtime_semantics::SuiteBodyRun)> {
        use crate::runtime_semantics::SuiteBodyRun;
        // The symbol's alias target and declarations, in any file (a project
        // script merging a declaration into a runner's global, a module
        // augmentation, a re-export edited elsewhere), are read from the
        // TypeScript table: a recorded read of each symbol visited, the root
        // included, as `intrinsic_tag_prop` does.
        let note = |id: &str| {
            self.note_probe_read(crate::base::memo::probes::Probe::SymbolInfo, || {
                Self::symbol_key(id)
            });
        };
        let mut symbol = self.symbol_at(file.path.as_str(), root)?;
        for _ in 0..MAX_ALIAS_HOPS {
            note(symbol.id());
            if symbol.alias_target().is_empty() {
                break;
            }
            // An absent target is an answer too.
            note(symbol.alias_target());
            let target = self.symbols_by_id().get(symbol.alias_target()).copied()?;
            symbol = target;
        }
        note(symbol.id());
        if !symbol.alias_target().is_empty() {
            return None;
        }
        let declarations = symbol.declarations();
        if declarations.is_empty() {
            return None;
        }
        let mut suite = None::<bool>;
        let mut bodies = None::<SuiteBodyRun>;
        for declaration in declarations {
            let path = declaration.location.path.as_ref();
            if self.file_by_path(path).is_some() {
                return None;
            }
            let name = declaration.name.as_ref();
            let timing = crate::runtime_semantics::test_runner_function_declaration(path, name)?;
            let is_suite = crate::runtime_semantics::test_runner_suite_function(name);
            if *suite.get_or_insert(is_suite) != is_suite {
                return None;
            }
            bodies = Some(match bodies {
                Some(seen) if seen != timing => SuiteBodyRun::Unproven,
                _ => timing,
            });
        }
        Some((suite?, bodies?))
    }
}

/// Re-export hops followed from an import to the declaration.
const MAX_ALIAS_HOPS: usize = 8;
