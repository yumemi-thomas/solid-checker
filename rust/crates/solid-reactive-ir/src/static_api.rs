//! Static validation for Solid 2 API shapes and explicit refresh writes.

use solid_dialect::Primitive;

use super::*;

pub(super) struct StaticDirectiveFileResult {
    pub(super) violations: Vec<StaticViolation>,
    pub(super) writes: Vec<ReactiveWrite>,
    pub(super) write_action_obligations: Vec<(&'static str, String, u64, u64)>,
}

pub(super) struct StaticApiContext<'a> {
    pub(super) lookup: &'a SemanticLookup<'a>,
    pub(super) entities: &'a EntitySymbols,
    pub(super) symbol_names: &'a HashMap<SymbolId, SymbolId>,
    pub(super) source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
    pub(super) source_owned_write: &'a HashMap<SymbolId, bool>,
    pub(super) accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(super) reachable_calls: &'a HashMap<Location, usize>,
}

impl StaticApiContext<'_> {
    pub(super) fn check_file(&self, file: &FileFacts) -> StaticDirectiveFileResult {
        let mut result = StaticDirectiveFileResult {
            violations: Vec::new(),
            writes: Vec::new(),
            write_action_obligations: Vec::new(),
        };
        let allowed = allowed_callback_spans(file, self.lookup);
        let dialect = self.lookup.dialect;
        for call in &file.ast.calls {
            let Some(primitive) = primitive_name(
                file.path.as_str(),
                call.callee,
                call.static_callee(&file.source),
                self.entities,
                self.symbol_names,
                dialect,
            ) else {
                continue;
            };
            // `primitive` stays as the spelling to report; `kind` is what the
            // rules below branch on. A name this dialect does not export
            // resolves to `None` and matches nothing.
            let kind = primitive.primitive();
            // Where the effect function has to be is a dialect question, and
            // the same one `callback_positions` already answers: 2.0 puts it
            // at index 1 of `createEffect(compute, apply)`, 1.x at index 0 of
            // `createEffect(fn, value?)`. Hardcoding index 1 would fire this
            // rule on every correct 1.x effect.
            if kind == Some(Primitive::CreateEffect)
                && let Some(&index) = dialect.callback_positions(Primitive::CreateEffect).last()
                && call.arguments.get(index).is_none_or(|argument| {
                    argument.value == solid_facts::ast::ArgumentValueKind::Undefined
                })
            {
                let signature = dialect.effect_signature();
                result.violations.push(StaticViolation {
                    id: "SC7001".into(),
                    rule: "missing-effect-function".into(),
                    message: format!(
                        "createEffect is called without an effect function; the signature is {}, where {}",
                        signature.signature, signature.roles
                    ),
                    hint: signature.remedy.into(),
                    location: location(file.path.shared(), call.callee),
                    analysis_context: String::new(),
                    fixes: vec![],
                });
            }
            // Where each primitive takes its options object -- a second index
            // vocabulary, and the dialect's to answer: 2.0's numbers read
            // 1.x's `createMemo(fn, seed)` seed as an options object.
            let options_index = kind.and_then(|kind| dialect.options_argument(kind));
            if let Some(options_index) = options_index
                && call.arguments.get(options_index).is_some_and(|argument| {
                    argument.boolean_properties.iter().any(|property| {
                        file.source_text(property.name) == Some("sync") && property.value
                    })
                })
                && call
                    .arguments
                    .first()
                    .is_some_and(|argument| computation_is_async(self.lookup, file, argument.span))
            {
                result.violations.push(StaticViolation {
                    id: "SC7002".into(),
                    rule: "sync-node-received-async".into(),
                    message: format!(
                        "{primitive} is marked sync: true but its computation can return a Promise or AsyncIterable; a sync node must settle in the same flush and cannot suspend, so an async result throws at runtime"
                    ),
                    hint: format!(
                        "Drop sync: true and let the read suspend to a <{}> boundary, or make the computation synchronous by moving the async work into its own computation and reading the settled accessor here.",
                        dialect.boundary_name(solid_dialect::Boundary::Async)
                    ),
                    location: location(file.path.shared(), call.callee),
                    analysis_context: String::new(),
                    fixes: vec![],
                });
            }
            let Some(kind @ (Primitive::Refresh | Primitive::Affects)) = kind else {
                continue;
            };
            let is_refresh = kind == Primitive::Refresh;
            let invalid_arity = call.arguments.is_empty()
                || is_refresh && call.arguments.len() != 1
                || !is_refresh && call.arguments.len() > 2;
            if invalid_arity {
                result.violations.push(StaticViolation {
                    id: "SC7003".into(),
                    rule: format!("invalid-{primitive}-target"),
                    message: if is_refresh {
                        "refresh() takes exactly one argument: the derived signal, store, or projection to recompute, or a thunk to re-run".into()
                    } else {
                        "affects() takes a source target and optionally an array of store keys".into()
                    },
                    hint: if is_refresh {
                        "Pass one target: refresh(source) to recompute a derived source, or refresh(() => expr) to re-run an expression and return its value.".into()
                    } else {
                        "Call affects(source) for signals, or affects(store, [\"key\"]) to scope invalidation to specific store paths.".into()
                    },
                    location: location(file.path.shared(), call.callee),
                    analysis_context: String::new(),
                    fixes: vec![],
                });
                continue;
            }
            let target = &call.arguments[0];
            if target.value != solid_facts::ast::ArgumentValueKind::Identifier {
                result.violations.push(StaticViolation {
                    id: "SC7003".into(),
                    rule: format!("invalid-{primitive}-target"),
                    message: format!(
                        "{primitive}() received a wrapper, read value, or literal instead of the original Solid source binding; the brand on the binding created by createSignal, createMemo, or createStore is how Solid identifies what to recompute"
                    ),
                    hint: if is_refresh {
                        "Pass the accessor or store exactly as returned by its create call, uncalled and unwrapped: refresh(user), not refresh(user()). To refresh an ad-hoc expression, use the thunk form refresh(() => expr).".into()
                    } else {
                        "Pass the accessor or store exactly as returned by its create call, uncalled and unwrapped: affects(user), not affects(user()).".into()
                    },
                    location: location(file.path.shared(), target.span),
                    analysis_context: String::new(),
                    fixes: vec![],
                });
                continue;
            }
            let target_location = location(file.path.shared(), target.span);
            let Some(symbol) = self.entities.get(&target_location) else {
                result.violations.push(StaticViolation {
                    id: "SC9003".into(),
                    rule: format!("{primitive}-target-unresolved"),
                    message: format!(
                        "cannot trace the target of {primitive}() back to a Solid source; solid-checker cannot prove it is a branded accessor, store, or projection, so this call may throw at runtime"
                    ),
                    hint: "Pass the binding created by createSignal, createMemo, createStore, or createProjection directly. If the source is re-exported or wrapped by a package, declare that export's return kind in the package's reactivity contract so the brand survives the import.".into(),
                    location: target_location,
                    analysis_context: String::new(),
                    fixes: vec![],
                });
                continue;
            };
            let Some(kind) = self.source_kinds.get(symbol).copied() else {
                result.violations.push(StaticViolation {
                    id: "SC9003".into(),
                    rule: format!("{primitive}-target-unresolved"),
                    message: format!(
                        "cannot trace the target of {primitive}() back to a Solid source; solid-checker cannot prove it is a branded accessor, store, or projection, so this call may throw at runtime"
                    ),
                    hint: "Pass the binding created by createSignal, createMemo, createStore, or createProjection directly. If the source is re-exported or wrapped by a package, declare that export's return kind in the package's reactivity contract so the brand survives the import.".into(),
                    location: target_location,
                    analysis_context: String::new(),
                    fixes: vec![],
                });
                continue;
            };
            if !is_refresh {
                if kind == ReactiveSourceKind::Accessor && call.arguments.len() == 2 {
                    result.violations.push(StaticViolation {
                        id: "SC7004".into(),
                        rule: "affects-keys-on-accessor".into(),
                        message: "affects() received keys but its target is a signal accessor; keys narrow invalidation to paths inside a store, and an accessor has no paths".into(),
                        hint: "Drop the key array for signal targets (affects(source)), or pass the store binding if you meant to scope invalidation to specific store keys (affects(store, [\"todos\"])).".into(),
                        location: location(file.path.shared(), call.callee),
                        analysis_context: String::new(),
                        fixes: vec![],
                    });
                }
                continue;
            }
            if file.ast.any_function_body_containing(call.span) {
                result.write_action_obligations.push((
                    "write",
                    file.path.to_string(),
                    u64::from(call.callee.start),
                    u64::from(call.callee.end),
                ));
            }
            let callee = location(file.path.shared(), call.callee);
            let Some(multiplicity) = self.reachable_calls.get(&callee).copied() else {
                continue;
            };
            let Some((name, declaration)) = self.accessors.get(symbol) else {
                continue;
            };
            for _ in 0..multiplicity {
                result.writes.push(ReactiveWrite {
                    setter: format!("refresh({name})").into(),
                    location: location(
                        file.path.as_str(),
                        Span::new(
                            call.span.start,
                            call.arguments
                                .last()
                                .map_or(call.span.end, |argument| argument.span.end),
                        ),
                    ),
                    declaration: declaration.clone(),
                    execution: semantic_execution_role(
                        file,
                        call.callee,
                        &allowed,
                        self.entities,
                        self.symbol_names,
                        self.lookup,
                    ),
                    allowed_by_option: self
                        .source_owned_write
                        .get(symbol)
                        .copied()
                        .unwrap_or(false),
                    context: analysis_context(
                        file,
                        call.span,
                        self.entities,
                        self.symbol_names,
                        dialect,
                    )
                    .into(),
                });
            }
        }
        result
    }
}
