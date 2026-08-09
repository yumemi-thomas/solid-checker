//! The local-access stage: per-file reads and writes of reactive
//! sources, classified by execution role, with per-file reuse.

use crate::*;

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::execution_role::{
    allowed_callback_spans, async_execution_role, control_flow_execution_role,
    named_callback_execution_role, read_analysis_context, semantic_execution_role,
};
use crate::identity::SymbolId;
use crate::indexes::{EntitySymbols, SemanticLookup};
use solid_facts::ProjectFacts;
use typefacts::{Declaration, Location};

pub(crate) struct LocalAccessContext<'a> {
    pub(crate) facts: &'a ProjectFacts,
    pub(crate) lookup: &'a SemanticLookup<'a>,
    pub(crate) entities: &'a EntitySymbols,
    pub(crate) symbol_names: &'a HashMap<SymbolId, SymbolId>,
    pub(crate) reachable_calls: &'a HashMap<Location, usize>,
    pub(crate) accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) accessor_origins: &'a HashMap<SymbolId, (SymbolId, SymbolId, Location)>,
    pub(crate) setters: &'a HashMap<SymbolId, (SymbolId, Location, bool)>,
    pub(crate) actions: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) source_primitives: &'a HashMap<SymbolId, SymbolId>,
    pub(crate) async_sources: &'a HashSet<SymbolId>,
    pub(crate) source_declarations: &'a HashMap<SymbolId, Declaration>,
    pub(crate) contract_reads: &'a HashMap<SymbolId, Vec<(String, String, Location, String)>>,
    pub(crate) contract_returns: &'a HashMap<SymbolId, (ContractReturn, Location)>,
    pub(crate) bundled_returns: &'a HashMap<SymbolId, ContractReturn>,
    pub(crate) source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
    pub(crate) prop_sources: &'a HashMap<SymbolId, (SymbolId, Location)>,
}

pub(crate) struct LocalAccessReuse<'a> {
    pub(crate) aggregate_reusable: bool,
    pub(crate) typescript_unchanged: bool,
    pub(crate) source_discovery_delta: Option<&'a SourceDiscoveryTypeScriptDelta>,
    pub(crate) changed_source_symbols: &'a HashSet<SymbolId>,
    pub(crate) retained_source_paths: &'a HashSet<String>,
    pub(crate) global_async_context_unchanged: bool,
}

impl LocalAccessContext<'_> {
    pub(crate) fn build(
        &self,
        cache: Option<&mut CachedLocalAccesses>,
        reuse: LocalAccessReuse<'_>,
    ) -> LocalAccessBuild {
        let LocalAccessReuse {
            aggregate_reusable,
            typescript_unchanged,
            source_discovery_delta,
            changed_source_symbols,
            retained_source_paths,
            global_async_context_unchanged,
        } = reuse;
        if aggregate_reusable
            && let Some(cached) = cache.as_deref().and_then(|cache| cache.aggregate.as_ref())
        {
            return LocalAccessBuild {
                result: cached.clone(),
                reused: true,
                reused_files: u64::try_from(self.facts.files.len()).unwrap_or(u64::MAX),
                recomputed_files: 0,
            };
        }

        let mut result = LocalAccessResult::default();
        let mut reused_files = 0;
        let mut recomputed_files = 0;
        if let Some(cache) = cache {
            let exact_typescript_delta = typescript_unchanged || source_discovery_delta.is_some();
            let mut candidate_dependencies = changed_source_symbols.clone();
            if let Some(delta) = source_discovery_delta {
                candidate_dependencies.extend(delta.semantic_symbol_ids.iter().cloned());
            }
            for (symbol, previous) in &cache.prop_sources {
                if self.prop_sources.get(symbol) != Some(previous) {
                    candidate_dependencies.insert(symbol.clone());
                }
            }
            for symbol in self.prop_sources.keys() {
                if !cache.prop_sources.contains_key(symbol) {
                    candidate_dependencies.insert(symbol.clone());
                }
            }
            let changed_dependencies = candidate_dependencies
                .into_iter()
                .filter(|symbol| {
                    cache
                        .dependency_states
                        .get(symbol)
                        .is_some_and(|previous| *previous != self.symbol_state(symbol))
                })
                .collect::<HashSet<_>>();
            let current_paths = self
                .facts
                .files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<HashSet<_>>();
            cache
                .files
                .retain(|path, _| current_paths.contains(path.as_str()));
            for file in &self.facts.files {
                if let Some(cached) = cache.files.get(file.path.as_str())
                    && exact_typescript_delta
                    && self.cached_matches(
                        file,
                        cached,
                        retained_source_paths.contains(file.path.as_str()),
                        &changed_dependencies,
                        global_async_context_unchanged,
                    )
                {
                    reused_files += 1;
                    continue;
                }
                let contribution = self.discover(file);
                cache.files.insert(
                    file.path.clone(),
                    CachedLocalAccessFile {
                        source_hash: file.source_hash.clone(),
                        cross_file_proofs: self.lookup.returned_callback_proof_digest(),
                        compiler: file.compiler.clone(),
                        dependencies: self.dependencies(file),
                        call_multiplicities: self.call_multiplicities(file),
                        contribution,
                    },
                );
                recomputed_files += 1;
            }
            let current_dependencies = cache
                .files
                .values()
                .flat_map(|file| file.dependencies.iter().cloned())
                .collect::<HashSet<_>>();
            cache
                .dependency_states
                .retain(|symbol, _| current_dependencies.contains(symbol));
            for symbol in current_dependencies {
                if !cache.dependency_states.contains_key(symbol.as_str())
                    || changed_dependencies.contains(symbol.as_str())
                {
                    cache
                        .dependency_states
                        .insert(symbol.clone(), self.symbol_state(&symbol));
                }
            }
            cache
                .prop_sources
                .retain(|symbol, _| self.prop_sources.contains_key(symbol));
            for (symbol, source) in self.prop_sources {
                if cache.prop_sources.get(symbol) != Some(source) {
                    cache.prop_sources.insert(symbol.clone(), source.clone());
                }
            }
            let local_access_files = &cache.files;
            for partial in parallel_file_chunk_results(&self.facts.files, |files| {
                let mut partial = LocalAccessResult::default();
                for file in files {
                    if let Some(cached) = local_access_files.get(file.path.as_str()) {
                        append_local_access_result(&mut partial, &cached.contribution);
                    }
                }
                partial
            }) {
                append_local_access_result_owned(&mut result, partial);
            }
            cache.aggregate = Some(result.clone());
        } else {
            for file in &self.facts.files {
                let contribution = self.discover(file);
                append_local_access_result(&mut result, &contribution);
                recomputed_files += 1;
            }
        }
        LocalAccessBuild {
            result,
            reused: false,
            reused_files,
            recomputed_files,
        }
    }

    pub(crate) fn dependencies(&self, file: &solid_facts::FileFacts) -> HashSet<SymbolId> {
        file.ast
            .calls
            .iter()
            .map(|call| call.callee)
            .chain(file.ast.members.iter().map(|member| member.object))
            .chain(file.ast.spreads.iter().map(|spread| spread.argument))
            .chain(
                file.ast
                    .jsx_elements
                    .iter()
                    .map(|element| element.name.span),
            )
            .filter_map(|span| {
                self.entities
                    .get(&location(file.path.shared(), span))
                    .cloned()
            })
            .collect()
    }

    pub(crate) fn symbol_state(&self, symbol: &str) -> LocalAccessSymbolState {
        LocalAccessSymbolState {
            accessor: self.accessors.get(symbol).cloned(),
            accessor_origin: self.accessor_origins.get(symbol).cloned(),
            setter: self.setters.get(symbol).cloned(),
            action: self.actions.get(symbol).cloned(),
            source_primitive: self.source_primitives.get(symbol).cloned(),
            async_source: self.async_sources.contains(symbol),
            contract_reads: self.contract_reads.get(symbol).cloned(),
            source_kind: self.source_kinds.get(symbol).copied(),
            prop_source: self.prop_sources.get(symbol).cloned(),
            source_declaration: self.source_declarations.get(symbol).cloned(),
            symbol_name: self.symbol_names.get(symbol).cloned(),
        }
    }

    pub(crate) fn call_multiplicities(
        &self,
        file: &solid_facts::FileFacts,
    ) -> Vec<(Location, Option<usize>)> {
        file.ast
            .calls
            .iter()
            .map(|call| {
                let callee = location(file.path.shared(), call.callee);
                let multiplicity = self.reachable_calls.get(&callee).copied();
                (callee, multiplicity)
            })
            .collect()
    }

    pub(crate) fn cached_matches(
        &self,
        file: &solid_facts::FileFacts,
        cached: &CachedLocalAccessFile,
        retained_source_path: bool,
        changed_dependencies: &HashSet<SymbolId>,
        global_async_context_unchanged: bool,
    ) -> bool {
        retained_source_path
            && cached.source_hash == file.source_hash
            && cached.cross_file_proofs == self.lookup.returned_callback_proof_digest()
            && (Arc::ptr_eq(&cached.compiler, &file.compiler)
                || same_compiler_semantics(&cached.compiler, &file.compiler))
            && (global_async_context_unchanged || cached.contribution.async_reads.is_empty())
            && cached.dependencies.is_disjoint(changed_dependencies)
            && cached
                .call_multiplicities
                .iter()
                .all(|(callee, previous)| self.reachable_calls.get(callee).copied() == *previous)
    }

    pub(crate) fn discover(&self, file: &solid_facts::FileFacts) -> LocalAccessResult {
        let mut result = LocalAccessResult::default();
        let mut seen = HashSet::new();
        let allowed = allowed_callback_spans(file, self.lookup);
        for call in &file.ast.calls {
            let callee = location(file.path.shared(), call.callee);
            // A returned accessor can be invoked immediately without ever
            // acquiring a binding symbol: `mapArray(list, map)()`. Preserve
            // the package's reactive-return contract across that exact AST
            // shape instead of making source discovery depend on `const x =`.
            // A member callee is a different shape: `factory(...).member()`
            // invokes the member, not the returned accessor, so no contracted
            // read is proven there.
            let immediate_return = file
                .ast
                .calls_within(call.callee)
                .filter(|nested| nested.span != call.span)
                .max_by_key(|nested| nested.span.end - nested.span.start)
                .filter(|_| !self.lookup.is_member_span(file, call.callee))
                .and_then(|factory| {
                    let symbol = self.lookup.callee_symbol(file, factory.callee)?;
                    self.contract_returns
                        .get(symbol)
                        .cloned()
                        .or_else(|| {
                            let primitive = primitive_name(
                                file.path.as_str(),
                                factory.callee,
                                factory.static_callee(&file.source),
                                self.entities,
                                self.symbol_names,
                                self.lookup.dialect,
                            )?;
                            self.bundled_returns
                                .get(primitive.as_str())
                                .cloned()
                                .map(|returned| {
                                    (
                                        returned,
                                        bundled_contract_location(self.lookup.dialect, &primitive),
                                    )
                                })
                        })
                        .map(|contract| (factory, contract))
                });
            if let Some((factory, (returned, declaration))) = immediate_return {
                let execution = semantic_execution_role(
                    file,
                    call.callee,
                    &allowed,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                );
                let reachable = self.reachable_calls.get(&callee).is_some()
                    || enclosing_render_function(file, call.callee);
                let key = (callee.path.clone(), callee.start_byte, callee.end_byte);
                if reachable && seen.insert(key) {
                    result.reads.push(Arc::new(ReactiveRead {
                        kind: returned.kind.into(),
                        accessor: returned.label.into(),
                        location: location(file.path.shared(), call.span),
                        declaration: declaration.clone(),
                        execution,
                        context: read_analysis_context(file, call.span, execution).into(),
                        via: file
                            .source_text(factory.callee)
                            .unwrap_or_default()
                            .to_owned()
                            .into(),
                        origin: Some(declaration.clone()),
                        origin_context: Arc::from("package return contract"),
                    }));
                    if counts_as_strict_read_root(file, call.span, execution) {
                        result.strict_read_obligations += 1;
                    }
                }
            }
            let Some(symbol) = self.lookup.callee_symbol(file, call.callee) else {
                continue;
            };
            if inside_known_value_function_argument(file, call.callee, self.lookup) {
                continue;
            }
            let inside_function = file.ast.any_function_body_containing(call.span);
            if inside_function && self.setters.contains_key(symbol) {
                result.write_action_obligations.insert((
                    "write",
                    callee.path.to_string(),
                    callee.start_byte,
                    callee.end_byte,
                ));
            }
            if inside_function && self.actions.contains_key(symbol) {
                result.write_action_obligations.insert((
                    "action",
                    callee.path.to_string(),
                    callee.start_byte,
                    callee.end_byte,
                ));
            }
            let execution = semantic_execution_role(
                file,
                call.callee,
                &allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            );
            let typed_effect_accessor = execution == ExecutionRole::EffectApply
                && call.arguments.is_empty()
                && typed_accessor_descriptor_at(self.lookup, file.path.as_str(), call.callee)
                    .is_some();
            let Some(multiplicity) = self.reachable_calls.get(&callee).copied().or_else(|| {
                (typed_effect_accessor
                    || (self.accessors.contains_key(symbol)
                        && (execution == ExecutionRole::EffectApply
                            || control_flow_execution_role(
                                file,
                                call.callee,
                                self.entities,
                                self.symbol_names,
                                self.lookup.dialect,
                            )
                            .is_some()
                            || named_callback_execution_role(file, call.callee, self.lookup)
                                .is_some()
                            || enclosing_render_function(file, call.callee))))
                .then_some(1)
            }) else {
                continue;
            };
            let key = (callee.path.clone(), callee.start_byte, callee.end_byte);
            if let Some((name, declaration)) = self.accessors.get(symbol)
                && (!inside_lowercase_named_function(file, call.callee, self.lookup.dialect)
                    || named_callback_execution_role(file, call.callee, self.lookup).is_some())
                && seen.insert(key.clone())
            {
                let origin = self.accessor_origins.get(symbol);
                let display_name = call.static_callee(&file.source).unwrap_or(name);
                result.reads.push(Arc::new(ReactiveRead {
                    kind: "accessor".into(),
                    accessor: origin
                        .map_or_else(|| display_name.to_string(), |origin| origin.0.to_string())
                        .into(),
                    location: location(file.path.shared(), call.span),
                    declaration: origin
                        .map_or_else(|| declaration.clone(), |origin| origin.2.clone()),
                    execution,
                    context: read_analysis_context(file, call.span, execution).into(),
                    via: origin.map_or_else(String::new, |_| name.to_string()).into(),
                    origin: origin.map(|origin| origin.2.clone()),
                    origin_context: origin
                        .map_or_else(String::new, |origin| origin.1.to_string())
                        .into(),
                }));
                if !matches!(
                    self.source_primitives.get(symbol).map(SymbolId::as_str),
                    Some("createOptimistic" | "createOptimisticStore")
                ) && counts_as_strict_read_root(file, call.span, execution)
                {
                    result.strict_read_obligations += 1;
                }
                if self.async_sources.contains(symbol) {
                    let async_execution = async_execution_role(file, call.callee, execution);
                    result.async_reads.push(Arc::new(AsyncRead {
                        accessor: format!("{name}()").into(),
                        location: location(file.path.shared(), call.span),
                        declaration: declaration.clone(),
                        execution: async_execution,
                        leaf_owner: containing_leaf_owner(
                            file,
                            call.callee,
                            self.entities,
                            self.symbol_names,
                            self.lookup,
                        )
                        .map(Into::into),
                        under_loading: read_is_under_loading(
                            self.lookup,
                            file,
                            call.callee,
                            self.symbol_names,
                        ),
                    }));
                }
            }
            if !self.accessors.contains_key(symbol)
                && execution == ExecutionRole::EffectApply
                && call.arguments.is_empty()
                && let Some(descriptor) =
                    typed_accessor_descriptor_at(self.lookup, file.path.as_str(), call.callee)
                && seen.insert(key.clone())
            {
                let display = usize::try_from(call.callee.start)
                    .ok()
                    .zip(usize::try_from(call.callee.end).ok())
                    .and_then(|(start, end)| file.source.get(start..end))
                    .unwrap_or("accessor")
                    .to_string();
                let declaration = descriptor.alias_declarations.first().map_or_else(
                    || callee.clone(),
                    |declaration| declaration.location.clone(),
                );
                result.reads.push(Arc::new(ReactiveRead {
                    kind: "accessor".into(),
                    accessor: display.into(),
                    location: location(file.path.shared(), call.span),
                    declaration,
                    execution,
                    context: read_analysis_context(file, call.span, execution).into(),
                    via: Arc::from(""),
                    origin: None,
                    origin_context: Arc::from(""),
                }));
                result.strict_read_obligations += 1;
            }
            if let Some(contracted) = self.contract_reads.get(symbol)
                && !inside_lowercase_named_function(file, call.callee, self.lookup.dialect)
            {
                for (index, (name, via, declaration, kind)) in contracted.iter().enumerate() {
                    let contract_key = (
                        callee.path.clone(),
                        callee.start_byte,
                        callee
                            .end_byte
                            .saturating_add(u64::try_from(index).unwrap_or(u64::MAX)),
                    );
                    if seen.insert(contract_key) {
                        result.reads.push(Arc::new(ReactiveRead {
                            kind: kind.clone().into(),
                            accessor: name.clone().into(),
                            location: location(file.path.shared(), call.span),
                            declaration: declaration.clone(),
                            execution,
                            context: read_analysis_context(file, call.span, execution).into(),
                            via: via.clone().into(),
                            origin: Some(declaration.clone()),
                            origin_context: via.clone().into(),
                        }));
                        if counts_as_strict_read_root(file, call.span, execution) {
                            result.strict_read_obligations += 1;
                        }
                    }
                }
            }
            if let Some((name, declaration, allowed_by_option)) = self.setters.get(symbol) {
                for _ in 0..multiplicity {
                    result.writes.push(Arc::new(ReactiveWrite {
                        setter: name.to_string().into(),
                        location: location(file.path.shared(), call.span),
                        declaration: declaration.clone(),
                        execution,
                        allowed_by_option: *allowed_by_option,
                        context: analysis_context(
                            file,
                            call.span,
                            self.entities,
                            self.symbol_names,
                            self.lookup.dialect,
                        )
                        .into(),
                    }));
                }
            }
            if let Some((name, declaration)) = self.actions.get(symbol) {
                for _ in 0..multiplicity {
                    result.action_invocations.push(Arc::new(ActionInvocation {
                        action: name.to_string().into(),
                        location: location(file.path.shared(), call.span),
                        declaration: declaration.clone(),
                        execution,
                        context: analysis_context(
                            file,
                            call.span,
                            self.entities,
                            self.symbol_names,
                            self.lookup.dialect,
                        )
                        .into(),
                    }));
                }
            }
        }
        for member in &file.ast.members {
            if file
                .ast
                .members
                .iter()
                .any(|candidate| candidate.object == member.span)
            {
                continue;
            }
            let object = location(file.path.shared(), member.object);
            let Some(symbol) = self.entities.get(&object) else {
                continue;
            };
            let execution = semantic_execution_role(
                file,
                member.span,
                &allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            );
            if (inside_lowercase_named_function(file, member.span, self.lookup.dialect)
                || inside_unclassified_callback(file, member.span))
                && named_callback_execution_role(file, member.span, self.lookup).is_none()
                && !matches!(
                    execution,
                    ExecutionRole::EffectApply | ExecutionRole::UntrackedCallback
                )
            {
                continue;
            }
            let source = if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
                self.accessors.get(symbol)
            } else {
                self.prop_sources.get(symbol)
            };
            let Some((name, declaration)) = source else {
                continue;
            };
            let key = (object.path.clone(), object.start_byte, object.end_byte);
            if !seen.insert(key) {
                continue;
            }
            let accessor = usize::try_from(member.span.start)
                .ok()
                .zip(usize::try_from(member.span.end).ok())
                .and_then(|(start, end)| file.source.get(start..end))
                .and_then(|path| {
                    path.find('.')
                        .map(|index| format!("{name}{}", &path[index..]))
                })
                .unwrap_or_else(|| {
                    format!(
                        "{name}.{}",
                        file.source_text(member.property).unwrap_or_default()
                    )
                });
            result.reads.push(Arc::new(ReactiveRead {
                kind: if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
                    "store-path".into()
                } else {
                    "component-props".into()
                },
                accessor: accessor.into(),
                location: location(file.path.shared(), member.span),
                declaration: declaration.clone(),
                execution,
                context: read_analysis_context(file, member.span, execution).into(),
                via: Arc::from(""),
                origin: None,
                origin_context: Arc::from(""),
            }));
            if !matches!(
                self.source_primitives.get(symbol).map(SymbolId::as_str),
                Some("createOptimistic" | "createOptimisticStore")
            ) && counts_as_strict_read_root(file, member.span, execution)
            {
                result.strict_read_obligations += 1;
            }
            if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store)
                && self.async_sources.contains(symbol)
            {
                let async_execution = async_execution_role(file, member.span, execution);
                result.async_reads.push(Arc::new(AsyncRead {
                    accessor: format!(
                        "{name}.{}",
                        file.source_text(member.property).unwrap_or_default()
                    )
                    .into(),
                    location: location(file.path.shared(), member.span),
                    declaration: declaration.clone(),
                    execution: async_execution,
                    leaf_owner: containing_leaf_owner(
                        file,
                        member.span,
                        self.entities,
                        self.symbol_names,
                        self.lookup,
                    )
                    .map(Into::into),
                    under_loading: read_is_under_loading(
                        self.lookup,
                        file,
                        member.span,
                        self.symbol_names,
                    ),
                }));
            }
        }
        for spread in &file.ast.spreads {
            let argument = location(file.path.shared(), spread.argument);
            let Some(symbol) = self.entities.get(&argument) else {
                continue;
            };
            let execution = semantic_execution_role(
                file,
                spread.span,
                &allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            );
            if (inside_lowercase_named_function(file, spread.span, self.lookup.dialect)
                || inside_unclassified_callback(file, spread.span))
                && named_callback_execution_role(file, spread.span, self.lookup).is_none()
                && !matches!(
                    execution,
                    ExecutionRole::EffectApply | ExecutionRole::UntrackedCallback
                )
            {
                continue;
            }
            let source = if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
                self.accessors.get(symbol)
            } else {
                self.prop_sources.get(symbol)
            };
            let Some((name, declaration)) = source else {
                continue;
            };
            result.reads.push(Arc::new(ReactiveRead {
                kind: if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
                    "store-path".into()
                } else {
                    "component-props".into()
                },
                accessor: format!("{name} spread").into(),
                location: location(file.path.shared(), spread.span),
                declaration: declaration.clone(),
                execution,
                context: read_analysis_context(file, spread.span, execution).into(),
                via: Arc::from(""),
                origin: None,
                origin_context: Arc::from(""),
            }));
            if !matches!(
                self.source_primitives.get(symbol).map(SymbolId::as_str),
                Some("createOptimistic" | "createOptimisticStore")
            ) && counts_as_strict_read_root(file, spread.span, execution)
            {
                result.strict_read_obligations += 1;
            }
        }
        for element in &file.ast.jsx_elements {
            let name_location = location(file.path.shared(), element.name.span);
            let Some(symbol) = self.entities.get(&name_location) else {
                continue;
            };
            if !self.async_sources.contains(symbol)
                || self.source_primitives.get(symbol).map(SymbolId::as_str) != Some("dynamic")
            {
                continue;
            }
            let execution = ExecutionRole::TrackedJsx;
            result.async_reads.push(Arc::new(AsyncRead {
                accessor: format!(
                    "<{}>",
                    file.source_text(element.name.span).unwrap_or_default()
                )
                .into(),
                location: location(file.path.shared(), element.span),
                declaration: self.source_declarations.get(symbol).map_or_else(
                    || name_location.clone(),
                    |declaration| declaration.location.clone(),
                ),
                execution,
                leaf_owner: containing_leaf_owner(
                    file,
                    element.name.span,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                )
                .map(Into::into),
                under_loading: read_is_under_loading(
                    self.lookup,
                    file,
                    element.name.span,
                    self.symbol_names,
                ),
            }));
        }
        result
    }
}

pub(crate) fn append_local_access_result(
    target: &mut LocalAccessResult,
    source: &LocalAccessResult,
) {
    target.reads.extend(source.reads.iter().cloned());
    target.writes.extend(source.writes.iter().cloned());
    target
        .action_invocations
        .extend(source.action_invocations.iter().cloned());
    target
        .async_reads
        .extend(source.async_reads.iter().cloned());
    target.strict_read_obligations += source.strict_read_obligations;
    target
        .write_action_obligations
        .extend(source.write_action_obligations.iter().cloned());
}

pub(crate) fn append_local_access_result_owned(
    target: &mut LocalAccessResult,
    source: LocalAccessResult,
) {
    target.reads.extend(source.reads);
    target.writes.extend(source.writes);
    target.action_invocations.extend(source.action_invocations);
    target.async_reads.extend(source.async_reads);
    target.strict_read_obligations += source.strict_read_obligations;
    target
        .write_action_obligations
        .extend(source.write_action_obligations);
}
