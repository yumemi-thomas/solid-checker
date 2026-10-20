//! Positive execution reachability. Other callers cannot cancel a browser edge.

use serde::{Deserialize, Serialize};
use solid_facts::core::Span;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct BrowserRootManifest {
    pub roots: BTreeSet<String>,
    pub reason: String,
    /// Canonical paths and content/presence identities. No parser nodes.
    pub inputs: BTreeMap<String, String>,
    /// Fresh ADR 0220 answers are part of the host graph's cache identity.
    #[serde(default)]
    pub runtime_resolution: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HostModule {
    pub path: String,
    pub refused: bool,
    /// Static imports and export-from dependencies must link before evaluation.
    pub static_dependencies: BTreeSet<String>,
    /// Whether initialization can finish. Unknown suspension/completion supplies
    /// no importer witness; a module's independently live prefix is separate.
    pub completion: Option<bool>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct HostExecutionRegion {
    pub span: Span,
    /// None is potentially live, but supplies no positive host witness.
    pub client: Option<bool>,
    pub server: Option<bool>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct HostScope {
    pub id: String,
    pub path: String,
    /// Callable/unsupported execution extent; includes parameters. None is top level.
    pub span: Option<Span>,
    pub edges: BTreeSet<String>,
    pub refused: bool,
    #[serde(default)]
    pub execution: Vec<HostExecutionRegion>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectHostIndex {
    pub manifest: BrowserRootManifest,
    scopes: BTreeMap<String, HostScope>,
    browser: BTreeSet<String>,
    refused_modules: BTreeSet<String>,
    /// Pending facts remain fingerprint inputs but supply no browser authority.
    pub callback_invocations: Vec<crate::callback_host::BrowserCallbackInvocation>,
    /// A Vite/Type Facts disagreement withholds just this import's host premise.
    pub blocked_imports: BTreeSet<(String, String)>,
    /// The host is known but resolution or opaque effects withhold runtime
    /// proof authority. Keep this source's baseline findings.
    pub unproven_semantics: BTreeSet<String>,
}

impl ProjectHostIndex {
    #[must_use]
    pub fn build(
        manifest: BrowserRootManifest,
        modules: &[HostModule],
        scopes: &[HostScope],
    ) -> Self {
        let scope_count = scopes.len();
        let scopes = scopes
            .iter()
            .map(|scope| (scope.id.clone(), scope.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut result = Self {
            manifest,
            scopes,
            ..Self::default()
        };
        if result.scopes.len() != scope_count {
            result.scopes.clear();
            return result;
        }
        result.refused_modules = modules
            .iter()
            .filter(|module| module.refused)
            .map(|module| module.path.clone())
            .collect::<BTreeSet<_>>();
        let by_path = modules
            .iter()
            .map(|module| (module.path.as_str(), module))
            .collect::<BTreeMap<_, _>>();
        if by_path.len() != modules.len() {
            result.scopes.clear();
            return result;
        }
        // Cyclic evaluation needs binding-initialization/order facts. Do not
        // infer a window host across an unproved TDZ/evaluation boundary.
        for module in modules {
            let mut pending = module.static_dependencies.clone();
            let mut visited = BTreeSet::new();
            while let Some(path) = pending.pop_first() {
                if path == module.path {
                    result.refused_modules.insert(module.path.clone());
                    break;
                }
                if visited.insert(path.clone())
                    && let Some(dependency) = by_path.get(path.as_str())
                {
                    pending.extend(dependency.static_dependencies.iter().cloned());
                }
            }
        }
        loop {
            let refused = modules
                .iter()
                .filter(|module| {
                    module.static_dependencies.iter().any(|path| {
                        result.refused_modules.contains(path)
                            || by_path
                                .get(path.as_str())
                                .is_none_or(|dependency| dependency.completion != Some(true))
                    })
                })
                .map(|module| module.path.clone())
                .collect::<BTreeSet<_>>();
            let count = result.refused_modules.len();
            result.refused_modules.extend(refused);
            if count == result.refused_modules.len() {
                break;
            }
        }
        loop {
            let pending = result
                .manifest
                .roots
                .iter()
                .chain(
                    result
                        .browser
                        .iter()
                        .filter_map(|id| result.scopes.get(id))
                        .flat_map(|scope| &scope.edges),
                )
                .cloned()
                .collect::<BTreeSet<_>>();
            let mut changed = false;
            for id in pending {
                let Some(scope) = result.scopes.get(&id) else {
                    continue;
                };
                if scope.refused || result.refused_modules.contains(scope.path.as_str()) {
                    continue;
                }
                // A foreign canonical symbol cannot jump across a refused
                // reexport/alias edge: its defining module must be reached too.
                if scope.span.is_some()
                    && !result.scopes.values().any(|top| {
                        top.path == scope.path
                            && top.span.is_none()
                            && result.browser.contains(&top.id)
                    })
                {
                    continue;
                }
                changed |= result.browser.insert(id);
            }
            if !changed {
                break;
            }
        }
        result
    }

    /// Close exact callback edges after independent browser host admission.
    /// Supplying a value and executing its exact invoker are conjunctive premises.
    pub fn with_callback_invocations(
        mut self,
        invocations: Vec<crate::callback_host::BrowserCallbackInvocation>,
    ) -> Self {
        self.callback_invocations = invocations;
        loop {
            let mut additions = Vec::new();
            for fact in &self.callback_invocations {
                let browser = |site: &crate::callback_host::InvocationSite| {
                    self.browser_proof_site(
                        &site.path,
                        u64::from(site.span.start),
                        u64::from(site.span.end),
                    )
                };
                let cancelled = fact.cancelled_by.as_ref().is_some_and(browser);
                if cancelled
                    || !fact.delivery.is_feasible()
                    || !browser(&fact.supplied)
                    || !browser(&fact.invoked)
                {
                    continue;
                }
                let mut targets = self.scopes.values().filter(|scope| {
                    scope.path == fact.target.path && scope.span == Some(fact.target.span)
                });
                let Some(target) = targets.next() else {
                    continue;
                };
                if targets.next().is_some()
                    || target.refused
                    || self.refused_modules.contains(&target.path)
                {
                    continue;
                }
                let mut sources = self
                    .scopes
                    .values()
                    .filter(|scope| {
                        scope.path == fact.supplied.path
                            && scope
                                .span
                                .is_some_and(|span| span.contains(fact.supplied.span))
                    })
                    .collect::<Vec<_>>();
                sources.sort_by_key(|scope| scope.span.map(|span| span.end - span.start));
                let source = sources.first().copied().or_else(|| {
                    self.scopes
                        .values()
                        .find(|scope| scope.path == fact.supplied.path && scope.span.is_none())
                });
                let Some(source) = source else {
                    continue;
                };
                if !source.edges.contains(&target.id) {
                    additions.push((source.id.clone(), target.id.clone()));
                }
            }
            if additions.is_empty() {
                break;
            }
            for (source, target) in additions {
                if let Some(scope) = self.scopes.get_mut(&source) {
                    scope.edges.insert(target);
                }
            }
            let modules = self
                .refused_modules
                .iter()
                .map(|path| HostModule {
                    path: path.clone(),
                    refused: true,
                    ..HostModule::default()
                })
                .collect::<Vec<_>>();
            let rebuilt = Self::build(
                self.manifest.clone(),
                &modules,
                &self.scopes.values().cloned().collect::<Vec<_>>(),
            );
            self.browser = rebuilt.browser;
        }
        self
    }

    /// Select the innermost execution boundary; never inherit its parent's host.
    #[must_use]
    pub fn browser_site(&self, path: &str, start: u64, end: u64) -> bool {
        let (Ok(start), Ok(end)) = (u32::try_from(start), u32::try_from(end)) else {
            return false;
        };
        let span = Span::new(start, end);
        let mut containing = self
            .scopes
            .values()
            .filter(|scope| {
                scope.path == path && scope.span.is_some_and(|extent| extent.contains(span))
            })
            .collect::<Vec<_>>();
        containing.sort_by_key(|scope| scope.span.map(|extent| extent.end - extent.start));
        if let Some(scope) = containing.first() {
            if containing
                .get(1)
                .is_some_and(|other| other.span == scope.span)
            {
                return false;
            }
            return self.browser.contains(&scope.id) && scope.client_live(span);
        }
        self.scopes.values().any(|scope| {
            scope.path == path
                && scope.span.is_none()
                && self.browser.contains(&scope.id)
                && scope.client_live(span)
        })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.browser.is_empty()
    }

    /// Number of positively inferred execution scopes, for run notes only.
    #[must_use]
    pub fn browser_scope_count(&self) -> usize {
        self.browser.len()
    }

    #[must_use]
    pub fn browser_proof_site(&self, path: &str, start: u64, end: u64) -> bool {
        !self.unproven_semantics.contains(path) && self.browser_site(path, start, end)
    }
}

impl HostScope {
    /// Missing facts and unknown predicates withhold strengthening. Only
    /// regions belonging to this execution scope are enrolled by the backend;
    /// lexical guards outside a nested callable cannot execute its body.
    #[must_use]
    pub fn client_live(&self, span: Span) -> bool {
        let mut regions = self
            .execution
            .iter()
            .filter(|region| region.span.contains(span))
            .peekable();
        regions.peek().is_some() && regions.all(|region| region.client == Some(true))
    }

    /// Exact site join for outgoing call/JSX/load edges.
    #[must_use]
    pub fn client_live_site(&self, span: Span) -> bool {
        let mut regions = self
            .execution
            .iter()
            .filter(|region| region.span == span)
            .peekable();
        regions.peek().is_some()
            && regions.all(|region| region.client == Some(true))
            && self.client_live(span)
    }
}

#[cfg(test)]
mod tests;
