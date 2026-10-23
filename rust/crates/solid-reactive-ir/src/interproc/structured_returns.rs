//! Structured returns: per-function return summaries for objects, tuples, and nested results.

use super::*;

pub(super) struct StructuredReturnDiscovery<'a, 'facts> {
    pub(super) facts: &'a ProjectFacts,
    pub(super) nodes: &'a [SummaryNode],
    pub(super) indexes: &'a HashMap<(String, Span), usize>,
    pub(super) by_symbol: &'a HashMap<SymbolId, usize>,
    pub(super) summaries: &'a [SummaryReads],
    pub(super) returned: &'a [SummaryReads],
    pub(super) structured_returns: &'a [Option<ReturnSummary>],
    /// Deterministic work units, one per node visited in a
    /// discovery round, per `leaf`, `instantiate_return` and `projection`
    /// evaluation (wrapper, projection and nested propagation), and per exported node and
    /// returned property examined for an unresolved shorthand.
    pub(super) work: &'a AtomicU64,
    pub(super) accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    /// The accessor declared at each `(path, start, end)`: the first in the
    /// iteration order of `accessors` (see [`accessor_declarations`]).
    pub(super) accessor_declarations: &'a AccessorDeclarations<'a>,
    /// The accessor whose declaring entity has each non-empty runtime identity:
    /// the first in the iteration order of `accessors` (see
    /// [`accessors_by_runtime_identity`]). Built on the first lookup that
    /// needs it, so a project with no imported shorthand never pays for it.
    pub(super) accessors_by_identity: &'a std::sync::OnceLock<AccessorsByIdentity<'facts>>,
    /// Each file's exports by exported name (see [`ModuleExports`]), built on
    /// the first import that reaches a module's exports.
    pub(super) module_exports: &'a std::sync::OnceLock<ModuleExports<'facts>>,
    pub(super) source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
    pub(super) entities: &'a EntitySymbols,
    pub(super) symbol_names: &'a HashMap<SymbolId, SymbolId>,
    pub(super) lookup: &'a SemanticLookup<'facts>,
}

/// The accessors by declared location.
pub(super) type AccessorDeclarations<'a> = HashMap<(&'a str, u32, u32), &'a SymbolId>;

/// Indexes `accessors` by declaration. Several accessors may share one
/// location; the first in `accessors`' iteration order wins, as the scan this
/// replaces returned.
pub(super) fn accessor_declarations(
    accessors: &HashMap<SymbolId, (SymbolId, Location)>,
) -> AccessorDeclarations<'_> {
    let mut declarations = AccessorDeclarations::default();
    for (symbol, (_, location)) in accessors {
        if let (Ok(start), Ok(end)) = (
            u32::try_from(location.start_byte),
            u32::try_from(location.end_byte),
        ) {
            declarations
                .entry((location.path.as_ref(), start, end))
                .or_insert(symbol);
        }
    }
    declarations
}

/// The accessors by the runtime identity of their declaring entity.
pub(super) type AccessorsByIdentity<'a> = HashMap<&'a str, SymbolId>;

/// Indexes `accessors` by runtime identity, once per analysis. Several
/// accessors may share an identity (a re-export or alias declares the same
/// runtime value); the first in `accessors`' iteration order wins, as the
/// per-lookup scan this replaces returned. `identity_at` answers the runtime
/// identity of the entity at a location (`None`: no entity there). An empty
/// identity proves nothing and is never indexed.
pub(super) fn accessors_by_runtime_identity<'a>(
    accessors: &HashMap<SymbolId, (SymbolId, Location)>,
    identity_at: impl Fn(&Location) -> Option<&'a str>,
) -> AccessorsByIdentity<'a> {
    let mut by_identity = AccessorsByIdentity::default();
    for (symbol, (_, location)) in accessors {
        if let Some(identity) = identity_at(location).filter(|identity| !identity.is_empty()) {
            by_identity
                .entry(identity)
                .or_insert_with(|| symbol.clone());
        }
    }
    by_identity
}

/// The exports of every analyzed file, by exported name, so resolving one
/// export of a module does not scan all of that module's exports (a barrel
/// of N exports resolved N times was quadratic).
#[derive(Default)]
pub(super) struct ModuleExports<'f> {
    by_path: HashMap<&'f str, FileExports<'f>>,
}

#[derive(Default)]
struct FileExports<'f> {
    /// The local span of each own (module-less, value) export of a name, in
    /// source order; more than one is ambiguous.
    direct: HashMap<&'f str, Vec<Span>>,
    /// The value re-exports with a named specifier for a name (the first such
    /// specifier of each export), in source order.
    named: HashMap<&'f str, Vec<(&'f solid_facts::ast::ExportFact, Span)>>,
    /// The value `export * from` re-exports, in source order.
    star: Vec<&'f solid_facts::ast::ExportFact>,
    /// The name span of the first binding (in source order) of each symbol.
    declarations: HashMap<&'f SymbolId, Span>,
}

impl<'f> ModuleExports<'f> {
    pub(super) fn new(files: &'f [solid_facts::FileFacts], entities: &'f EntitySymbols) -> Self {
        let mut by_path = HashMap::default();
        for file in files {
            let mut exports = FileExports::default();
            for name in file.ast.bindings.iter().flat_map(|binding| &binding.names) {
                if let Some(symbol) = entities.at(file.path.as_str(), name.span) {
                    exports.declarations.entry(symbol).or_insert(name.span);
                }
            }
            for export in file.ast.exports.iter().filter(|export| !export.type_only) {
                if export.module.is_none() {
                    for specifier in export
                        .specifiers
                        .iter()
                        .chain(&export.declarations)
                        .filter(|specifier| !specifier.type_only)
                    {
                        exports
                            .direct
                            .entry(specifier.exported.as_str())
                            .or_default()
                            .push(specifier.local.span);
                    }
                    continue;
                }
                match export.kind {
                    solid_facts::ast::ExportKind::Named => {
                        let mut seen = HashSet::default();
                        for specifier in export.specifiers.iter().filter(|s| !s.type_only) {
                            if seen.insert(specifier.exported.as_str()) {
                                exports
                                    .named
                                    .entry(specifier.exported.as_str())
                                    .or_default()
                                    .push((export, specifier.local.span));
                            }
                        }
                    }
                    solid_facts::ast::ExportKind::All => exports.star.push(export),
                    _ => {}
                }
            }
            // A later file with the same path replaces an earlier one, as
            // `file_by_path` does.
            by_path.insert(file.path.as_str(), exports);
        }
        Self { by_path }
    }
}

/// The runtime identity of the entity declared at exactly `location`
/// (path, start and end): the first such entity, or `None` when there is none.
pub(super) fn entity_identity_at<'a>(
    entities: &HashMap<(&'a str, u64, u64), &'a typefacts::EntityFact>,
    location: &Location,
) -> Option<&'a str> {
    entities
        .get(&(
            location.path.as_ref(),
            location.start_byte,
            location.end_byte,
        ))
        .map(|entity| entity.runtime_identity.as_ref())
}

impl StructuredReturnDiscovery<'_, '_> {
    /// Why a shorthand return value cannot be resolved strongly enough to
    /// prove either a reactive leaf or an exact non-reactive project value.
    /// `None` means the binder/import graph closed the question.
    pub(super) fn unresolved_shorthand_reason(
        &self,
        file: &solid_facts::FileFacts,
        span: Span,
    ) -> Option<String> {
        let property = file
            .ast
            .object_properties
            .iter()
            .find(|property| property.value == span && property.key == property.value)?;
        let Some(declaration) = property.shorthand_binding else {
            return Some(
                "the shorthand resolves only through a global or unavailable binding".into(),
            );
        };
        let Some((module, binding)) = file.ast.imports.iter().find_map(|import| {
            import
                .bindings
                .iter()
                .find(|binding| binding.local.span == declaration)
                .map(|binding| (import.module.as_str(), binding))
        }) else {
            // A local declaration is exact. Whether it is reactive is answered
            // by the local accessor/source indexes, so absence there is a
            // proven non-reactive result rather than uncertainty.
            return None;
        };
        if binding.type_only || matches!(binding.kind, solid_facts::ast::ImportKind::Namespace) {
            return None;
        }
        if !matches!(
            binding.kind,
            solid_facts::ast::ImportKind::Named | solid_facts::ast::ImportKind::Default
        ) {
            return None;
        }
        // TypeScript already resolved the import, including tsconfig paths,
        // extension priority, and re-export cycles. Follow its exact symbol
        // chain and require a declaration in an analyzed project source file;
        // merely seeing the identity at a project re-export is insufficient,
        // because that export may forward an external package value. Local
        // accessor/source maps decide reactivity from there. External package
        // declarations therefore remain fail-closed even behind a relative
        // project re-export.
        if self.imported_binding_has_project_source_declaration(file, declaration) {
            return None;
        }
        Some(format!(
            "{module:?} resolves to no runtime declaration in the analyzed project"
        ))
    }

    fn parameter_return(
        &self,
        file: &solid_facts::FileFacts,
        function: &solid_facts::ast::FunctionFact,
        span: Span,
    ) -> Option<ReturnSummary> {
        if function.r#async
            || function.generator
            || file
                .ast
                .identifiers_within(function.span)
                .any(|identifier| {
                    matches!(
                        file.source_text(identifier.span),
                        Some("eval" | "arguments")
                    )
                })
        {
            return None;
        }
        let span = file.ast.peel_ts_sugar_span(span);
        if !file.ast.has_reference_identifier_at(span) {
            return None;
        }
        let symbol = self.entities.at(file.path.as_str(), span)?;
        if let Some(parameter) = function.parameters.iter().position(|parameter| {
            parameter
                .names
                .iter()
                .any(|name| self.entities.at(file.path.as_str(), name.span) == Some(symbol))
        }) {
            let binding = &function.parameters[parameter];
            if binding.initializer.is_some()
                || binding.names.len() != 1
                || binding.pattern != binding.names[0].span
                || function
                    .parameters
                    .iter()
                    .filter(|other| {
                        other.names.iter().any(|name| {
                            file.source_text(name.span) == file.source_text(binding.names[0].span)
                        })
                    })
                    .count()
                    != 1
                || file.ast.identifiers.iter().any(|identifier| {
                    self.entities.at(file.path.as_str(), identifier.span) == Some(symbol)
                        && file
                            .ast
                            .assignments
                            .iter()
                            .any(|assignment| assignment.target.contains(identifier.span))
                })
            {
                return None;
            }
            return Some(ReturnSummary {
                kind: "argument".into(),
                parameter: Some(parameter),
                ..ReturnSummary::default()
            });
        }
        None
    }

    fn instantiate_return(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        returned: &ReturnSummary,
        fallback_label: &str,
        depth: usize,
    ) -> Option<ReturnSummary> {
        self.work.fetch_add(1, Ordering::Relaxed);
        if depth == 0 {
            return None;
        }
        match returned.kind.as_str() {
            "argument" => call
                .arguments
                .get(returned.parameter?)
                .and_then(|argument| {
                    self.leaf_with_depth(file, argument.span, fallback_label, depth - 1)
                }),
            "callback-result" => call
                .arguments
                .get(returned.parameter?)
                .and_then(|argument| {
                    let initializer = self
                        .binding_initializer(file, argument.span)
                        .unwrap_or(argument.span);
                    let function = file.ast.function_at(initializer)?;
                    let index = self.indexes.get(&(file.path.to_string(), function.span))?;
                    let resolved = self.structured_returns[*index].as_ref()?;
                    (!matches!(
                        resolved.kind.as_str(),
                        "argument" | "callback-result" | "callback-result-function"
                    ))
                    .then(|| resolved.clone())
                }),
            "tuple" => Some(ReturnSummary {
                elements: returned
                    .elements
                    .iter()
                    .map(|element| {
                        element.as_ref().and_then(|element| {
                            self.instantiate_return(file, call, element, fallback_label, depth - 1)
                        })
                    })
                    .collect(),
                ..returned.clone()
            }),
            "object" => Some(ReturnSummary {
                properties: returned
                    .properties
                    .iter()
                    .filter_map(|(name, property)| {
                        self.instantiate_return(file, call, property, name, depth - 1)
                            .map(|property| (name.clone(), property))
                    })
                    .collect(),
                ..returned.clone()
            }),
            _ => Some(returned.clone()),
        }
    }

    fn context_return(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        fallback_label: &str,
        depth: usize,
    ) -> Option<ReturnSummary> {
        if depth == 0 {
            return None;
        }
        let context = call
            .arguments
            .first()
            .and_then(|argument| self.entities.at(file.path.as_str(), argument.span))?;
        self.facts.files.iter().find_map(|provider_file| {
            let jsx_value = provider_file.ast.jsx_elements.iter().find_map(|element| {
                let provider_span =
                    if let Some(provider_member) = self.lookup.dialect.context_provider_member() {
                        (element
                            .member_property
                            .and_then(|property| provider_file.source_text(property))
                            == Some(provider_member))
                        .then_some(element.member_object)
                        .flatten()
                    } else {
                        Some(element.name.span)
                    }?;
                (self.entities.at(provider_file.path.as_str(), provider_span) == Some(context))
                    .then_some(())?;
                let value = element.attributes.iter().find_map(|attribute| {
                    (provider_file.source_text(attribute.local_name) == Some("value"))
                        .then_some(attribute.expression)
                        .flatten()
                })?;
                self.leaf_with_depth(provider_file, value, fallback_label, depth - 1)
            });
            jsx_value.or_else(|| {
                provider_file.ast.calls.iter().find_map(|provider| {
                    let component = provider.arguments.first()?.span;
                    let properties = provider.arguments.get(1)?.span;
                    let provider_context = if let Some(provider_member) =
                        self.lookup.dialect.context_provider_member()
                    {
                        let member = provider_file.ast.member_at(component)?;
                        (provider_file.source_text(member.property) == Some(provider_member))
                            .then_some(member.object)?
                    } else {
                        component
                    };
                    (self
                        .entities
                        .at(provider_file.path.as_str(), provider_context)
                        == Some(context))
                    .then_some(())?;
                    let value =
                        provider_file
                            .ast
                            .object_properties
                            .iter()
                            .find_map(|property| {
                                (properties.contains(property.span)
                                    && provider_file.source_text(property.key) == Some("value"))
                                .then_some(property.value)
                            })?;
                    self.leaf_with_depth(provider_file, value, fallback_label, depth - 1)
                })
            })
        })
    }

    fn reactive_proxy_return(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        fallback_label: &str,
    ) -> Option<ReturnSummary> {
        let resolved = self.lookup.resolved_callee_call(file, call.callee)?;
        let declaration = resolved.declaration.as_ref()?;
        if resolved.kind != CallKind::Construct
            || !declaration.standard_library
            || declaration.qualified_name.as_ref() != "ProxyConstructor.construct"
        {
            return None;
        }
        let handler = call.arguments.get(1)?.span;
        let reactive = file.ast.calls.iter().any(|nested| {
            handler.contains(nested.span)
                && crate::known_primitive(&crate::call_primitive_name_cached(
                    self.lookup,
                    file,
                    nested,
                    self.entities,
                    self.symbol_names,
                    self.lookup.dialect,
                ))
                .is_some_and(|primitive| self.lookup.dialect.creates_reactive_source(primitive))
        });
        reactive.then(|| ReturnSummary {
            kind: "store-path".into(),
            label: fallback_label.into(),
            ..ReturnSummary::default()
        })
    }

    fn callable_value_return(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        span: Span,
        fallback_label: &str,
        depth: usize,
    ) -> Option<ReturnSummary> {
        if depth == 0 {
            return None;
        }
        if let Some(conditional) = file.ast.conditional_at(span) {
            let consequent = self.callable_value_return(
                file,
                call,
                conditional.consequent,
                fallback_label,
                depth - 1,
            );
            let alternate = self.callable_value_return(
                file,
                call,
                conditional.alternate,
                fallback_label,
                depth - 1,
            );
            return match (consequent, alternate) {
                (Some(left), Some(right)) if left.kind == right.kind => {
                    let mut merged = Some(left);
                    merge_structured_return(&mut merged, right);
                    merged
                }
                (Some(returned), None) | (None, Some(returned)) => Some(returned),
                _ => None,
            };
        }
        let function = file.ast.function_at(span)?;
        let index = self.indexes.get(&(file.path.to_string(), function.span))?;
        self.structured_returns[*index]
            .as_ref()
            .and_then(|returned| {
                self.instantiate_return(file, call, returned, fallback_label, depth - 1)
            })
    }

    fn call_return(
        &self,
        file: &solid_facts::FileFacts,
        call: &solid_facts::ast::CallFact,
        fallback_label: &str,
        depth: usize,
    ) -> Option<ReturnSummary> {
        if depth == 0 {
            return None;
        }
        let symbol = self.entities.at(file.path.as_str(), call.callee);
        let primitive = crate::call_primitive_name_cached(
            self.lookup,
            file,
            call,
            self.entities,
            self.symbol_names,
            self.lookup.dialect,
        );
        if crate::known_primitive(&primitive) == Some(Primitive::UseContext)
            && let Some(returned) = self.context_return(file, call, fallback_label, depth - 1)
        {
            return Some(returned);
        }
        if let Some(primitive) = crate::known_primitive(&primitive)
            && self.lookup.dialect.creates_reactive_source(primitive)
        {
            return Some(ReturnSummary {
                kind: if self.lookup.dialect.returns_store(primitive) {
                    "store-path".into()
                } else {
                    "accessor".into()
                },
                label: fallback_label.into(),
                ..ReturnSummary::default()
            });
        }
        if let Some(returned) = self.reactive_proxy_return(file, call, fallback_label) {
            return Some(returned);
        }
        if file.ast.members.iter().any(|member| {
            member.span == call.callee
                && file
                    .ast
                    .calls
                    .iter()
                    .any(|receiver| receiver.span == member.object)
        }) {
            return None;
        }
        if let Some(initializer) = self.binding_initializer(file, call.callee)
            && let Some(returned) =
                self.callable_value_return(file, call, initializer, fallback_label, depth - 1)
        {
            return Some(returned);
        }
        let index = symbol.and_then(|symbol| self.by_symbol.get(symbol))?;
        if let Some(returned) = self.structured_returns[*index].as_ref() {
            return self.instantiate_return(file, call, returned, fallback_label, depth - 1);
        }
        self.returned[*index]
            .iter()
            .find(|read| !read.undecided)
            .map(|read| self.return_summary_from_read(read, fallback_label))
    }

    /// The initializer a reference is bound to *in its entirety*.
    ///
    /// A destructuring pattern is deliberately excluded. `const [value] =
    /// createSpring(...)` binds `value` to the first *item* of the call's
    /// result, never to the result, so answering with the initializer hands a
    /// consumer the tuple where the item was asked for -- which is how
    /// `createDerivedSpring`'s `return value` came to claim
    /// `createSpring`'s whole `[Accessor<T>, SpringSetter<T>]`. Deriving the
    /// slot's own shape needs the element index, and `array_slots` cannot
    /// supply it exactly: a nested pattern, a defaulted element, or an object
    /// pattern element all report their *first identifier* there, which names
    /// a value inside the item rather than the item. So this answers nothing
    /// for a destructured name rather than guessing, and the exact slot stays
    /// available through an element access with a literal index, which
    /// [`Self::projection`] reads from the tuple.
    fn binding_initializer(&self, file: &solid_facts::FileFacts, span: Span) -> Option<Span> {
        self.bound_initializer(file, span)
            .filter(|bound| bound.whole)
            .map(|bound| bound.initializer)
    }

    /// The binding a reference resolves to: the initializer it was declared
    /// with, the compiler symbol of the pattern name this reference actually
    /// names, and whether that name is bound to the whole initializer.
    fn bound_initializer(
        &self,
        file: &solid_facts::FileFacts,
        span: Span,
    ) -> Option<BoundInitializer> {
        if let Some((binding_file, binding, symbol)) =
            self.lookup.binding_at_reference(file.path.as_str(), span)
            && binding_file.path == file.path
            && let Some(initializer) = binding.initializer
        {
            return Some(BoundInitializer {
                initializer,
                symbol: Some(symbol),
                whole: binding.shape == solid_facts::ast::BindingShape::Identifier,
            });
        }
        if let Some(symbol) = self.entities.at(file.path.as_str(), span)
            // One symbol can be declared by several bindings -- `var pair;`
            // then `var pair = createPair(x)` are two `BindingFact`s for the
            // same name, and only the second carries the value. The scan
            // therefore keeps looking until it finds the matching name *with*
            // an initializer instead of committing to the first name match and
            // then failing the initializer test.
            && let Some((bound, initializer)) = self
                .lookup
                .initialized_binding_of_symbol(file.path.as_str(), symbol)
                .and_then(|binding| binding.initializer.map(|initializer| (binding, initializer)))
        {
            return Some(BoundInitializer {
                initializer,
                symbol: Some(symbol.clone()),
                whole: bound.shape == solid_facts::ast::BindingShape::Identifier,
            });
        }
        // TypeScript exposes a shorthand property's own property symbol at
        // `{ value }`, not the referenced value symbol, so neither lookup
        // above names the value. The binder that built these AST facts did
        // resolve that exact reference, so its declaration is the evidence
        // here -- exact, and block-scope aware.
        let declaration = self.shorthand_value_declaration(file, span)?;
        let bound = self
            .lookup
            .binding_named_at(file.path.as_str(), declaration)?;
        Some(BoundInitializer {
            initializer: bound.initializer?,
            symbol: self.entities.at(file.path.as_str(), declaration).cloned(),
            whole: bound.shape == solid_facts::ast::BindingShape::Identifier,
        })
    }

    /// The declaration a shorthand property's value refers to, when `span` is
    /// that shorthand.
    ///
    /// `None` for every other span, and for a shorthand the binder resolved
    /// to no declaration in this file -- a global, or an import namespace
    /// member. Callers treat that as a missing fact, not as proof.
    fn shorthand_value_declaration(
        &self,
        file: &solid_facts::FileFacts,
        span: Span,
    ) -> Option<Span> {
        file.ast
            .object_property_with_value(span)
            .and_then(|property| property.shorthand_binding)
    }

    fn projection(
        &self,
        file: &solid_facts::FileFacts,
        span: Span,
        fallback_label: &str,
        depth: usize,
    ) -> Option<ReturnSummary> {
        self.work.fetch_add(1, Ordering::Relaxed);
        if depth == 0 {
            return None;
        }
        let member = file.ast.member_at(span)?;
        let base = if let Some(call) = file
            .ast
            .calls
            .iter()
            .find(|call| call.span == member.object)
        {
            self.call_return(file, call, fallback_label, depth - 1)
        } else if file
            .ast
            .members
            .iter()
            .any(|candidate| candidate.span == member.object)
        {
            self.projection(file, member.object, fallback_label, depth - 1)
        } else if let Some(initializer) = self.binding_initializer(file, member.object) {
            self.leaf_with_depth(file, initializer, fallback_label, depth - 1)
        } else {
            self.entities
                .at(file.path.as_str(), member.object)
                .filter(|symbol| self.source_kinds.get(*symbol) == Some(&ReactiveSourceKind::Store))
                .map(|_| ReturnSummary {
                    kind: "store-path".into(),
                    label: fallback_label.into(),
                    ..ReturnSummary::default()
                })
        }?;
        if matches!(base.kind.as_str(), "tuple" | "object")
            && let Some(symbol) = self.entities.at(
                file.path.as_str(),
                file.ast.peel_ts_sugar_span(member.object),
            )
            && !crate::value_identity::structural_binding_is_stable(
                file,
                self.entities,
                symbol,
                &base,
                self.lookup.tuple_iteration_may_be_mutated(),
            )
        {
            return None;
        }
        let property = file.source_text(member.property).unwrap_or_default();
        // A computed member's property span is an *expression*, not a name, so
        // its source text is not the property it reads: `createRecord(x)[key]`
        // with `key: "value" | "other"` spells `key` and would match no
        // property, but `createRecord(x)["value"]` spells `"value"` with the
        // quotes and a same-spelled property would be a coincidence rather
        // than a resolution. Only a static name resolves a property here. The
        // tuple arm below is already safe: `parse::<usize>()` rejects every
        // spelling that is not a literal index.
        let computed = file
            .ast
            .computed_members
            .binary_search(&member.span)
            .is_ok();
        match base.kind.as_str() {
            "object" if !computed => base.properties.get(property).cloned(),
            "tuple" if !self.lookup.tuple_iteration_may_be_mutated() => property
                .parse::<usize>()
                .ok()
                .and_then(|index| base.elements.get(index))
                .and_then(Clone::clone),
            "store-path" => Some(ReturnSummary {
                kind: "store-path".into(),
                label: fallback_label.into(),
                ..ReturnSummary::default()
            }),
            _ => None,
        }
    }

    /// The discovered accessor a shorthand property's value names.
    ///
    /// TypeScript projects a shorthand property's *own* symbol at
    /// `{ pathname }`, never the value binding's, so the entity table cannot
    /// answer here. The binder's resolution of that reference can, and it
    /// identifies the declaration exactly: a same-spelled binding in a
    /// sibling block declares a different symbol at a different span and
    /// cannot be mistaken for this one.
    fn named_accessor(&self, file: &solid_facts::FileFacts, span: Span) -> Option<&SymbolId> {
        let declaration = self.shorthand_value_declaration(file, span)?;
        self.accessor_declarations
            .get(&(file.path.as_str(), declaration.start, declaration.end))
            .copied()
            .or_else(|| self.imported_accessor(file, declaration))
    }

    /// The accessor a named import specifier re-exports, when `declaration`
    /// is that specifier's local name span.
    ///
    /// The binder resolves an imported shorthand's value to the import
    /// specifier in this file, where no accessor is declared, and the
    /// TypeScript entity at that span is the alias symbol rather than the
    /// original declaration's. For a plain relative specifier the exporting
    /// file is part of the analyzed project, so the join is exact ESM
    /// resolution against the project's own file set: direct exports,
    /// named/default re-exports, and export-all chains are followed with a
    /// cycle guard, then matched in the accessor map exactly as the
    /// same-file arm matches. Bare or path-mapped specifiers, namespace
    /// imports, ambiguous modules, and unresolved cycles prove nothing and
    /// stay fail-closed.
    fn imported_accessor(
        &self,
        file: &solid_facts::FileFacts,
        declaration: Span,
    ) -> Option<&SymbolId> {
        if let Some(accessor) = self.accessor_with_same_runtime_identity(file, declaration) {
            return Some(accessor);
        }
        let (module, imported) = file.ast.imports.iter().find_map(|import| {
            if import.type_only {
                return None;
            }
            import.bindings.iter().find_map(|binding| {
                (binding.local.span == declaration
                    && matches!(
                        binding.kind,
                        solid_facts::ast::ImportKind::Named | solid_facts::ast::ImportKind::Default
                    )
                    && !binding.type_only)
                    .then(|| {
                        (
                            import.module.as_str(),
                            binding.imported.as_deref().unwrap_or("default"),
                        )
                    })
            })
        })?;
        let sibling = self.relative_module_file(file, module)?;
        let mut visiting = HashSet::default();
        let (target_path, target) =
            self.resolve_export_binding(sibling.path.as_str(), imported, &mut visiting)?;
        self.accessor_declarations
            .get(&(target_path.as_str(), target.start, target.end))
            .copied()
    }

    fn accessor_with_same_runtime_identity(
        &self,
        file: &solid_facts::FileFacts,
        declaration: Span,
    ) -> Option<&SymbolId> {
        let identity = self
            .lookup
            .entity_at(file.path.as_str(), declaration)?
            .runtime_identity
            .as_ref();
        if identity.is_empty() {
            return None;
        }
        self.accessors_by_identity
            .get_or_init(|| {
                accessors_by_runtime_identity(self.accessors, |location| {
                    entity_identity_at(self.lookup.entities_by_location(), location)
                })
            })
            .get(identity)
    }

    fn imported_binding_has_project_source_declaration(
        &self,
        file: &solid_facts::FileFacts,
        declaration: Span,
    ) -> bool {
        let Some(entity) = self.lookup.entity_at(file.path.as_str(), declaration) else {
            return false;
        };
        if entity.runtime_identity.is_empty() || entity.symbol.is_empty() {
            return false;
        }
        let mut symbol_id = entity.symbol.as_ref();
        let mut seen = HashSet::default();
        while seen.insert(symbol_id) {
            let Some(symbol) = self.facts.typescript.symbol(symbol_id) else {
                return false;
            };
            if symbol.declarations().iter().any(|declaration| {
                !declaration.location.path.ends_with(".d.ts")
                    && self
                        .lookup
                        .file_by_path(declaration.location.path.as_ref())
                        .is_some_and(|target_file| {
                            let is_binding = target_file.ast.identifiers.iter().any(|identifier| {
                                identifier.role == solid_facts::ast::IdentifierRole::Binding
                                    && u64::from(identifier.span.start)
                                        == declaration.location.start_byte
                                    && u64::from(identifier.span.end)
                                        == declaration.location.end_byte
                            });
                            let is_import = target_file.ast.imports.iter().any(|import| {
                                import.bindings.iter().any(|binding| {
                                    u64::from(binding.local.span.start)
                                        == declaration.location.start_byte
                                        && u64::from(binding.local.span.end)
                                            == declaration.location.end_byte
                                })
                            });
                            is_binding && !is_import
                        })
            }) {
                return true;
            }
            let alias = symbol.alias_target();
            if alias.is_empty() {
                return false;
            }
            symbol_id = alias;
        }
        false
    }

    /// The project file a plain relative import specifier resolves to, by
    /// textual normalization against the analyzed file set — never the
    /// filesystem, so a file outside the project cannot resolve. `None` for
    /// bare specifiers and any path that walks above the root.
    ///
    /// Extensionless specifiers are ambiguous on their own: `./values` can
    /// name `values.ts`, `values.tsx`, or `values/index.ts`, and which one a
    /// bundler picks depends on resolution settings this pass does not model.
    /// The suffixes are tried in the usual priority order, but when more than
    /// one distinct project file matches the specifier the answer is **not**
    /// the first one enumerated — file order is not evidence — and the
    /// resolution fails closed with `None`. A summary built from the wrong
    /// module is a wrong proven claim; no claim is the correct one.
    fn relative_module_file(
        &self,
        from: &solid_facts::FileFacts,
        module: &str,
    ) -> Option<&solid_facts::FileFacts> {
        let path = self
            .lookup
            .relative_module_path(from.path.as_str(), module)?;
        self.lookup.file_by_path(path)
    }

    fn module_exports(&self) -> &ModuleExports<'_> {
        self.module_exports
            .get_or_init(|| ModuleExports::new(self.lookup.files(), self.lookup.entities()))
    }

    fn resolve_export_binding(
        &self,
        path: &str,
        exported: &str,
        visiting: &mut HashSet<(String, String)>,
    ) -> Option<(String, Span)> {
        let key = (path.to_owned(), exported.to_owned());
        if !visiting.insert(key.clone()) {
            return None;
        }
        let result = (|| {
            let file = self.lookup.file_by_path(path)?;
            let exports = self.module_exports().by_path.get(file.path.as_str())?;
            let direct = exports.direct.get(exported).map_or(&[][..], Vec::as_slice);
            if direct.len() > 1 {
                return None;
            }
            if let Some(local) = direct.first().copied() {
                return self.resolve_export_local(file, local, visiting);
            }

            // Every re-export that publishes `exported`. The outcome does not
            // depend on their order: an unresolvable module or two different
            // bindings fail the whole resolution wherever they fall.
            let named = exports.named.get(exported).map_or(&[][..], Vec::as_slice);
            let star = if exported == "default" {
                &[][..]
            } else {
                exports.star.as_slice()
            };
            let mut result = None;
            for (export, source_name) in named
                .iter()
                .map(|(export, local)| {
                    (
                        *export,
                        file.source_text(*local).map(|name| {
                            name.trim_matches(|character| character == '"' || character == '\'')
                        }),
                    )
                })
                .chain(star.iter().map(|export| (*export, Some(exported))))
            {
                let Some(source_name) = source_name else {
                    continue;
                };
                let module = export.module.as_deref()?;
                let sibling = self.relative_module_file(file, module)?;
                let candidate =
                    self.resolve_export_binding(sibling.path.as_str(), source_name, visiting);
                let Some(candidate) = candidate else {
                    continue;
                };
                if let Some(existing) = &result
                    && existing != &candidate
                {
                    return None;
                }
                result = Some(candidate);
            }
            result
        })();
        visiting.remove(&key);
        result
    }

    fn resolve_export_local(
        &self,
        file: &solid_facts::FileFacts,
        local: Span,
        visiting: &mut HashSet<(String, String)>,
    ) -> Option<(String, Span)> {
        let symbol = self.lookup.entities().at(file.path.as_str(), local);
        if let Some((import, binding)) = file
            .ast
            .imports
            .iter()
            .flat_map(|import| import.bindings.iter().map(move |binding| (import, binding)))
            .find(|(_, binding)| {
                !binding.type_only
                    && matches!(
                        binding.kind,
                        solid_facts::ast::ImportKind::Named | solid_facts::ast::ImportKind::Default
                    )
                    && (binding.local.span == local
                        || symbol.is_some_and(|symbol| {
                            self.lookup
                                .entities()
                                .at(file.path.as_str(), binding.local.span)
                                == Some(symbol)
                        }))
            })
        {
            let sibling = self.relative_module_file(file, import.module.as_str())?;
            let imported = binding.imported.as_deref().unwrap_or("default");
            return self.resolve_export_binding(sibling.path.as_str(), imported, visiting);
        }
        let declaration = symbol.and_then(|symbol| {
            self.module_exports()
                .by_path
                .get(file.path.as_str())?
                .declarations
                .get(symbol)
                .copied()
        });
        Some((file.path.to_string(), declaration.unwrap_or(local)))
    }

    fn leaf(
        &self,
        file: &solid_facts::FileFacts,
        span: Span,
        fallback_label: &str,
    ) -> Option<ReturnSummary> {
        self.leaf_with_depth(file, span, fallback_label, 16)
    }

    fn leaf_with_depth(
        &self,
        file: &solid_facts::FileFacts,
        span: Span,
        fallback_label: &str,
        depth: usize,
    ) -> Option<ReturnSummary> {
        self.work.fetch_add(1, Ordering::Relaxed);
        if depth == 0 {
            return None;
        }
        if let Some(conditional) = file.ast.conditional_at(span) {
            let consequent =
                self.leaf_with_depth(file, conditional.consequent, fallback_label, depth - 1);
            let alternate =
                self.leaf_with_depth(file, conditional.alternate, fallback_label, depth - 1);
            return match (consequent, alternate) {
                (Some(left), Some(right)) if left.kind == right.kind => {
                    let mut merged = Some(left);
                    merge_structured_return(&mut merged, right);
                    merged
                }
                (Some(returned), None) | (None, Some(returned)) => Some(returned),
                _ => None,
            };
        }
        if let Some(logical) = file.ast.logical_at(span) {
            let left = self.leaf_with_depth(file, logical.left, fallback_label, depth - 1);
            let right = self.leaf_with_depth(file, logical.right, fallback_label, depth - 1);
            return match (left, right) {
                (Some(left), Some(right)) if left.kind == right.kind => {
                    let mut merged = Some(left);
                    merge_structured_return(&mut merged, right);
                    merged
                }
                (Some(returned), None) | (None, Some(returned)) => Some(returned),
                _ => None,
            };
        }
        let source = file.source_text(span).unwrap_or_default().trim();
        if source.starts_with('{') && source.ends_with('}') {
            let properties = file
                .ast
                .object_properties
                .iter()
                .filter(|property| {
                    span.contains(property.span)
                        && !file.ast.object_properties.iter().any(|parent| {
                            parent.span != property.span
                                && span.contains(parent.span)
                                && parent.span.contains(property.span)
                        })
                })
                .filter_map(|property| {
                    let name = file.source_text(property.key)?.trim_matches(['\'', '"']);
                    self.leaf_with_depth(file, property.value, name, depth - 1)
                        .map(|returned| (name.to_owned(), returned))
                })
                .collect::<BTreeMap<_, _>>();
            if !properties.is_empty() {
                return Some(ReturnSummary {
                    kind: "object".into(),
                    properties,
                    ..ReturnSummary::default()
                });
            }
        }
        if file.ast.members.iter().any(|member| member.span == span) {
            return self.projection(file, span, fallback_label, depth - 1);
        }
        if let Some(function) = file
            .ast
            .functions_within(span)
            .filter(|function| function.span == span)
            .min_by_key(|function| function.span.end - function.span.start)
        {
            let summarized = self
                .indexes
                .get(&(file.path.to_string(), function.span))
                .is_some_and(|index| self.summaries[*index].iter().any(|read| !read.undecided));
            let calls_accessor = file.ast.calls.iter().any(|call| {
                function.body.contains(call.span)
                    && self
                        .entities
                        .at(file.path.as_str(), call.callee)
                        .or_else(|| self.named_accessor(file, call.callee))
                        .is_some_and(|symbol| self.accessors.contains_key(symbol))
            });
            if summarized || calls_accessor {
                return Some(ReturnSummary {
                    kind: "accessor".into(),
                    label: fallback_label.into(),
                    ..ReturnSummary::default()
                });
            }
        }
        // `createSpring(x)[0] as Accessor<T>` is an element access wearing a
        // transparent TypeScript wrapper. The widened lookup below matches a
        // *call* by its start byte, so it would answer with `createSpring`'s
        // whole tuple and discard the `[0]`. Peel the sugar first, which puts
        // the element access back where [`Self::projection`] can read the item
        // the index names. Only syntax that preserves the runtime value is
        // peeled, and every earlier branch has already declined this span.
        let peeled = file.ast.peel_ts_sugar_span(span);
        if peeled != span {
            // A member access under a wrapper is *committed* to, exactly as
            // the unwrapped exact-member branch above returns. Falling through
            // on `None` would hand the widened call lookup the original span,
            // whose start byte still matches the base call, and publish the
            // base's whole shape for a projection that named nothing --
            // `createPair(x)[at] as Accessor<T>` would answer with the tuple
            // and `createRecord(x).other as number` with the whole object.
            if file.ast.members.iter().any(|member| member.span == peeled) {
                return self.projection(file, peeled, fallback_label, depth - 1);
            }
            if let Some(returned) = self.leaf_with_depth(file, peeled, fallback_label, depth - 1) {
                return Some(returned);
            }
        }
        if let Some(call) = file
            .ast
            .calls_within(span)
            .filter(|call| call.span.start == span.start)
            .max_by_key(|call| call.span.end - call.span.start)
            && let Some(returned) = self.call_return(file, call, fallback_label, depth - 1)
        {
            return Some(returned);
        }
        // The entity at a shorthand span is the property's (or an import
        // alias's) symbol, which no source map knows — only when TypeScript's
        // answer misses every map may the binder's shorthand resolution
        // speak, so the entity lookup is filtered to known symbols rather
        // than short-circuiting the join.
        if let Some(symbol) = self
            .entities
            .at(file.path.as_str(), span)
            .filter(|symbol| {
                self.accessors.contains_key(*symbol) || self.by_symbol.contains_key(*symbol)
            })
            .or_else(|| self.named_accessor(file, span))
        {
            if self.accessors.contains_key(symbol) {
                return Some(ReturnSummary {
                    kind: if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
                        "store-path".into()
                    } else {
                        "accessor".into()
                    },
                    label: fallback_label.into(),
                    ..ReturnSummary::default()
                });
            }
            if let Some(index) = self.by_symbol.get(symbol)
                && self.summaries[*index].iter().any(|read| !read.undecided)
            {
                return Some(ReturnSummary {
                    kind: "accessor".into(),
                    label: fallback_label.into(),
                    ..ReturnSummary::default()
                });
            }
        }
        if let Some(bound) = self.bound_initializer(file, span) {
            // A shorthand property (`{ pathname }`) carries the property's own
            // symbol at this span, not the value binding's, so the lookup above
            // cannot see that the value is a discovered source. The binding
            // this reference resolved to can: ask for the symbol of the
            // pattern name it named. Without this the leaf falls through to
            // the initializing call and inherits the primitive's generic label
            // ("memo result") instead of the structural position the consumer
            // actually reads.
            if let Some(symbol) = bound
                .symbol
                .as_ref()
                .filter(|symbol| self.accessors.contains_key(*symbol))
            {
                return Some(ReturnSummary {
                    kind: if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
                        "store-path".into()
                    } else {
                        "accessor".into()
                    },
                    label: fallback_label.into(),
                    ..ReturnSummary::default()
                });
            }
            // Only a name bound to the *whole* initializer carries the
            // initializer's shape -- see [`Self::binding_initializer`].
            if bound.whole
                && bound.initializer != span
                && let Some(returned) =
                    self.leaf_with_depth(file, bound.initializer, fallback_label, depth - 1)
            {
                return Some(returned);
            }
        }
        if let Some(member) = file.ast.member_at(span)
            && let Some(symbol) = self.entities.at(file.path.as_str(), member.object)
            && self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store)
        {
            return Some(ReturnSummary {
                kind: "store-path".into(),
                label: fallback_label.into(),
                ..ReturnSummary::default()
            });
        }
        None
    }

    fn return_summary_from_read(&self, read: &SummaryRead, fallback_label: &str) -> ReturnSummary {
        ReturnSummary {
            kind: if self.source_kinds.get(&read.symbol) == Some(&ReactiveSourceKind::Store) {
                "store-path".into()
            } else {
                "accessor".into()
            },
            label: fallback_label.to_owned(),
            ..ReturnSummary::default()
        }
    }
}

pub(super) fn discover_structured_returns(
    discovery: &StructuredReturnDiscovery<'_, '_>,
) -> Vec<Option<ReturnSummary>> {
    parallel_slice_results(discovery.nodes, |node| {
        discovery.work.fetch_add(1, Ordering::Relaxed);
        let file = discovery.lookup.file_by_path(node.path.as_str())?;
        let function = file.ast.function_at(node.span)?;
        let returned = function
            .expression_return
            .iter()
            .chain(file.ast.returns_within(function.body).filter(|returned| {
                containing_ast_function(&file.ast, returned.span)
                    .is_some_and(|owner| owner.span == function.span)
            }))
            .collect::<Vec<_>>();
        let resolve = |returned: &&solid_facts::ast::ReturnFact| {
            if !returned.elements().is_empty() {
                // A missing element is a hole or a spread. After a spread the
                // syntactic position is not the runtime index, so no later
                // element is placed.
                let placed = returned
                    .elements()
                    .iter()
                    .position(Option::is_none)
                    .unwrap_or(returned.elements().len());
                let elements = returned
                    .elements()
                    .iter()
                    .enumerate()
                    .map(|(index, element)| {
                        element.filter(|_| index < placed).and_then(|span| {
                            discovery.leaf(file, span, &format!("result[{index}]"))
                        })
                    })
                    .collect::<Vec<_>>();
                if elements.iter().any(Option::is_some) {
                    return Some(ReturnSummary {
                        kind: "tuple".into(),
                        elements,
                        ..ReturnSummary::default()
                    });
                }
            }
            if !returned.properties().is_empty() {
                let properties = returned
                    .properties()
                    .iter()
                    .filter_map(|property| {
                        discovery
                            .leaf(file, property.value, property.name.as_str())
                            .map(|returned| (property.name.to_string(), returned))
                    })
                    .collect::<BTreeMap<_, _>>();
                if !properties.is_empty() {
                    return Some(ReturnSummary {
                        kind: "object".into(),
                        properties,
                        ..ReturnSummary::default()
                    });
                }
            }
            let value = returned.argument?;
            if let Some(argument) = discovery.parameter_return(file, function, value) {
                return Some(argument);
            }
            discovery.leaf(file, value, "result")
        };
        let first = returned.iter().find_map(&resolve)?;
        if matches!(
            first.kind.as_str(),
            "argument" | "callback-result" | "callback-result-function"
        ) {
            // A relation is universal over the exported call, not evidence that
            // one branch once returned a parameter. Every explicit return path
            // must prove the same relation, including its parameter index.
            // Otherwise predicates (`isObject`), comparators and fallback
            // helpers become false identity returns merely because one
            // branch contains `return value`.
            returned
                .iter()
                .all(|candidate| {
                    candidate.control_tests.is_empty()
                        && !candidate.conditional
                        && resolve(candidate).as_ref() == Some(&first)
                })
                .then_some(first)
        } else {
            Some(first)
        }
    })
}

pub(super) fn merge_structured_return(
    target: &mut Option<ReturnSummary>,
    incoming: ReturnSummary,
) -> bool {
    let Some(current) = target.as_mut() else {
        *target = Some(incoming);
        return true;
    };
    if current.kind != incoming.kind {
        return false;
    }
    match current.kind.as_str() {
        "tuple" => {
            if current.elements.len() < incoming.elements.len() {
                current.elements.resize(incoming.elements.len(), None);
            }
            incoming
                .elements
                .into_iter()
                .enumerate()
                .fold(false, |changed, (index, returned)| {
                    changed
                        | returned.is_some_and(|returned| {
                            merge_structured_return(&mut current.elements[index], returned)
                        })
                })
        }
        "object" => incoming
            .properties
            .into_iter()
            .fold(false, |changed, (property, returned)| {
                if let Some(existing) = current.properties.get_mut(&property) {
                    let mut slot = Some(existing.clone());
                    let property_changed = merge_structured_return(&mut slot, returned);
                    *existing = slot.expect("an occupied property remains occupied");
                    changed | property_changed
                } else {
                    current.properties.insert(property, returned);
                    true
                }
            }),
        _ => false,
    }
}
