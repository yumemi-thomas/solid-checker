//! Completion proofs: never-returning calls, parameters called on every completion, and result-access forwarding.

use super::project_memos::{NeverReturning, ParameterCompletionCalls};
use super::*;

impl<'a> SemanticLookup<'a> {
    /// ADR 0277: whether the synchronous project function at `function` in
    /// `path` calls its single-name parameter `index`, by that parameter's
    /// symbol, at a completion site of its own body.
    pub(crate) fn parameter_called_on_every_completion(
        &self,
        path: &str,
        function: Span,
        index: usize,
    ) -> bool {
        self.parameter_completion_calls()
            .contains(&(path, function, index))
    }

    /// ADR 0278: whether a call TypeScript resolves to a `never`-returning
    /// signature is written directly in `function` before `site`. It may run
    /// first and throw, so `site` is not on every completion a caller can
    /// continue from.
    pub(crate) fn never_returning_call_before(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
        site: Span,
    ) -> bool {
        self.never_returning_calls()
            .get(file.path.as_str())
            .is_some_and(|calls| {
                // In the body or the parameter list (a default may call one),
                // never in a nested function.
                calls.iter().any(|call| {
                    call.end <= site.start
                        && function.span.contains(*call)
                        && !file.ast.functions_within(function.span).any(|nested| {
                            nested.span != function.span && nested.span.contains(*call)
                        })
                })
            })
    }

    /// ADR 0280: the `never` and `asserts` calls written in `function`'s
    /// body or parameter list, outside nested functions.
    pub(crate) fn never_returning_calls_within(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> Vec<Span> {
        self.never_returning_calls()
            .get(file.path.as_str())
            .map(|calls| {
                calls
                    .iter()
                    .copied()
                    .filter(|call| {
                        function.span.contains(*call)
                            && !file.ast.functions_within(function.span).any(|nested| {
                                nested.span != function.span && nested.span.contains(*call)
                            })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// ADR 0280: whether `function` tests one of its parameters, so that a
    /// call's arguments may decide one of its branches.
    pub(crate) fn tests_its_parameters(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
    ) -> bool {
        self.parameter_testing_functions
            .get_or_init(|| {
                let facts: &'a ProjectFacts = self.facts;
                let mut testing = HashSet::default();
                for file in &facts.files {
                    let ast = &file.ast;
                    let mut owners = HashMap::<Span, Span>::default();
                    for function in &ast.functions {
                        for parameter in &function.parameters {
                            for name in &parameter.names {
                                owners.insert(name.span, function.span);
                            }
                        }
                    }
                    if owners.is_empty() {
                        continue;
                    }
                    let tests = ast
                        .conditional_tests
                        .iter()
                        .copied()
                        .chain(ast.logical_expressions.iter().map(|logical| logical.left))
                        .collect::<Vec<_>>();
                    for (reference, declaration) in &ast.reference_declarations {
                        if let Some(function) = owners.get(declaration)
                            && function.contains(*reference)
                            && tests.iter().any(|test| test.contains(*reference))
                        {
                            testing.insert((file.path.as_str(), *function));
                        }
                    }
                }
                testing
            })
            .contains(&(file.path.as_str(), function.span))
    }

    /// ADR 0278: the `never` and `asserts` calls of each path, a view of the
    /// per-file [`NeverReturning`] memos built once per build.
    pub(super) fn never_returning_calls(&self) -> &HashMap<&'a str, Vec<Span>> {
        crate::base::reads::note(crate::base::reads::Section::NeverReturning);
        self.never_returning_calls
            .get_or_init(|| crate::base::reads::suppress(|| self.never_returning_calls_uncached()))
    }

    fn never_returning_calls_uncached(&self) -> HashMap<&'a str, Vec<Span>> {
        {
            let world = self.memo_world();
            let mut calls = HashMap::<&'a str, Vec<Span>>::default();
            for file in self.files() {
                let path = file.path.as_str();
                if calls.contains_key(path) {
                    continue;
                }
                let Some(id) = world.views().table.id_of(path) else {
                    continue;
                };
                let spans = world.answer::<NeverReturning>(&id);
                if !spans.is_empty() {
                    calls.insert(path, spans);
                }
            }
            calls
        }
    }

    /// ADR 0277: a view of the [`ParameterCompletionCalls`] memo, built once
    /// per build.
    pub(super) fn parameter_completion_calls(&self) -> &HashSet<(&'a str, Span, usize)> {
        self.parameter_completion_calls.get_or_init(|| {
            self.whole_program_answer::<ParameterCompletionCalls>()
                .into_iter()
                .filter_map(|(path, span, index)| {
                    Some((self.file_by_path(&path)?.path.as_str(), span, index))
                })
                .collect()
        })
    }

    /// The memo's computation: every `(path, function, parameter)` called on
    /// the function's every completion, sorted. The forwards fixed point
    /// reads the whole program (study S7 moves it onto its cone).
    pub(super) fn compute_parameter_completion_calls(&self) -> Vec<(String, Span, usize)> {
        {
            let facts: &'a ProjectFacts = self.facts;
            let mut calls = HashSet::default();
            // ADR 0285: calls passing a parameter on to another function.
            let mut forwards = Vec::new();
            for file in &facts.files {
                // Direct `eval` can write any parameter it sees (review
                // round 1 of ADR 0285).
                if file
                    .ast
                    .module_hazards
                    .iter()
                    .any(|hazard| hazard.kind == solid_facts::ast::ModuleHazardKind::Eval)
                    || file.ast.calls.iter().any(|call| {
                        file.source_text(file.ast.peel_ts_sugar_span(call.callee)) == Some("eval")
                    })
                {
                    continue;
                }
                let mut parameters = HashMap::<&str, (Span, usize)>::default();
                for function in &file.ast.functions {
                    if function.r#async || function.generator {
                        continue;
                    }
                    for (index, parameter) in function.parameters.iter().enumerate() {
                        if parameter.shape != solid_facts::ast::BindingShape::Identifier {
                            continue;
                        }
                        let Some(name) = parameter.names.first() else {
                            continue;
                        };
                        // A written parameter may hold a replacement by the
                        // time it is called (`cb = () => 0; cb()`).
                        if binding_written(file, name.span) {
                            continue;
                        }
                        if let Some(symbol) = self.entities().at(file.path.as_str(), name.span) {
                            parameters.insert(symbol.as_str(), (function.span, index));
                        }
                    }
                }
                if parameters.is_empty() {
                    continue;
                }
                for call in &file.ast.calls {
                    let Some(&(function, index)) = self
                        .callee_symbol(file, call.callee)
                        .and_then(|symbol| parameters.get(symbol))
                    else {
                        continue;
                    };
                    if crate::owners::containing_ast_function(&file.ast, call.span)
                        .is_some_and(|owner| owner.span == function)
                        && crate::interproc::site_on_every_completion(
                            self, file, function, call.span,
                        )
                    {
                        calls.insert((file.path.as_str(), function, index));
                    }
                }
                for call in &file.ast.calls {
                    for (position, argument) in call.arguments.iter().enumerate() {
                        let span = file.ast.peel_ts_sugar_span(argument.span);
                        if argument.spread
                            || file
                                .ast
                                .identifiers
                                .binary_search_by_key(&span, |identifier| identifier.span)
                                .is_err()
                        {
                            continue;
                        }
                        let Some(&(function, index)) = self
                            .entities()
                            .at(file.path.as_str(), span)
                            .and_then(|symbol| parameters.get(symbol.as_str()))
                        else {
                            continue;
                        };
                        if crate::owners::containing_ast_function(&file.ast, call.span)
                            .is_some_and(|owner| owner.span == function)
                            && crate::interproc::site_on_every_completion(
                                self, file, function, call.span,
                            )
                        {
                            forwards.push((file, call, position, function, index));
                        }
                    }
                }
            }
            // ADR 0285: `function f(cb) { g(cb) }` calls `cb` on every
            // completion when the call is on f's every-completion path, the
            // argument is the parameter itself, unchanged, at an exactly
            // mapped position, and `g` calls that position on every
            // completion. `g` must be a function declaration: a `const`
            // callee may be in its temporal dead zone (ADR 0218). To a
            // fixpoint, so a chain of forwards composes.
            let mut changed = true;
            while changed {
                changed = false;
                for &(file, call, position, function, index) in &forwards {
                    let key = (file.path.as_str(), function, index);
                    if calls.contains(&key) {
                        continue;
                    }
                    let forwarded = crate::execution_role::project_callee_at_parameter(
                        file, call, position, self,
                    )
                    .is_some_and(|(callee_file, callee, exact)| {
                        exact
                            && callee.kind == solid_facts::ast::FunctionKind::Declaration
                            && calls.contains(&(callee_file.path.as_str(), callee.span, position))
                    });
                    if forwarded {
                        calls.insert(key);
                        changed = true;
                    }
                }
            }
            let mut answer = calls
                .into_iter()
                .map(|(path, span, index)| (path.to_owned(), span, index))
                .collect::<Vec<_>>();
            answer.sort_unstable();
            answer
        }
    }

    /// The arguments in `path` that a project function forwards, unchanged,
    /// into a slot the dialect says runs when the call's returned object is
    /// read ([`solid_dialect::Dialect::callback_runs_on_result_access`]).
    ///
    /// `function hideBy(props, hidden) { return omit(props, hidden); }` makes
    /// the second argument of every in-project `hideBy(…)` call such a
    /// callback: its code runs on reads of the view, wherever those happen,
    /// exactly as if it were written at `omit`. One level of forwarding, by
    /// exact parameter identity (a single-name parameter binding the
    /// argument's declaration resolves to) and exact call-site resolution
    /// ([`Self::function_call_sites`]); a wrapper of a wrapper, a parameter
    /// renamed through a local, or a destructured parameter is not followed.
    pub(crate) fn result_access_forwarded_arguments(&self, path: &str) -> &[ResultAccessForwarded] {
        self.result_access_forwarded_index()
            .get(path)
            .map_or(&[], Vec::as_slice)
    }

    pub(super) fn result_access_forwarded_index(
        &self,
    ) -> &HashMap<&'a str, Vec<ResultAccessForwarded>> {
        self.result_access_forwarded.get_or_init(|| {
            let table = self.memos.table();
            let files = self.files();
            self.memo_world()
                .answer::<super::ResultAccessForwardedAll>(&())
                .iter()
                .filter_map(|(id, entries)| {
                    let slot = *table.slots_of(*id).first()?;
                    Some((
                        files[slot].path.as_str(),
                        entries
                            .iter()
                            .map(|(argument, primitive)| ResultAccessForwarded {
                                argument: *argument,
                                primitive: *primitive,
                            })
                            .collect(),
                    ))
                })
                .collect()
        })
    }

    /// Whether the dialect answers a result-access slot for some primitive the
    /// project names: most variants answer none, and the walk is skipped.
    pub(super) fn result_access_slots_answered(&self) -> bool {
        self.project_primitives().iter().any(|primitive| {
            (0..PROBED_ARGUMENTS).any(|count| {
                (0..count).any(|argument| {
                    self.dialect
                        .callback_runs_on_result_access(*primitive, argument, count)
                })
            })
        })
    }
}
