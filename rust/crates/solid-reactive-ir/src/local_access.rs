//! The local-access stage: per-file reads and writes of reactive
//! sources, classified by execution role, with per-file reuse.

use crate::cache::{
    CachedLocalAccessFile, CachedLocalAccesses, LocalAccessBuild, LocalAccessResult,
    LocalAccessSymbolState, SourceDiscoveryTypeScriptDelta, same_compiler_semantics,
};
use crate::owners::{
    LoadingCover, analysis_context, async_read_role, containing_ast_function,
    containing_leaf_owner, counts_as_strict_read_root, enclosing_render_function,
    function_binding_name, inside_known_value_function_argument, inside_non_component_function,
    inside_unclassified_callback, read_loading_cover, solid_accessor_declaration,
    typed_accessor_descriptor_at,
};
use crate::pipeline::parallel_file_chunk_results;
use crate::source_discovery::{AsyncSourceOptions, PropUse, PropsReactivityIndex};
use crate::{
    ActionInvocation, AsyncRead, ContractReturn, ExecutionRole, ReactiveRead, ReactiveSourceKind,
    ReactiveWrite, location,
};

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::execution_role::{
    RootBodyGuard, allowed_callback_spans, async_execution_role, callee_callback_timing,
    contract_tracked_accessor_read_role, control_flow_execution_role,
    direct_control_flow_body_role, host_callback_timing, missing_jsx_census,
    named_callback_execution_role, nested_literal_runs_during_body, pending_accessor_probe,
    read_analysis_context, semantic_execution_role, semantic_write_execution_role,
};
use crate::identity::SymbolId;
use crate::indexes::{EntitySymbols, SemanticLookup};
use solid_facts::ProjectFacts;
use typefacts::{Declaration, Location};

/// Do not apply an incoming-prop witness to a replaced or escaping root.
/// Unknown writes through aliases/views need their own value-flow proof.
pub(super) fn props_root_is_current(
    file: &solid_facts::FileFacts,
    declaration: &Location,
    symbol: &SymbolId,
    entities: &EntitySymbols,
) -> bool {
    use solid_facts::core::Span;
    if declaration.path.as_ref() != file.path.as_str() {
        return false;
    }
    let (Ok(start), Ok(end)) = (
        u32::try_from(declaration.start_byte),
        u32::try_from(declaration.end_byte),
    ) else {
        return false;
    };
    if crate::indexes::binding_written(file, Span::new(start, end)) {
        return false;
    }
    let names_root = |span| {
        let span = file.ast.peel_ts_sugar_span(span);
        file.ast.identifiers.iter().any(|identifier| {
            identifier.span == span
                && identifier.role == solid_facts::ast::IdentifierRole::Reference
        }) && entities.at(file.path.as_str(), span) == Some(symbol)
    };
    let member_written = file
        .ast
        .assignments
        .iter()
        .map(|assignment| assignment.target)
        .chain(file.ast.deleted_targets.iter().copied())
        .chain(file.ast.iteration_targets.iter().copied())
        .any(|target| {
            file.ast
                .members
                .iter()
                .any(|member| target.contains(member.span) && names_root(member.object))
        });
    // Every unaccounted use of the object can alias or escape it, including
    // shorthand properties, returns and nested parameter defaults. Erased
    // parts of transparent TS wrappers do not evaluate the object.
    let escapes = file.ast.identifiers.iter().any(|identifier| {
        names_root(identifier.span)
            && !file.ast.transparent_wrappers.iter().any(|wrapper| {
                wrapper.span.contains(identifier.span) && !wrapper.inner.contains(identifier.span)
            })
            && !file
                .ast
                .members
                .iter()
                .any(|member| file.ast.peel_ts_sugar_span(member.object) == identifier.span)
    });
    !member_written && !escapes
}

pub(crate) struct LocalAccessContext<'a, 'facts> {
    pub(crate) facts: &'a ProjectFacts,
    pub(crate) lookup: &'a SemanticLookup<'facts>,
    pub(crate) entities: &'a EntitySymbols,
    pub(crate) symbol_names: &'a HashMap<SymbolId, SymbolId>,
    pub(crate) reachable_calls: &'a HashMap<Location, usize>,
    pub(crate) accessors: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) accessor_origins: &'a HashMap<SymbolId, (SymbolId, SymbolId, Location)>,
    pub(crate) setters: &'a HashMap<SymbolId, (SymbolId, Location, bool, ReactiveSourceKind)>,
    pub(crate) actions: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) source_primitives: &'a HashMap<SymbolId, SymbolId>,
    pub(crate) async_sources: &'a HashSet<SymbolId>,
    pub(crate) source_async_options: &'a HashMap<SymbolId, AsyncSourceOptions>,
    /// Whether any file in the analyzed project imports a server rendering
    /// entry point. False means unresolved, not proven CSR: the server entry
    /// may live in another tsconfig or package.
    pub(crate) server_rendering: crate::source_discovery::ServerRenderingPremise,
    pub(crate) source_declarations: &'a HashMap<SymbolId, Declaration>,
    pub(crate) contract_reads: &'a HashMap<SymbolId, Vec<crate::ContractReadSite>>,
    pub(crate) contract_parameter_reads:
        &'a HashMap<SymbolId, Vec<crate::ContractParameterReadSite>>,
    pub(crate) contract_returns: &'a HashMap<SymbolId, (ContractReturn, Location)>,
    pub(crate) source_kinds: &'a HashMap<SymbolId, ReactiveSourceKind>,
    pub(crate) prop_sources: &'a HashMap<SymbolId, (SymbolId, Location)>,
    pub(crate) uncertain_prop_sources: &'a HashSet<SymbolId>,
    pub(crate) props_reactivity: &'a PropsReactivityIndex,
}

/// Whether a `parameter-member` argument is proven not to be a reactive
/// source, so the package's member call on it cannot read reactively.
///
/// A Solid store is a proxy typed as the object it wraps, so a declared type
/// never proves the negative. Two things do:
///
/// * the argument's own syntax -- an inline literal is a value created at the
///   call site, and `createStore` never produced it. This holds for a literal
///   that *spreads* a store too, and deliberately so: `[...storeArray]` copies
///   out of the proxy at the call site, so what the callee receives really is
///   snapshot data, and the reactive read is the spread itself. The spread
///   pass below owns that read and reports it in its own execution role;
///   claiming the callee's parameter read as well would report one dependency
///   twice. What stays uncovered is a *nested* proxy surviving the shallow
///   copy — see docs/precision-backlog.md; and
/// * an analyzed local binding initialized by a resolved standard-library
///   call, such as `document.createElement("button")`. That value's origin is
///   platform code, which cannot return a Solid store.
///
/// Anything the project cannot see the origin of -- a parameter, a prop, an
/// import, a bare `declare const` -- stays unproven and keeps its SC9012
/// obligation. This is the negative proof, never a default.
pub(crate) fn argument_proves_non_reactive(
    file: &solid_facts::FileFacts,
    argument: &solid_facts::ast::ArgumentFact,
    entities: &EntitySymbols,
    source_kinds: &HashMap<SymbolId, ReactiveSourceKind>,
    lookup: &SemanticLookup<'_>,
) -> bool {
    use solid_facts::ast::RuntimeValueKind;
    if matches!(
        argument.runtime_value_kind,
        RuntimeValueKind::Primitive
            | RuntimeValueKind::Array
            | RuntimeValueKind::Object
            | RuntimeValueKind::Nullish
    ) {
        return true;
    }
    let Some(symbol) = entities.get(&location(file.path.shared(), argument.span)) else {
        return false;
    };
    if source_kinds.contains_key(symbol.as_str()) {
        return false;
    }
    file.ast.bindings.iter().any(|binding| {
        binding.names.iter().any(|name| {
            entities
                .at(file.path.as_str(), name.span)
                .is_some_and(|candidate| candidate == symbol)
        }) && binding.call_initializer.is_some_and(|initializer| {
            file.ast.call_at(initializer).is_some_and(|call| {
                lookup
                    .resolved_callee_call(file, call.callee)
                    .and_then(|resolved| resolved.declaration.as_ref())
                    .is_some_and(|declaration| declaration.standard_library)
            })
        })
    })
}

pub(crate) struct LocalAccessReuse<'a> {
    pub(crate) aggregate_reusable: bool,
    pub(crate) typescript_unchanged: bool,
    pub(crate) source_discovery_delta: Option<&'a SourceDiscoveryTypeScriptDelta>,
    pub(crate) changed_source_symbols: &'a HashSet<SymbolId>,
    pub(crate) retained_source_paths: &'a HashSet<String>,
    pub(crate) global_async_context_unchanged: bool,
}

impl LocalAccessContext<'_, '_> {
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
                if self.prop_state(symbol) != Some(previous.clone()) {
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
                        cross_file_proofs: self.lookup.cross_file_proof_digest(),
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
            for symbol in self.prop_sources.keys() {
                let state = self
                    .prop_state(symbol)
                    .expect("prop state exists for every prop source");
                if cache.prop_sources.get(symbol) != Some(&state) {
                    cache.prop_sources.insert(symbol.clone(), state);
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
            .chain(
                file.ast
                    .members
                    .iter()
                    .map(|member| file.ast.peel_ts_sugar_span(member.object)),
            )
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
            .chain(file.ast.calls.iter().filter_map(|call| {
                self.lookup
                    .callee_symbol(file, call.callee)
                    .map(SymbolId::from)
            }))
            .collect()
    }

    /// The source's declared async/hydration options with the project-level
    /// server-render fact folded into a violation/uncertifiable distinction.
    fn effective_async_options(&self, symbol: &str) -> AsyncSourceOptions {
        let mut options = self
            .source_async_options
            .get(symbol)
            .copied()
            .unwrap_or_default();
        // Three states, not two. A bare `ssrSource: "client"` source is a
        // server-render hole only where server rendering is proven; it is a
        // proof obligation only where the premise is unresolved. Where an
        // explicit rendering selector proves the application is client-only,
        // the hole cannot exist and neither claim is made.
        use crate::source_discovery::ServerRenderingPremise;
        options.server_rendering_unresolved =
            options.ssr_client_bare && self.server_rendering == ServerRenderingPremise::Unresolved;
        options.ssr_client_bare &= self.server_rendering.renders();
        options
    }

    /// The prop-source identity plus its caller classification, the pair the
    /// incremental cache fingerprints: a call site in one file deciding a
    /// prop's reactivity must invalidate the component's file.
    pub(crate) fn prop_state(
        &self,
        symbol: &str,
    ) -> Option<(
        SymbolId,
        Location,
        Option<crate::source_discovery::PropsReactivity>,
        bool,
    )> {
        self.prop_sources.get(symbol).map(|(name, declaration)| {
            (
                name.clone(),
                declaration.clone(),
                self.props_reactivity.for_declaration(declaration).cloned(),
                self.uncertain_prop_sources.contains(symbol),
            )
        })
    }

    pub(crate) fn symbol_state(&self, symbol: &str) -> LocalAccessSymbolState {
        LocalAccessSymbolState {
            accessor: self.accessors.get(symbol).cloned(),
            accessor_origin: self.accessor_origins.get(symbol).cloned(),
            setter: self.setters.get(symbol).cloned(),
            action: self.actions.get(symbol).cloned(),
            source_primitive: self.source_primitives.get(symbol).cloned(),
            async_source: self.async_sources.contains(symbol),
            async_options: self.effective_async_options(symbol),
            contract_reads: self.contract_reads.get(symbol).cloned(),
            contract_parameter_reads: self.contract_parameter_reads.get(symbol).cloned(),
            source_kind: self.source_kinds.get(symbol).copied(),
            prop_source: self.prop_state(symbol),
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
            && cached.cross_file_proofs == self.lookup.cross_file_proof_digest()
            && (Arc::ptr_eq(&cached.compiler, &file.compiler)
                || same_compiler_semantics(&cached.compiler, &file.compiler))
            && (global_async_context_unchanged || cached.contribution.async_reads.is_empty())
            && cached.dependencies.is_disjoint(changed_dependencies)
            && cached
                .call_multiplicities
                .iter()
                .all(|(callee, previous)| self.reachable_calls.get(callee).copied() == *previous)
    }

    /// Own body only: nested parameter defaults retain ADR 0204's existing
    /// authored-site proof. Select the innermost function before admitting a
    /// named helper, so an anonymous nested closure is not its parent's read.
    fn component_prop_helper<'file>(
        &self,
        file: &'file solid_facts::FileFacts,
        site: solid_facts::core::Span,
    ) -> Option<&'file solid_facts::ast::FunctionFact> {
        let helper = file
            .ast
            .functions
            .iter()
            .filter(|function| function.span.contains(site))
            .min_by_key(|function| function.span.end - function.span.start)?;
        (helper.body.contains(site)
            && function_binding_name(file, helper).is_some()
            && !self.lookup.function_may_be_component(file, helper)
            && file.ast.functions.iter().any(|component| {
                component.body.contains(helper.span)
                    && self.lookup.function_is_component(file, component)
            }))
        .then_some(helper)
    }

    /// An exact callback slot's execution role, independent of any other use
    /// of the binding. Arbitrary callback props and returned invokers remain
    /// escapes. An effect apply slot is strict, despite being deferred.
    fn helper_value_execution(
        &self,
        file: &solid_facts::FileFacts,
        reference: solid_facts::core::Span,
    ) -> Option<ExecutionRole> {
        if file.compiler.callback_roles.iter().any(|callback| {
            callback.span == reference
                && callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
        }) {
            return Some(ExecutionRole::EventCallback);
        }
        file.ast
            .arguments_containing(reference)
            .find_map(|(call, index)| {
                let argument = &call.arguments[index];
                if argument.spread
                    || file.ast.peel_ts_sugar_span(argument.span) != reference
                    || !self
                        .lookup
                        .resolved_callee_call(file, call.callee)
                        .is_some_and(|resolved| {
                            resolved.validity == typefacts::ResolvedCallValidity::Valid
                        })
                {
                    return None;
                }
                let primitive = self.lookup.primitive_at_call(file, call.span)?;
                let dialect = self.lookup.dialect;
                let semantics =
                    dialect.callback_semantics_at(primitive, index, call.arguments.len());
                let execution = semantics.execution?;
                if semantics.requires_return_invocation
                    || semantics.stores_as_value
                    || dialect.callback_runs_on_result_access(
                        primitive,
                        index,
                        call.arguments.len(),
                    )
                {
                    return None;
                }
                if dialect.reports_untracked_reads_at(primitive, index, call.arguments.len()) {
                    return Some(
                        if dialect.apply_callback_argument(primitive) == Some(index) {
                            ExecutionRole::EffectApply
                        } else {
                            ExecutionRole::UntrackedCallback
                        },
                    );
                }
                if semantics.tracks_reads {
                    return Some(ExecutionRole::TrackedJsx);
                }
                if execution == solid_dialect::Execution::Deferred
                    || dialect.runs_callback_deferred(primitive)
                {
                    return Some(ExecutionRole::DeferredCallback);
                }
                None
            })
    }

    /// A named event caller's role is not exclusive when the same binding is
    /// also invoked directly (MixedNamedCaller). Refuse that deeper chain;
    /// do not let its named callback role erase a possible render-time call.
    fn exclusive_event_caller(
        &self,
        file: &solid_facts::FileFacts,
        caller: &solid_facts::ast::FunctionFact,
    ) -> bool {
        if file.compiler.callback_roles.iter().any(|callback| {
            callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
                && callback.span.contains(caller.span)
                && file
                    .ast
                    .functions_within(callback.span)
                    .max_by_key(|function| function.span.end - function.span.start)
                    .is_some_and(|function| function.span == caller.span)
        }) {
            return true;
        }
        let Some(name) = function_binding_name(file, caller) else {
            return false;
        };
        let mut observed = false;
        for identifier in &file.ast.identifiers {
            if identifier.role != solid_facts::ast::IdentifierRole::Reference
                || file.ast.reference_declaration(identifier.span) != Some(name.span)
            {
                continue;
            }
            observed = true;
            if !file.compiler.callback_roles.iter().any(|callback| {
                callback.span == identifier.span
                    && callback.role == solid_facts::compiler::CallbackRoleKind::EventHandler
            }) {
                return false;
            }
        }
        observed
    }

    /// Each exact local call has its own execution proof. An escape opens a
    /// separate definition-site obligation; it cannot erase another call's
    /// valid immutable target, own-body prefix, or incoming-prop witness.
    #[allow(clippy::too_many_arguments)]
    fn project_helper_prop_read(
        &self,
        file: &solid_facts::FileFacts,
        helper: &solid_facts::ast::FunctionFact,
        site: solid_facts::core::Span,
        allowed: &[solid_facts::core::Span],
        read: ReactiveRead,
        definition_admitted: bool,
        result: &mut LocalAccessResult,
    ) {
        if read.execution == ExecutionRole::DiscardedRendering {
            return;
        }
        let Some(name) = function_binding_name(file, helper) else {
            return;
        };
        // Dynamic lexical observation may replace a declaration or the
        // captured props root without an ordinary reference/write fact.
        let dynamic_lexical_observation = file.ast.identifiers.iter().any(|identifier| {
            identifier.role == solid_facts::ast::IdentifierRole::Reference
                && matches!(
                    file.source_text(identifier.span),
                    Some("eval" | "arguments")
                )
                && file.ast.functions.iter().any(|component| {
                    self.lookup.function_is_component(file, component)
                        && component.body.contains(helper.span)
                        && component.span.contains(identifier.span)
                })
        });
        let symbol = self.entities.at(file.path.as_str(), name.span);
        let current = !dynamic_lexical_observation
            && symbol.is_some_and(|symbol| {
                self.lookup.function_value_is_current(file, helper)
                    && !crate::value_identity::binding_has_write(file, self.entities, symbol)
                    && self
                        .lookup
                        .function_for_symbol(symbol.as_str())
                        .is_some_and(|(target_file, target)| {
                            target_file.path == file.path && target.span == helper.span
                        })
            });
        let body_runs = crate::interproc::body_site_runs_during_call(file, helper, site);
        // The lexical census includes undemanded references. Missing call
        // facts open the proof rather than hiding an observed call or escape.
        let mut unproven_use = dynamic_lexical_observation;
        for reference in file.ast.identifiers.iter().filter(|identifier| {
            identifier.role == solid_facts::ast::IdentifierRole::Reference
                && file.ast.reference_declaration(identifier.span) == Some(name.span)
        }) {
            if crate::execution_role::discarded_region_contains(file, reference.span)
                || file.ast.transparent_wrappers.iter().any(|wrapper| {
                    wrapper.span.contains(reference.span) && !wrapper.inner.contains(reference.span)
                })
            {
                continue;
            }
            let Some(call) = file.ast.calls.iter().find(|call| {
                call.direct_callee
                    && !call.construct
                    && file.ast.peel_ts_sugar_span(call.callee) == reference.span
            }) else {
                if current
                    && body_runs
                    && let Some(execution) = self.helper_value_execution(file, reference.span)
                {
                    if execution.reports_untracked_read() {
                        let mut attributed = read.clone();
                        attributed.location = location(file.path.shared(), reference.span);
                        attributed.execution = execution;
                        attributed.context =
                            read_analysis_context(file, site, execution, self.lookup).into();
                        attributed.via =
                            file.source_text(name.span).unwrap_or("local helper").into();
                        attributed.origin = Some(read.location.clone());
                        attributed.origin_context = attributed.via.clone();
                        attributed.summary_attributed = false;
                        attributed.missing_jsx_census =
                            missing_jsx_census(file, reference.span, execution);
                        attributed.host_callback_timing = false;
                        attributed.callee_callback_timing = false;
                        result.reads.push(Arc::new(attributed));
                        result.strict_read_obligations += 1;
                    }
                } else {
                    unproven_use = true;
                }
                continue;
            };
            let execution = semantic_execution_role(
                file,
                call.span,
                allowed,
                self.entities,
                self.symbol_names,
                self.lookup,
            );
            if execution == ExecutionRole::DiscardedRendering {
                continue;
            }
            let candidates = self.lookup.callee_symbols(file, call.callee);
            let exact = current
                && candidates.len() == 1
                && symbol.is_some_and(|symbol| candidates.first() == Some(symbol))
                && self
                    .lookup
                    .resolved_callee_call(file, call.callee)
                    .is_some_and(|resolved| {
                        resolved.validity == typefacts::ResolvedCallValidity::Valid
                    });
            let caller = containing_ast_function(&file.ast, call.span);
            let control = direct_control_flow_body_role(
                file,
                call.span,
                self.entities,
                self.symbol_names,
                self.lookup.dialect,
            );
            let component =
                caller.is_some_and(|caller| self.lookup.function_is_component(file, caller));
            let primitive_callback = caller.is_some_and(|caller| {
                file.ast
                    .arguments_containing(caller.span)
                    .any(|(outer, index)| {
                        !outer.arguments[index].spread
                            && file.ast.peel_ts_sugar_span(outer.arguments[index].span)
                                == caller.span
                            && self.lookup.primitive_at_call(file, outer.span).is_some_and(
                                |primitive| {
                                    self.lookup
                                        .dialect
                                        .callback_semantics_at(
                                            primitive,
                                            index,
                                            outer.arguments.len(),
                                        )
                                        .execution
                                        .is_some()
                                },
                            )
                    })
            });
            let event = caller.is_some_and(|caller| self.exclusive_event_caller(file, caller));
            let non_strict_role = matches!(
                execution,
                ExecutionRole::TrackedJsx
                    | ExecutionRole::DeferredCallback
                    | ExecutionRole::EventCallback
            );
            // JSX-contained tracked expressions, control-flow bodies and
            // events execute in the compiler/dialect-proven scope, not while
            // constructing the outer JSX. Do not use ADR 0201's no-JSX entry
            // gate to decide that these known scopes are unproven.
            let prefix = caller.is_some_and(|caller| {
                crate::interproc::body_site_runs_during_call(file, caller, call.span)
                    || (!caller.r#async
                        && !caller.generator
                        && crate::owners::written_directly_in(&file.ast, caller, call.span)
                        && (control.is_some() || (non_strict_role && (component || event))))
            });
            let admitted =
                component || primitive_callback || control.is_some() || (non_strict_role && event);
            let host_timing = host_callback_timing(file, call.span, execution, self.lookup);
            let callee_timing = callee_callback_timing(file, call.span, execution, self.lookup);
            let timing_proven =
                exact && body_runs && prefix && admitted && !host_timing && !callee_timing;
            if timing_proven && non_strict_role {
                continue;
            }
            let direct = timing_proven && execution.reports_untracked_read();
            if !direct {
                unproven_use = true;
                continue;
            }
            let mut attributed = read.clone();
            attributed.location = location(file.path.shared(), call.span);
            attributed.execution = execution;
            attributed.context =
                read_analysis_context(file, call.span, execution, self.lookup).into();
            attributed.via = file
                .source_text(call.callee)
                .unwrap_or("local helper")
                .into();
            attributed.origin = Some(read.location.clone());
            attributed.origin_context =
                file.source_text(name.span).unwrap_or("local helper").into();
            attributed.summary_attributed = false;
            attributed.missing_jsx_census = missing_jsx_census(file, call.span, execution);
            attributed.host_callback_timing = host_timing;
            attributed.callee_callback_timing = callee_timing;
            result.reads.push(Arc::new(attributed));
            result.strict_read_obligations += 1;
        }
        // An unproven use (an escape, an unproven call) adds no new
        // obligation: the read keeps what the definition site answered before
        // this path existed, and nothing where that gate refused it. A new
        // definition-site obligation for every such helper buried the corpus
        // in derived getters and handler helpers (ADR 0222).
        if unproven_use && definition_admitted {
            let counts = counts_as_strict_read_root(file, site, read.execution, self.lookup);
            result.reads.push(Arc::new(read));
            if counts {
                result.strict_read_obligations += 1;
            }
        }
    }

    /// Initial lazy-cache consumer: no priming/dominance inference and no
    /// violation emission. Every unsupported Get/escape is an obligation.
    fn lazy_getter_obligations(
        &self,
        file: &solid_facts::FileFacts,
        result: &mut LocalAccessResult,
    ) {
        use crate::contracts::LAZY_GETTER_OBJECT;
        use crate::execution_role::{ObserverPresence, observer_presence_at};
        use solid_facts::ast::{BindingShape, IdentifierRole, RuntimeValueKind};
        let allowed = allowed_callback_spans(file, self.lookup);
        let obligation = |span, export: &str, context: &str| crate::StaticDefect {
            kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                callee: export.to_owned(),
                member: None,
            },
            location: crate::location(file.path.shared(), span),
            analysis_context: context.to_owned(),
            fixes: vec![],
            uncertain: true,
        };
        for call in &file.ast.calls {
            if call.result_discarded {
                continue;
            }
            let Some((returned, _)) = self
                .lookup
                .callee_symbol(file, call.callee)
                .and_then(|symbol| self.contract_returns.get(symbol))
            else {
                continue;
            };
            let (recipe, tuple_index) = if returned.kind == LAZY_GETTER_OBJECT {
                (returned, None)
            } else if returned.kind == "tuple" {
                let recipes = returned
                    .elements
                    .iter()
                    .enumerate()
                    .filter_map(|(index, value)| {
                        value
                            .as_ref()
                            .filter(|value| value.kind == LAZY_GETTER_OBJECT)
                            .map(|value| (value, index))
                    })
                    .collect::<Vec<_>>();
                if recipes.len() != 1 {
                    continue;
                }
                (recipes[0].0, Some(recipes[0].1))
            } else {
                continue;
            };
            let export = file.source_text(call.callee).unwrap_or_default();
            let binding = file.ast.bindings.iter().find(|binding| {
                binding.initializer.is_some_and(|initializer| {
                    file.ast.peel_ts_sugar_span(initializer) == call.span
                })
            });
            let root = binding.and_then(|binding| {
                if !binding.immutable {
                    return None;
                }
                match (binding.shape, tuple_index) {
                    (BindingShape::Identifier, None) if binding.names.len() == 1 => {
                        Some(binding.names[0].span)
                    }
                    (BindingShape::Array, Some(index))
                        if binding
                            .array_slots
                            .iter()
                            .enumerate()
                            .all(|(position, slot)| position == index || slot.is_none()) =>
                    {
                        binding
                            .array_slots
                            .get(index)
                            .and_then(Option::as_ref)
                            .map(|slot| slot.span)
                    }
                    _ => None,
                }
            });
            let Some(root) = root else {
                result.dispatch_obligations.push(obligation(call.span, export,
                    "lazy getter result lacks one exact immutable receiver; setter retention, destructuring, wrappers and escape need their own proof"));
                continue;
            };
            let names_root = |span| file.ast.reference_declaration(span) == Some(root);
            let mutated = crate::indexes::binding_written(file, root)
                || file
                    .ast
                    .assignments
                    .iter()
                    .map(|assignment| assignment.target)
                    .chain(file.ast.deleted_targets.iter().copied())
                    .chain(file.ast.iteration_targets.iter().copied())
                    .any(|target| {
                        file.ast.members.iter().any(|member| {
                            target.contains(member.span)
                                && names_root(file.ast.peel_ts_sugar_span(member.object))
                        })
                    });
            let exported = file
                .ast
                .exports
                .iter()
                .filter(|export| !export.type_only)
                .flat_map(|export| export.declarations.iter().chain(&export.specifiers))
                .any(|export| export.local.span == root || names_root(export.local.span));
            // A missing binder answer can only weaken this proof. Spelling
            // is never used to establish a receiver or select a clean Get.
            let unresolved = file.ast.identifiers.iter().any(|identifier| {
                identifier.role == IdentifierRole::Reference
                    && file.ast.reference_declaration(identifier.span).is_none()
                    && file.source_text(identifier.span) == file.source_text(root)
                    && !file.ast.transparent_wrappers.iter().any(|wrapper| {
                        wrapper.span.contains(identifier.span)
                            && !wrapper.inner.contains(identifier.span)
                    })
            });
            let escaped = unresolved
                || exported
                || file.ast.identifiers.iter().any(|identifier| {
                    identifier.role == IdentifierRole::Reference
                        && names_root(identifier.span)
                        && !file.ast.transparent_wrappers.iter().any(|wrapper| {
                            wrapper.span.contains(identifier.span)
                                && !wrapper.inner.contains(identifier.span)
                        })
                        && !file.ast.members.iter().any(|member| {
                            file.ast.peel_ts_sugar_span(member.object) == identifier.span
                        })
                });
            // Key provenance never trusts names or a widened generic type.
            // Dynamic inputs, getters, function values and setters stay open.
            let keys = if let Some(index) = recipe.parameter {
                call.arguments
                    .get(index)
                    .filter(|argument| argument.exact_object_literal && !argument.spread)
                    .and_then(|argument| {
                        let properties = file
                            .ast
                            .object_properties
                            .iter()
                            .filter(|property| argument.property_names.contains(&property.key))
                            .collect::<Vec<_>>();
                        (properties.len() == argument.property_names.len()
                            && properties.iter().all(|property| {
                                property.data
                                    && !property.computed
                                    && !property.runtime_type_escape
                                    && matches!(
                                        property.value_kind,
                                        RuntimeValueKind::Primitive | RuntimeValueKind::Nullish
                                    )
                            }))
                        .then(|| {
                            argument
                                .property_names
                                .iter()
                                .filter_map(|key| file.source_text(*key))
                                .map(str::to_owned)
                                .collect::<Vec<_>>()
                        })
                    })
            } else {
                Some(recipe.properties.keys().cloned().collect::<Vec<_>>())
            };
            let keys = keys.filter(|keys| {
                !keys
                    .iter()
                    .any(|key| crate::contract_semantics::lazy_getter_cache_key_is_reserved(key))
            });
            if mutated || escaped || keys.is_none() {
                result.dispatch_obligations.push(obligation(call.span, export,
                    "lazy getter receiver or initial key values escape the proven static data shape"));
            }
            for member in &file.ast.members {
                let receiver = file.ast.peel_ts_sugar_span(member.object);
                if !names_root(receiver) {
                    continue;
                }
                let execution = semantic_execution_role(
                    file,
                    member.span,
                    &allowed,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                );
                if execution == ExecutionRole::DiscardedRendering {
                    continue;
                }
                let key = file.source_text(member.property).unwrap_or_default();
                let computed = file.ast.computed_members.contains(&member.span);
                // The recipe proves only these own keys. A different named
                // member does not become a reactive getter; in particular,
                // do not duplicate tsc's nonexistent-property diagnostic.
                if !computed
                    && keys
                        .as_ref()
                        .is_some_and(|keys| !keys.iter().any(|candidate| candidate == key))
                {
                    continue;
                }
                let selected = !computed
                    && keys
                        .as_ref()
                        .is_some_and(|keys| keys.iter().any(|candidate| candidate == key));
                let present = observer_presence_at(
                    file,
                    member.span,
                    &allowed,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                ) == ObserverPresence::Present;
                // This operation is the Get, not an alleged call of the
                // returned scalar. Noncallability is TypeScript's claim.
                if !selected || !present || mutated || escaped {
                    result.dispatch_obligations.push(obligation(member.span, export,
                        "lazy per-key Get needs an exact receiver/key and observer; untracked reads require retained-cache priming and dominance proof"));
                }
            }
        }
    }

    /// ADR 0234: a member of a package's returned tuple or object whose
    /// invocation the contract does not describe (`callable`, `unknown`).
    /// Each call of it in a component body, module scope or compiler callback,
    /// and each use that lets it escape (anything but a call or a JSX `on*`
    /// handler value), is a proof obligation. So is a contracted call whose
    /// result holding such a member is not destructured: the member may then
    /// be reached through the value, wherever it goes.
    fn opaque_member_obligations(
        &self,
        file: &solid_facts::FileFacts,
        result: &mut LocalAccessResult,
    ) {
        // ADR 0235: a member with described effects is opaque everywhere a
        // call of it is not bound to those effects.
        let opaque = |returned: &ContractReturn| {
            returned.kind == crate::contracts::OPAQUE_MEMBER
                || returned.kind == crate::contracts::EFFECTFUL_MEMBER
        };
        let effectful =
            |returned: &ContractReturn| returned.kind == crate::contracts::EFFECTFUL_MEMBER;
        let holds_opaque = |returned: &ContractReturn| {
            returned.kind != crate::contracts::LAZY_GETTER_OBJECT
                && (returned.elements.iter().flatten().any(opaque)
                    || returned.properties.values().any(opaque))
        };
        let obligation =
            |span: solid_facts::core::Span, callee: &str, context: &str| crate::StaticDefect {
                kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                    callee: callee.to_owned(),
                    member: None,
                },
                location: location(file.path.shared(), span),
                analysis_context: context.to_owned(),
                fixes: vec![],
                uncertain: true,
            };
        // Declarations of destructured opaque members, with the export.
        // With the export, and whether a direct call of it is bound to
        // described effects (`bind_returned_member_effects`: `const` only).
        let mut members = HashMap::<solid_facts::core::Span, (String, bool)>::new();
        let mut destructured = HashSet::<solid_facts::core::Span>::new();
        for binding in &file.ast.bindings {
            let Some(initializer) = binding.initializer else {
                continue;
            };
            let initializer = file.ast.peel_ts_sugar_span(initializer);
            let Some(call) = file.ast.calls.iter().find(|call| call.span == initializer) else {
                continue;
            };
            let Some((returned, _)) = self
                .lookup
                .callee_symbol(file, call.callee)
                .and_then(|symbol| self.contract_returns.get(symbol))
            else {
                continue;
            };
            if returned.kind == crate::contracts::RETURNED_CALLABLE || !holds_opaque(returned) {
                continue;
            }
            let export = file.source_text(call.callee).unwrap_or_default().to_owned();
            match binding.shape {
                solid_facts::ast::BindingShape::Array if returned.kind == "tuple" => {
                    destructured.insert(call.span);
                    for (slot, member) in binding.array_slots.iter().zip(&returned.elements) {
                        if let (Some(slot), Some(member)) = (slot, member)
                            && opaque(member)
                        {
                            let bound = binding.immutable && effectful(member);
                            members.insert(slot.span, (export.clone(), bound));
                        }
                    }
                }
                solid_facts::ast::BindingShape::Object if returned.kind == "object" => {
                    destructured.insert(call.span);
                    for slot in &binding.object_slots {
                        if let Some(member) = returned.properties.get(slot.property.as_str())
                            && opaque(member)
                        {
                            let bound = binding.immutable && effectful(member);
                            members.insert(slot.local.span, (export.clone(), bound));
                        }
                    }
                }
                _ => {}
            }
        }
        // A result holding an opaque member that is neither destructured nor
        // discarded: the member travels with the value. Bound to one name
        // that never escapes and has no alias, only the member accesses
        // through that name can reach it, and each one selecting an opaque
        // member is the obligation instead.
        for call in &file.ast.calls {
            if call.result_discarded || destructured.contains(&call.span) {
                continue;
            }
            let Some((returned, _)) = self
                .lookup
                .callee_symbol(file, call.callee)
                .and_then(|symbol| self.contract_returns.get(symbol))
            else {
                continue;
            };
            if returned.kind == crate::contracts::RETURNED_CALLABLE || !holds_opaque(returned) {
                continue;
            }
            let export = file.source_text(call.callee).unwrap_or_default();
            // ADR 0250: references are matched by the binder's declaration,
            // not by TypeFacts entities. An entity exists only where the
            // analysis demanded one, so a reference without one (an array
            // element, a shorthand property) would drop out of the escape
            // test and leave an escaping value looking member-only.
            let root = file
                .ast
                .bindings
                .iter()
                .find(|binding| {
                    binding.shape == solid_facts::ast::BindingShape::Identifier
                        && binding.names.len() == 1
                        && binding
                            .initializer
                            .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == call.span)
                })
                .map(|binding| binding.names[0].span);
            let refers_to = |span: solid_facts::core::Span, root: solid_facts::core::Span| {
                file.ast.reference_declaration(span) == Some(root)
            };
            let aliased = |root: solid_facts::core::Span| {
                file.ast.bindings.iter().any(|binding| {
                    binding
                        .initializer_identifier
                        .as_ref()
                        .is_some_and(|initializer| refers_to(initializer.span, root))
                })
            };
            // Every reference to the name is a direct member receiver: the
            // value itself goes nowhere, so a member is reached only through
            // the accesses enumerated below.
            let only_members = |root: solid_facts::core::Span| {
                file.ast
                    .identifiers
                    .iter()
                    .filter(|id| id.role == solid_facts::ast::IdentifierRole::Reference)
                    .filter(|id| refers_to(id.span, root))
                    .all(|id| {
                        file.ast
                            .members
                            .iter()
                            .any(|member| member.object == id.span)
                    })
            };
            let tracked_root = root.filter(|root| {
                !aliased(*root)
                    && only_members(*root)
                    && (returned.kind != "object"
                        || crate::contracts::returned_object_members_are_stable(
                            file, *root, returned,
                        ))
            });
            let Some(root) = tracked_root else {
                result.dispatch_obligations.push(obligation(
                    call.span,
                    export,
                    "the returned value holds a function its package contract does not describe, and it is not destructured here",
                ));
                continue;
            };
            for member in &file.ast.members {
                if !refers_to(member.object, root) {
                    continue;
                }
                let computed = file.ast.computed_members.contains(&member.span);
                let key = file.source_text(member.property).unwrap_or_default();
                let selected = match returned.kind.as_str() {
                    "tuple" if computed => key
                        .parse::<usize>()
                        .ok()
                        .and_then(|index| returned.elements.get(index))
                        .and_then(Option::as_ref),
                    "object" if !computed => returned.properties.get(key),
                    _ => None,
                };
                if !selected.is_some_and(opaque) {
                    continue;
                }
                if selected.is_some_and(effectful)
                    && file.ast.calls.iter().any(|call| {
                        call.callee == member.span
                            && self.lookup.contract_member_call_is_bound(file, call.callee)
                    })
                {
                    continue;
                }
                let called = file
                    .ast
                    .calls
                    .iter()
                    .any(|call| file.ast.peel_ts_sugar_span(call.callee) == member.span);
                if !called || !inside_non_component_function(file, member.span, self.lookup) {
                    result.dispatch_obligations.push(obligation(
                        member.span,
                        export,
                        "reaches a returned function whose behavior its package contract does not describe",
                    ));
                }
            }
        }
        if members.is_empty() {
            return;
        }
        let callees = file
            .ast
            .calls
            .iter()
            .map(|call| (file.ast.peel_ts_sugar_span(call.callee), call.span))
            .collect::<HashMap<_, _>>();
        let handlers = file
            .ast
            .jsx_elements
            .iter()
            .flat_map(|element| &element.attributes)
            .filter(|attribute| {
                file.source_text(attribute.local_name)
                    .is_some_and(|name| name.starts_with("on"))
            })
            .filter_map(|attribute| attribute.expression)
            .map(|expression| file.ast.peel_ts_sugar_span(expression))
            .collect::<HashSet<_>>();
        for identifier in &file.ast.identifiers {
            if identifier.role != solid_facts::ast::IdentifierRole::Reference {
                continue;
            }
            let Some((export, bound)) = file
                .ast
                .reference_declaration(identifier.span)
                .and_then(|declaration| members.get(&declaration))
            else {
                continue;
            };
            if let Some(call) = callees.get(&identifier.span) {
                // ADR 0250: the effects bind to a call written on the name
                // itself. `(start as T)()` is peeled to the name here but is
                // not instantiated, so it stays an obligation.
                let wrapped = !file.ast.calls.iter().any(|candidate| {
                    candidate.span == *call && candidate.callee == identifier.span
                });
                let bound = *bound && !wrapped;
                if !bound && !inside_non_component_function(file, identifier.span, self.lookup) {
                    result.dispatch_obligations.push(obligation(
                        *call,
                        export,
                        "calls a returned function whose behavior its package contract does not describe",
                    ));
                }
            } else if !handlers.contains(&identifier.span) {
                result.dispatch_obligations.push(obligation(
                    identifier.span,
                    export,
                    "passes on a returned function whose behavior its package contract does not describe",
                ));
            }
        }
    }

    /// A closed factory return enumerates a function, never permission to
    /// call or pass it on. Successful bindings alone discharge direct calls;
    /// no graph, mutable/unresolved bindings and every escape stay obligations.
    fn returned_callable_obligations(
        &self,
        file: &solid_facts::FileFacts,
        result: &mut LocalAccessResult,
    ) {
        let obligation = |span, callee: &str, context: &str| crate::StaticDefect {
            kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                callee: callee.to_owned(),
                member: None,
            },
            location: location(file.path.shared(), span),
            analysis_context: context.to_owned(),
            fixes: vec![],
            uncertain: true,
        };
        let handlers = file
            .ast
            .jsx_elements
            .iter()
            .flat_map(|element| &element.attributes)
            .filter(|attribute| {
                file.source_text(attribute.local_name)
                    .is_some_and(|name| name.starts_with("on"))
            })
            .filter_map(|attribute| attribute.expression)
            .map(|span| file.ast.peel_ts_sugar_span(span))
            .collect::<HashSet<_>>();
        for factory in &file.ast.calls {
            let has_captures = self
                .lookup
                .callee_symbol(file, factory.callee)
                .and_then(|symbol| self.lookup.returned_capture_sources(symbol))
                .is_some_and(|sources| !sources.is_empty());
            if let Some(sources) = self
                .lookup
                .callee_symbol(file, factory.callee)
                .and_then(|symbol| self.lookup.returned_capture_sources(symbol))
            {
                for source in sources.values() {
                    let crate::contract_semantics::ValueSource::Parameter { index, path } = source
                    else {
                        continue;
                    };
                    if !path.is_empty() {
                        continue;
                    }
                    let Some(argument) = factory.arguments.get(usize::from(*index)) else {
                        continue;
                    };
                    for escape in crate::contracts::capture_argument_escapes(
                        file,
                        self.entities,
                        factory,
                        argument,
                    ) {
                        result.dispatch_obligations.push(obligation(
                            escape,
                            file.source_text(factory.callee).unwrap_or_default(),
                            "a captured caller value escapes or has no exact immutable identity",
                        ));
                    }
                }
            }
            let Some((returned, _)) = self
                .lookup
                .callee_symbol(file, factory.callee)
                .and_then(|symbol| self.contract_returns.get(symbol))
            else {
                continue;
            };
            if returned.kind != crate::contracts::RETURNED_CALLABLE || factory.result_discarded {
                continue;
            }
            let export = file.source_text(factory.callee).unwrap_or_default();
            let binding = file.ast.bindings.iter().find(|binding| {
                binding
                    .initializer
                    .is_some_and(|value| file.ast.peel_ts_sugar_span(value) == factory.span)
                    && binding.shape == solid_facts::ast::BindingShape::Identifier
                    && binding.names.len() == 1
            });
            let root = binding
                .and_then(|binding| self.entities.at(file.path.as_str(), binding.names[0].span));
            let Some((binding, root)) = binding.zip(root) else {
                result.dispatch_obligations.push(obligation(
                    factory.span,
                    export,
                    "the returned function is used without an exact immutable binding",
                ));
                continue;
            };
            if has_captures && binding.initializer != Some(factory.span) {
                result.dispatch_obligations.push(obligation(
                    factory.span,
                    export,
                    "a wrapped returned function has no exact capture instance binding",
                ));
            }
            if !binding.immutable
                || crate::value_identity::binding_has_write(file, self.entities, root)
            {
                result.dispatch_obligations.push(obligation(
                    factory.span,
                    export,
                    "a mutable returned function binding cannot establish its later dispatch",
                ));
            }
            // Export declarations need no Reference identifier in the AST.
            for exported in file
                .ast
                .exports
                .iter()
                .filter(|export| !export.type_only)
                .flat_map(|export| export.declarations.iter().chain(&export.specifiers))
                .filter(|export| !export.type_only)
            {
                if self.entities.at(file.path.as_str(), exported.local.span) == Some(root) {
                    result.dispatch_obligations.push(obligation(
                        exported.local.span,
                        export,
                        "exports a returned function whose future dispatch is not established",
                    ));
                }
            }
            // `{ f }` stores the function in an object. The binder records the
            // exact declaration a shorthand refers to; that is an escape.
            for property in file
                .ast
                .object_properties
                .iter()
                .filter(|property| property.shorthand_binding == Some(binding.names[0].span))
            {
                result.dispatch_obligations.push(obligation(
                    property.span,
                    export,
                    "passes on a returned function whose future dispatch is not established",
                ));
            }
            // Fail closed on references TypeFacts emitted no entity for (an
            // array element, a shorthand property): a reference spelled like
            // the binding inside the binding's scope, with no resolved
            // symbol, may be this function escaping. The spelling only ever
            // adds an obligation here; it never discharges one.
            let name = file.source_text(binding.names[0].span).unwrap_or_default();
            let scope = file
                .ast
                .functions
                .iter()
                .filter(|function| function.body.contains(binding.names[0].span))
                .min_by_key(|function| function.body.end - function.body.start)
                .map(|function| function.body);
            for id in file
                .ast
                .identifiers
                .iter()
                .filter(|id| id.role == solid_facts::ast::IdentifierRole::Reference)
                .filter(|id| match self.entities.at(file.path.as_str(), id.span) {
                    Some(symbol) => symbol == root,
                    None => {
                        !name.is_empty()
                            && file.source_text(id.span) == Some(name)
                            && scope.is_none_or(|scope| scope.contains(id.span))
                    }
                })
            {
                let resolved = self.entities.at(file.path.as_str(), id.span).is_some();
                if let Some(call) = file
                    .ast
                    .calls
                    .iter()
                    .find(|call| file.ast.peel_ts_sugar_span(call.callee) == id.span)
                {
                    // A bound call is discharged only where the graph is
                    // instantiated: the callee must resolve to this exact
                    // binding, wrappers included.
                    let instantiated = resolved
                        && self.lookup.returned_callable_is_bound(root.as_str())
                        && self.lookup.callee_symbol(file, call.callee) == Some(root.as_str());
                    if !instantiated {
                        result.dispatch_obligations.push(obligation(
                            call.span,
                            export,
                            "calls a returned function without a stated and bound call graph",
                        ));
                    }
                    continue;
                }
                if let Some(member) = file
                    .ast
                    .members
                    .iter()
                    .find(|member| file.ast.peel_ts_sugar_span(member.object) == id.span)
                {
                    let computed = file.ast.computed_members.contains(&member.span);
                    let selected = (!computed)
                        .then(|| file.source_text(member.property))
                        .flatten()
                        .and_then(|key| returned.properties.get(key));
                    let called = file
                        .ast
                        .calls
                        .iter()
                        .any(|call| file.ast.peel_ts_sugar_span(call.callee) == member.span);
                    if called && self.lookup.returned_member_is_bound(file, member.span) {
                        continue;
                    }
                    // Reactive accessor calls also require a successful
                    // instance-local binding. Unbound uses keep ADR 0234's
                    // nested-function and event-handler exemptions.
                    if !has_captures
                        && selected.is_some()
                        && ((called
                            && inside_non_component_function(file, member.span, self.lookup))
                            || (!called && handlers.contains(&member.span)))
                    {
                        continue;
                    }
                    result.dispatch_obligations.push(obligation(
                        member.span,
                        export,
                        "reaches a function-object member without established dispatch",
                    ));
                    continue;
                }
                // Match ADR 0234 for passing the function as an on* value.
                if has_captures || !handlers.contains(&id.span) {
                    result.dispatch_obligations.push(obligation(
                        id.span,
                        export,
                        "passes on a returned function whose future dispatch is not established",
                    ));
                }
            }
        }
    }

    fn callback_result_obligations(
        &self,
        file: &solid_facts::FileFacts,
        result: &mut LocalAccessResult,
    ) {
        for call in &file.ast.calls {
            if self
                .lookup
                .resolved_callee_call(file, call.callee)
                .is_some_and(|resolved| {
                    resolved.validity == typefacts::ResolvedCallValidity::Recovery
                })
            {
                // TypeScript already owns the invalid-call diagnostic.
                continue;
            }
            let Some(results) = self
                .lookup
                .callee_symbol(file, call.callee)
                .and_then(|symbol| self.lookup.contract_callback_results(symbol))
            else {
                continue;
            };
            for produced in results {
                let clean = call
                    .arguments
                    .get(produced.parameter)
                    .is_some_and(|argument| {
                        callback_result_is_plain_data(file, argument, produced)
                            || (self.lookup.callee_symbol(file, call.callee).is_some_and(
                                |symbol| self.lookup.contract_result_census_is_closed(symbol),
                            ) && callback_result_literal_is_tracked(
                                file, argument, produced, true,
                            ))
                    });
                if clean {
                    continue;
                }
                result.dispatch_obligations.push(crate::StaticDefect {
                    kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                        callee: file.source_text(call.callee).unwrap_or_default().to_owned(), member: None,
                    },
                    location: location(file.path.shared(), call.span),
                    analysis_context: format!("the value returned by callback argument {} has result uses whose exact target, shape or execution is not established",
                        produced.parameter + 1),
                    fixes: vec![], uncertain: true,
                });
            }
        }
    }

    pub(crate) fn discover(&self, file: &solid_facts::FileFacts) -> LocalAccessResult {
        let mut result = LocalAccessResult::default();
        self.lazy_getter_obligations(file, &mut result);
        self.opaque_member_obligations(file, &mut result);
        self.returned_callable_obligations(file, &mut result);
        self.callback_result_obligations(file, &mut result);
        let mut seen = HashSet::new();
        let allowed = allowed_callback_spans(file, self.lookup);
        let capture_bodies = self.lookup.bound_capture_literal_bodies(file);
        for call in &file.ast.calls {
            if let Some(captures) = self
                .lookup
                .callee_symbol(file, call.callee)
                .and_then(|symbol| self.lookup.captured_arguments(symbol))
            {
                for argument in captures.values() {
                    let literal = file
                        .ast
                        .functions
                        .iter()
                        .any(|function| function.span == argument.span);
                    let symbol = self.entities.at(file.path.as_str(), argument.span);
                    let known = literal
                        || symbol.is_some_and(|symbol| {
                            self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Accessor)
                                || self.lookup.function_for_symbol(symbol).is_some_and(
                                    |(source, function)| {
                                        self.lookup.function_value_is_current(source, function)
                                    },
                                )
                        });
                    if !known {
                        result.dispatch_obligations.push(crate::StaticDefect {
                            kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                                callee: file
                                    .source_text(call.callee)
                                    .unwrap_or_default()
                                    .to_owned(),
                                member: None,
                            },
                            location: crate::location(file.path.shared(), call.span),
                            analysis_context:
                                "the captured callable's implementation is not established".into(),
                            fixes: vec![],
                            uncertain: true,
                        });
                    }
                }
            }
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
                    || enclosing_render_function(file, call.callee, self.lookup);
                let key = (callee.path.clone(), callee.start_byte, callee.end_byte);
                if reachable && seen.insert(key) {
                    result.reads.push(Arc::new(ReactiveRead {
                        package_internal: false,
                        summary_attributed: false,
                        kind: returned.kind.into(),
                        accessor: returned.label.into(),
                        location: location(file.path.shared(), call.span),
                        declaration: declaration.clone(),
                        execution,
                        context: read_analysis_context(file, call.span, execution, self.lookup)
                            .into(),
                        via: file
                            .source_text(factory.callee)
                            .unwrap_or_default()
                            .to_owned()
                            .into(),
                        origin: Some(declaration.clone()),
                        origin_context: Arc::from("package return contract"),
                        uncertain: self.lookup.inside_possible_component(file, call.span),
                        missing_jsx_census: missing_jsx_census(file, call.span, execution),
                        host_callback_timing: host_callback_timing(
                            file,
                            call.span,
                            execution,
                            self.lookup,
                        ),
                        project_consumer_non_strict: false,
                        callback_invocation_unproven: false,
                        callee_callback_timing: callee_callback_timing(
                            file,
                            call.span,
                            execution,
                            self.lookup,
                        ),
                    }));
                    if counts_as_strict_read_root(file, call.span, execution, self.lookup) {
                        result.strict_read_obligations += 1;
                    }
                }
            }
            let Some(symbol) = self.lookup.callee_symbol(file, call.callee) else {
                continue;
            };
            if (self.accessors.contains_key(symbol)
                && capture_bodies.iter().any(|body| body.contains(call.span)))
                || inside_known_value_function_argument(file, call.callee, self.lookup)
            {
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
            // A universal callback-context proof can certify an accessor
            // read even when invocation is optional or resource-triggered.
            // Keep write/action and owner classification on their existing
            // execution proofs; this override applies only to known accessors.
            let execution = if self.accessors.contains_key(symbol) {
                contract_tracked_accessor_read_role(file, call.callee, self.lookup)
                    .unwrap_or(execution)
            } else {
                execution
            };
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
                            || enclosing_render_function(file, call.callee, self.lookup))))
                .then_some(1)
            }) else {
                continue;
            };
            let key = (callee.path.clone(), callee.start_byte, callee.end_byte);
            if let Some((name, declaration)) = self.accessors.get(symbol)
                && (!inside_non_component_function(file, call.callee, self.lookup)
                    || named_callback_execution_role(file, call.callee, self.lookup).is_some())
                && seen.insert(key.clone())
            {
                let origin = self.accessor_origins.get(symbol);
                let display_name = call.static_callee(&file.source).unwrap_or(name);
                result.reads.push(Arc::new(ReactiveRead {
                    package_internal: false,
                    summary_attributed: false,
                    kind: "accessor".into(),
                    accessor: origin
                        .map_or_else(|| display_name.to_string(), |origin| origin.0.to_string())
                        .into(),
                    location: location(file.path.shared(), call.span),
                    declaration: origin
                        .map_or_else(|| declaration.clone(), |origin| origin.2.clone()),
                    execution,
                    context: read_analysis_context(file, call.span, execution, self.lookup).into(),
                    via: origin.map_or_else(String::new, |_| name.to_string()).into(),
                    origin: origin.map(|origin| origin.2.clone()),
                    origin_context: origin
                        .map_or_else(String::new, |origin| origin.1.to_string())
                        .into(),
                    uncertain: self.lookup.inside_possible_component(file, call.span),
                    missing_jsx_census: missing_jsx_census(file, call.span, execution),
                    host_callback_timing: host_callback_timing(
                        file,
                        call.span,
                        execution,
                        self.lookup,
                    ),
                    project_consumer_non_strict: false,
                    callback_invocation_unproven: false,
                    callee_callback_timing: callee_callback_timing(
                        file,
                        call.span,
                        execution,
                        self.lookup,
                    ),
                }));
                if counts_as_strict_read_root(file, call.span, execution, self.lookup) {
                    result.strict_read_obligations += 1;
                }
                let async_provenance = self.async_sources.contains(symbol);
                let async_options = self.effective_async_options(symbol);
                if (async_provenance
                    || async_options.ssr_client_bare
                    || async_options.server_rendering_unresolved)
                    && !pending_accessor_probe(file, call.callee, self.lookup)
                {
                    let async_execution = async_execution_role(file, call.callee, execution);
                    let async_execution = if async_options.ssr_client_bare
                        || async_options.server_rendering_unresolved
                    {
                        async_execution
                    } else {
                        async_read_role(
                            file,
                            call.callee,
                            async_execution,
                            self.entities,
                            self.symbol_names,
                            self.lookup,
                        )
                    };
                    let cover =
                        read_loading_cover(self.lookup, file, call.callee, self.symbol_names);
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
                        under_loading: cover == LoadingCover::Covered,
                        mount_unresolved: cover == LoadingCover::Unresolved,
                        async_provenance,
                        declared_loading: async_options.declared_loading,
                        options_opaque: async_options.opaque,
                        ssr_client_hole: async_options.ssr_client_bare,
                        server_rendering_unresolved: async_options.server_rendering_unresolved,
                        host_callback_timing: host_callback_timing(
                            file,
                            call.callee,
                            async_execution,
                            self.lookup,
                        ),
                        callee_callback_timing: callee_callback_timing(
                            file,
                            call.callee,
                            async_execution,
                            self.lookup,
                        ),
                        invocation_context_unproven: async_execution
                            == ExecutionRole::UntrackedRendering
                            && !nested_literal_runs_during_body(
                                file,
                                call.callee,
                                self.entities,
                                self.symbol_names,
                                self.lookup,
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
                let declaration = solid_accessor_declaration(descriptor, self.lookup.dialect)
                    .map_or_else(
                        || callee.clone(),
                        |declaration| declaration.location.clone(),
                    );
                result.reads.push(Arc::new(ReactiveRead {
                    package_internal: false,
                    summary_attributed: false,
                    kind: "accessor".into(),
                    accessor: display.into(),
                    location: location(file.path.shared(), call.span),
                    declaration,
                    execution,
                    context: read_analysis_context(file, call.span, execution, self.lookup).into(),
                    via: Arc::from(""),
                    origin: None,
                    origin_context: Arc::from(""),
                    uncertain: self.lookup.inside_possible_component(file, call.span),
                    missing_jsx_census: missing_jsx_census(file, call.span, execution),
                    host_callback_timing: host_callback_timing(
                        file,
                        call.span,
                        execution,
                        self.lookup,
                    ),
                    project_consumer_non_strict: false,
                    callback_invocation_unproven: false,
                    callee_callback_timing: callee_callback_timing(
                        file,
                        call.span,
                        execution,
                        self.lookup,
                    ),
                }));
                result.strict_read_obligations += 1;
            }
            if let Some(contracted) = self.contract_reads.get(symbol)
                && !inside_non_component_function(file, call.callee, self.lookup)
            {
                for (index, (name, via, declaration, kind, read_context)) in
                    contracted.iter().enumerate()
                {
                    let (execution, read_unproven) = read_context
                        .as_ref()
                        .map_or((execution, false), |context| context.at_call(execution));
                    let contract_key = (
                        callee.path.clone(),
                        callee.start_byte,
                        callee
                            .end_byte
                            .saturating_add(u64::try_from(index).unwrap_or(u64::MAX)),
                    );
                    if seen.insert(contract_key) {
                        result.reads.push(Arc::new(ReactiveRead {
                            // The export reading state its contract declares
                            // as its own, not a value this call passes in.
                            package_internal: crate::contract_declared_state(declaration),
                            summary_attributed: false,
                            kind: kind.clone().into(),
                            accessor: name.clone().into(),
                            location: location(file.path.shared(), call.span),
                            declaration: declaration.clone(),
                            execution,
                            context: read_analysis_context(file, call.span, execution, self.lookup)
                                .into(),
                            via: via.clone().into(),
                            origin: Some(declaration.clone()),
                            origin_context: via.clone().into(),
                            uncertain: read_unproven
                                || self.lookup.inside_possible_component(file, call.span),
                            missing_jsx_census: missing_jsx_census(file, call.span, execution),
                            host_callback_timing: host_callback_timing(
                                file,
                                call.span,
                                execution,
                                self.lookup,
                            ),
                            project_consumer_non_strict: false,
                            callback_invocation_unproven: false,
                            callee_callback_timing: callee_callback_timing(
                                file,
                                call.span,
                                execution,
                                self.lookup,
                            ),
                        }));
                        if counts_as_strict_read_root(file, call.span, execution, self.lookup) {
                            result.strict_read_obligations += 1;
                        }
                    }
                }
            }
            if let Some(contracted) = self.contract_parameter_reads.get(symbol)
                && !inside_non_component_function(file, call.callee, self.lookup)
            {
                for (parameter, name, via, declaration, read_context) in contracted {
                    let (execution, read_unproven) = read_context
                        .as_ref()
                        .map_or((execution, false), |context| context.at_call(execution));
                    let Some(argument) = call.arguments.get(*parameter) else {
                        // ADR 0232: with no argument at that position and no
                        // spread that could supply one, the caller passed no
                        // value whose member the package could read.
                        if !call.arguments.iter().any(|argument| argument.spread) {
                            continue;
                        }
                        result.dispatch_obligations.push(crate::StaticDefect {
                            kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                                callee: name.clone(),
                                member: None,
                            },
                            location: location(file.path.shared(), call.span),
                            analysis_context: format!(
                                "package contract parameter-member read requires argument {parameter}, but the call has no exact argument at that position"
                            ),
                            fixes: vec![],
                            uncertain: true,
                        });
                        continue;
                    };
                    let argument_location = location(file.path.shared(), argument.span);
                    let reactive_symbol = self.entities.get(&argument_location).filter(|symbol| {
                        self.source_kinds.get(symbol.as_str()) == Some(&ReactiveSourceKind::Store)
                    });
                    if let Some(reactive_symbol) = reactive_symbol {
                        result.reads.push(Arc::new(ReactiveRead {
                            package_internal: false,
                            summary_attributed: false,
                            kind: "store-path".into(),
                            accessor: file
                                .source_text(argument.span)
                                .unwrap_or(reactive_symbol.as_str())
                                .to_string()
                                .into(),
                            location: location(file.path.shared(), call.span),
                            declaration: self
                                .accessors
                                .get(reactive_symbol.as_str())
                                .map(|(_, location)| location.clone())
                                .unwrap_or_else(|| declaration.clone()),
                            execution,
                            context: read_analysis_context(file, call.span, execution, self.lookup)
                                .into(),
                            via: via.clone().into(),
                            origin: Some(declaration.clone()),
                            origin_context: via.clone().into(),
                            uncertain: read_unproven
                                || self.lookup.inside_possible_component(file, call.span),
                            missing_jsx_census: missing_jsx_census(file, call.span, execution),
                            host_callback_timing: host_callback_timing(
                                file,
                                call.span,
                                execution,
                                self.lookup,
                            ),
                            project_consumer_non_strict: false,
                            callback_invocation_unproven: false,
                            callee_callback_timing: callee_callback_timing(
                                file,
                                call.span,
                                execution,
                                self.lookup,
                            ),
                        }));
                        if counts_as_strict_read_root(file, call.span, execution, self.lookup) {
                            result.strict_read_obligations += 1;
                        }
                    } else if !argument_proves_non_reactive(
                        file,
                        argument,
                        self.entities,
                        self.source_kinds,
                        self.lookup,
                    ) {
                        result.dispatch_obligations.push(crate::StaticDefect {
                            kind: crate::StaticDefectKind::ReactiveDispatchUnresolved {
                                callee: name.clone(),
                                member: None,
                            },
                            location: location(file.path.shared(), call.span),
                            analysis_context: format!(
                                "argument {parameter} to {name} is neither a proven reactive store nor proven plain data"
                            ),
                            fixes: vec![],
                            uncertain: true,
                        });
                    }
                }
            }
            if let Some((name, declaration, allowed_by_option, source_kind)) =
                self.setters.get(symbol)
            {
                // Under a root owner `setSignal`'s guard throws on every
                // release; a store setter's only where the resolved signals
                // dropped the root exemption (rc.9). In dev a component body
                // runs under a root too, so the same answer covers it.
                let root_body = match source_kind {
                    ReactiveSourceKind::Accessor => RootBodyGuard::Rejects,
                    ReactiveSourceKind::Store
                        if !self.lookup.dialect.store_setter_guard_exempts_roots() =>
                    {
                        RootBodyGuard::Rejects
                    }
                    ReactiveSourceKind::Store => RootBodyGuard::Exempts,
                };
                let mut write_execution = semantic_write_execution_role(
                    file,
                    call.callee,
                    &allowed,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                    root_body,
                );
                // Where the resolved release's optimistic-store setter meets no
                // owned-scope guard (rc.0), a store setter is reported only when
                // it is proven to be `createStore`'s, whose writes do reach the
                // guard. An optimistic-store setter is legal in every owned
                // scope, and a store setter found only by its type could be
                // either, so both take the role that reports nothing, as an
                // exempted root-body write does: the second is a miss, never a
                // violation the runtime does not raise.
                if write_execution.reports_disallowed_write()
                    && *source_kind == ReactiveSourceKind::Store
                    && !self.lookup.dialect.optimistic_store_setter_guarded()
                    && self
                        .source_primitives
                        .get(symbol)
                        .and_then(|primitive| self.lookup.dialect.primitive(primitive))
                        != Some(solid_dialect::Primitive::CreateStore)
                {
                    write_execution = ExecutionRole::UntrackedCallback;
                }
                for _ in 0..multiplicity {
                    result.writes.push(Arc::new(ReactiveWrite {
                        setter: name.to_string().into(),
                        operation: crate::ReactiveWriteOperation::Setter,
                        source_kind: *source_kind,
                        location: location(file.path.shared(), call.span),
                        declaration: declaration.clone(),
                        execution: write_execution,
                        allowed_by_option: *allowed_by_option,
                        context: analysis_context(
                            file,
                            call.span,
                            self.entities,
                            self.symbol_names,
                            self.lookup.dialect,
                            self.lookup,
                        )
                        .into(),
                    }));
                }
            }
            if let Some((name, declaration)) = self.actions.get(symbol) {
                // An action call's guard has no root exemption on any release:
                // it throws `ACTION_CALLED_IN_OWNED_SCOPE` directly in a
                // `createRoot` body, and in a memo nested in one (probed
                // rc.0-rc.9, dev).
                let action_execution = semantic_write_execution_role(
                    file,
                    call.callee,
                    &allowed,
                    self.entities,
                    self.symbol_names,
                    self.lookup,
                    RootBodyGuard::Rejects,
                );
                for _ in 0..multiplicity {
                    result.action_invocations.push(Arc::new(ActionInvocation {
                        action: name.to_string().into(),
                        location: location(file.path.shared(), call.span),
                        declaration: declaration.clone(),
                        execution: action_execution,
                        context: analysis_context(
                            file,
                            call.span,
                            self.entities,
                            self.symbol_names,
                            self.lookup.dialect,
                            self.lookup,
                        )
                        .into(),
                    }));
                }
            }
        }
        for member in &file.ast.members {
            // A member that is another member's object is read as part of the
            // longer path, which is reported instead -- except a prop read at
            // the head of a chain (`props.snapshot` in
            // `props.snapshot?.messages[i]`): the getter that runs is that
            // prop's, and the longer path's object resolves to no source.
            let receiver = file.ast.peel_ts_sugar_span(member.object);
            let receiver_is_identifier = file.ast.identifiers.iter().any(|identifier| {
                identifier.span == receiver
                    && identifier.role == solid_facts::ast::IdentifierRole::Reference
            });
            let prop_head = receiver_is_identifier
                && self
                    .entities
                    .get(&location(
                        file.path.shared(),
                        file.ast.peel_ts_sugar_span(member.object),
                    ))
                    .is_some_and(|symbol| {
                        self.prop_sources.contains_key(symbol)
                            && self.source_kinds.get(symbol) != Some(&ReactiveSourceKind::Store)
                    });
            if !prop_head
                && file
                    .ast
                    .members
                    .iter()
                    .any(|candidate| file.ast.peel_ts_sugar_span(candidate.object) == member.span)
            {
                continue;
            }
            // A plain assignment writes this exact member without reading its
            // old value. Compound assignments, updates, and members nested
            // inside a target (a computed key, a destructuring default) are
            // retained as reads.
            if file.ast.is_plain_assignment_target(member.span)
                || file
                    .ast
                    .deleted_targets
                    .iter()
                    .any(|target| file.ast.peel_ts_sugar_span(*target) == member.span)
                || file
                    .ast
                    .iteration_targets
                    .iter()
                    .any(|target| file.ast.peel_ts_sugar_span(*target) == member.span)
            {
                continue;
            }
            let object = location(
                file.path.shared(),
                if receiver_is_identifier {
                    receiver
                } else {
                    member.object
                },
            );
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
            let helper = self
                .prop_sources
                .get(symbol)
                .filter(|_| self.source_kinds.get(symbol) != Some(&ReactiveSourceKind::Store))
                .and_then(|_| self.component_prop_helper(file, member.span));
            let gate_refuses = (inside_non_component_function(file, member.span, self.lookup)
                || inside_unclassified_callback(file, member.span))
                && named_callback_execution_role(file, member.span, self.lookup).is_none()
                // This literal is the exact dialect-owned children callback,
                // and this site is in its own body (nested defaults excluded).
                && direct_control_flow_body_role(
                    file,
                    member.span,
                    self.entities,
                    self.symbol_names,
                    self.lookup.dialect,
                ).is_none()
                && !matches!(
                    execution,
                    ExecutionRole::EffectApply | ExecutionRole::UntrackedCallback
                );
            if helper.is_none() && gate_refuses {
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
            // A dynamic first key is not the prop bearing that key's source
            // spelling. Literal keys use the parser's cooked property name.
            let property = if file.ast.computed_members.contains(&member.span) {
                file.ast
                    .literal_computed_members
                    .iter()
                    .find(|key| key.span == member.span)
                    .map(|key| key.key.as_str())
            } else {
                file.source_text(member.property)
            };
            // A component's `ref` prop is an imperative output channel: the
            // child calls it once to publish its handle. This is not a
            // reactive read, but only when the complete member expression is
            // the direct callee. Reading or aliasing `props.ref` still flows
            // through strict-read analysis like every other prop.
            if self.prop_sources.contains_key(symbol)
                && property == Some("ref")
                && file
                    .ast
                    .calls
                    .iter()
                    .any(|call| file.ast.peel_ts_sugar_span(call.callee) == member.span)
            {
                continue;
            }
            // Caller-proven props: a prop every call site passes statically
            // compiles to a plain property — reading it is not a reactive
            // read at all. Unprovable backing stays a proof obligation.
            let mut uncertain = self.uncertain_prop_sources.contains(symbol)
                || self.lookup.inside_possible_component(file, member.span);
            if self.source_kinds.get(symbol) != Some(&ReactiveSourceKind::Store)
                && let Some((_, prop_declaration)) = self.prop_sources.get(symbol)
            {
                // A view/alias can have different keys from the original
                // parameter. ADR 0216's classification is for that parameter,
                // not permission to certify a merge view's local head Get.
                let exact_root = self.entities.get(prop_declaration) == Some(symbol)
                    && props_root_is_current(file, prop_declaration, symbol, self.entities)
                    && !self.prop_sources.iter().any(|(other, (_, declaration))| {
                        other != symbol && declaration == prop_declaration
                    });
                let backing = if exact_root {
                    property.map_or(PropUse::Unknown, |property| {
                        self.props_reactivity.prop_use(prop_declaration, property)
                    })
                } else {
                    PropUse::Unknown
                };
                match backing {
                    PropUse::Static => continue,
                    PropUse::Reactive => {}
                    PropUse::Unknown => uncertain = true,
                }
            }
            let key = (object.path.clone(), object.start_byte, object.end_byte);
            if !seen.insert(key) {
                continue;
            }
            let accessor = if prop_head {
                // Name the Get we proved, including brackets/optional syntax,
                // rather than a suffix's property or an invented dotted key.
                file.source_text(member.span)
                    .unwrap_or(name.as_str())
                    .to_owned()
            } else {
                usize::try_from(member.span.start)
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
                    })
            };
            let read = ReactiveRead {
                package_internal: false,
                summary_attributed: false,
                kind: if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
                    "store-path".into()
                } else {
                    "component-props".into()
                },
                accessor: accessor.into(),
                location: location(file.path.shared(), member.span),
                declaration: declaration.clone(),
                execution,
                context: read_analysis_context(file, member.span, execution, self.lookup).into(),
                via: Arc::from(""),
                origin: None,
                origin_context: Arc::from(""),
                uncertain,
                missing_jsx_census: missing_jsx_census(file, member.span, execution),
                host_callback_timing: host_callback_timing(
                    file,
                    member.span,
                    execution,
                    self.lookup,
                ),
                project_consumer_non_strict: false,
                callback_invocation_unproven: false,
                callee_callback_timing: callee_callback_timing(
                    file,
                    member.span,
                    execution,
                    self.lookup,
                ),
            };
            if let Some(helper) = helper {
                self.project_helper_prop_read(
                    file,
                    helper,
                    member.span,
                    &allowed,
                    read,
                    !gate_refuses,
                    &mut result,
                );
                continue;
            }
            result.reads.push(Arc::new(read));
            if counts_as_strict_read_root(file, member.span, execution, self.lookup) {
                result.strict_read_obligations += 1;
            }
            let member_async = self.async_sources.contains(symbol);
            let member_options = self.effective_async_options(symbol);
            if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store)
                && (member_async
                    || member_options.ssr_client_bare
                    || member_options.server_rendering_unresolved)
            {
                let async_execution = async_execution_role(file, member.span, execution);
                let async_execution = if member_options.ssr_client_bare
                    || member_options.server_rendering_unresolved
                {
                    async_execution
                } else {
                    async_read_role(
                        file,
                        member.span,
                        async_execution,
                        self.entities,
                        self.symbol_names,
                        self.lookup,
                    )
                };
                let cover = read_loading_cover(self.lookup, file, member.span, self.symbol_names);
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
                    under_loading: cover == LoadingCover::Covered,
                    mount_unresolved: cover == LoadingCover::Unresolved,
                    async_provenance: member_async,
                    declared_loading: member_options.declared_loading,
                    options_opaque: member_options.opaque,
                    ssr_client_hole: member_options.ssr_client_bare,
                    server_rendering_unresolved: member_options.server_rendering_unresolved,
                    host_callback_timing: host_callback_timing(
                        file,
                        member.span,
                        async_execution,
                        self.lookup,
                    ),
                    callee_callback_timing: callee_callback_timing(
                        file,
                        member.span,
                        async_execution,
                        self.lookup,
                    ),
                    invocation_context_unproven: async_execution
                        == ExecutionRole::UntrackedRendering
                        && !nested_literal_runs_during_body(
                            file,
                            member.span,
                            self.entities,
                            self.symbol_names,
                            self.lookup,
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
            if (inside_non_component_function(file, spread.span, self.lookup)
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
            // A spread unwraps every prop, so it follows the whole-object
            // caller classification.
            let mut uncertain = self.uncertain_prop_sources.contains(symbol)
                || self.lookup.inside_possible_component(file, spread.span);
            if self.source_kinds.get(symbol) != Some(&ReactiveSourceKind::Store)
                && self.prop_sources.contains_key(symbol)
            {
                match self.props_reactivity.object_use(declaration) {
                    PropUse::Static => continue,
                    PropUse::Reactive => {}
                    PropUse::Unknown => uncertain = true,
                }
            }
            result.reads.push(Arc::new(ReactiveRead {
                package_internal: false,
                summary_attributed: false,
                kind: if self.source_kinds.get(symbol) == Some(&ReactiveSourceKind::Store) {
                    "store-path".into()
                } else {
                    "component-props".into()
                },
                accessor: format!("{name} spread").into(),
                location: location(file.path.shared(), spread.span),
                declaration: declaration.clone(),
                execution,
                context: read_analysis_context(file, spread.span, execution, self.lookup).into(),
                via: Arc::from(""),
                origin: None,
                origin_context: Arc::from(""),
                uncertain,
                missing_jsx_census: missing_jsx_census(file, spread.span, execution),
                host_callback_timing: host_callback_timing(
                    file,
                    spread.span,
                    execution,
                    self.lookup,
                ),
                project_consumer_non_strict: false,
                callback_invocation_unproven: false,
                callee_callback_timing: callee_callback_timing(
                    file,
                    spread.span,
                    execution,
                    self.lookup,
                ),
            }));
            if counts_as_strict_read_root(file, spread.span, execution, self.lookup) {
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
            let cover = read_loading_cover(self.lookup, file, element.name.span, self.symbol_names);
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
                under_loading: cover == LoadingCover::Covered,
                mount_unresolved: cover == LoadingCover::Unresolved,
                async_provenance: true,
                declared_loading: false,
                options_opaque: false,
                ssr_client_hole: false,
                server_rendering_unresolved: false,
                host_callback_timing: false,
                callee_callback_timing: false,
                invocation_context_unproven: false,
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
    target
        .dispatch_obligations
        .extend(source.dispatch_obligations.iter().cloned());
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
    target
        .dispatch_obligations
        .extend(source.dispatch_obligations);
}

/// A negative is a syntax proof about fresh values, never an inference from
/// an annotation, an unknown symbol or the contract's upper-bound shape.
fn callback_result_is_plain_data(
    file: &solid_facts::FileFacts,
    argument: &solid_facts::ast::ArgumentFact,
    result: &crate::ContractCallbackResult,
) -> bool {
    use solid_facts::ast::FunctionKind;
    if argument.spread || !result.parameter_path.is_empty() || !result.uses.is_closed() {
        return false;
    }
    if result.uses.items().is_empty() {
        return true;
    }
    let span = file.ast.peel_ts_sugar_span(argument.span);
    let Some(function) = file.ast.functions.iter().find(|function| {
        function.span == span
            && function.kind == FunctionKind::Arrow
            && !function.r#async
            && !function.generator
    }) else {
        return false;
    };
    let own_returns = file
        .ast
        .returns
        .iter()
        .filter(|returned| {
            crate::owners::containing_ast_function(&file.ast, returned.span)
                .is_some_and(|owner| owner.span == function.span)
        })
        .chain(function.expression_return.iter())
        .collect::<Vec<_>>();
    own_returns
        .iter()
        .all(|returned| callback_result_completion_is_plain_data(file, returned, result))
}

fn callback_result_completion_is_plain_data(
    file: &solid_facts::FileFacts,
    returned: &solid_facts::ast::ReturnFact,
    result: &crate::ContractCallbackResult,
) -> bool {
    use crate::contract_semantics::{InvokeProtocol, Schedule};
    use solid_facts::ast::RuntimeValueKind;
    let completion = returned
        .argument
        .map_or(returned.span, |span| file.ast.peel_ts_sugar_span(span));
    let primitive = matches!(returned.runtime_value_kind, RuntimeValueKind::Primitive | RuntimeValueKind::Nullish)
        // Oxc's broad runtime kind also contains a RegExp allocation. It is
        // an object, so it cannot supply the primitive negative premise.
        && file.source_text(completion).is_some_and(|text| !text.trim_start().starts_with('/'));
    let plain_object = returned.runtime_value_kind == RuntimeValueKind::Object
        && returned
            .structure
            .as_deref()
            .is_some_and(|structure| structure.complete_literal);
    result.uses.items().iter().all(|use_| {
        if !use_.path.is_empty() {
            return false;
        }
        match use_.operation.invoke_protocol() {
            InvokeProtocol::Call => {
                primitive && use_.callable_only && use_.operation.cardinality.min == Some(0)
            }
            InvokeProtocol::Get | InvokeProtocol::Coerce => primitive,
            InvokeProtocol::Iterate => primitive,
            // A fresh literal object has no getters, spreads, computed keys,
            // prototype replacement or method definitions. Later retained
            // uses need an escape/mutation proof not supplied by this shape.
            InvokeProtocol::GetEnumerableStringValues | InvokeProtocol::GetOwnEnumerableValues => {
                primitive
                    || (plain_object
                        && use_.operation.at == Some(crate::contract_semantics::Event::Call)
                        && use_.operation.schedule == Some(Schedule::SameStack))
            }
            InvokeProtocol::HasInstance => false,
        }
    })
}

/// Exact returned arrow, with no alias, wrapper callee, retained value or
/// recursive callable traversal. `guaranteed` is required for write proof
/// and for discharging dispatch; read classification needs universal context.
pub(crate) fn callback_result_literal_is_tracked(
    file: &solid_facts::FileFacts,
    argument: &solid_facts::ast::ArgumentFact,
    result: &crate::ContractCallbackResult,
    guaranteed: bool,
) -> bool {
    use crate::contract_semantics::{
        CapabilityKnowledge, CardinalityScope, Event, InvokeProtocol, OwnerSource, Schedule,
        Tracking,
    };
    let exact_at_call = |operation: &crate::contract_semantics::Operation| {
        operation.guard.is_none()
            && operation.trigger == Some(crate::contract_semantics::Trigger::Event(Event::Call))
            && operation.at == Some(Event::Call)
            && operation.schedule == Some(Schedule::SameStack)
            && operation.cardinality.scope == Some(CardinalityScope::Call)
            && operation.cardinality.min.is_some_and(|min| min >= 1)
    };
    if !result.uses.is_closed()
        || result.uses.items().is_empty()
        || !result.parameter_path.is_empty()
        || !result.non_escaping
        || argument.spread
        || (guaranteed && !exact_at_call(&result.producer))
    {
        return false;
    }
    let span = file.ast.peel_ts_sugar_span(argument.span);
    let Some(producer) = file.ast.functions.iter().find(|function| {
        function.span == span
            && function.kind == solid_facts::ast::FunctionKind::Arrow
            && !function.r#async
            && !function.generator
            && function.expression_body
    }) else {
        return false;
    };
    let Some(returned) = producer
        .expression_return
        .as_ref()
        .and_then(|returned| returned.argument)
    else {
        return false;
    };
    if !file.ast.functions.iter().any(|function| {
        function.span == file.ast.peel_ts_sugar_span(returned)
            && function.kind == solid_facts::ast::FunctionKind::Arrow
            && !function.r#async
            && !function.generator
    }) {
        return false;
    }
    result.uses.items().iter().all(|use_| {
        use_.path.is_empty()
            && use_.operation.invoke_protocol() == InvokeProtocol::Call
            && use_.operation.tracking == Tracking::Tracked
            && matches!(use_.operation.owner.source, OwnerSource::Created(_))
            && use_.operation.owner.capabilities.child_owners == CapabilityKnowledge::Allowed
            && (!guaranteed || exact_at_call(&use_.operation))
    })
}

// Insert at the end of the existing local_access.rs module.
#[cfg(test)]
mod callback_result_tests {
    use crate::contract_semantics::{
        Cardinality, CardinalityScope, Event, InvokeProtocol, KnowledgeSet, Operation, OperationId,
        OperationKind, OwnerRelation, OwnerSource, Schedule, Tracking, UpperBound, ValueShape,
    };
    use crate::{ContractCallbackResult, ContractCallbackResultUse};
    use solid_facts::{
        FileFacts, ast,
        compiler::{COMPILER_FACTS_PROTOCOL, ExecutionMap},
        core::Generation,
    };
    use std::collections::BTreeSet;

    fn file(source: &str) -> FileFacts {
        let ast = ast::extract("case.tsx", source).unwrap();
        let compiler = ExecutionMap {
            compiler_facts_protocol: COMPILER_FACTS_PROTOCOL,
            source_hash: ast.source.hash.clone(),
            semantic_model: Default::default(),
            tracked_regions: vec![],
            untracked_regions: vec![],
            discarded_regions: vec![],
            ownership_regions: vec![],
            callback_roles: vec![],
            jsx_operations: vec![],
        };
        FileFacts::new(Generation::new(1).unwrap(), source, ast, compiler).unwrap()
    }

    fn operation(protocol: InvokeProtocol) -> Operation {
        Operation {
            id: OperationId("result-use".into()),
            kind: OperationKind::Invoke,
            guard: None,
            trigger: Some(crate::contract_semantics::Trigger::Event(Event::Call)),
            at: Some(Event::Call),
            schedule: Some(Schedule::SameStack),
            tracking: Tracking::AmbientAtExecution,
            strict_read: None,
            owner: OwnerRelation {
                source: OwnerSource::AmbientAtExecution,
                ..OwnerRelation::default()
            },
            cardinality: Cardinality {
                scope: Some(CardinalityScope::Call),
                min: Some(0),
                max: Some(UpperBound::Many),
            },
            inputs: vec![],
            output: None,
            resources: BTreeSet::new(),
            composed_from: None,
            protocol: (protocol != InvokeProtocol::Call).then_some(protocol),
        }
    }

    fn result(protocol: InvokeProtocol, gated: bool) -> ContractCallbackResult {
        ContractCallbackResult {
            parameter: 0,
            parameter_path: vec![],
            non_escaping: true,
            producer: operation(InvokeProtocol::Call),
            shape: ValueShape::Unknown,
            uses: KnowledgeSet::Complete(vec![ContractCallbackResultUse {
                path: vec![],
                operation: operation(protocol),
                callable_only: gated,
            }]),
        }
    }

    fn clean(source: &str, result: &ContractCallbackResult) -> bool {
        let file = file(source);
        let argument = &file
            .ast
            .calls
            .iter()
            .find(|call| file.source_text(call.callee) == Some("take"))
            .unwrap()
            .arguments[0];
        crate::local_access::callback_result_is_plain_data(&file, argument, result)
    }

    #[test]
    fn primitive_result_negative_requires_the_callable_test_not_only_min_zero() {
        assert!(clean("take(() => 1)", &result(InvokeProtocol::Call, true)));
        assert!(!clean(
            "take(() => 1)",
            &result(InvokeProtocol::Call, false)
        ));
        for protocol in [
            InvokeProtocol::Get,
            InvokeProtocol::Iterate,
            InvokeProtocol::Coerce,
        ] {
            assert!(clean(
                "take((() => 1) satisfies (() => number))",
                &result(protocol, false)
            ));
            assert!(!clean("take(() => /regexp/)", &result(protocol, false)));
            assert!(!clean(
                "take(() => (/regexp/ satisfies object))",
                &result(protocol, false)
            ));
        }
        let mut partial = result(InvokeProtocol::Coerce, false);
        partial.uses = KnowledgeSet::Partial(partial.uses.items().to_vec());
        assert!(!clean("take(() => 1)", &partial));
    }

    #[test]
    fn returned_arrow_requires_exact_guaranteed_context_and_no_escape() {
        use crate::contract_semantics::{CapabilityKnowledge, ResourceId};
        let mut result = result(InvokeProtocol::Call, false);
        result.producer.cardinality.min = Some(1);
        let mut uses = result.uses.items().to_vec();
        uses[0].operation.cardinality.min = Some(1);
        uses[0].operation.tracking = Tracking::Tracked;
        uses[0].operation.owner.source = OwnerSource::Created(ResourceId("owner".into()));
        uses[0].operation.owner.capabilities.child_owners = CapabilityKnowledge::Allowed;
        result.uses = KnowledgeSet::Complete(uses);
        let tracked = |source: &str, result: &ContractCallbackResult| {
            let file = file(source);
            let argument = &file
                .ast
                .calls
                .iter()
                .find(|call| file.source_text(call.callee) == Some("take"))
                .unwrap()
                .arguments[0];
            crate::local_access::callback_result_literal_is_tracked(&file, argument, result, true)
        };
        assert!(tracked("take(() => () => read())", &result));
        assert!(tracked(
            "take((() => (() => read()) satisfies (() => number)) satisfies (() => () => number))",
            &result
        ));
        for source in [
            "take(() => known)",
            "take(() => { return () => read(); })",
            "take(async () => () => read())",
            "take(() => async () => read())",
        ] {
            assert!(!tracked(source, &result), "{source}");
        }
        let mut escapes = result.clone();
        escapes.non_escaping = false;
        assert!(!tracked("take(() => () => read())", &escapes));
        let mut optional = result.clone();
        let mut uses = optional.uses.items().to_vec();
        uses[0].operation.cardinality.min = Some(0);
        optional.uses = KnowledgeSet::Complete(uses);
        assert!(!tracked("take(() => () => read())", &optional));
        let mut no_trigger = result;
        no_trigger.producer.trigger = None;
        assert!(!tracked("take(() => () => read())", &no_trigger));
    }

    #[test]
    fn result_get_is_shallow_and_never_suppresses_getters_proxies_or_later_mutation() {
        let result = result(InvokeProtocol::GetOwnEnumerableValues, false);
        assert!(clean("take(() => ({ value: 1 }))", &result));
        assert!(clean("take(() => { return { value: 1 }; })", &result));
        for source in [
            "take(() => ({ get value() { return 1; } }))",
            "take(() => ({ ...other }))",
            "take(() => other)",
            "take(() => ({ [key]: 1 }))",
            "take(async () => ({ value: 1 }))",
        ] {
            assert!(!clean(source, &result), "{source}");
        }
        let mut later = result;
        let mut uses = later.uses.items().to_vec();
        uses[0].operation.at = Some(Event::ResultAccess);
        later.uses = KnowledgeSet::Complete(uses);
        assert!(!clean("take(() => ({ value: 1 }))", &later));
    }
}
