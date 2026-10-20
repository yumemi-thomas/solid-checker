//! Completion proofs: never-returning calls, parameters called on every completion, and result-access forwarding.

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

    pub(super) fn never_returning_calls(&self) -> &HashMap<&'a str, Vec<Span>> {
        self.never_returning_calls.get_or_init(|| {
            let facts: &'a ProjectFacts = self.facts;
            let mut calls = HashMap::<&'a str, Vec<Span>>::default();
            for file in &facts.files {
                for call in &file.ast.calls {
                    if self
                        .resolved_callee_call(file, call.callee)
                        .is_some_and(|resolved| {
                            // The compiler's own type flag, not the display
                            // text (docs/typefacts/compiler-semantic-facts.md).
                            resolved.never_returns || resolved.assertion_signature
                        })
                    {
                        calls.entry(file.path.as_str()).or_default().push(call.span);
                    }
                }
            }
            for spans in calls.values_mut() {
                spans.sort_unstable();
            }
            calls
        })
    }

    pub(super) fn parameter_completion_calls(&self) -> &HashSet<(&'a str, Span, usize)> {
        self.parameter_completion_calls.get_or_init(|| {
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
            calls
        })
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
            let mut forwarded = HashMap::<&'a str, Vec<ResultAccessForwarded>>::default();
            // Most dialect variants answer no slot at all; skip the walk.
            let answered = self.project_primitives().iter().any(|primitive| {
                (0..PROBED_ARGUMENTS).any(|count| {
                    (0..count).any(|argument| {
                        self.dialect
                            .callback_runs_on_result_access(*primitive, argument, count)
                    })
                })
            });
            if !answered {
                return forwarded;
            }
            let facts: &'a ProjectFacts = self.facts;
            for file in &facts.files {
                for call in &file.ast.calls {
                    let Some(primitive) = self.primitive_at_call(file, call.span) else {
                        continue;
                    };
                    let count = call.arguments.len();
                    for (index, argument) in call.arguments.iter().enumerate() {
                        if argument.spread
                            || argument.value != solid_facts::ast::ArgumentValueKind::Identifier
                            || !self
                                .dialect
                                .callback_runs_on_result_access(primitive, index, count)
                        {
                            continue;
                        }
                        let Some(declaration) = argument.binding_declaration else {
                            continue;
                        };
                        for function in file.ast.functions_body_containing(call.span) {
                            let Some(parameter) = function.parameters.iter().position(|binding| {
                                binding.names.len() == 1 && binding.names[0].span == declaration
                            }) else {
                                continue;
                            };
                            for (caller, callee) in
                                self.function_call_sites(file.path.as_str(), function.span)
                            {
                                let Some(site) =
                                    caller.ast.calls.iter().find(|site| site.callee == callee)
                                else {
                                    continue;
                                };
                                // A spread up to the slot moves arguments into it.
                                if site
                                    .arguments
                                    .iter()
                                    .take(parameter.saturating_add(1))
                                    .any(|candidate| candidate.spread)
                                {
                                    continue;
                                }
                                if let Some(forwarded_argument) = site.arguments.get(parameter) {
                                    forwarded.entry(caller.path.as_str()).or_default().push(
                                        ResultAccessForwarded {
                                            argument: forwarded_argument.span,
                                            primitive,
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
            }
            for entries in forwarded.values_mut() {
                entries.sort_unstable_by_key(|entry| (entry.argument.start, entry.argument.end));
                entries.dedup_by_key(|entry| entry.argument);
            }
            forwarded
        })
    }
}
