//! Extraction entry point: `extract`, the `Collector` state and its non-visitor methods.

use super::*;

/// Whether `path`'s extension marks it as a JSON module rather than
/// JavaScript or TypeScript.
///
/// A `.json` specifier is legitimate ESM (`import pkg from "./package.json"`,
/// with or without a bundler-inferred `type: "json"` assertion): the module
/// system resolves it to a real file, but that file has no JS grammar to
/// speak of. This is a fact about the module *kind*, never about one
/// filename -- `package.json`, `data.json`, and any other `.json` specifier
/// all take the same inert path through [`extract`].
#[must_use]
pub fn is_json_module_path(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
}

pub fn extract(path: impl Into<String>, source: &str) -> Result<AstFacts, AstFactsError> {
    let path = path.into();
    let identity = SourceIdentity::new(path.clone(), source)?;
    if is_json_module_path(&path) {
        // Enroll the JSON module as an analyzed file with a proven-empty
        // fact table instead of asking Oxc's JS/TS parser to make sense of
        // non-JS content, or failing the whole build closed on a source the
        // module graph legitimately reaches. See [`AstFacts::empty`] for why
        // "empty" here is a proof of inertness, not an approximation.
        return Ok(AstFacts::empty(identity));
    }
    let source_type = SourceType::from_path(&path).map_err(|error| AstFactsError::SourceType {
        path: path.clone(),
        message: error.to_string(),
    })?;
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, source_type)
        .with_options(ParseOptions {
            preserve_parens: false,
            ..ParseOptions::default()
        })
        .parse();
    if let Some(error) = parsed.errors.into_iter().next() {
        return Err(AstFactsError::Parse(error.to_string()));
    }

    // JSX namespace names are not IdentifierReference nodes, so TypeScript's
    // GetSymbolAtLocation deliberately returns no symbol for `use:name`.
    // Build Oxc's semantic scope tree once and retain the exact declaration
    // chosen by its binder instead of approximating scope with source text.
    let built = SemanticBuilder::new().build(&parsed.program);
    let execution_semantics_valid = built.errors.is_empty();
    let semantic = built.semantic;
    let mut collector = Collector::new(source, semantic.scoping());
    collector.ambient_variable = source_type.is_typescript_definition();
    collector.visit_program(&parsed.program);
    let mut facts = collector.finish(identity);
    if execution_semantics_valid {
        (facts.host_execution, facts.host_defaults) = host_execution::host_execution(&semantic);
        facts.host_undefined_arguments = host_execution::undefined_values(&semantic);
        facts.host_call_sequences = host_execution::call_sequences(&semantic);
        (facts.host_empty_arrays, facts.host_zero_values) =
            host_execution::empty_and_zero_values(&semantic);
    }
    Ok(facts)
}

pub(super) struct Collector<'s, 'semantic> {
    pub(super) source: &'s str,
    pub(super) scoping: &'semantic Scoping,
    pub(super) scope_stack: Vec<ScopeId>,
    pub(super) calls: Vec<CallFact>,
    pub(super) bindings: Vec<BindingFact>,
    pub(super) functions: Vec<FunctionFact>,
    pub(super) function_declarations: Vec<FunctionDeclarationFact>,
    pub(super) classes: Vec<ClassFact>,
    pub(super) namespace_declarations: Vec<NamedSpan>,
    pub(super) imports: Vec<ImportFact>,
    pub(super) exports: Vec<ExportFact>,
    pub(super) module_loads: Vec<ModuleLoadFact>,
    pub(super) import_equals: Vec<ImportEqualsFact>,
    pub(super) type_imports: Vec<TypeImportFact>,
    pub(super) type_queries: Vec<Span>,
    pub(super) type_names: Vec<Span>,
    pub(super) tagged_template_tags: Vec<Span>,
    /// The declaration spans of `export import S = require("m")`, recorded by
    /// the export visitor before the walk reaches the declaration itself.
    pub(super) exported_import_equals: Vec<Span>,
    pub(super) module_hazards: Vec<ModuleHazardFact>,
    pub(super) module_blocks: Vec<Span>,
    pub(super) ambient_module_depth: usize,
    pub(super) ambient_variable: bool,
    pub(super) identifiers: Vec<IdentifierFact>,
    pub(super) reference_declarations: Vec<(Span, Span)>,
    pub(super) awaits: Vec<Span>,
    pub(super) unconditional_awaits: Vec<Span>,
    pub(super) dominating_awaits: Vec<(Span, Span)>,
    pub(super) try_head_awaits: Vec<TryHeadAwaitFact>,
    /// Open blocks: span, conditional depth at entry, function nesting level.
    pub(super) open_blocks: Vec<(Span, usize, usize)>,
    pub(super) straight_line_calls: Vec<Span>,
    pub(super) straight_line_constructs: Vec<Span>,
    pub(super) completion_sites: Vec<Span>,
    pub(super) returns: Vec<ReturnFact>,
    pub(super) jsx_elements: Vec<JsxElementFact>,
    pub(super) jsx_fragments: Vec<Span>,
    pub(super) transparent_wrappers: Vec<TransparentWrapperFact>,
    pub(super) members: Vec<MemberFact>,
    pub(super) computed_members: Vec<Span>,
    pub(super) array_literals: Vec<Span>,
    pub(super) literal_computed_members: Vec<LiteralMemberKeyFact>,
    pub(super) optional_members: Vec<Span>,
    pub(super) parameter_properties: Vec<Span>,
    pub(super) spreads: Vec<SpreadFact>,
    pub(super) conditional_tests: Vec<Span>,
    pub(super) nullish_test_operands: Vec<Span>,
    pub(super) conditional_expressions: Vec<ConditionalExpressionFact>,
    pub(super) logical_expressions: Vec<LogicalExpressionFact>,
    pub(super) object_properties: Vec<ObjectPropertyFact>,
    pub(super) object_get_shapes: Vec<ObjectGetShapeFact>,
    pub(super) template_literals: Vec<TemplateLiteralFact>,
    pub(super) coercive_operands: Vec<CoerciveOperandFact>,
    pub(super) coercing_operands: Vec<Span>,
    pub(super) primitive_results: Vec<Span>,
    pub(super) iterated_operands: Vec<Span>,
    pub(super) assignments: Vec<AssignmentFact>,
    pub(super) write_targets: Vec<Span>,
    pub(super) deleted_targets: Vec<Span>,
    pub(super) if_regions: Vec<IfRegionFact>,
    pub(super) jump_statements: Vec<Span>,
    pub(super) iteration_targets: Vec<Span>,
    pub(super) loop_statements: Vec<Span>,
    pub(super) implicit_suspensions: Vec<Span>,
    pub(super) module_directives: Vec<DirectiveFact>,
    pub(super) tagged_templates: Vec<TaggedTemplateFact>,
    pub(super) array_literal_elements: Vec<ArrayLiteralFact>,
    pub(super) pattern_elements: Vec<PatternElementFact>,
    pub(super) await_operands: Vec<Span>,
    pub(super) jsx_child_expressions: Vec<Span>,
    pub(super) disposal_initializers: Vec<Span>,
    pub(super) conditional_control_stack: Vec<Span>,
    pub(super) method_names: Vec<Option<NamedSpan>>,
    /// How many conditional/repeated/aborting constructs (if, ternary,
    /// logical right operand, loop, switch case, try) enclose the current
    /// node. Compared against the depth recorded at the innermost function
    /// entry to decide whether an await is in that function's straight-line
    /// flow.
    pub(super) conditional_flow_depth: usize,
    /// The [`Collector::conditional_flow_depth`] at each enclosing function's
    /// entry, innermost last.
    pub(super) function_flow_depths: Vec<usize>,
    /// Whether each enclosing function, innermost last, has so far written a
    /// `return`, `break` or `continue` that may skip what follows it.
    pub(super) function_exits: Vec<bool>,
    /// For each enclosing function, innermost last, how many `try` blocks of
    /// its own enclose the walk. Under the normal-completion premise
    /// (ADR 0279, option (b)) a `try` block runs to its end, so its depth does
    /// not make a completion site conditional; its `catch` and `finally` do.
    pub(super) function_try_blocks: Vec<usize>,
    /// ADR 0276: the chain elements an enclosing optional chain's `?.` links
    /// may skip, as exact spans, and the argument and computed-key ranges
    /// they own. What precedes a chain's first link always runs.
    pub(super) skippable_chain_sites: Vec<Span>,
    pub(super) skippable_chain_ranges: Vec<Span>,
    /// ADR 0276: how many logical assignments (`??=`, `||=`, `&&=`) enclose
    /// the current node. Their right side runs only when the target allows,
    /// and their target is read conservatively the same way.
    pub(super) logical_assignment_depth: usize,
    /// How many optional chains enclose the current node. Kept apart from
    /// [`Collector::conditional_flow_depth`], whose answer for awaits is
    /// unchanged; read only for [`AstFacts::straight_line_calls`].
    pub(super) optional_chain_depth: usize,
    /// The span of the expression the innermost enclosing `ExpressionStatement`
    /// discards, while that statement's own expression is being walked.
    ///
    /// Compared by span equality rather than tracked as a depth, so a nested
    /// call answers correctly without any unwinding: in `f(g())` as a
    /// statement, only `f(...)`'s span matches.
    pub(super) discarded_expression: Option<OxcSpan>,
    /// The concise arrow bodies enclosing the walk, innermost last.
    ///
    /// Oxc models `() => f()` as a body holding one `ExpressionStatement`, so
    /// the syntax that discards a result and the syntax that *returns* one are
    /// the same node. Without this the arrow's returned call would be recorded
    /// as discarded, which is the one direction `result_discarded` may never
    /// be wrong in.
    pub(super) concise_arrow_bodies: Vec<OxcSpan>,
}

pub(super) struct UnresolvedAssignmentTargets<'semantic> {
    pub(super) scoping: &'semantic Scoping,
    pub(super) found: bool,
}

impl UnresolvedAssignmentTargets<'_> {
    fn record(&mut self, identifier: &IdentifierReference<'_>) {
        if identifier
            .reference_id
            .get()
            .is_some_and(|reference| self.scoping.get_reference(reference).symbol_id().is_none())
        {
            self.found = true;
        }
    }
}

impl<'a> Visit<'a> for UnresolvedAssignmentTargets<'_> {
    fn visit_simple_assignment_target(
        &mut self,
        target: &oxc_ast::ast::SimpleAssignmentTarget<'a>,
    ) {
        if let oxc_ast::ast::SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) = target
        {
            self.record(identifier);
        } else if let Some(Expression::Identifier(identifier)) =
            target.get_expression().map(peel_ts_sugar)
        {
            self.record(identifier);
        }
        // Member targets mutate a property rather than the object binding.
    }

    fn visit_assignment_target_with_default(
        &mut self,
        target: &oxc_ast::ast::AssignmentTargetWithDefault<'a>,
    ) {
        self.visit_assignment_target(&target.binding);
    }

    fn visit_assignment_target_property_identifier(
        &mut self,
        property: &oxc_ast::ast::AssignmentTargetPropertyIdentifier<'a>,
    ) {
        self.record(&property.binding);
    }

    fn visit_assignment_target_property_property(
        &mut self,
        property: &oxc_ast::ast::AssignmentTargetPropertyProperty<'a>,
    ) {
        self.visit_assignment_target_maybe_default(&property.binding);
    }
}

#[derive(Default)]
pub(super) struct BindingMetadata {
    pub(super) initializer: Option<OxcSpan>,
    pub(super) call_initializer: Option<OxcSpan>,
    pub(super) initializer_function: bool,
    pub(super) initializer_identifier: Option<NamedSpan>,
    pub(super) initializer_value_kind: RuntimeValueKind,
    pub(super) immutable: bool,
    pub(super) runtime_let: bool,
}

impl<'s, 'semantic> Collector<'s, 'semantic> {
    /// ADR 0276: a site in the innermost function's straight-line flow, outside
    /// optional chains, that no earlier jump of that function may skip.
    pub(super) fn record_completion_site(&mut self, site: oxc_span::Span) {
        let site = span(site);
        if self.logical_assignment_depth == 0
            && !self.skippable_chain_sites.contains(&site)
            && !self
                .skippable_chain_ranges
                .iter()
                .any(|range| range.contains(site))
            && self.conditional_flow_depth
                - self.function_try_blocks.last().copied().unwrap_or_default()
                == self
                    .function_flow_depths
                    .last()
                    .copied()
                    .unwrap_or_default()
            && !self.function_exits.last().copied().unwrap_or(false)
        {
            self.completion_sites.push(site);
        }
    }

    /// A `return`, `break`, `continue` or `throw` may skip everything after it
    /// in the innermost function. A thrown guard is an exit too: the caller
    /// may catch it and carry on, with the rest of this body skipped.
    pub(super) fn mark_function_exit(&mut self) {
        if let Some(exits) = self.function_exits.last_mut() {
            *exits = true;
        }
    }

    fn new(source: &'s str, scoping: &'semantic Scoping) -> Self {
        Self {
            source,
            scoping,
            scope_stack: Vec::new(),
            calls: Vec::new(),
            bindings: Vec::new(),
            functions: Vec::new(),
            function_declarations: Vec::new(),
            classes: Vec::new(),
            namespace_declarations: Vec::new(),
            imports: Vec::new(),
            exports: Vec::new(),
            module_loads: Vec::new(),
            import_equals: Vec::new(),
            type_imports: Vec::new(),
            type_queries: Vec::new(),
            type_names: Vec::new(),
            tagged_template_tags: Vec::new(),
            exported_import_equals: Vec::new(),
            module_hazards: Vec::new(),
            module_blocks: Vec::new(),
            ambient_module_depth: 0,
            ambient_variable: false,
            identifiers: Vec::new(),
            reference_declarations: Vec::new(),
            awaits: Vec::new(),
            unconditional_awaits: Vec::new(),
            dominating_awaits: Vec::new(),
            try_head_awaits: Vec::new(),
            open_blocks: Vec::new(),
            straight_line_calls: Vec::new(),
            straight_line_constructs: Vec::new(),
            completion_sites: Vec::new(),
            returns: Vec::new(),
            jsx_elements: Vec::new(),
            jsx_fragments: Vec::new(),
            transparent_wrappers: Vec::new(),
            members: Vec::new(),
            computed_members: Vec::new(),
            array_literals: Vec::new(),
            literal_computed_members: Vec::new(),
            optional_members: Vec::new(),
            parameter_properties: Vec::new(),
            spreads: Vec::new(),
            conditional_tests: Vec::new(),
            nullish_test_operands: Vec::new(),
            conditional_expressions: Vec::new(),
            logical_expressions: Vec::new(),
            object_properties: Vec::new(),
            object_get_shapes: Vec::new(),
            template_literals: Vec::new(),
            coercive_operands: Vec::new(),
            coercing_operands: Vec::new(),
            primitive_results: Vec::new(),
            iterated_operands: Vec::new(),
            assignments: Vec::new(),
            write_targets: Vec::new(),
            deleted_targets: Vec::new(),
            if_regions: Vec::new(),
            jump_statements: Vec::new(),
            iteration_targets: Vec::new(),
            loop_statements: Vec::new(),
            implicit_suspensions: Vec::new(),
            module_directives: Vec::new(),
            tagged_templates: Vec::new(),
            array_literal_elements: Vec::new(),
            pattern_elements: Vec::new(),
            await_operands: Vec::new(),
            jsx_child_expressions: Vec::new(),
            disposal_initializers: Vec::new(),
            conditional_control_stack: Vec::new(),
            discarded_expression: None,
            concise_arrow_bodies: Vec::new(),
            method_names: Vec::new(),
            conditional_flow_depth: 0,
            function_flow_depths: Vec::new(),
            function_exits: Vec::new(),
            function_try_blocks: Vec::new(),
            skippable_chain_sites: Vec::new(),
            skippable_chain_ranges: Vec::new(),
            logical_assignment_depth: 0,
            optional_chain_depth: 0,
        }
    }

    fn finish(mut self, source: SourceIdentity) -> AstFacts {
        self.calls.sort_by_key(|fact| fact.span);
        self.bindings.sort_by_key(|fact| fact.declaration);
        self.functions.sort_by_key(|fact| fact.span);
        self.function_declarations
            .sort_by_key(|declaration| declaration.name.span);
        self.classes.sort_by_key(|fact| fact.span);
        self.namespace_declarations.sort_by_key(|name| name.span);
        self.imports.sort_by_key(|fact| fact.span);
        self.exports.sort_by_key(|fact| fact.span);
        self.module_loads.sort_by_key(|fact| fact.span);
        self.import_equals.sort_by_key(|fact| fact.span);
        self.type_imports.sort_by_key(|fact| fact.span);
        self.module_hazards.sort_by_key(|fact| fact.span);
        self.type_queries.sort_unstable();
        self.type_queries.dedup();
        self.type_names.sort_unstable();
        self.type_names.dedup();
        self.tagged_template_tags.sort_unstable();
        self.module_blocks.sort_unstable();
        self.identifiers.sort_by_key(|identifier| identifier.span);
        self.reference_declarations.sort_unstable();
        self.awaits.sort_unstable();
        self.unconditional_awaits.sort_unstable();
        self.dominating_awaits.sort_unstable();
        self.try_head_awaits
            .sort_unstable_by_key(|fact| fact.awaited);
        self.straight_line_calls.sort_unstable();
        self.straight_line_constructs.sort_unstable();
        self.completion_sites.sort_unstable();
        self.completion_sites.dedup();
        self.returns.sort_by_key(|fact| fact.span);
        self.jsx_elements.sort_by_key(|fact| fact.span);
        self.jsx_fragments.sort();
        self.transparent_wrappers.sort_by_key(|fact| fact.span);
        self.members.sort_by_key(|fact| fact.span);
        self.computed_members.sort_unstable();
        self.array_literals.sort_unstable();
        self.literal_computed_members.sort_by_key(|fact| fact.span);
        self.optional_members.sort_unstable();
        self.parameter_properties.sort_unstable();
        self.spreads.sort_by_key(|fact| fact.span);
        self.conditional_tests.sort_unstable();
        self.nullish_test_operands.sort_unstable();
        self.conditional_expressions.sort_by_key(|fact| fact.span);
        self.logical_expressions.sort_by_key(|fact| fact.span);
        self.object_properties.sort_by_key(|fact| fact.span);
        self.template_literals.sort_by_key(|fact| fact.span);
        self.coercive_operands.sort_by_key(|fact| fact.span);
        self.coercing_operands.sort_unstable();
        self.coercing_operands.dedup();
        self.primitive_results.sort_unstable();
        self.primitive_results.dedup();
        self.iterated_operands.sort_unstable();
        self.iterated_operands.dedup();
        self.assignments.sort_by_key(|fact| fact.target);
        self.write_targets.sort_unstable();
        self.write_targets.dedup();
        self.if_regions.sort_by_key(|fact| fact.consequent);
        self.jump_statements.sort_unstable();
        self.iteration_targets.sort_unstable();
        self.loop_statements.sort_unstable();
        self.implicit_suspensions.sort_unstable();
        self.module_directives.sort_by_key(|fact| fact.span);
        self.tagged_templates.sort_by_key(|fact| fact.span);
        self.array_literal_elements.sort_by_key(|fact| fact.span);
        self.pattern_elements
            .sort_by_key(|fact| (fact.pattern, fact.target, fact.default));
        self.await_operands.sort_unstable();
        self.jsx_child_expressions.sort_unstable();
        self.disposal_initializers.sort_unstable();
        AstFacts {
            schema: AST_FACTS_SCHEMA,
            source,
            span_index: LazySpanIndex::default(),
            calls: self.calls,
            host_execution: Vec::new(),
            host_defaults: Vec::new(),
            host_call_sequences: Vec::new(),
            host_empty_arrays: Vec::new(),
            host_zero_values: Vec::new(),
            host_undefined_arguments: Vec::new(),
            bindings: self.bindings,
            functions: self.functions,
            function_declarations: self.function_declarations,
            classes: self.classes,
            namespace_declarations: self.namespace_declarations,
            imports: self.imports,
            exports: self.exports,
            module_loads: self.module_loads,
            import_equals: self.import_equals,
            type_imports: self.type_imports,
            type_queries: self.type_queries,
            type_names: self.type_names,
            tagged_template_tags: self.tagged_template_tags,
            module_hazards: self.module_hazards,
            module_blocks: self.module_blocks,
            identifiers: self.identifiers,
            reference_declarations: self.reference_declarations,
            awaits: self.awaits,
            unconditional_awaits: self.unconditional_awaits,
            dominating_awaits: self.dominating_awaits,
            try_head_awaits: self.try_head_awaits,
            straight_line_calls: self.straight_line_calls,
            straight_line_constructs: self.straight_line_constructs,
            completion_sites: self.completion_sites,
            returns: self.returns,
            jsx_elements: self.jsx_elements,
            jsx_fragments: self.jsx_fragments,
            transparent_wrappers: self.transparent_wrappers,
            members: self.members,
            computed_members: self.computed_members,
            array_literals: self.array_literals,
            literal_computed_members: self.literal_computed_members,
            optional_members: self.optional_members,
            parameter_properties: self.parameter_properties,
            spreads: self.spreads,
            conditional_tests: self.conditional_tests,
            nullish_test_operands: self.nullish_test_operands,
            conditional_expressions: self.conditional_expressions,
            logical_expressions: self.logical_expressions,
            object_properties: self.object_properties,
            object_get_shapes: self.object_get_shapes,
            template_literals: self.template_literals,
            coercive_operands: self.coercive_operands,
            coercing_operands: self.coercing_operands,
            primitive_results: self.primitive_results,
            iterated_operands: self.iterated_operands,
            assignments: self.assignments,
            write_targets: self.write_targets,
            deleted_targets: self.deleted_targets,
            if_regions: self.if_regions,
            jump_statements: self.jump_statements,
            iteration_targets: self.iteration_targets,
            loop_statements: self.loop_statements,
            implicit_suspensions: self.implicit_suspensions,
            module_directives: self.module_directives,
            tagged_templates: self.tagged_templates,
            array_literal_elements: self.array_literal_elements,
            pattern_elements: self.pattern_elements,
            await_operands: self.await_operands,
            jsx_child_expressions: self.jsx_child_expressions,
            disposal_initializers: self.disposal_initializers,
        }
    }

    pub(super) fn rest_parameter_names(
        &self,
        rest: Option<&oxc_ast::ast::FormalParameterRest<'_>>,
    ) -> Vec<NamedSpan> {
        rest.map(|rest| {
            self.binding_fact(
                rest.rest.span,
                &rest.rest.argument,
                BindingMetadata::default(),
            )
            .names
        })
        .unwrap_or_default()
    }

    pub(super) fn binding_fact(
        &self,
        declaration: OxcSpan,
        pattern: &BindingPattern<'_>,
        metadata: BindingMetadata,
    ) -> BindingFact {
        let shape = match pattern {
            BindingPattern::BindingIdentifier(_) | BindingPattern::AssignmentPattern(_) => {
                BindingShape::Identifier
            }
            BindingPattern::ArrayPattern(_) => BindingShape::Array,
            BindingPattern::ObjectPattern(_) => BindingShape::Object,
        };
        BindingFact {
            declaration: span(declaration),
            pattern: span(pattern.span()),
            shape,
            names: pattern
                .get_binding_identifiers()
                .into_iter()
                .map(|identifier| NamedSpan {
                    span: span(identifier.span),
                })
                .collect(),
            array_slots: match pattern {
                BindingPattern::ArrayPattern(array) => array
                    .elements
                    .iter()
                    .map(|element| {
                        element.as_ref().and_then(|pattern| match pattern {
                            BindingPattern::BindingIdentifier(identifier) => Some(NamedSpan {
                                span: span(identifier.span),
                            }),
                            _ => None,
                        })
                    })
                    .collect(),
                _ => vec![],
            },
            object_slots: match pattern {
                BindingPattern::ObjectPattern(object) => object
                    .properties
                    .iter()
                    .filter_map(|property| {
                        let BindingPattern::BindingIdentifier(local) = &property.value else {
                            return None;
                        };
                        if property.computed {
                            return None;
                        }
                        let property_name = self
                            .source
                            .get(
                                property.key.span().start as usize
                                    ..property.key.span().end as usize,
                            )?
                            .trim_matches(['\'', '"']);
                        Some(ObjectBindingFact {
                            property: property_name.into(),
                            plain_identifier_key: matches!(&property.key,
                                PropertyKey::StaticIdentifier(key)
                                    if self.source.get(key.span.start as usize..key.span.end as usize)
                                        == Some(key.name.as_str())),
                            local: NamedSpan {
                                span: span(local.span),
                            },
                        })
                    })
                    .collect(),
                _ => Vec::new(),
            },
            immutable: metadata.immutable,
            runtime_let: metadata.runtime_let,
            initializer: metadata.initializer.map(span),
            call_initializer: metadata.call_initializer.map(span),
            initializer_function: metadata.initializer_function,
            initializer_identifier: metadata.initializer_identifier,
            initializer_value_kind: metadata.initializer_value_kind,
        }
    }

    pub(super) fn return_fact(
        &self,
        expression: Option<&Expression<'_>>,
        fallback: OxcSpan,
    ) -> ReturnFact {
        let Some(expression) = expression else {
            return ReturnFact {
                span: span(fallback),
                argument: None,
                control_tests: self.conditional_control_stack.clone().into_boxed_slice(),
                value: ReturnValueKind::Undefined,
                conditional: false,
                callee: None,
                structure: None,
                runtime_value_kind: RuntimeValueKind::Nullish,
                straight_line: false,
            };
        };
        let argument_span = span(expression.span());
        let expression = expression.get_inner_expression();
        let elements = match expression {
            Expression::ArrayExpression(array) => array
                .elements
                .iter()
                .map(|element| {
                    (!matches!(
                        element,
                        ArrayExpressionElement::Elision(_)
                            | ArrayExpressionElement::SpreadElement(_)
                    ))
                    .then(|| span(element.span()))
                })
                .collect(),
            _ => Vec::new(),
        };
        let properties = match expression {
            Expression::ObjectExpression(object) => object
                .properties
                .iter()
                .filter_map(|property| {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return None;
                    };
                    let key = self
                        .source
                        .get(property.key.span().start as usize..property.key.span().end as usize)?
                        .trim_matches(['\'', '"']);
                    (!key.is_empty()).then(|| ReturnPropertyFact {
                        name: key.into(),
                        value: span(property.value.span()),
                    })
                })
                .collect(),
            _ => Vec::new(),
        };
        let conditional = matches!(expression, Expression::ConditionalExpression(_));
        let complete_literal = match expression {
            Expression::ArrayExpression(array) => array.elements.iter().all(|element| {
                !matches!(
                    element,
                    ArrayExpressionElement::Elision(_) | ArrayExpressionElement::SpreadElement(_)
                )
            }),
            Expression::ObjectExpression(object) => {
                let mut keys = std::collections::BTreeSet::new();
                object.properties.iter().all(|property| {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return false;
                    };
                    // ADR 0181: a shorthand `{ value }` is `{ value: value }`;
                    // its value is the identifier, resolved by the binder.
                    if property.kind != PropertyKind::Init
                        || property.method
                        || property.computed
                        || (property.shorthand
                            && !matches!(property.value, Expression::Identifier(_)))
                    {
                        return false;
                    }
                    let key = match &property.key {
                        PropertyKey::StaticIdentifier(key) => key.name.as_str(),
                        PropertyKey::StringLiteral(key) => key.value.as_str(),
                        _ => return false,
                    };
                    key != "__proto__" && keys.insert(key)
                })
            }
            _ => false,
        };
        let (value, callee) = match expression {
            Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => {
                (ReturnValueKind::Function, None)
            }
            Expression::Identifier(identifier) if self.is_global_undefined(identifier) => {
                (ReturnValueKind::Undefined, None)
            }
            Expression::Identifier(_) => (ReturnValueKind::Identifier, None),
            Expression::CallExpression(call) => {
                (ReturnValueKind::Call, Some(span(call.callee.span())))
            }
            Expression::NewExpression(expression) => {
                (ReturnValueKind::Call, Some(span(expression.callee.span())))
            }
            Expression::StaticMemberExpression(_)
            | Expression::ComputedMemberExpression(_)
            | Expression::PrivateFieldExpression(_) => (ReturnValueKind::Member, None),
            Expression::UnaryExpression(unary)
                if unary.operator == oxc_syntax::operator::UnaryOperator::Void =>
            {
                (ReturnValueKind::Undefined, None)
            }
            _ => (ReturnValueKind::Other, None),
        };
        ReturnFact {
            runtime_value_kind: self.runtime_value_kind(expression),
            span: span(expression.span()),
            argument: Some(argument_span),
            control_tests: self.conditional_control_stack.clone().into_boxed_slice(),
            value,
            conditional,
            callee,
            structure: (!elements.is_empty() || !properties.is_empty()).then(|| {
                Box::new(ReturnStructureFact {
                    complete_literal,
                    elements,
                    properties,
                })
            }),
            straight_line: false,
        }
    }

    pub(super) fn is_static_callee(&self, callee: OxcSpan) -> bool {
        self.source
            .get(callee.start as usize..callee.end as usize)
            .is_some_and(|text| {
                text.bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$' | b'.'))
            })
    }

    /// The declaration a shorthand property's value binding refers to.
    ///
    /// The binder already resolved this exact reference, so its answer is
    /// used rather than a scope-chain lookup by spelling: it distinguishes
    /// sibling block scopes, and an unresolved reference stays `None` instead
    /// of silently matching a same-named binding somewhere else in the file.
    pub(super) fn shorthand_binding(&self, property: &ObjectProperty<'_>) -> Option<Span> {
        if !property.shorthand {
            return None;
        }
        let Expression::Identifier(value) = &property.value else {
            return None;
        };
        let symbol = self
            .scoping
            .get_reference(value.reference_id.get()?)
            .symbol_id()?;
        Some(span(self.scoping.symbol_span(symbol)))
    }

    pub(super) fn argument_fact(&self, argument: &Argument<'_>) -> ArgumentFact {
        let value = match argument {
            Argument::Identifier(identifier) if self.is_global_undefined(identifier) => {
                ArgumentValueKind::Undefined
            }
            Argument::Identifier(_) => ArgumentValueKind::Identifier,
            Argument::NullLiteral(_) => ArgumentValueKind::Null,
            Argument::ArrowFunctionExpression(function) if function.r#async => {
                ArgumentValueKind::AsyncFunction
            }
            Argument::FunctionExpression(function) if function.r#async => {
                ArgumentValueKind::AsyncFunction
            }
            Argument::ArrowFunctionExpression(_) | Argument::FunctionExpression(_) => {
                ArgumentValueKind::Function
            }
            _ => ArgumentValueKind::Other,
        };
        let boolean_properties = match argument {
            Argument::ObjectExpression(object) => object
                .properties
                .iter()
                .filter_map(|property| {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return None;
                    };
                    let PropertyKey::StaticIdentifier(key) = &property.key else {
                        return None;
                    };
                    let Expression::BooleanLiteral(value) = &property.value else {
                        return None;
                    };
                    Some(BooleanPropertyFact {
                        name: span(key.span),
                        value: value.value,
                    })
                })
                .collect(),
            _ => vec![],
        };
        let identifier_properties = match argument {
            Argument::ObjectExpression(object) => object
                .properties
                .iter()
                .filter_map(|property| {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return None;
                    };
                    let PropertyKey::StaticIdentifier(key) = &property.key else {
                        return None;
                    };
                    if !matches!(key.name.as_str(), "effect" | "error") {
                        return None;
                    }
                    let Expression::Identifier(value) = &property.value else {
                        return None;
                    };
                    Some(NamedSpan {
                        span: span(value.span),
                    })
                })
                .collect(),
            _ => vec![],
        };
        let string_properties = match argument {
            Argument::ObjectExpression(object) => object
                .properties
                .iter()
                .filter_map(|property| {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return None;
                    };
                    let PropertyKey::StaticIdentifier(key) = &property.key else {
                        return None;
                    };
                    let Expression::StringLiteral(value) = &property.value else {
                        return None;
                    };
                    Some(StringPropertyFact {
                        name: span(key.span),
                        value: value.value.as_str().into(),
                    })
                })
                .collect(),
            _ => vec![],
        };
        // Property completeness belongs to the runtime object, not to the
        // transparent TypeScript wrapper around it. This keeps
        // `({ effect }) as EffectBundle` equivalent to `{ effect }` and lets
        // consumers prove absence on `{} as SomeObject` without parsing
        // source text.
        let expression = argument.as_expression().map(peel_ts_sugar);
        let property_names = match expression {
            Some(Expression::ObjectExpression(object)) => object
                .properties
                .iter()
                .filter_map(|property| {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return None;
                    };
                    let PropertyKey::StaticIdentifier(key) = &property.key else {
                        return None;
                    };
                    Some(span(key.span))
                })
                .collect(),
            _ => vec![],
        };
        // Getter/setter pairs are `ObjectProperty` nodes with a non-`Init`
        // kind; the presence of a `get loadingValue()` still puts the key in
        // `property_names` above (an `in` check sees it), but a literal that
        // contains one no longer proves final option *values*, so it is not
        // exact.
        let exact_object_literal = match expression {
            Some(Expression::ObjectExpression(object)) => {
                object.properties.iter().all(|property| {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return false;
                    };
                    property.kind == PropertyKind::Init
                        && matches!(&property.key, PropertyKey::StaticIdentifier(_))
                })
            }
            _ => false,
        };
        let closed_object_literal = match expression {
            Some(Expression::ObjectExpression(object)) => {
                object.properties.iter().all(|property| {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return false;
                    };
                    matches!(&property.key, PropertyKey::StaticIdentifier(_))
                })
            }
            _ => false,
        };
        // Classify the runtime value behind TypeScript sugar so downstream
        // rules can reason about what actually reaches the call: a
        // `value as const` still passes the literal, and `target!` still
        // passes the member chain.
        let runtime_value_kind = expression.map_or(RuntimeValueKind::Unknown, |expression| {
            self.runtime_value_kind(expression)
        });
        let value_span = expression
            .map(|expression| span(expression.span()))
            .filter(|inner| *inner != span(argument.span()));
        let runtime_type_escape = argument
            .as_expression()
            .is_some_and(contains_runtime_type_escape);
        let binding_declaration = match argument {
            Argument::Identifier(identifier) => identifier
                .reference_id
                .get()
                .and_then(|reference| self.scoping.get_reference(reference).symbol_id())
                .map(|symbol| span(self.scoping.symbol_span(symbol))),
            _ => None,
        };
        let literal_members = expression.map_or_else(Vec::new, literal_members_of);
        ArgumentFact {
            span: span(argument.span()),
            literal_members,
            literal_value: expression.map_or(ArgumentLiteralFact::Unknown, |expression| {
                argument_literal_fact(expression, 0)
            }),
            binding_declaration,
            spread: argument.is_spread(),
            value,
            boolean_properties,
            identifier_properties,
            string_properties,
            property_names,
            exact_object_literal,
            closed_object_literal,
            runtime_value_kind,
            value_span,
            runtime_type_escape,
        }
    }

    pub(super) fn is_global_undefined(&self, identifier: &IdentifierReference<'_>) -> bool {
        self.is_unresolved_named(identifier, "undefined")
    }

    pub(super) fn assignment_target_has_unresolved(&self, target: &AssignmentTarget<'_>) -> bool {
        let mut visitor = UnresolvedAssignmentTargets {
            scoping: self.scoping,
            found: false,
        };
        visitor.visit_assignment_target(target);
        visitor.found
    }

    pub(super) fn simple_assignment_target_has_unresolved(
        &self,
        target: &oxc_ast::ast::SimpleAssignmentTarget<'_>,
    ) -> bool {
        let mut visitor = UnresolvedAssignmentTargets {
            scoping: self.scoping,
            found: false,
        };
        visitor.visit_simple_assignment_target(target);
        visitor.found
    }

    /// Whether the binder resolved this spelling to no declaration in the
    /// current program. This is deliberately stricter than a name check:
    /// shadowed `require`, `eval`, and `WebAssembly` values are ordinary local
    /// behavior and must not become package-closure hazards.
    /// Member names that install a property accessor, or a prototype that
    /// carries one, at run time.
    ///
    /// Matched by name alone rather than by receiver. `Object`, `Reflect` and
    /// `globalThis.Object` all reach the same intrinsic, and an alias
    /// (`const dp = Object.defineProperty`) reads the member before it calls
    /// anything, so a receiver test would miss both. Over-refusing is free
    /// here: the corpus measurement found the broad set and the narrow one
    /// refuse exactly the same packages.
    pub(super) const ACCESSOR_INSTALLING_MEMBERS: [&'static str; 6] = [
        "defineProperty",
        "defineProperties",
        "setPrototypeOf",
        "__defineGetter__",
        "__defineSetter__",
        "__proto__",
    ];

    pub(super) fn record_accessor_installation(&mut self, at: oxc_span::Span) {
        self.module_hazards.push(ModuleHazardFact {
            span: span(at),
            kind: ModuleHazardKind::RuntimeAccessorInstallation,
        });
    }

    pub(super) fn is_unresolved_named(
        &self,
        identifier: &IdentifierReference<'_>,
        name: &str,
    ) -> bool {
        identifier.name == name
            && identifier.reference_id.get().is_some_and(|reference| {
                self.scoping.get_reference(reference).symbol_id().is_none()
            })
    }

    pub(super) fn runtime_value_kind(&self, expression: &Expression<'_>) -> RuntimeValueKind {
        match peel_ts_sugar(expression) {
            Expression::NullLiteral(_) => RuntimeValueKind::Nullish,
            Expression::Identifier(identifier) if self.is_global_undefined(identifier) => {
                RuntimeValueKind::Nullish
            }
            Expression::StringLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::BigIntLiteral(_)
            | Expression::TemplateLiteral(_)
            | Expression::RegExpLiteral(_) => RuntimeValueKind::Primitive,
            Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => {
                RuntimeValueKind::Function
            }
            Expression::ObjectExpression(_) => RuntimeValueKind::Object,
            Expression::ArrayExpression(_) => RuntimeValueKind::Array,
            _ => RuntimeValueKind::Unknown,
        }
    }
}
