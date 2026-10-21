//! The oxc `Visit` implementation for `Collector`, which records facts as the walk proceeds.

use super::*;

impl<'a> Visit<'a> for Collector<'_, '_> {
    fn enter_scope(&mut self, _flags: ScopeFlags, scope_id: &std::cell::Cell<Option<ScopeId>>) {
        // SemanticBuilder populated every scope cell in this same AST before
        // fact collection. Falling back to the root keeps extraction total if
        // Oxc ever introduces an unpopulated synthetic scope.
        self.scope_stack.push(
            scope_id
                .get()
                .unwrap_or_else(|| self.scoping.root_scope_id()),
        );
    }

    fn leave_scope(&mut self) {
        self.scope_stack.pop();
    }

    fn visit_program(&mut self, program: &oxc_ast::ast::Program<'a>) {
        // The parser has already classified the directive prologue: only the
        // leading string-literal statements land in `directives`, so this is
        // a transcription, not a re-derivation.
        self.module_directives
            .extend(program.directives.iter().map(|directive| DirectiveFact {
                span: span(directive.span),
                value: directive.expression.value.as_str().into(),
            }));
        walk::walk_program(self, program);
    }

    fn visit_expression(&mut self, expression: &Expression<'a>) {
        if let Some(inner) = transparent_inner_expression(expression) {
            self.transparent_wrappers.push(TransparentWrapperFact {
                span: span(expression.span()),
                inner: span(inner.span()),
            });
        }
        walk::walk_expression(self, expression);
    }

    fn visit_import_expression(&mut self, expression: &ImportExpression<'a>) {
        let specifier = if expression.options.is_none() {
            match &expression.source {
                Expression::StringLiteral(source) => {
                    Some((source.value.as_str().into(), span(source.span)))
                }
                Expression::TemplateLiteral(source) if source.expressions.is_empty() => source
                    .quasis
                    .first()
                    .and_then(|quasi| quasi.value.cooked.as_ref())
                    .map(|value| (value.as_str().into(), span(source.span))),
                _ => None,
            }
        } else {
            None
        };
        let (specifier, specifier_span) = specifier.unzip();
        self.module_loads.push(ModuleLoadFact {
            span: span(expression.span),
            kind: ModuleLoadKind::DynamicImport,
            specifier: specifier.clone(),
            specifier_span,
        });
        if specifier.is_none() {
            self.module_hazards.push(ModuleHazardFact {
                span: span(expression.span),
                kind: ModuleHazardKind::NonliteralDynamicLoading,
            });
        }
        walk::walk_import_expression(self, expression);
    }

    fn visit_expression_statement(&mut self, statement: &oxc_ast::ast::ExpressionStatement<'a>) {
        // Saved and restored rather than cleared: an expression statement
        // inside a function inside this statement's own expression is a
        // different discard, and the outer one has to survive it.
        let expression = statement.expression.span();
        let returned = self.concise_arrow_bodies.last() == Some(&expression);
        let enclosing = if returned {
            self.discarded_expression.take()
        } else {
            self.discarded_expression.replace(expression)
        };
        walk::walk_expression_statement(self, statement);
        self.discarded_expression = enclosing;
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if self.optional_chain_depth == 0
            && self.conditional_flow_depth
                == self
                    .function_flow_depths
                    .last()
                    .copied()
                    .unwrap_or_default()
        {
            self.straight_line_calls.push(span(call.span));
        }
        self.record_completion_site(call.span);
        // TypeScript wrappers erase at run time: `(require as any)("./b")`,
        // `require!("./b")` and `require satisfies X` call `require` directly.
        if let Expression::Identifier(callee) = peel_ts_sugar(&call.callee) {
            if self.is_unresolved_named(callee, "require") {
                self.direct_require_callees.push(span(callee.span));
                let specifier = match call.arguments.as_slice() {
                    [Argument::StringLiteral(source)] => {
                        Some((source.value.as_str().into(), span(source.span)))
                    }
                    [Argument::TemplateLiteral(source)] if source.expressions.is_empty() => source
                        .quasis
                        .first()
                        .and_then(|quasi| quasi.value.cooked.as_ref())
                        .map(|value| (value.as_str().into(), span(source.span))),
                    _ => None,
                };
                let (specifier, specifier_span) = specifier.unzip();
                self.module_loads.push(ModuleLoadFact {
                    span: span(call.span),
                    kind: ModuleLoadKind::Require,
                    specifier: specifier.clone(),
                    specifier_span,
                });
                if specifier.is_none() {
                    self.module_hazards.push(ModuleHazardFact {
                        span: span(call.span),
                        kind: ModuleHazardKind::NonliteralDynamicLoading,
                    });
                }
            } else if self.is_unresolved_named(callee, "eval") {
                self.module_hazards.push(ModuleHazardFact {
                    span: span(call.span),
                    kind: ModuleHazardKind::Eval,
                });
            }
        }
        // `Object.create(prototype, descriptors)` installs the descriptors it
        // is handed. The one-argument form installs nothing and is ordinary
        // code, so the arity is the whole distinction.
        if let Expression::StaticMemberExpression(member) = &call.callee
            && member.property.name == "create"
            && call.arguments.len() >= 2
            && let Expression::Identifier(object) = &member.object
            && self.is_unresolved_named(object, "Object")
        {
            self.record_accessor_installation(call.span);
        }
        if let Expression::StaticMemberExpression(member) = call.callee.get_inner_expression()
            && !member.optional
            && member.property.name == "glob"
            && Self::is_import_meta(&member.object)
            && Self::import_glob_is_lazy(&call.arguments)
        {
            self.lazy_import_glob_callees.push(span(member.span));
        }
        let callee_span = call.callee.span();
        self.calls.push(CallFact {
            span: span(call.span),
            callee: span(callee_span),
            direct_callee: matches!(call.callee, Expression::Identifier(_)),
            type_arguments: call.type_arguments.is_some(),
            arguments: call
                .arguments
                .iter()
                .map(|argument| self.argument_fact(argument))
                .collect(),
            static_callee: self.is_static_callee(callee_span),
            result_discarded: self.discarded_expression == Some(call.span),
            owned_write_option: call.arguments.get(1).is_some_and(|argument| {
                let Argument::ObjectExpression(options) = argument else {
                    return false;
                };
                options.properties.iter().any(|property| {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return false;
                    };
                    let PropertyKey::StaticIdentifier(key) = &property.key else {
                        return false;
                    };
                    key.name == "ownedWrite"
                        && matches!(
                            &property.value,
                            oxc_ast::ast::Expression::BooleanLiteral(value) if value.value
                        )
                })
            }),
            construct: false,
        });
        walk::walk_call_expression(self, call);
    }

    fn visit_new_expression(&mut self, expression: &NewExpression<'a>) {
        if self.optional_chain_depth == 0
            && self.conditional_flow_depth
                == self
                    .function_flow_depths
                    .last()
                    .copied()
                    .unwrap_or_default()
        {
            self.straight_line_constructs.push(span(expression.span));
        }
        self.record_completion_site(expression.span);
        let callee_span = expression.callee.span();
        self.calls.push(CallFact {
            span: span(expression.span),
            callee: span(callee_span),
            direct_callee: matches!(expression.callee, Expression::Identifier(_)),
            type_arguments: expression.type_arguments.is_some(),
            arguments: expression
                .arguments
                .iter()
                .map(|argument| self.argument_fact(argument))
                .collect(),
            static_callee: self.is_static_callee(callee_span),
            result_discarded: self.discarded_expression == Some(expression.span),
            owned_write_option: false,
            construct: true,
        });
        walk::walk_new_expression(self, expression);
    }

    fn visit_variable_declaration(&mut self, declaration: &oxc_ast::ast::VariableDeclaration<'a>) {
        let previous = self.ambient_variable;
        self.ambient_variable |= declaration.declare || self.ambient_module_depth != 0;
        walk::walk_variable_declaration(self, declaration);
        self.ambient_variable = previous;
    }

    fn visit_variable_declarator(&mut self, declaration: &VariableDeclarator<'a>) {
        // The disposal is awaited where the block exits, which is after this
        // declarator; recording the declarator itself is the earlier, and so
        // the conservative, position.
        if declaration.kind == oxc_ast::ast::VariableDeclarationKind::AwaitUsing {
            self.implicit_suspensions.push(span(declaration.span));
        }
        if matches!(
            declaration.kind,
            oxc_ast::ast::VariableDeclarationKind::Using
                | oxc_ast::ast::VariableDeclarationKind::AwaitUsing
        ) && let Some(init) = &declaration.init
        {
            self.disposal_initializers.push(span(init.span()));
        }
        if matches!(declaration.id, BindingPattern::ArrayPattern(_))
            && let Some(init) = &declaration.init
        {
            self.iterated_operands.push(span(init.span()));
        }
        let initializer = declaration.init.as_ref().map(GetSpan::span);
        let initializer_function = declaration.init.as_ref().is_some_and(|expression| {
            matches!(
                expression.get_inner_expression(),
                Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
            )
        });
        let initializer_identifier = declaration.init.as_ref().and_then(|expression| {
            let Expression::Identifier(identifier) = expression.get_inner_expression() else {
                return None;
            };
            Some(NamedSpan {
                span: span(identifier.span),
            })
        });
        let call_initializer = declaration.init.as_ref().and_then(|expression| {
            match expression.get_inner_expression() {
                oxc_ast::ast::Expression::CallExpression(call) => Some(call.span),
                _ => None,
            }
        });
        self.bindings.push(
            self.binding_fact(
                declaration.span,
                &declaration.id,
                BindingMetadata {
                    initializer,
                    call_initializer,
                    initializer_function,
                    initializer_identifier,
                    initializer_value_kind: declaration
                        .init
                        .as_ref()
                        .map(|expression| self.runtime_value_kind(expression))
                        .unwrap_or_default(),
                    immutable: declaration.kind == oxc_ast::ast::VariableDeclarationKind::Const,
                    in_switch_case: self
                        .switch_case_declarators
                        .contains(&span(declaration.span)),
                    runtime_let: declaration.kind == oxc_ast::ast::VariableDeclarationKind::Let
                        && !self.ambient_variable
                        && self.ambient_module_depth == 0
                        && !self
                            .module_blocks
                            .iter()
                            .any(|block| block.contains(span(declaration.span))),
                },
            ),
        );
        walk::walk_variable_declarator(self, declaration);
    }

    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if matches!(
            function.r#type,
            FunctionType::FunctionDeclaration | FunctionType::TSDeclareFunction
        ) && let Some(name) = &function.id
        {
            self.function_declarations.push(FunctionDeclarationFact {
                name: NamedSpan {
                    span: span(name.span),
                },
                has_body: function.body.is_some(),
            });
        }
        if let Some(body) = &function.body {
            self.functions.push(FunctionFact {
                span: span(function.span),
                body: span(body.span),
                kind: match function.r#type {
                    FunctionType::FunctionExpression
                    | FunctionType::TSEmptyBodyFunctionExpression => FunctionKind::Expression,
                    FunctionType::FunctionDeclaration | FunctionType::TSDeclareFunction => {
                        FunctionKind::Declaration
                    }
                },
                name: function.id.as_ref().map(|identifier| NamedSpan {
                    span: span(identifier.span),
                }),
                method_name: self.method_names.last().cloned().flatten(),
                parameters: function
                    .params
                    .items
                    .iter()
                    .map(|parameter| {
                        self.binding_fact(
                            parameter.span,
                            &parameter.pattern,
                            BindingMetadata {
                                initializer: parameter
                                    .initializer
                                    .as_ref()
                                    .map(|value| value.span()),
                                ..BindingMetadata::default()
                            },
                        )
                    })
                    .collect(),
                rest_parameter: function.params.rest.is_some(),
                parameters_inert: self.parameters_inert(&function.params),
                rest_parameter_names: self.rest_parameter_names(function.params.rest.as_deref()),
                r#async: function.r#async,
                generator: function.generator,
                expression_body: false,
                expression_return: None,
                directives: body
                    .directives
                    .iter()
                    .map(|directive| directive.expression.value.as_str().into())
                    .collect(),
            });
        }
        self.function_flow_depths.push(self.conditional_flow_depth);
        self.function_exits.push(false);
        self.function_try_blocks.push(0);
        walk::walk_function(self, function, flags);
        self.function_try_blocks.pop();
        self.function_exits.pop();
        self.function_flow_depths.pop();
    }

    fn visit_method_definition(&mut self, method: &oxc_ast::ast::MethodDefinition<'a>) {
        self.method_names.push(static_property_name(&method.key));
        walk::walk_method_definition(self, method);
        self.method_names.pop();
    }

    fn visit_class(&mut self, class: &oxc_ast::ast::Class<'a>) {
        use oxc_ast::ast::{ClassElement, MethodDefinitionKind, PropertyKey};
        let key = |key: &PropertyKey<'_>, computed: bool| {
            (!computed && !matches!(key, PropertyKey::PrivateIdentifier(_)))
                .then(|| span(key.span()))
        };
        let elements = class
            .body
            .body
            .iter()
            .map(|element| match element {
                ClassElement::MethodDefinition(method) => ClassElementFact {
                    span: span(method.span),
                    kind: match method.kind {
                        MethodDefinitionKind::Constructor => ClassElementKind::Constructor,
                        MethodDefinitionKind::Method => ClassElementKind::Method,
                        MethodDefinitionKind::Get => ClassElementKind::Getter,
                        MethodDefinitionKind::Set => ClassElementKind::Setter,
                    },
                    key: key(&method.key, method.computed),
                    computed: method.computed,
                    r#static: method.r#static,
                    value: Some(span(method.value.span)),
                },
                ClassElement::PropertyDefinition(property) => ClassElementFact {
                    span: span(property.span),
                    kind: ClassElementKind::Field,
                    key: key(&property.key, property.computed),
                    computed: property.computed,
                    r#static: property.r#static,
                    value: property.value.as_ref().map(|value| span(value.span())),
                },
                ClassElement::AccessorProperty(property) => ClassElementFact {
                    span: span(property.span),
                    kind: ClassElementKind::Field,
                    key: key(&property.key, property.computed),
                    computed: property.computed,
                    r#static: property.r#static,
                    value: property.value.as_ref().map(|value| span(value.span())),
                },
                ClassElement::StaticBlock(block) => ClassElementFact {
                    span: span(block.span),
                    kind: ClassElementKind::StaticBlock,
                    key: None,
                    computed: false,
                    r#static: true,
                    value: None,
                },
                ClassElement::TSIndexSignature(signature) => ClassElementFact {
                    span: span(signature.span),
                    kind: ClassElementKind::IndexSignature,
                    key: None,
                    computed: false,
                    r#static: signature.r#static,
                    value: None,
                },
            })
            .collect();
        self.classes.push(ClassFact {
            span: span(class.span),
            name: class.id.as_ref().map(|id| NamedSpan {
                span: span(id.span),
            }),
            heritage: class
                .super_class
                .as_ref()
                .map(|heritage| span(heritage.span())),
            elements,
        });
        walk::walk_class(self, class);
    }

    fn visit_arrow_function_expression(&mut self, function: &ArrowFunctionExpression<'a>) {
        let expression_return = function
            .get_expression()
            .map(|expression| self.return_fact(Some(expression), expression.span()));
        self.functions.push(FunctionFact {
            span: span(function.span),
            body: span(function.body.span),
            kind: FunctionKind::Arrow,
            name: None,
            method_name: None,
            parameters: function
                .params
                .items
                .iter()
                .map(|parameter| {
                    self.binding_fact(
                        parameter.span,
                        &parameter.pattern,
                        BindingMetadata {
                            initializer: parameter.initializer.as_ref().map(|value| value.span()),
                            ..BindingMetadata::default()
                        },
                    )
                })
                .collect(),
            rest_parameter: function.params.rest.is_some(),
            parameters_inert: self.parameters_inert(&function.params),
            rest_parameter_names: self.rest_parameter_names(function.params.rest.as_deref()),
            r#async: function.r#async,
            generator: false,
            expression_body: function.expression,
            expression_return,
            // An expression-bodied arrow has no statement prologue; a
            // block-bodied one carries its directives like any function.
            directives: if function.expression {
                Vec::new()
            } else {
                function
                    .body
                    .directives
                    .iter()
                    .map(|directive| directive.expression.value.as_str().into())
                    .collect()
            },
        });
        self.function_flow_depths.push(self.conditional_flow_depth);
        self.function_exits.push(false);
        self.function_try_blocks.push(0);
        let concise = function.get_expression().map(GetSpan::span);
        if let Some(concise) = concise {
            self.concise_arrow_bodies.push(concise);
        }
        walk::walk_arrow_function_expression(self, function);
        if concise.is_some() {
            self.concise_arrow_bodies.pop();
        }
        self.function_try_blocks.pop();
        self.function_exits.pop();
        self.function_flow_depths.pop();
    }

    fn visit_import_declaration(&mut self, declaration: &ImportDeclaration<'a>) {
        let mut bindings = Vec::new();
        for specifier in declaration.specifiers.iter().flatten() {
            let (kind, local) = match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                    (ImportKind::Named, &specifier.local)
                }
                ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                    (ImportKind::Default, &specifier.local)
                }
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                    (ImportKind::Namespace, &specifier.local)
                }
            };
            bindings.push(ImportBindingFact {
                kind,
                local: NamedSpan {
                    span: span(local.span),
                },
                imported: match specifier {
                    ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                        Some(match &specifier.imported {
                            ModuleExportName::IdentifierName(name) => name.name.as_str().into(),
                            ModuleExportName::IdentifierReference(name) => {
                                name.name.as_str().into()
                            }
                            ModuleExportName::StringLiteral(name) => name.value.as_str().into(),
                        })
                    }
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(_) => Some("default".into()),
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => None,
                },
                type_only: declaration.import_kind.is_type()
                    || matches!(
                        specifier,
                        ImportDeclarationSpecifier::ImportSpecifier(specifier)
                            if specifier.import_kind.is_type()
                    ),
                runtime_referenced: local.symbol_id.get().is_some_and(|symbol| {
                    self.scoping
                        .get_resolved_references(symbol)
                        .any(|reference| {
                            reference.is_value() && !reference.flags().is_value_as_type()
                        })
                }),
            });
        }
        if bindings.is_empty() {
            bindings.push(ImportBindingFact {
                kind: ImportKind::SideEffect,
                local: NamedSpan {
                    span: span(declaration.source.span),
                },
                imported: None,
                type_only: false,
                runtime_referenced: true,
            });
        }
        self.imports.push(ImportFact {
            span: span(declaration.span),
            module: declaration.source.value.as_str().into(),
            specifier_span: Some(span(declaration.source.span)),
            type_only: declaration.import_kind.is_type(),
            bindings,
        });
        walk::walk_import_declaration(self, declaration);
    }

    fn visit_ts_import_equals_declaration(&mut self, declaration: &TSImportEqualsDeclaration<'a>) {
        if let TSModuleReference::ExternalModuleReference(reference) = &declaration.module_reference
        {
            let declaration_span = span(declaration.span);
            self.import_equals.push(ImportEqualsFact {
                span: declaration_span,
                module: reference.expression.value.as_str().into(),
                specifier_span: Some(span(reference.expression.span)),
                local: NamedSpan {
                    span: span(declaration.id.span),
                },
                type_only: declaration.import_kind.is_type(),
                exported: self.exported_import_equals.contains(&declaration_span),
            });
        }
        walk::walk_ts_import_equals_declaration(self, declaration);
    }

    fn visit_ts_type_query(&mut self, query: &TSTypeQuery<'a>) {
        self.type_queries.push(span(query.span));
        walk::walk_ts_type_query(self, query);
    }

    fn visit_ts_type_reference(&mut self, reference: &oxc_ast::ast::TSTypeReference<'a>) {
        self.type_names.push(span(reference.type_name.span()));
        walk::walk_ts_type_reference(self, reference);
    }

    fn visit_ts_class_implements(&mut self, implements: &oxc_ast::ast::TSClassImplements<'a>) {
        self.type_names.push(span(implements.expression.span()));
        walk::walk_ts_class_implements(self, implements);
    }

    fn visit_ts_interface_heritage(&mut self, heritage: &oxc_ast::ast::TSInterfaceHeritage<'a>) {
        self.type_names.push(span(heritage.expression.span()));
        walk::walk_ts_interface_heritage(self, heritage);
    }

    fn visit_ts_import_type(&mut self, import: &TSImportType<'a>) {
        let mut qualifier = import.qualifier.as_ref();
        let mut member = None;
        while let Some(current) = qualifier {
            match current {
                TSImportTypeQualifier::Identifier(name) => {
                    member = Some(name.name.as_str().into());
                    qualifier = None;
                }
                TSImportTypeQualifier::QualifiedName(name) => qualifier = Some(&name.left),
            }
        }
        self.type_imports.push(TypeImportFact {
            span: span(import.span),
            module: import.source.value.as_str().into(),
            specifier_span: Some(span(import.source.span)),
            member,
        });
        walk::walk_ts_import_type(self, import);
    }

    fn visit_export_named_declaration(&mut self, declaration: &ExportNamedDeclaration<'a>) {
        if let Some(Declaration::TSImportEqualsDeclaration(inner)) = &declaration.declaration {
            self.exported_import_equals.push(span(inner.span));
        }
        // Oxc represents a local `export { value }` name as a module-export
        // name and does not walk it through `visit_identifier_reference`.
        // Preserve the binder-selected declaration explicitly so consumers do
        // not have to reconnect the export by spelling. This is load-bearing
        // for a named export of a same-file default function/class declaration:
        // TypeScript gives that declaration the canonical export identity
        // `default`, even though its local binding has a source-level name.
        for specifier in &declaration.specifiers {
            if let ModuleExportName::IdentifierReference(identifier) = &specifier.local
                && let Some(target) = identifier
                    .reference_id
                    .get()
                    .and_then(|reference| self.scoping.get_reference(reference).symbol_id())
                    .map(|symbol| span(self.scoping.symbol_span(symbol)))
            {
                self.reference_declarations
                    .push((span(identifier.span), target));
            }
        }
        self.exports.push(ExportFact {
            span: span(declaration.span),
            kind: ExportKind::Named,
            module: declaration
                .source
                .as_ref()
                .map(|source| source.value.as_str().into()),
            specifier_span: declaration.source.as_ref().map(|source| span(source.span)),
            type_only: declaration.export_kind.is_type(),
            namespace: None,
            namespace_binding: None,
            specifiers: declaration
                .specifiers
                .iter()
                .map(|specifier| ExportSpecifierFact {
                    local: NamedSpan {
                        span: span(specifier.local.span()),
                    },
                    exported: module_export_name(&specifier.exported),
                    type_only: specifier.export_kind.is_type(),
                })
                .collect(),
            declarations: declaration
                .declaration
                .as_ref()
                .map_or_else(Vec::new, export_declaration_names),
            declaration_surface_only: declaration
                .declaration
                .as_ref()
                .map_or_else(Vec::new, export_declaration_surface_names),
        });
        walk::walk_export_named_declaration(self, declaration);
    }

    fn visit_ts_module_declaration(&mut self, declaration: &TSModuleDeclaration<'a>) {
        let introduces_ambient_context = declaration.declare
            || matches!(&declaration.id, TSModuleDeclarationName::StringLiteral(_));
        if self.ambient_module_depth == 0
            && !introduces_ambient_context
            && let TSModuleDeclarationName::Identifier(name) = &declaration.id
        {
            self.namespace_declarations.push(NamedSpan {
                span: span(name.span),
            });
        }
        self.ambient_module_depth += usize::from(introduces_ambient_context);
        walk::walk_ts_module_declaration(self, declaration);
        self.ambient_module_depth -= usize::from(introduces_ambient_context);
    }

    fn visit_ts_global_declaration(&mut self, declaration: &TSGlobalDeclaration<'a>) {
        self.ambient_module_depth += 1;
        walk::walk_ts_global_declaration(self, declaration);
        self.ambient_module_depth -= 1;
    }

    /// Record the body of every `namespace`, `module`, and `declare global`
    /// declaration. One hook covers all three shapes: `namespace A.B {}` nests
    /// module declarations and only the innermost carries a block, and
    /// `declare global {}` is a distinct node whose body is the same block
    /// type. See [`AstFacts::module_blocks`] for what consumers owe it.
    fn visit_ts_module_block(&mut self, block: &TSModuleBlock<'a>) {
        self.module_blocks.push(span(block.span));
        walk::walk_ts_module_block(self, block);
    }

    fn visit_export_default_declaration(&mut self, declaration: &ExportDefaultDeclaration<'a>) {
        let local = match &declaration.declaration {
            ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                function.id.as_ref().map_or(function.span, |id| id.span)
            }
            ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                class.id.as_ref().map_or(class.span, |id| id.span)
            }
            declaration => declaration.span(),
        };
        self.exports.push(ExportFact {
            span: span(declaration.span),
            kind: ExportKind::Default,
            module: None,
            specifier_span: None,
            type_only: false,
            namespace: None,
            namespace_binding: None,
            specifiers: vec![],
            declarations: vec![ExportSpecifierFact {
                local: NamedSpan { span: span(local) },
                exported: "default".into(),
                type_only: false,
            }],
            declaration_surface_only: vec![],
        });
        walk::walk_export_default_declaration(self, declaration);
    }

    fn visit_export_all_declaration(&mut self, declaration: &ExportAllDeclaration<'a>) {
        self.exports.push(ExportFact {
            span: span(declaration.span),
            kind: ExportKind::All,
            module: Some(declaration.source.value.as_str().into()),
            specifier_span: Some(span(declaration.source.span)),
            type_only: declaration.export_kind.is_type(),
            namespace: declaration.exported.as_ref().map(module_export_name),
            namespace_binding: declaration.exported.as_ref().map(|name| NamedSpan {
                span: span(name.span()),
            }),
            specifiers: vec![],
            declarations: vec![],
            declaration_surface_only: vec![],
        });
        walk::walk_export_all_declaration(self, declaration);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.identifiers.push(IdentifierFact {
            span: span(identifier.span),
            role: IdentifierRole::Reference,
        });
        // Resolved or not: a `declare function require` shadows the host's
        // `require` in the binder, and the host's is what runs.
        if identifier.name == "require"
            && !self.direct_require_callees.contains(&span(identifier.span))
        {
            self.unmodeled_require_references
                .push(span(identifier.span));
        }
        // The identifier, not `new Proxy(`: `Proxy.revocable` builds the same
        // thing, and `const P = Proxy` defers the construction to a name this
        // pass does not follow. A shadowed `Proxy` is somebody's own class and
        // states nothing.
        if self.is_unresolved_named(identifier, "Proxy") {
            self.record_accessor_installation(identifier.span);
        }
        if let Some(declaration) = identifier
            .reference_id
            .get()
            .and_then(|reference| self.scoping.get_reference(reference).symbol_id())
            .map(|symbol| span(self.scoping.symbol_span(symbol)))
        {
            self.reference_declarations
                .push((span(identifier.span), declaration));
        }
        walk::walk_identifier_reference(self, identifier);
    }

    fn visit_binding_identifier(&mut self, identifier: &oxc_ast::ast::BindingIdentifier<'a>) {
        self.identifiers.push(IdentifierFact {
            span: span(identifier.span),
            role: IdentifierRole::Binding,
        });
        walk::walk_binding_identifier(self, identifier);
    }

    fn visit_await_expression(&mut self, expression: &AwaitExpression<'a>) {
        self.awaits.push(span(expression.span));
        self.await_operands.push(span(expression.argument.span()));
        // `g?.(await p)` and `t?.[await p]` skip their arguments and keys
        // when the base is nullish (facts schema 64); what precedes a chain's
        // first `?.` always runs.
        let await_span = span(expression.span);
        let skippable = self.skippable_chain_sites.contains(&await_span)
            || self
                .skippable_chain_ranges
                .iter()
                .any(|range| range.contains(await_span));
        if !skippable
            && self.conditional_flow_depth
                == self
                    .function_flow_depths
                    .last()
                    .copied()
                    .unwrap_or_default()
        {
            self.unconditional_awaits.push(await_span);
        }
        let level = self.function_flow_depths.len();
        if !skippable
            && let Some(&(block, depth, block_level)) = self.open_blocks.last()
            && block_level == level
            && depth == self.conditional_flow_depth
        {
            self.dominating_awaits
                .push((await_span, Span::new(await_span.end, block.end)));
        }
        walk::walk_await_expression(self, expression);
    }

    fn visit_return_statement(&mut self, statement: &ReturnStatement<'a>) {
        let mut returned = self.return_fact(statement.argument.as_ref(), statement.span);
        returned.straight_line = self.conditional_flow_depth
            == self
                .function_flow_depths
                .last()
                .copied()
                .unwrap_or_default();
        self.returns.push(returned);
        walk::walk_return_statement(self, statement);
        self.mark_function_exit();
    }

    fn visit_if_statement(&mut self, statement: &IfStatement<'a>) {
        let test = span(statement.test.span());
        self.conditional_tests.push(test);
        self.if_regions.push(IfRegionFact {
            test,
            consequent: span(statement.consequent.span()),
        });
        self.visit_expression(&statement.test);
        self.conditional_control_stack.push(test);
        self.conditional_flow_depth += 1;
        self.visit_statement(&statement.consequent);
        if let Some(alternate) = &statement.alternate {
            self.visit_statement(alternate);
        }
        self.conditional_flow_depth -= 1;
        self.conditional_control_stack.pop();
    }

    /// A switch discriminant selects among the statement's clauses exactly as
    /// an if test selects a branch: it joins the conditional-test table, and
    /// returns inside the clauses carry it as a control test.
    fn visit_switch_statement(&mut self, statement: &oxc_ast::ast::SwitchStatement<'a>) {
        let discriminant = span(statement.discriminant.span());
        self.conditional_tests.push(discriminant);
        self.conditional_control_stack.push(discriminant);
        self.conditional_flow_depth += 1;
        walk::walk_switch_statement(self, statement);
        self.conditional_flow_depth -= 1;
        self.conditional_control_stack.pop();
    }

    fn visit_switch_case(&mut self, case: &oxc_ast::ast::SwitchCase<'a>) {
        for statement in &case.consequent {
            if let oxc_ast::ast::Statement::VariableDeclaration(declaration) = statement {
                self.switch_case_declarators.extend(
                    declaration
                        .declarations
                        .iter()
                        .map(|declarator| span(declarator.span)),
                );
            }
        }
        walk::walk_switch_case(self, case);
    }

    fn visit_assignment_expression(&mut self, expression: &AssignmentExpression<'a>) {
        if self.assignment_target_has_unresolved(&expression.left) {
            self.module_hazards.push(ModuleHazardFact {
                span: span(expression.span),
                kind: ModuleHazardKind::MutableUnboundGlobal,
            });
        }
        let plain = expression.operator == AssignmentOperator::Assign;
        if plain && matches!(expression.left, AssignmentTarget::ArrayAssignmentTarget(_)) {
            self.iterated_operands.push(span(expression.right.span()));
        }
        // A compound assignment coerces exactly as its binary operator does:
        // `total += obj` is `total + obj`. The logical assignments coerce
        // nothing.
        if matches!(
            expression.operator,
            AssignmentOperator::Addition
                | AssignmentOperator::Subtraction
                | AssignmentOperator::Multiplication
                | AssignmentOperator::Exponential
                | AssignmentOperator::Division
                | AssignmentOperator::Remainder
                | AssignmentOperator::ShiftLeft
                | AssignmentOperator::ShiftRight
                | AssignmentOperator::ShiftRightZeroFill
                | AssignmentOperator::BitwiseAnd
                | AssignmentOperator::BitwiseOR
                | AssignmentOperator::BitwiseXOR
        ) {
            self.coercing_operands
                .extend([span(expression.left.span()), span(expression.right.span())]);
        }
        let inner = expression.right.get_inner_expression();
        self.assignments.push(AssignmentFact {
            target: span(expression.left.span()),
            value_span: span(expression.right.span()),
            value: if plain {
                match expression.right {
                    Expression::ArrayExpression(_) => AssignmentValueKind::Array,
                    Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => {
                        AssignmentValueKind::Function
                    }
                    _ => AssignmentValueKind::Other,
                }
            } else {
                AssignmentValueKind::Other
            },
            reads_target: !plain,
            call_initializer: if plain {
                match inner {
                    Expression::CallExpression(call) => Some(span(call.span)),
                    _ => None,
                }
            } else {
                None
            },
            array_slots: if plain {
                match &expression.left {
                    AssignmentTarget::ArrayAssignmentTarget(array) => array
                        .elements
                        .iter()
                        .map(|element| element.as_ref().map(|element| span(element.span())))
                        .collect(),
                    _ => Vec::new(),
                }
            } else {
                Vec::new()
            },
        });
        let logical = matches!(
            expression.operator,
            AssignmentOperator::LogicalAnd
                | AssignmentOperator::LogicalOr
                | AssignmentOperator::LogicalNullish
        );
        self.logical_assignment_depth += usize::from(logical);
        // The right side of `??=`, `||=` and `&&=` runs only when the target
        // allows: an await or call there is not straight-line (facts schema 64).
        self.conditional_flow_depth += usize::from(logical);
        walk::walk_assignment_expression(self, expression);
        self.conditional_flow_depth -= usize::from(logical);
        self.logical_assignment_depth -= usize::from(logical);
    }

    /// A labelled statement's body can be left early by `break label` (or a
    /// labelled `continue`), skipping whatever follows the jump inside it
    /// while the code after the statement still runs (facts schema 64).
    fn visit_labeled_statement(&mut self, statement: &oxc_ast::ast::LabeledStatement<'a>) {
        self.conditional_flow_depth += 1;
        walk::walk_labeled_statement(self, statement);
        self.conditional_flow_depth -= 1;
    }

    /// A destructuring default runs only when the value is `undefined`
    /// (facts schema 64).
    fn visit_assignment_pattern(&mut self, pattern: &oxc_ast::ast::AssignmentPattern<'a>) {
        self.visit_binding_pattern(&pattern.left);
        self.conditional_flow_depth += 1;
        self.visit_expression(&pattern.right);
        self.conditional_flow_depth -= 1;
    }

    fn visit_assignment_target_with_default(
        &mut self,
        target: &oxc_ast::ast::AssignmentTargetWithDefault<'a>,
    ) {
        self.visit_assignment_target(&target.binding);
        self.conditional_flow_depth += 1;
        self.visit_expression(&target.init);
        self.conditional_flow_depth -= 1;
    }

    fn visit_simple_assignment_target(
        &mut self,
        target: &oxc_ast::ast::SimpleAssignmentTarget<'a>,
    ) {
        let written = target.get_expression().map_or_else(
            || span(target.span()),
            |expression| span(peel_ts_sugar(expression).span()),
        );
        self.write_targets.push(written);
        walk::walk_simple_assignment_target(self, target);
    }

    fn visit_update_expression(&mut self, expression: &UpdateExpression<'a>) {
        self.primitive_results.push(span(expression.span));
        if self.simple_assignment_target_has_unresolved(&expression.argument) {
            self.module_hazards.push(ModuleHazardFact {
                span: span(expression.span),
                kind: ModuleHazardKind::MutableUnboundGlobal,
            });
        }
        self.assignments.push(AssignmentFact {
            target: span(expression.argument.span()),
            value_span: span(expression.span()),
            value: AssignmentValueKind::Other,
            reads_target: true,
            call_initializer: None,
            array_slots: Vec::new(),
        });
        // Prefix and postfix `++`/`--` apply ToNumeric to the old value.
        self.coercing_operands
            .push(span(expression.argument.span()));
        walk::walk_update_expression(self, expression);
    }

    fn visit_for_statement_left(&mut self, left: &oxc_ast::ast::ForStatementLeft<'a>) {
        if let Some(target) = left.as_assignment_target() {
            // Not a declaration: every iteration writes an existing binding.
            self.iteration_targets.push(span(left.span()));
            if self.assignment_target_has_unresolved(target) {
                self.module_hazards.push(ModuleHazardFact {
                    span: span(left.span()),
                    kind: ModuleHazardKind::MutableUnboundGlobal,
                });
            }
        }
        walk::walk_for_statement_left(self, left);
    }

    fn visit_break_statement(&mut self, statement: &oxc_ast::ast::BreakStatement<'a>) {
        self.jump_statements.push(span(statement.span));
        walk::walk_break_statement(self, statement);
        self.mark_function_exit();
    }

    fn visit_continue_statement(&mut self, statement: &oxc_ast::ast::ContinueStatement<'a>) {
        self.jump_statements.push(span(statement.span));
        walk::walk_continue_statement(self, statement);
        self.mark_function_exit();
    }

    fn visit_throw_statement(&mut self, statement: &oxc_ast::ast::ThrowStatement<'a>) {
        walk::walk_throw_statement(self, statement);
        self.mark_function_exit();
    }

    fn visit_formal_parameter(&mut self, parameter: &FormalParameter<'a>) {
        if parameter.accessibility.is_some() || parameter.readonly {
            self.parameter_properties.extend(
                parameter
                    .pattern
                    .get_binding_identifiers()
                    .into_iter()
                    .map(|identifier| span(identifier.span)),
            );
        }
        walk::walk_formal_parameter(self, parameter);
    }

    fn visit_chain_expression(&mut self, expression: &oxc_ast::ast::ChainExpression<'a>) {
        let (sites, ranges) = (
            self.skippable_chain_sites.len(),
            self.skippable_chain_ranges.len(),
        );
        skippable_chain_parts(
            &expression.expression,
            &mut self.skippable_chain_sites,
            &mut self.skippable_chain_ranges,
        );
        self.optional_chain_depth += 1;
        walk::walk_chain_expression(self, expression);
        self.optional_chain_depth -= 1;
        self.skippable_chain_sites.truncate(sites);
        self.skippable_chain_ranges.truncate(ranges);
    }

    fn visit_conditional_expression(&mut self, expression: &ConditionalExpression<'a>) {
        self.conditional_tests.push(span(expression.test.span()));
        self.conditional_expressions
            .push(ConditionalExpressionFact {
                span: span(expression.span),
                test: span(expression.test.span()),
                consequent: span(expression.consequent.span()),
                alternate: span(expression.alternate.span()),
                consequent_array: array_literal_elements(&expression.consequent),
                alternate_array: array_literal_elements(&expression.alternate),
            });
        self.visit_expression(&expression.test);
        self.conditional_flow_depth += 1;
        self.visit_expression(&expression.consequent);
        self.visit_expression(&expression.alternate);
        self.conditional_flow_depth -= 1;
    }

    fn visit_logical_expression(&mut self, expression: &LogicalExpression<'a>) {
        if expression.operator == LogicalOperator::Coalesce
            && self.discarded_expression == Some(expression.span)
        {
            self.nullish_test_operands
                .push(span(peel_ts_sugar(&expression.left).span()));
        }
        self.logical_expressions.push(LogicalExpressionFact {
            span: span(expression.span),
            left: span(expression.left.span()),
            right: span(expression.right.span()),
            operator: match expression.operator {
                LogicalOperator::And => LogicalOperatorKind::And,
                LogicalOperator::Or => LogicalOperatorKind::Or,
                LogicalOperator::Coalesce => LogicalOperatorKind::Coalesce,
            },
        });
        self.visit_expression(&expression.left);
        self.conditional_flow_depth += 1;
        self.visit_expression(&expression.right);
        self.conditional_flow_depth -= 1;
    }

    fn visit_for_statement(&mut self, statement: &oxc_ast::ast::ForStatement<'a>) {
        self.loop_statements.push(span(statement.span));
        self.conditional_flow_depth += 1;
        walk::walk_for_statement(self, statement);
        self.conditional_flow_depth -= 1;
    }

    fn visit_for_in_statement(&mut self, statement: &oxc_ast::ast::ForInStatement<'a>) {
        self.loop_statements.push(span(statement.span));
        self.conditional_flow_depth += 1;
        walk::walk_for_in_statement(self, statement);
        self.conditional_flow_depth -= 1;
    }

    fn visit_for_of_statement(&mut self, statement: &oxc_ast::ast::ForOfStatement<'a>) {
        self.iterated_operands.push(span(statement.right.span()));
        self.loop_statements.push(span(statement.span));
        if statement.r#await {
            self.implicit_suspensions.push(span(statement.span));
        }
        self.conditional_flow_depth += 1;
        walk::walk_for_of_statement(self, statement);
        self.conditional_flow_depth -= 1;
    }

    fn visit_while_statement(&mut self, statement: &oxc_ast::ast::WhileStatement<'a>) {
        self.loop_statements.push(span(statement.span));
        self.conditional_flow_depth += 1;
        walk::walk_while_statement(self, statement);
        self.conditional_flow_depth -= 1;
    }

    fn visit_do_while_statement(&mut self, statement: &oxc_ast::ast::DoWhileStatement<'a>) {
        self.loop_statements.push(span(statement.span));
        self.conditional_flow_depth += 1;
        walk::walk_do_while_statement(self, statement);
        self.conditional_flow_depth -= 1;
    }

    fn visit_block_statement(&mut self, block: &oxc_ast::ast::BlockStatement<'a>) {
        self.open_blocks.push((
            span(block.span),
            self.conditional_flow_depth,
            self.function_flow_depths.len(),
        ));
        walk::walk_block_statement(self, block);
        self.open_blocks.pop();
    }

    fn visit_function_body(&mut self, body: &oxc_ast::ast::FunctionBody<'a>) {
        self.open_blocks.push((
            span(body.span),
            self.conditional_flow_depth,
            self.function_flow_depths.len(),
        ));
        walk::walk_function_body(self, body);
        self.open_blocks.pop();
    }

    fn visit_try_statement(&mut self, statement: &oxc_ast::ast::TryStatement<'a>) {
        let head = statement.block.body.first().and_then(|first| match first {
            oxc_ast::ast::Statement::ExpressionStatement(expression) => {
                match &expression.expression {
                    Expression::AwaitExpression(awaited) => Some(awaited),
                    _ => None,
                }
            }
            oxc_ast::ast::Statement::VariableDeclaration(declaration) => {
                match declaration.declarations.as_slice() {
                    [declarator] => match &declarator.init {
                        Some(Expression::AwaitExpression(awaited)) => Some(awaited),
                        _ => None,
                    },
                    _ => None,
                }
            }
            oxc_ast::ast::Statement::ReturnStatement(returned) => match &returned.argument {
                Some(Expression::AwaitExpression(awaited)) => Some(awaited),
                _ => None,
            },
            _ => None,
        });
        if let Some(awaited) = head {
            let literal_arguments = match &awaited.argument {
                Expression::CallExpression(call) => call
                    .arguments
                    .iter()
                    .filter(|argument| {
                        matches!(
                            argument,
                            oxc_ast::ast::Argument::StringLiteral(_)
                                | oxc_ast::ast::Argument::NumericLiteral(_)
                                | oxc_ast::ast::Argument::BigIntLiteral(_)
                                | oxc_ast::ast::Argument::BooleanLiteral(_)
                                | oxc_ast::ast::Argument::NullLiteral(_)
                        )
                    })
                    .map(|argument| span(argument.span()))
                    .collect(),
                _ => Vec::new(),
            };
            self.try_head_awaits.push(TryHeadAwaitFact {
                literal_arguments,
                awaited: span(awaited.span),
                operand: span(awaited.argument.span()),
                regions: statement
                    .handler
                    .iter()
                    .map(|handler| span(handler.body.span))
                    .chain(
                        statement
                            .finalizer
                            .iter()
                            .map(|finalizer| span(finalizer.span)),
                    )
                    .collect(),
            });
        }
        self.conditional_flow_depth += 1;
        // The `try` block runs to its end under the normal-completion premise:
        // discount it for completion sites only (straight-line calls and
        // unconditional awaits keep the whole statement conditional).
        if let Some(blocks) = self.function_try_blocks.last_mut() {
            *blocks += 1;
        }
        self.visit_block_statement(&statement.block);
        if let Some(blocks) = self.function_try_blocks.last_mut() {
            *blocks -= 1;
        }
        if let Some(handler) = &statement.handler {
            self.visit_catch_clause(handler);
        }
        if let Some(finalizer) = &statement.finalizer {
            self.visit_block_statement(finalizer);
        }
        self.conditional_flow_depth -= 1;
    }

    fn visit_object_expression(&mut self, object: &oxc_ast::ast::ObjectExpression<'a>) {
        let mut shape = ObjectGetShapeFact {
            span: span(object.span),
            closed: true,
            properties: Vec::new(),
        };
        for item in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = item else {
                shape.closed = false;
                continue;
            };
            let key: Option<CompactString> = match &property.key {
                PropertyKey::StaticIdentifier(key) => Some(key.name.as_str().into()),
                PropertyKey::StringLiteral(key) if !key.lone_surrogates => {
                    Some(key.value.as_str().into())
                }
                _ if property.computed => {
                    property.key.as_expression().and_then(literal_property_key)
                }
                _ => None,
            };
            let Some(key) = key else {
                shape.closed = false;
                continue;
            };
            // Integer-index keys reorder enumeration. Refuse them here until
            // the fact domain records canonical ECMAScript key order.
            if property.computed
                || key == "__proto__"
                || key.parse::<u32>().is_ok()
                || property.kind == PropertyKind::Set
                || shape.properties.iter().any(|prior| prior.key == key)
            {
                shape.closed = false;
            }
            shape.properties.push(ObjectGetPropertyFact {
                key,
                getter: (property.kind == PropertyKind::Get).then(|| span(property.value.span())),
                entry_call: (property.kind == PropertyKind::Get)
                    .then(|| getter_entry_call(&property.value))
                    .flatten(),
            });
        }
        self.object_get_shapes.push(shape);
        walk::walk_object_expression(self, object);
    }

    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        // `{ __proto__: p }` sets the prototype at construction, and no member
        // expression exists for a literal key, so the static-member arm never
        // sees it. A *computed* `{ ["__proto__"]: p }` is an ordinary own
        // property per the specification and installs nothing.
        let literal_proto = match &property.key {
            PropertyKey::StaticIdentifier(key) => key.name == "__proto__",
            PropertyKey::StringLiteral(key) => key.value == "__proto__",
            _ => false,
        };
        if literal_proto && !property.computed && !property.shorthand {
            self.record_accessor_installation(property.span);
        }
        let method_name = property
            .method
            .then(|| static_property_name(&property.key))
            .flatten();
        self.method_names.push(method_name);
        self.object_properties.push(ObjectPropertyFact {
            span: span(property.span),
            key: span(property.key.span()),
            value: span(property.value.span()),
            computed: property.computed,
            value_kind: self.runtime_value_kind(&property.value),
            runtime_type_escape: contains_runtime_type_escape(&property.value),
            shorthand_binding: self.shorthand_binding(property),
            data: property.kind == PropertyKind::Init && !property.method,
            method: property.kind == PropertyKind::Init && property.method,
        });
        walk::walk_object_property(self, property);
        self.method_names.pop();
    }

    fn visit_tagged_template_expression(
        &mut self,
        expression: &oxc_ast::ast::TaggedTemplateExpression<'a>,
    ) {
        // Not the default walk: that would visit the quasi through
        // `visit_template_literal` and record every tagged template's quasi
        // in `template_literals`, violating that table's untagged-only
        // invariant. The tag and the interpolated expressions are visited
        // directly so their own nested nodes — including genuinely untagged
        // templates inside an interpolation — are still collected.
        self.tagged_template_tags.push(span(expression.tag.span()));
        self.tagged_templates.push(TaggedTemplateFact {
            span: span(expression.span),
            tag: span(expression.tag.span()),
            substitutions: expression
                .quasi
                .expressions
                .iter()
                .map(|interpolated| span(interpolated.span()))
                .collect(),
        });
        self.visit_expression(&expression.tag);
        for interpolated in &expression.quasi.expressions {
            self.visit_expression(interpolated);
        }
    }

    fn visit_template_literal(&mut self, literal: &oxc_ast::ast::TemplateLiteral<'a>) {
        // Untagged only (see `visit_tagged_template_expression`): each
        // substitution is converted with ToString.
        self.coercing_operands.extend(
            literal
                .expressions
                .iter()
                .map(|interpolated| span(interpolated.span())),
        );
        self.template_literals.push(TemplateLiteralFact {
            span: span(literal.span),
            expressions: literal
                .expressions
                .iter()
                .map(|interpolated| span(interpolated.span()))
                .collect(),
        });
        walk::walk_template_literal(self, literal);
    }

    fn visit_jsx_child(&mut self, child: &oxc_ast::ast::JSXChild<'a>) {
        if let oxc_ast::ast::JSXChild::ExpressionContainer(container) = child
            && let Some(expression) = container.expression.as_expression()
        {
            self.jsx_child_expressions.push(span(expression.span()));
        }
        walk::walk_jsx_child(self, child);
    }

    fn visit_array_pattern(&mut self, pattern: &oxc_ast::ast::ArrayPattern<'a>) {
        let at = span(pattern.span);
        for (index, element) in pattern.elements.iter().enumerate() {
            let Some(element) = element else { continue };
            let (target, default) = binding_target_and_default(element);
            self.pattern_elements.push(PatternElementFact {
                pattern: at,
                slot: PatternSlot::Index(u32::try_from(index).unwrap_or(u32::MAX)),
                key: None,
                target,
                default,
                assignment: false,
            });
        }
        if let Some(rest) = &pattern.rest {
            self.pattern_elements.push(PatternElementFact {
                pattern: at,
                slot: PatternSlot::Rest,
                key: None,
                target: span(rest.argument.span()),
                default: None,
                assignment: false,
            });
        }
        walk::walk_array_pattern(self, pattern);
    }

    fn visit_object_pattern(&mut self, pattern: &oxc_ast::ast::ObjectPattern<'a>) {
        let at = span(pattern.span);
        for property in &pattern.properties {
            let (target, default) = binding_target_and_default(&property.value);
            self.pattern_elements.push(PatternElementFact {
                pattern: at,
                slot: pattern_key_slot(&property.key, property.computed),
                key: Some(span(property.key.span())),
                target,
                default,
                assignment: false,
            });
        }
        if let Some(rest) = &pattern.rest {
            self.pattern_elements.push(PatternElementFact {
                pattern: at,
                slot: PatternSlot::Rest,
                key: None,
                target: span(rest.argument.span()),
                default: None,
                assignment: false,
            });
        }
        walk::walk_object_pattern(self, pattern);
    }

    fn visit_array_assignment_target(&mut self, pattern: &oxc_ast::ast::ArrayAssignmentTarget<'a>) {
        let at = span(pattern.span);
        for (index, element) in pattern.elements.iter().enumerate() {
            let Some(element) = element else { continue };
            let (target, default) = assignment_target_and_default(element);
            self.pattern_elements.push(PatternElementFact {
                pattern: at,
                slot: PatternSlot::Index(u32::try_from(index).unwrap_or(u32::MAX)),
                key: None,
                target,
                default,
                assignment: true,
            });
        }
        if let Some(rest) = &pattern.rest {
            self.pattern_elements.push(PatternElementFact {
                pattern: at,
                slot: PatternSlot::Rest,
                key: None,
                target: span(rest.target.span()),
                default: None,
                assignment: true,
            });
        }
        walk::walk_array_assignment_target(self, pattern);
    }

    fn visit_object_assignment_target(
        &mut self,
        pattern: &oxc_ast::ast::ObjectAssignmentTarget<'a>,
    ) {
        use oxc_ast::ast::AssignmentTargetProperty;

        let at = span(pattern.span);
        for property in &pattern.properties {
            let element = match property {
                AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(property) => {
                    PatternElementFact {
                        pattern: at,
                        slot: PatternSlot::Key(property.binding.name.as_str().into()),
                        key: Some(span(property.binding.span)),
                        target: span(property.binding.span),
                        default: property.init.as_ref().map(|init| span(init.span())),
                        assignment: true,
                    }
                }
                AssignmentTargetProperty::AssignmentTargetPropertyProperty(property) => {
                    let (target, default) = assignment_target_and_default(&property.binding);
                    PatternElementFact {
                        pattern: at,
                        slot: pattern_key_slot(&property.name, property.computed),
                        key: Some(span(property.name.span())),
                        target,
                        default,
                        assignment: true,
                    }
                }
            };
            self.pattern_elements.push(element);
        }
        if let Some(rest) = &pattern.rest {
            self.pattern_elements.push(PatternElementFact {
                pattern: at,
                slot: PatternSlot::Rest,
                key: None,
                target: span(rest.target.span()),
                default: None,
                assignment: true,
            });
        }
        walk::walk_object_assignment_target(self, pattern);
    }

    fn visit_jsx_fragment(&mut self, fragment: &oxc_ast::ast::JSXFragment<'a>) {
        self.jsx_fragments.push(span(fragment.span));
        walk::walk_jsx_fragment(self, fragment);
    }

    fn visit_jsx_element(&mut self, element: &JSXElement<'a>) {
        let current_scope = self
            .scope_stack
            .last()
            .copied()
            .unwrap_or_else(|| self.scoping.root_scope_id());
        let name_span = element.opening_element.name.span();
        let (member_object, member_property) =
            if let JSXElementName::MemberExpression(member) = &element.opening_element.name {
                (
                    Some(span(member.object.span())),
                    Some(span(member.property.span)),
                )
            } else {
                (None, None)
            };
        self.jsx_elements.push(JsxElementFact {
            span: span(element.span),
            opening: span(element.opening_element.span),
            name: NamedSpan {
                span: span(name_span),
            },
            closing_name: element
                .closing_element
                .as_ref()
                .map(|closing| span(closing.name.span())),
            member_object,
            member_property,
            properties: element
                .opening_element
                .attributes
                .iter()
                .filter_map(|item| {
                    let JSXAttributeItem::Attribute(attribute) = item else {
                        return None;
                    };
                    let JSXAttributeName::Identifier(name) = &attribute.name else {
                        return None;
                    };
                    Some(span(name.span))
                })
                .collect(),
            boolean_properties: element
                .opening_element
                .attributes
                .iter()
                .filter_map(|item| {
                    let JSXAttributeItem::Attribute(attribute) = item else {
                        return None;
                    };
                    let JSXAttributeName::Identifier(name) = &attribute.name else {
                        return None;
                    };
                    let value = match attribute.value.as_ref() {
                        None => true,
                        Some(JSXAttributeValue::ExpressionContainer(container)) => {
                            let JSXExpression::BooleanLiteral(value) = &container.expression else {
                                return None;
                            };
                            value.value
                        }
                        _ => return None,
                    };
                    Some(BooleanPropertyFact {
                        name: span(name.span),
                        value,
                    })
                })
                .collect(),
            attributes: element
                .opening_element
                .attributes
                .iter()
                .filter_map(|item| {
                    let JSXAttributeItem::Attribute(attribute) = item else {
                        return None;
                    };
                    let (name, namespace, local_name, directive_binding) = match &attribute.name {
                        JSXAttributeName::Identifier(name) => (name.span, None, name.span, None),
                        JSXAttributeName::NamespacedName(name) => {
                            let directive_binding = (name.namespace.name == "use")
                                .then(|| {
                                    self.scoping
                                        .find_binding(current_scope, name.name.name.as_str().into())
                                        .filter(|&symbol| {
                                            self.scoping
                                                .symbol_flags(symbol)
                                                .can_be_referenced_by_value()
                                        })
                                        .map(|symbol| span(self.scoping.symbol_span(symbol)))
                                })
                                .flatten();
                            (
                                name.span,
                                Some(name.namespace.span),
                                name.name.span,
                                directive_binding,
                            )
                        }
                    };
                    let (value, expression, runtime_type_escape, value_kind) =
                        match attribute.value.as_ref() {
                            None => (None, None, false, JsxAttributeValueKind::Boolean),
                            Some(JSXAttributeValue::StringLiteral(value)) => (
                                Some(span(value.span)),
                                None,
                                false,
                                JsxAttributeValueKind::String,
                            ),
                            Some(JSXAttributeValue::ExpressionContainer(container)) => (
                                Some(span(container.span)),
                                Some(span(container.expression.span())),
                                container
                                    .expression
                                    .as_expression()
                                    .is_some_and(contains_runtime_type_escape),
                                JsxAttributeValueKind::Expression,
                            ),
                            Some(JSXAttributeValue::Element(value)) => (
                                Some(span(value.span)),
                                None,
                                false,
                                JsxAttributeValueKind::Element,
                            ),
                            Some(JSXAttributeValue::Fragment(value)) => (
                                Some(span(value.span)),
                                None,
                                false,
                                JsxAttributeValueKind::Fragment,
                            ),
                        };
                    Some(JsxAttributeFact {
                        span: span(attribute.span),
                        name: span(name),
                        namespace: namespace.map(span),
                        local_name: span(local_name),
                        directive_binding,
                        value,
                        expression,
                        runtime_type_escape,
                        value_kind,
                    })
                })
                .collect(),
            spreads: element
                .opening_element
                .attributes
                .iter()
                .filter_map(|item| {
                    let JSXAttributeItem::SpreadAttribute(attribute) = item else {
                        return None;
                    };
                    Some(JsxSpreadAttributeFact {
                        span: span(attribute.span),
                        argument: span(attribute.argument.span()),
                    })
                })
                .collect(),
            self_closing: element.closing_element.is_none(),
            children: element
                .children
                .iter()
                .map(|child| span(child.span()))
                .collect(),
        });
        walk::walk_jsx_element(self, element);
    }

    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        if Self::ACCESSOR_INSTALLING_MEMBERS.contains(&member.property.name.as_str()) {
            self.record_accessor_installation(member.span);
        }
        if let Expression::Identifier(object) = &member.object
            && self.is_unresolved_named(object, "WebAssembly")
        {
            self.module_hazards.push(ModuleHazardFact {
                span: span(member.span),
                kind: ModuleHazardKind::OpaqueWasm,
            });
        }
        if Self::is_import_meta(&member.object)
            && matches!(member.property.name.as_str(), "glob" | "globEager")
            && !self.lazy_import_glob_callees.contains(&span(member.span))
        {
            self.in_place_import_globs.push(span(member.span));
        }
        self.members.push(MemberFact {
            span: span(member.span),
            object: span(member.object.span()),
            property: span(member.property.span),
        });
        if member.optional {
            self.optional_members.push(span(member.span));
        }
        self.record_completion_site(member.span);
        walk::walk_static_member_expression(self, member);
    }

    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        // A computed member on an intrinsic namespace can spell any of the
        // names above, so the namespace itself is the fact.
        if let Expression::Identifier(object) = &member.object
            && (self.is_unresolved_named(object, "Object")
                || self.is_unresolved_named(object, "Reflect"))
        {
            self.record_accessor_installation(member.span);
        }
        // `import.meta["glob"]`, or a computed key that may spell it.
        if Self::is_import_meta(&member.object)
            && literal_property_key(&member.expression)
                .is_none_or(|key| matches!(key.as_str(), "glob" | "globEager"))
        {
            self.in_place_import_globs.push(span(member.span));
        }
        let property = member.expression.span();
        let member_span = span(member.span);
        self.members.push(MemberFact {
            span: member_span,
            object: span(member.object.span()),
            property: span(property),
        });
        if member.optional {
            self.optional_members.push(member_span);
        }
        self.computed_members.push(member_span);
        if let Some(key) = literal_property_key(&member.expression) {
            self.literal_computed_members.push(LiteralMemberKeyFact {
                span: member_span,
                key,
            });
        }
        self.record_completion_site(member.span);
        walk::walk_computed_member_expression(self, member);
    }

    fn visit_array_expression(&mut self, array: &oxc_ast::ast::ArrayExpression<'a>) {
        self.array_literals.push(span(array.span));
        self.array_literal_elements.push(ArrayLiteralFact {
            span: span(array.span),
            elements: array
                .elements
                .iter()
                .map(|element| match element {
                    ArrayExpressionElement::SpreadElement(spread) => ArrayElementFact {
                        kind: ArrayElementKind::Spread,
                        span: span(spread.argument.span()),
                    },
                    ArrayExpressionElement::Elision(hole) => ArrayElementFact {
                        kind: ArrayElementKind::Hole,
                        span: span(hole.span),
                    },
                    value => ArrayElementFact {
                        kind: ArrayElementKind::Value,
                        span: span(value.span()),
                    },
                })
                .collect(),
        });
        walk::walk_array_expression(self, array);
    }

    fn visit_array_expression_element(&mut self, element: &ArrayExpressionElement<'a>) {
        if let ArrayExpressionElement::SpreadElement(spread) = element {
            self.iterated_operands.push(span(spread.argument.span()));
        }
        walk::walk_array_expression_element(self, element);
    }

    // `walk_arguments` visits a spread argument through `visit_spread_element`
    // directly, never through `visit_argument`, so the list is the hook.
    fn visit_arguments(&mut self, arguments: &oxc_allocator::Vec<'a, Argument<'a>>) {
        for argument in arguments {
            if let Argument::SpreadElement(spread) = argument {
                self.iterated_operands.push(span(spread.argument.span()));
            }
        }
        walk::walk_arguments(self, arguments);
    }

    fn visit_yield_expression(&mut self, expression: &oxc_ast::ast::YieldExpression<'a>) {
        if expression.delegate {
            self.implicit_suspensions.push(span(expression.span));
        }
        if expression.delegate
            && let Some(argument) = &expression.argument
        {
            self.iterated_operands.push(span(argument.span()));
        }
        walk::walk_yield_expression(self, expression);
    }

    fn visit_spread_element(&mut self, spread: &SpreadElement<'a>) {
        self.spreads.push(SpreadFact {
            span: span(spread.span),
            argument: span(spread.argument.span()),
        });
        walk::walk_spread_element(self, spread);
    }

    fn visit_binary_expression(&mut self, expression: &BinaryExpression<'a>) {
        use oxc_syntax::operator::BinaryOperator;
        // Every binary operator yields a number, bigint, string or boolean.
        self.primitive_results.push(span(expression.span));

        if matches!(
            expression.operator,
            BinaryOperator::StrictEquality
                | BinaryOperator::StrictInequality
                | BinaryOperator::Equality
                | BinaryOperator::Inequality
        ) {
            for (value, tested) in [
                (&expression.left, &expression.right),
                (&expression.right, &expression.left),
            ] {
                if matches!(peel_ts_sugar(tested), Expression::NullLiteral(_))
                    || matches!(peel_ts_sugar(tested), Expression::Identifier(id) if self.is_global_undefined(id))
                {
                    self.nullish_test_operands
                        .push(span(peel_ts_sugar(value).span()));
                }
            }
        }

        // The producer's `coercingBinaryOperators`, exactly: every operator
        // that applies ToPrimitive to both operands. `===`/`!==`, `in` and
        // `instanceof` are absent there and here. And its one exception, also
        // exactly (`binaryFormLocked`): a loose (in)equality with the `null`
        // keyword itself as an operand -- unparenthesized, as the producer
        // compares the operand's own node kind -- takes IsLooselyEqual's
        // null arm and applies ToPrimitive to nothing. `undefined` is an
        // identifier there, not a keyword, and stays a coercion on both sides.
        let null_comparison = matches!(
            expression.operator,
            BinaryOperator::Equality | BinaryOperator::Inequality
        ) && (matches!(&expression.left, Expression::NullLiteral(_))
            || matches!(&expression.right, Expression::NullLiteral(_)));
        if !null_comparison
            && matches!(
                expression.operator,
                BinaryOperator::Addition
                    | BinaryOperator::Subtraction
                    | BinaryOperator::Multiplication
                    | BinaryOperator::Exponential
                    | BinaryOperator::Division
                    | BinaryOperator::Remainder
                    | BinaryOperator::LessThan
                    | BinaryOperator::GreaterThan
                    | BinaryOperator::LessEqualThan
                    | BinaryOperator::GreaterEqualThan
                    | BinaryOperator::Equality
                    | BinaryOperator::Inequality
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::ShiftRightZeroFill
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseOR
                    | BinaryOperator::BitwiseXOR
            )
        {
            self.coercing_operands
                .extend([span(expression.left.span()), span(expression.right.span())]);
        }

        // Most binary operators reject function operands themselves, so a
        // checker finding there would duplicate TypeScript. Keep only the
        // syntactically proven string-concatenation subset: `+` with a string
        // literal on one side. Broader concatenation requires the operator's
        // resolved signature, which this structural fact deliberately does
        // not guess.
        if expression.operator == BinaryOperator::Addition
            && (matches!(&expression.left, Expression::StringLiteral(_))
                || matches!(&expression.right, Expression::StringLiteral(_)))
        {
            self.coercive_operands.extend([
                CoerciveOperandFact {
                    span: span(expression.left.span()),
                    kind: CoerciveOperandKind::StringConcatenation,
                },
                CoerciveOperandFact {
                    span: span(expression.right.span()),
                    kind: CoerciveOperandKind::StringConcatenation,
                },
            ]);
        }
        walk::walk_binary_expression(self, expression);
    }

    fn visit_unary_expression(&mut self, expression: &UnaryExpression<'a>) {
        use oxc_syntax::operator::UnaryOperator;
        if expression.operator != UnaryOperator::Void {
            self.primitive_results.push(span(expression.span));
        }
        if expression.operator == UnaryOperator::Delete {
            self.deleted_targets.push(span(expression.argument.span()));
        }

        // Every unary operator here accepts a function operand in TypeScript
        // (probed against the published typings: `-f`, `+f`, `~f`, and `!f`
        // are all clean in the strict and loose passes), so a coerced accessor
        // in one of these slots is this checker's to report rather than a
        // duplicate of a type error. `typeof`, `void`, and `delete` stay out:
        // they inspect the function itself.
        let kind = match expression.operator {
            UnaryOperator::LogicalNot => Some(CoerciveOperandKind::LogicalNot),
            UnaryOperator::UnaryPlus | UnaryOperator::UnaryNegation | UnaryOperator::BitwiseNot => {
                Some(CoerciveOperandKind::NumericCoercion)
            }
            _ => None,
        };
        if let Some(kind) = kind {
            self.coercive_operands.push(CoerciveOperandFact {
                span: span(expression.argument.span()),
                kind,
            });
        }
        // The producer's `coercingUnaryOperators`: `!` is ToBoolean and runs
        // no user code, so it is in the table above and not in this one.
        if matches!(
            expression.operator,
            UnaryOperator::UnaryPlus | UnaryOperator::UnaryNegation | UnaryOperator::BitwiseNot
        ) {
            self.coercing_operands
                .push(span(expression.argument.span()));
        }
        walk::walk_unary_expression(self, expression);
    }
}
