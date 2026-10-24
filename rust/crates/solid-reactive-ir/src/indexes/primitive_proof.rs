//! Primitive proofs: whether a value is proven primitive or builtin, and the cross-file consts that feed it.

use super::*;

impl<'a> SemanticLookup<'a> {
    /// The expression inside any parentheses around `value`; `None` when a
    /// type assertion (`as`, `<T>`, `!`, `satisfies`) wraps it on the way,
    /// which changes the declared type and never the value.
    pub(crate) fn peel_parentheses(file: &FileFacts, value: Span) -> Option<Span> {
        let mut value = value;
        while let Ok(index) = file
            .ast
            .transparent_wrappers
            .binary_search_by_key(&value, |wrapper| wrapper.span)
        {
            let wrapper = &file.ast.transparent_wrappers[index];
            if wrapper.inner == value {
                break;
            }
            let parenthesized = wrapper.inner.start > value.start
                && file.source.as_bytes().get(value.start as usize) == Some(&b'(');
            if !parenthesized {
                return None;
            }
            value = wrapper.inner;
        }
        Some(value)
    }

    /// ADR 0320: whether the expression at `value` evaluates to a primitive
    /// other than `undefined`, proven from what it is rather than from its
    /// declared type: a literal or template, an operator whose result is
    /// always primitive, a `.length` of a proven built-in value, a primitive
    /// result of an audited standard-library function, a conditional or
    /// logical expression all of whose results are, or an unexported `const`
    /// of this file initialized with one. A type assertion (`as`, `<T>`, `!`)
    /// anywhere on the way refuses: it changes the declared type, never the
    /// value. Only this file is read, so a cached answer follows its source.
    pub(crate) fn proven_primitive(&self, file: &FileFacts, value: Span, depth: usize) -> bool {
        if depth == 0 {
            return false;
        }
        let Some(value) = Self::peel_parentheses(file, value) else {
            return false;
        };
        if file.ast.primitive_results.binary_search(&value).is_ok() {
            return true;
        }
        let Some(text) = file.source_text(value) else {
            return false;
        };
        if primitive_literal_text(text) {
            return true;
        }
        if let Some(conditional) = file.ast.conditional_at(value) {
            return self.proven_primitive(file, conditional.consequent, depth - 1)
                && self.proven_primitive(file, conditional.alternate, depth - 1);
        }
        if let Some(logical) = file.ast.logical_at(value) {
            return self.proven_primitive(file, logical.left, depth - 1)
                && self.proven_primitive(file, logical.right, depth - 1);
        }
        if let Some(member) = file.ast.member_at(value) {
            return file.source_text(member.property) == Some("length")
                && file
                    .ast
                    .computed_members
                    .binary_search(&member.span)
                    .is_err()
                && !self.member_may_be_reassigned_on_builtin("length")
                && matches!(
                    self.value_origin(
                        file,
                        member.object,
                        solid_facts::ast::RuntimeValueKind::Unknown,
                        depth - 1
                    ),
                    Some(ValueOrigin::Builtin)
                );
        }
        if let Some(call) = file.ast.call_at(value) {
            if call.construct || call.arguments.iter().any(|argument| argument.spread) {
                return false;
            }
            return self
                .standard_library_declaration(file, call, typefacts::CallKind::Call)
                .is_some_and(|declaration| {
                    PRIMITIVE_RESULT_FUNCTIONS.contains(&declaration.qualified_name.as_ref())
                });
        }
        let Some((binding_file, binding, _)) = self.binding_at_reference(file.path.as_str(), value)
        else {
            return false;
        };
        // ADR 0325: a named import of another module's exported constant.
        if binding_file.path != file.path {
            return self.imported_const_is_primitive(file, value, binding_file, binding, depth);
        }
        if !binding.immutable
            || binding.shape != solid_facts::ast::BindingShape::Identifier
            || crate::execution_role::flow_binding_exported(
                binding_file,
                binding
                    .names
                    .first()
                    .map_or(binding.declaration, |name| name.span),
            )
            || binding_file.ast.functions.iter().any(|function| {
                function.parameters.iter().any(|parameter| {
                    parameter
                        .names
                        .iter()
                        .any(|name| binding.names.iter().any(|bound| bound.span == name.span))
                })
            })
        {
            return false;
        }
        binding
            .initializer
            .is_some_and(|initializer| self.proven_primitive(file, initializer, depth - 1))
    }

    /// ADR 0325: whether the identifier at `value`, a named import of
    /// `binding` (declared in `binding_file`), holds a primitive other than
    /// `undefined`. Only the exact shape:
    ///
    /// - the identifier's declaration is the local of a named, value import
    ///   (not a namespace, a default or a type-only one) of a module that the
    ///   compiler's attested resolution lands on `binding_file` itself, with
    ///   no re-export hop;
    /// - `binding_file` is a module, and `binding` is a module-level `const`
    ///   of one plain identifier that the module exports under exactly the
    ///   imported name (`export const X`, or `export { N as X }`), not through
    ///   an `export … from`;
    /// - the initializer is proven primitive by [`Self::proven_primitive`],
    ///   in `binding_file`.
    ///
    /// A `const` cannot be reassigned and an ES module namespace is frozen, so
    /// nothing can change the value after initialization; anything that is
    /// not exactly this refuses (a barrel, an `export *`, a default import, an
    /// object, a call the audited table does not name, an assertion).
    fn imported_const_is_primitive(
        &self,
        file: &FileFacts,
        value: Span,
        binding_file: &'a FileFacts,
        binding: &solid_facts::ast::BindingFact,
        depth: usize,
    ) -> bool {
        use solid_facts::ast::{BindingShape, ExportKind, ImportKind};
        // Another file's constant: the answer is one of the set
        // [`Self::exported_primitive_consts`] holds, which the section binds.
        crate::base::reads::note(crate::base::reads::Section::ExportedPrimitiveConsts);
        let value = file.ast.peel_ts_sugar_span(value);
        let Some(declaration) = file.ast.reference_declaration(value) else {
            return false;
        };
        let Some((import, imported)) = file.ast.imports.iter().find_map(|import| {
            import.bindings.iter().find_map(|candidate| {
                (candidate.local.span == declaration
                    && candidate.kind == ImportKind::Named
                    && !candidate.type_only
                    && !import.type_only)
                    .then_some((import, candidate.imported.as_deref()))
            })
        }) else {
            return false;
        };
        let (Some(imported), Some(imports)) = (imported, self.resolved_imports()) else {
            return false;
        };
        let solid_facts::SpecifierAttestation::Attested(row) =
            imports.specifier(file.path.as_str(), import.span, import.module.as_str())
        else {
            return false;
        };
        let landing = if row.included_path.is_empty() {
            row.resolved_path.as_ref()
        } else {
            row.included_path.as_ref()
        };
        let [name] = binding.names.as_slice() else {
            return false;
        };
        if landing != binding_file.path.as_str()
            || row.resolution == solid_facts::ImportResolution::Unresolved
            || !crate::execution_role::file_is_module(binding_file)
            || file_names_commonjs_exports(binding_file)
            || !binding.immutable
            || binding.shape != BindingShape::Identifier
            || binding_file
                .ast
                .functions
                .iter()
                .any(|function| function.span.contains(binding.declaration))
        {
            return false;
        }
        let exported = binding_file.ast.exports.iter().any(|export| {
            export.module.is_none()
                && export.kind == ExportKind::Named
                && !export.type_only
                && (export.declarations.iter().any(|specifier| {
                    specifier.local.span == name.span
                        && specifier.exported.as_str() == imported
                        && !specifier.type_only
                }) || export.specifiers.iter().any(|specifier| {
                    specifier.exported.as_str() == imported
                        && !specifier.type_only
                        && binding_file.ast.reference_declaration(specifier.local.span)
                            == Some(name.span)
                }))
        });
        exported
            && binding.initializer.is_some_and(|initializer| {
                self.proven_primitive(binding_file, initializer, depth.saturating_sub(1))
            })
    }

    /// ADR 0325: the imported-constant answers the cross-file proofs read,
    /// as `(path, declaration span)` of every exported module-level `const`
    /// of one identifier whose initializer is proven primitive: the set
    /// [`Self::imported_const_is_primitive`] can accept. Empty when no file
    /// imports a binding by name.
    pub(super) fn exported_primitive_consts(&self) -> Vec<(String, Span)> {
        self.whole_program_answer::<super::project_memos::ExportedPrimitiveConsts>()
    }

    /// [`Self::exported_primitive_consts`]'s computation (the memo's).
    pub(super) fn compute_exported_primitive_consts(&self) -> Vec<(String, Span)> {
        use solid_facts::ast::{BindingShape, ImportKind};
        if !self.files().iter().any(|file| {
            file.ast.imports.iter().any(|import| {
                import
                    .bindings
                    .iter()
                    .any(|binding| binding.kind == ImportKind::Named)
            })
        }) {
            return Vec::new();
        }
        let mut found = Vec::new();
        for file in self.files() {
            if !crate::execution_role::file_is_module(file) || file_names_commonjs_exports(file) {
                continue;
            }
            for binding in &file.ast.bindings {
                let [name] = binding.names.as_slice() else {
                    continue;
                };
                if !binding.immutable
                    || binding.shape != BindingShape::Identifier
                    || file
                        .ast
                        .functions
                        .iter()
                        .any(|function| function.span.contains(binding.declaration))
                    || !file.ast.exports.iter().any(|export| {
                        export.module.is_none()
                            && (export
                                .declarations
                                .iter()
                                .any(|specifier| specifier.local.span == name.span)
                                || export.specifiers.iter().any(|specifier| {
                                    file.ast.reference_declaration(specifier.local.span)
                                        == Some(name.span)
                                }))
                    })
                {
                    continue;
                }
                if binding
                    .initializer
                    .is_some_and(|initializer| self.proven_primitive(file, initializer, 6))
                {
                    found.push((file.path.to_string(), name.span));
                }
            }
        }
        found.sort_unstable();
        found
    }

    /// ADR 0211: what the origin of the value at `value` proves about the
    /// object it evaluates to, followed through `const` bindings.
    ///
    /// `kind` is the value's syntactic runtime kind where the fact supplies
    /// one (an argument's, a binding initializer's), `Unknown` elsewhere.
    pub(crate) fn value_origin(
        &self,
        file: &FileFacts,
        value: Span,
        kind: solid_facts::ast::RuntimeValueKind,
        depth: usize,
    ) -> Option<ValueOrigin<'a>> {
        let origin = self.value_origin_assuming(file, value, kind, depth, None)?;
        // ADR 0320: a project instance's constructor may hand `this` to code
        // that copies keys into it; a built-in constructor hands its result
        // to nothing, and an unshared binding holds it.
        match origin {
            ValueOrigin::ProjectClass(..) if self.objects_may_be_assigned_into() => None,
            origin => Some(origin),
        }
    }

    /// [`Self::value_origin`], with one parameter -- a setter updater's
    /// previous value -- assumed to hold a built-in value already. The
    /// assumption is the induction step: a signal every write of which keeps a
    /// built-in value built-in, from a built-in initial value, only ever holds
    /// one.
    pub(super) fn value_origin_assuming(
        &self,
        file: &FileFacts,
        value: Span,
        kind: solid_facts::ast::RuntimeValueKind,
        depth: usize,
        assumed: Option<(&str, Span)>,
    ) -> Option<ValueOrigin<'a>> {
        use solid_facts::ast::RuntimeValueKind;
        if depth == 0 {
            return None;
        }
        // A fresh array: its members are `Array.prototype`'s. A primitive's
        // are its wrapper prototype's, and a nullish value has none to run.
        if matches!(
            kind,
            RuntimeValueKind::Array | RuntimeValueKind::Primitive | RuntimeValueKind::Nullish
        ) {
            return Some(ValueOrigin::Builtin);
        }
        let value = file.ast.peel_ts_sugar_span(value);
        if file.ast.array_literals.binary_search(&value).is_ok() {
            return Some(ValueOrigin::Builtin);
        }
        // `null`, and an `undefined` no local declaration shadows: nothing.
        match file.source_text(value) {
            Some("null") => return Some(ValueOrigin::Builtin),
            Some("undefined") if file.ast.reference_declaration(value).is_none() => {
                return Some(ValueOrigin::Builtin);
            }
            _ => {}
        }
        if let Some(call) = file.ast.call_at(value) {
            return self.call_origin(file, call, depth, assumed);
        }
        if let Some(conditional) = file.ast.conditional_at(value) {
            let arm = |span| {
                matches!(
                    self.value_origin_assuming(
                        file,
                        span,
                        RuntimeValueKind::Unknown,
                        depth - 1,
                        assumed
                    ),
                    Some(ValueOrigin::Builtin)
                )
            };
            return (arm(conditional.consequent) && arm(conditional.alternate))
                .then_some(ValueOrigin::Builtin);
        }
        // `a ?? b`, `a || b`, `a && b`: the value is one operand or the other.
        if let Some(logical) = file.ast.logical_at(value) {
            let operand = |span| {
                matches!(
                    self.value_origin_assuming(
                        file,
                        span,
                        RuntimeValueKind::Unknown,
                        depth - 1,
                        assumed
                    ),
                    Some(ValueOrigin::Builtin)
                )
            };
            return (operand(logical.left) && operand(logical.right))
                .then_some(ValueOrigin::Builtin);
        }
        if let Some((path, parameter)) = assumed
            && path == file.path.as_str()
            && file.ast.reference_declaration(value) == Some(parameter)
        {
            return Some(ValueOrigin::Builtin);
        }
        let (binding_file, binding, symbol) =
            self.binding_at_reference(file.path.as_str(), value)?;
        if !binding.immutable || binding.shape != solid_facts::ast::BindingShape::Identifier {
            return None;
        }
        // ADR 0320: a parameter's default holds only when no argument is
        // passed; an object a binding holds must stay unshared.
        let [name] = binding.names.as_slice() else {
            return None;
        };
        if binding_file.ast.functions.iter().any(|function| {
            function
                .parameters
                .iter()
                .any(|parameter| parameter.names.iter().any(|n| n.span == name.span))
        }) || (!matches!(
            binding.initializer_value_kind,
            solid_facts::ast::RuntimeValueKind::Primitive
                | solid_facts::ast::RuntimeValueKind::Nullish
        ) && !{
            // Untracked consumers: only callees in the binding's own file,
            // whose source this file's cached answers already depend on.
            let mut consulted = Vec::new();
            self.binding_stays_closed(
                binding_file,
                name.span,
                &symbol,
                file,
                value,
                &mut consulted,
            ) && consulted
                .iter()
                .all(|(path, _)| path.as_str() == binding_file.path.as_str())
        }) {
            return None;
        }
        self.value_origin_assuming(
            binding_file,
            binding.initializer?,
            binding.initializer_value_kind,
            depth - 1,
            assumed,
        )
    }

    fn call_origin(
        &self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        depth: usize,
        assumed: Option<(&str, Span)>,
    ) -> Option<ValueOrigin<'a>> {
        if !call.construct
            && call.arguments.is_empty()
            && let Some(origin) = self.accessor_origin(file, call, depth)
        {
            return Some(origin);
        }
        // A plain call of an exact project function returns what its body
        // returns. A method call is not followed: its receiver selects it.
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        if !call.construct
            && !self.is_member_span(file, callee)
            && file.ast.computed_members.binary_search(&callee).is_err()
            && let Some((function_file, function)) = self
                .callee_symbol(file, call.callee)
                .and_then(|symbol| self.function_for_symbol(symbol))
                .filter(|(function_file, function)| {
                    self.function_value_is_current(function_file, function)
                })
        {
            return self
                .returns_builtin(function_file, function, None, depth)
                .then_some(ValueOrigin::Builtin);
        }
        if call.construct {
            if let Some((class_file, class)) = self
                .callee_symbol(file, call.callee)
                .and_then(|symbol| self.class_for_symbol(symbol))
            {
                return Some(ValueOrigin::ProjectClass(class_file, class));
            }
            let declaration =
                self.standard_library_declaration(file, call, typefacts::CallKind::Construct)?;
            return BUILTIN_VALUE_CONSTRUCTORS
                .contains(&declaration.qualified_name.as_ref())
                .then_some(ValueOrigin::Builtin);
        }
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        let member = file.ast.member_at(callee);
        // A member of a proven built-in receiver is that receiver's own: its
        // class's prototype selects it, so no declaration is needed. An
        // unproven receiver falls through to the declaration below.
        // `split` defers to its separator's `Symbol.split`, user code unless
        // the separator is a primitive (or a RegExp literal).
        let separator_is_primitive = call.arguments.first().is_none_or(|separator| {
            !separator.spread
                && separator.runtime_value_kind == solid_facts::ast::RuntimeValueKind::Primitive
        });
        if let Some(member) = member
            && let Some(method) = file.source_text(member.property)
            && (RECEIVER_RETURNING_METHODS.contains(&method) || FRESH_VALUE_METHODS.contains(&method))
            && (method != "split" || separator_is_primitive)
            // ADR 0314: the receiver below is a proven built-in value.
            && !self.member_may_be_reassigned_on_builtin(method)
            // A species-built result follows the receiver's `constructor`.
            && (RECEIVER_RETURNING_METHODS.contains(&method)
                || !self.member_may_be_reassigned_on_builtin("constructor"))
            && matches!(
                self.value_origin_assuming(
                    file,
                    member.object,
                    solid_facts::ast::RuntimeValueKind::Unknown,
                    depth - 1,
                    assumed,
                ),
                Some(ValueOrigin::Builtin)
            )
        {
            return Some(ValueOrigin::Builtin);
        }
        let declaration =
            self.standard_library_declaration(file, call, typefacts::CallKind::Call)?;
        let name = declaration.qualified_name.as_ref();
        if FRESH_ARRAY_STATICS.contains(&name) {
            // `Array.from` called through a subclass (`List.from`) builds that
            // subclass, under the same declaration.
            let owner = name.split('.').next().unwrap_or_default();
            let global = owner.strip_suffix("Constructor").unwrap_or(owner);
            // The receiver must be that global, not a local of its name.
            return member
                .is_some_and(|member| {
                    let receiver = file.ast.peel_ts_sugar_span(member.object);
                    file.source_text(receiver) == Some(global)
                        && file.ast.reference_declaration(receiver).is_none()
                })
                .then_some(ValueOrigin::Builtin);
        }
        if name == "String.split" {
            return separator_is_primitive.then_some(ValueOrigin::Builtin);
        }
        None
    }

    /// ADR 0211, phase 2: the value an accessor call `items()` returns, when
    /// every value the accessor can hold is proven.
    ///
    /// - `const [items, setItems] = createSignal(initial)` declared in a
    ///   function: the initial value, and every value written through the
    ///   setter, whose every reference in its file is the callee of a call --
    ///   a setter handed anywhere else may write anything. An updater's
    ///   returns are proven with its previous value assumed.
    /// - `const sorted = createMemo(() => …)`: every value its compute, a
    ///   synchronous function literal, returns.
    fn accessor_origin(
        &self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
        depth: usize,
    ) -> Option<ValueOrigin<'a>> {
        use solid_dialect::Primitive;
        let callee = file.ast.peel_ts_sugar_span(call.callee);
        let (binding_file, binding, _) = self.binding_at_reference(file.path.as_str(), callee)?;
        if !binding.immutable {
            return None;
        }
        let initializer_span = binding_file
            .ast
            .peel_ts_sugar_span(binding.call_initializer?);
        let index = binding_file
            .ast
            .calls
            .iter()
            .position(|candidate| candidate.span == initializer_span)?;
        let initializer = &binding_file.ast.calls[index];
        let primitive = self.primitives(binding_file).calls[index]
            .as_ref()
            .and_then(crate::PrimitiveName::primitive)?;
        let builtin = |file: &FileFacts, argument: &solid_facts::ast::ArgumentFact, assumed| {
            !argument.spread
                && matches!(
                    self.value_origin_assuming(
                        file,
                        argument.span,
                        argument.runtime_value_kind,
                        depth - 1,
                        assumed,
                    ),
                    Some(ValueOrigin::Builtin)
                )
        };
        match (binding.shape, primitive) {
            (solid_facts::ast::BindingShape::Array, Primitive::CreateSignal) => {
                // The accessor is the tuple's first slot, read in its own file.
                let accessor = binding.array_slots.first()?.as_ref()?;
                if binding_file.path != file.path
                    || file.ast.reference_declaration(callee) != Some(accessor.span)
                    || !binding_file
                        .ast
                        .functions
                        .iter()
                        .any(|function| function.body.contains(binding.declaration))
                {
                    return None;
                }
                // `createSignal(fn)` is a writable memo; its value is `fn`'s.
                if let Some(initial) = initializer.arguments.first()
                    && (matches!(
                        initial.value,
                        solid_facts::ast::ArgumentValueKind::Function
                            | solid_facts::ast::ArgumentValueKind::AsyncFunction
                    ) || !builtin(binding_file, initial, None))
                {
                    return None;
                }
                if let Some(Some(setter)) = binding.array_slots.get(1)
                    && !self.setter_writes_builtin(binding_file, setter.span, depth)
                {
                    return None;
                }
                Some(ValueOrigin::Builtin)
            }
            (solid_facts::ast::BindingShape::Identifier, Primitive::CreateMemo) => {
                // Options can supply a value no compute returns
                // (`loadingValue`), so only the bare form is followed.
                let [compute] = initializer.arguments.as_slice() else {
                    return None;
                };
                let function =
                    crate::cleanup::callback_argument_literal(binding_file, compute.span)?;
                self.returns_builtin(binding_file, function, None, depth)
                    .then_some(ValueOrigin::Builtin)
            }
            _ => None,
        }
    }

    /// Whether every write through the setter declared at `setter` is proven
    /// a built-in value: each reference to it in its file is a call's callee,
    /// and each call writes nothing, a built-in value, or an updater whose
    /// returns are built-in given a built-in previous value.
    fn setter_writes_builtin(&self, file: &FileFacts, setter: Span, depth: usize) -> bool {
        file.ast
            .reference_declarations
            .iter()
            .filter(|(_, declaration)| *declaration == setter)
            .all(|(reference, _)| {
                let Some(call) = self
                    .call_by_callee(file, *reference)
                    .filter(|call| file.ast.peel_ts_sugar_span(call.callee) == *reference)
                else {
                    return false;
                };
                let Some(written) = call.arguments.first() else {
                    return true;
                };
                if written.spread {
                    return false;
                }
                if matches!(
                    written.value,
                    solid_facts::ast::ArgumentValueKind::Function
                        | solid_facts::ast::ArgumentValueKind::AsyncFunction
                ) {
                    let Some(updater) =
                        crate::cleanup::callback_argument_literal(file, written.span)
                    else {
                        return false;
                    };
                    let previous = updater
                        .parameters
                        .first()
                        // A default replaces `undefined` with a value nothing
                        // proves, so a defaulted previous value is not assumed.
                        .filter(|parameter| {
                            parameter.shape == solid_facts::ast::BindingShape::Identifier
                                && parameter.initializer.is_none()
                        })
                        .and_then(|parameter| parameter.names.first())
                        .map(|name| name.span)
                        .filter(|name| !binding_written(file, *name));
                    return self.returns_builtin(
                        file,
                        updater,
                        previous.map(|name| (file.path.as_str(), name)),
                        depth,
                    );
                }
                matches!(
                    self.value_origin(file, written.span, written.runtime_value_kind, depth - 1),
                    Some(ValueOrigin::Builtin)
                )
            })
    }

    /// Whether every value the synchronous `function` returns is proven a
    /// built-in value, with `assumed` (a parameter's name) taken as one.
    fn returns_builtin(
        &self,
        file: &FileFacts,
        function: &solid_facts::ast::FunctionFact,
        assumed: Option<(&str, Span)>,
        depth: usize,
    ) -> bool {
        if function.r#async || function.generator {
            return false;
        }
        let proven = |returned: &solid_facts::ast::ReturnFact| {
            returned.argument.is_none_or(|argument| {
                matches!(
                    self.value_origin_assuming(
                        file,
                        argument,
                        returned.runtime_value_kind,
                        depth - 1,
                        assumed,
                    ),
                    Some(ValueOrigin::Builtin)
                )
            })
        };
        if function.expression_body {
            return function.expression_return.as_ref().is_some_and(proven);
        }
        file.ast.own_returns(function).all(proven)
    }
}
