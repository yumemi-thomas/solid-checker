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

/// The one question a contract view asks of a host: for this importer's
/// import of this specifier, is the browser view the one that answers?
///
/// It is derived from a [`ProjectHostIndex`] and is not a host: it answers no
/// proof-site, callback, execution-region or manifest query, and nothing can
/// recover a host from it. An importer is selected when any execution scope in
/// that file has a positive browser witness (the test `browser_at` applies, by
/// scope id), and a blocked `(importer, specifier)` pair is never selected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserViewSelection {
    importers: BTreeSet<String>,
    blocked: BTreeSet<(String, String)>,
}

impl BrowserViewSelection {
    #[must_use]
    pub fn selects(&self, importer: &str, specifier: &str) -> bool {
        self.importers.contains(importer)
            && !self
                .blocked
                .contains(&(importer.to_owned(), specifier.to_owned()))
    }

    pub(crate) fn importers(&self) -> &BTreeSet<String> {
        &self.importers
    }

    pub(crate) fn blocked(&self) -> &BTreeSet<(String, String)> {
        &self.blocked
    }
}

impl ProjectHostIndex {
    /// The view selection this host implies. Take it after the host's last
    /// mutation: it is a snapshot, so a later change to `blocked_imports`
    /// makes it differ from a fresh one.
    #[must_use]
    pub fn browser_view_selection(&self) -> BrowserViewSelection {
        BrowserViewSelection {
            importers: self
                .scopes
                .values()
                .filter(|scope| self.browser.contains(&scope.id))
                .map(|scope| scope.path.clone())
                .collect(),
            blocked: self.blocked_imports.clone(),
        }
    }

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

    /// Close exact callback edges after independent browser contract admission.
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

    /// Whether any execution scope in this file has a positive browser witness.
    #[must_use]
    pub fn browser_at(&self, path: &str) -> bool {
        self.scopes
            .values()
            .any(|scope| scope.path == path && self.browser.contains(&scope.id))
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
mod tests {
    use super::*;

    #[test]
    fn static_linking_and_completion_are_conjunctive_and_transitive() {
        let module = |path: &str, dependencies: &[&str], refused, completion| HostModule {
            path: path.into(),
            static_dependencies: dependencies.iter().map(|path| (*path).into()).collect(),
            refused,
            completion,
        };
        let build = |modules: &[HostModule]| {
            let scopes = [
                HostScope {
                    path: "entry".into(),
                    ..scope("entry", None, &["barrel"])
                },
                HostScope {
                    path: "barrel".into(),
                    ..scope("barrel", None, &["leaf"])
                },
                HostScope {
                    path: "leaf".into(),
                    ..scope("leaf", None, &[])
                },
            ];
            ProjectHostIndex::build(
                BrowserRootManifest {
                    roots: BTreeSet::from(["entry".into()]),
                    ..Default::default()
                },
                modules,
                &scopes,
            )
        };
        let mut modules = [
            module("entry", &["barrel"], false, Some(true)),
            module("barrel", &["leaf"], false, Some(true)),
            module("leaf", &[], false, Some(true)),
        ];
        assert!(build(&modules).browser_site("entry", 0, 1));
        assert!(build(&modules).browser_site("leaf", 0, 1));
        modules[2].refused = true;
        assert!(build(&modules).is_empty());
        modules[2].refused = false;
        modules[2].completion = Some(false);
        assert!(build(&modules).is_empty());
        modules[2].completion = None;
        assert!(build(&modules).is_empty());
        modules[2].completion = Some(true);
        modules[2].static_dependencies.insert("barrel".into());
        assert!(build(&modules).is_empty());
        modules[2].static_dependencies = BTreeSet::from(["not-in-program".into()]);
        assert!(build(&modules).is_empty());
        // A distinct root can still execute the throwing module's live prefix.
        modules[0].static_dependencies.clear();
        modules[1].static_dependencies.clear();
        modules[2].static_dependencies.clear();
        modules[2].completion = Some(false);
        let index = build(&modules);
        assert!(index.browser_site("entry", 0, 1));
        assert!(index.browser_site("leaf", 0, 1));
    }

    #[test]
    fn exact_outgoing_site_also_requires_containing_expression_completion() {
        let mut top = scope("top", None, &[]);
        let call = Span::new(20, 25);
        top.execution.extend([
            HostExecutionRegion {
                span: Span::new(10, 30),
                client: None,
                server: None,
            },
            HostExecutionRegion {
                span: call,
                client: Some(true),
                server: Some(true),
            },
        ]);
        assert!(!top.client_live_site(call));
    }

    fn scope(id: &str, span: Option<Span>, edges: &[&str]) -> HostScope {
        HostScope {
            id: id.into(),
            path: "shared".into(),
            span,
            edges: edges.iter().map(|edge| (*edge).into()).collect(),
            execution: vec![HostExecutionRegion {
                span: span.unwrap_or(Span::new(0, u32::MAX)),
                client: Some(true),
                server: Some(true),
            }],
            ..HostScope::default()
        }
    }

    #[test]
    fn incoming_server_callers_do_not_cancel_browser_and_uncalled_bodies_stay_none() {
        let scopes = [
            scope("top", None, &["called", "missing"]),
            scope("called", Some(Span::new(10, 30)), &[]),
            scope("server-only", Some(Span::new(40, 60)), &["called"]),
        ];
        let index = ProjectHostIndex::build(
            BrowserRootManifest {
                roots: BTreeSet::from(["top".into()]),
                ..BrowserRootManifest::default()
            },
            &[],
            &scopes,
        );
        assert!(index.browser_site("shared", 0, 1));
        assert!(index.browser_site("shared", 15, 16));
        assert!(!index.browser_site("shared", 45, 46));
    }

    #[test]
    fn server_boundary_does_not_propagate_or_cancel_its_browser_caller() {
        let mut server = scope("server", Some(Span::new(10, 30)), &["hidden"]);
        server.refused = true;
        let scopes = [
            scope("top", None, &["server"]),
            server,
            scope("hidden", Some(Span::new(40, 60)), &[]),
        ];
        let index = ProjectHostIndex::build(
            BrowserRootManifest {
                roots: BTreeSet::from(["top".into()]),
                ..BrowserRootManifest::default()
            },
            &[],
            &scopes,
        );
        assert!(index.browser_site("shared", 0, 1));
        assert!(!index.browser_site("shared", 15, 16));
        assert!(!index.browser_site("shared", 45, 46));
    }

    fn callback(
        delivery: crate::callback_host::CallbackDelivery,
        invoker: Span,
    ) -> crate::callback_host::BrowserCallbackInvocation {
        use crate::callback_host::{BrowserCallbackInvocation, InvocationSite};
        let site = |span| InvocationSite {
            path: "shared".into(),
            span,
        };
        BrowserCallbackInvocation {
            supplied: site(Span::new(1, 2)),
            invoked: site(invoker),
            target: site(Span::new(70, 90)),
            delivery,
            cancelled_by: None,
            premise: "exact-local-path".into(),
        }
    }

    #[test]
    fn callback_needs_supplier_and_exact_browser_invoker() {
        use crate::callback_host::CallbackDelivery;
        let build = |invoker_edge: bool| {
            ProjectHostIndex::build(
                BrowserRootManifest {
                    roots: BTreeSet::from(["top".into()]),
                    ..BrowserRootManifest::default()
                },
                &[],
                &[
                    scope("top", None, if invoker_edge { &["invoker"] } else { &[] }),
                    scope("invoker", Some(Span::new(10, 30)), &[]),
                    scope("callback", Some(Span::new(70, 90)), &[]),
                ],
            )
        };
        let fact = callback(CallbackDelivery::SameStack, Span::new(15, 16));
        assert!(
            !build(false)
                .with_callback_invocations(vec![fact.clone()])
                .browser_site("shared", 75, 76)
        );
        assert!(
            build(true)
                .with_callback_invocations(vec![fact])
                .browser_site("shared", 75, 76)
        );
    }

    #[test]
    fn feasible_trigger_upgrades_but_exact_client_live_cancellation_refuses() {
        use crate::callback_host::{CallbackDelivery, InvocationSite};
        let build = || {
            ProjectHostIndex::build(
                BrowserRootManifest {
                    roots: BTreeSet::from(["top".into()]),
                    ..Default::default()
                },
                &[],
                &[
                    scope("top", None, &[]),
                    scope("callback", Some(Span::new(70, 90)), &[]),
                ],
            )
        };
        let mut fact = callback(
            CallbackDelivery::FeasibleHostTrigger(solid_dialect::BrowserCallbackTrigger::TimerTask),
            Span::new(1, 2),
        );
        assert!(
            build()
                .with_callback_invocations(vec![fact.clone()])
                .browser_site("shared", 75, 76)
        );
        fact.cancelled_by = Some(InvocationSite {
            path: "shared".into(),
            span: Span::new(3, 4),
        });
        assert!(
            !build()
                .with_callback_invocations(vec![fact])
                .browser_site("shared", 75, 76)
        );
    }

    #[test]
    fn cancellation_registration_and_dead_invokers_never_upgrade_callbacks() {
        use crate::callback_host::CallbackDelivery;
        use solid_dialect::BrowserCallbackTrigger;
        let mut invoker = scope("invoker", Some(Span::new(10, 30)), &[]);
        invoker.refused = true;
        let build = || {
            ProjectHostIndex::build(
                BrowserRootManifest {
                    roots: BTreeSet::from(["top".into()]),
                    ..BrowserRootManifest::default()
                },
                &[],
                &[
                    scope("top", None, &["invoker"]),
                    invoker.clone(),
                    scope("callback", Some(Span::new(70, 90)), &[]),
                ],
            )
        };
        assert!(
            !build()
                .with_callback_invocations(vec![callback(
                    CallbackDelivery::SameStack,
                    Span::new(15, 16)
                )])
                .browser_site("shared", 75, 76)
        );
        for trigger in [
            BrowserCallbackTrigger::TimerTask,
            BrowserCallbackTrigger::DomEvent,
            BrowserCallbackTrigger::OwnerDisposal,
            BrowserCallbackTrigger::RenderSelection,
        ] {
            assert!(
                !build()
                    .with_callback_invocations(vec![callback(
                        CallbackDelivery::Pending(trigger),
                        Span::new(1, 2)
                    )])
                    .browser_site("shared", 75, 76)
            );
        }
    }

    #[test]
    fn dead_unknown_and_unreached_default_sites_withhold_only_their_own_authority() {
        let mut top = scope("top", None, &[]);
        top.execution.extend([
            HostExecutionRegion {
                span: Span::new(10, 20),
                client: Some(false),
                server: Some(true),
            },
            HostExecutionRegion {
                span: Span::new(30, 40),
                client: None,
                server: None,
            },
        ]);
        let scopes = [top, scope("default", Some(Span::new(50, 60)), &[])];
        let index = ProjectHostIndex::build(
            BrowserRootManifest {
                roots: BTreeSet::from(["top".into()]),
                ..BrowserRootManifest::default()
            },
            &[],
            &scopes,
        );
        assert!(index.browser_site("shared", 1, 2));
        assert!(!index.browser_site("shared", 11, 12));
        assert!(!index.browser_site("shared", 31, 32));
        assert!(!index.browser_site("shared", 51, 52));
    }

    mod selection {
        use super::*;
        use crate::{AnalysisInputs, HostPairingError, contract_semantics::AcceptedContractIndex};

        fn in_file(path: &str, id: &str, span: Option<Span>, edges: &[&str]) -> HostScope {
            HostScope {
                path: path.into(),
                ..scope(id, span, edges)
            }
        }

        /// Browser: a (root), f (edge from a), e (root; its callable is not
        /// reached). Not browser: b (unreached), c (refused scope), d (refused
        /// module).
        fn host() -> ProjectHostIndex {
            let mut refused = in_file("c.ts", "c-top", None, &[]);
            refused.refused = true;
            let scopes = [
                in_file("a.ts", "a-top", None, &["f-top"]),
                in_file("f.ts", "f-top", None, &[]),
                in_file("e.ts", "e-top", None, &[]),
                in_file("e.ts", "e-fn", Some(Span::new(10, 30)), &[]),
                in_file("b.ts", "b-top", None, &[]),
                in_file("b.ts", "b-fn", Some(Span::new(10, 30)), &[]),
                refused,
                in_file("d.ts", "d-top", None, &[]),
            ];
            let modules = [HostModule {
                path: "d.ts".into(),
                refused: true,
                ..HostModule::default()
            }];
            let mut index = ProjectHostIndex::build(
                BrowserRootManifest {
                    roots: ["a-top", "e-top", "c-top", "d-top"]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                    reason: "grid".into(),
                    inputs: BTreeMap::from([("/p/vite.config.ts".into(), "digest".into())]),
                    ..BrowserRootManifest::default()
                },
                &modules,
                &scopes,
            );
            index.blocked_imports = [("a.ts", "x"), ("e.ts", "y"), ("b.ts", "x"), ("zz.ts", "x")]
                .into_iter()
                .map(|(importer, specifier)| (importer.into(), specifier.into()))
                .collect();
            index
        }

        #[test]
        fn selection_is_browser_at_and_not_blocked_for_every_import_in_the_grid() {
            let index = host();
            let selection = index.browser_view_selection();
            let importers = [
                "a.ts", "b.ts", "c.ts", "d.ts", "e.ts", "f.ts", "zz.ts", "none.ts",
            ];
            let specifiers = ["x", "y", "z"];
            let mut selected = 0;
            for importer in importers {
                for specifier in specifiers {
                    let expected = index.browser_at(importer)
                        && !index
                            .blocked_imports
                            .contains(&(importer.into(), specifier.into()));
                    assert_eq!(
                        selection.selects(importer, specifier),
                        expected,
                        "{importer} {specifier}"
                    );
                    selected += usize::from(expected);
                }
            }
            // The grid is not vacuous: the browser importers are exactly a, e
            // and f, two of whose imports are blocked.
            let browser = importers
                .into_iter()
                .filter(|importer| index.browser_at(importer))
                .collect::<Vec<_>>();
            assert_eq!(browser, ["a.ts", "e.ts", "f.ts"]);
            assert_eq!(selected, 3 * 3 - 2);
            assert!(selection.selects("f.ts", "x") && !selection.selects("a.ts", "x"));
            assert!(!selection.selects("b.ts", "z") && !selection.selects("d.ts", "z"));
        }

        #[test]
        fn a_host_changed_after_its_selection_is_taken_selects_differently() {
            let mut index = host();
            let before = index.browser_view_selection();
            assert!(before.selects("f.ts", "x"));
            index.blocked_imports.insert(("f.ts".into(), "x".into()));
            let after = index.browser_view_selection();
            assert_ne!(before, after);
            assert!(!after.selects("f.ts", "x"));
            // The snapshot is unchanged by the mutation.
            assert!(before.selects("f.ts", "x"));
        }

        #[test]
        fn unproven_semantics_is_not_a_selection_field_but_is_a_proof_site_and_identity_field() {
            let mut index = host();
            let selection = index.browser_view_selection();
            assert!(index.browser_proof_site("a.ts", 1, 2));
            let plain = AcceptedContractIndex::default();
            let before = AnalysisInputs::contracts_only(&plain)
                .with_hosts(&index)
                .cache_fingerprints();
            index.unproven_semantics.insert("a.ts".into());
            assert_eq!(selection, index.browser_view_selection());
            assert!(!index.browser_proof_site("a.ts", 1, 2));
            let after = AnalysisInputs::contracts_only(&plain)
                .with_hosts(&index)
                .cache_fingerprints();
            assert_ne!(before, after);
        }

        fn viewed(index: &ProjectHostIndex) -> AcceptedContractIndex {
            AcceptedContractIndex::default().with_browser_view(
                index.browser_view_selection(),
                AcceptedContractIndex::default(),
            )
        }

        #[test]
        fn host_only_fields_change_the_analysis_identity_and_leave_the_view_identity() {
            let base = host();
            let contracts = viewed(&base);
            let identity = |index: &ProjectHostIndex| {
                AnalysisInputs::contracts_only(&contracts)
                    .with_hosts(index)
                    .cache_fingerprints()
            };
            let reference = identity(&base);
            assert_eq!(reference.len(), 2);
            assert_eq!(reference[0], contracts.cache_fingerprint());
            let mut reason = base.clone();
            reason.manifest.reason.push_str(" changed");
            let mut unproven = base.clone();
            unproven.unproven_semantics.insert("e.ts".into());
            let callbacks = base.clone().with_callback_invocations(vec![callback(
                crate::callback_host::CallbackDelivery::SameStack,
                Span::new(15, 16),
            )]);
            let mut inputs = base.clone();
            inputs
                .manifest
                .inputs
                .insert("/p/other.ts".into(), "digest".into());
            for changed in [&reason, &unproven, &callbacks, &inputs] {
                // Same selection, so the view the contracts carry is the same.
                assert_eq!(
                    changed.browser_view_selection(),
                    base.browser_view_selection()
                );
                assert_eq!(
                    viewed(changed).cache_fingerprint(),
                    contracts.cache_fingerprint()
                );
                assert_ne!(identity(changed), reference);
            }
        }

        #[test]
        fn selection_changes_the_view_identity_and_hosts_without_a_view_are_not_hashed_into_it() {
            let base = host();
            let mut blocked = base.clone();
            blocked.blocked_imports.insert(("f.ts".into(), "x".into()));
            assert_ne!(
                viewed(&base).cache_fingerprint(),
                viewed(&blocked).cache_fingerprint()
            );
            assert_ne!(
                viewed(&base).cache_fingerprint(),
                AcceptedContractIndex::default().cache_fingerprint()
            );
            // Boundaries between collections cannot shift: moving an importer
            // string from the importer set into a blocked pair is not a no-op.
            let one = |importers: &[&str], blocked: &[(&str, &str)]| {
                let index = ProjectHostIndex {
                    blocked_imports: blocked
                        .iter()
                        .map(|(a, b)| ((*a).into(), (*b).into()))
                        .collect(),
                    ..ProjectHostIndex::default()
                };
                let mut selection = index.browser_view_selection();
                selection.importers = importers.iter().map(|path| (*path).into()).collect();
                AcceptedContractIndex::default()
                    .with_browser_view(selection, AcceptedContractIndex::default())
                    .cache_fingerprint()
            };
            assert_ne!(one(&["a", "b"], &[]), one(&["a"], &[("b", "")]));
            assert_ne!(one(&["ab"], &[]), one(&["a", "b"], &[]));
            assert_ne!(one(&[], &[("ab", "c")]), one(&[], &[("a", "bc")]));
        }

        /// Derived without this code: sha256 of the bytes `cache_fingerprint`
        /// wrote at 326f820f9 for these indexes, computed by an independent
        /// script from the encoding in `consumer.rs` (see the P4 report).
        #[test]
        fn an_index_with_no_browser_view_keeps_the_identity_it_had_before_the_extraction() {
            let hex = |digest: [u8; 32]| {
                digest
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            };
            let default = AcceptedContractIndex::default();
            assert_eq!(
                hex(default.cache_fingerprint()),
                "a6dde35aa80e877cc9471c4bd1ccb0471bd3657d02ab1300fa0be138f51d3254"
            );
            let refused = AcceptedContractIndex::default()
                .with_admission_refusals([("pkg".into(), "because".into())]);
            assert_eq!(
                hex(refused.cache_fingerprint()),
                "babb6ea232b7ed4c66a8225ba8d5bf9c44558dd0c20746a6201941341770e711"
            );
            let context = refused.with_context_provided_packages(["ctx".into()]);
            assert_eq!(
                hex(context.cache_fingerprint()),
                "94396caef919a84d480cd0c4b85792ea67e24ed04aaa9ebbf083be7f3d019d20"
            );
            // No host: the analysis identity is the index's alone.
            assert_eq!(
                AnalysisInputs::contracts_only(&context).cache_fingerprints(),
                vec![context.cache_fingerprint()]
            );
        }

        #[test]
        fn contracts_only_carries_no_host_and_the_index_hands_none_back() {
            let index = host();
            let contracts = viewed(&index);
            assert!(AnalysisInputs::contracts_only(&contracts).hosts.is_none());
            // The view is queryable and the same on both halves of the fork.
            let (baseline, browser) = contracts.execution_views().unwrap();
            assert!(baseline.browser_view_state().is_none());
            assert_eq!(
                browser.browser_view_state().map(|(_, projects)| projects),
                Some(false)
            );
        }

        #[test]
        fn pairing_refuses_only_inconsistent_outer_inputs() {
            let index = host();
            let contracts = viewed(&index);
            let plain = AcceptedContractIndex::default();
            let ok = |inputs: AnalysisInputs<'_>| inputs.check_host_pairing();
            // Valid: the outer pair, the baseline half, the browser half
            // (a view with no host and no projected findings), no view at all.
            assert_eq!(
                ok(AnalysisInputs::contracts_only(&contracts).with_hosts(&index)),
                Ok(())
            );
            let (baseline, browser) = contracts.execution_views().unwrap();
            assert_eq!(ok(AnalysisInputs::contracts_only(&baseline)), Ok(()));
            assert_eq!(ok(AnalysisInputs::contracts_only(&browser)), Ok(()));
            assert_eq!(ok(AnalysisInputs::contracts_only(&plain)), Ok(()));
            // Refused: a fork without its host, a host without its view, and a
            // host that changed after the view was selected from it.
            assert_eq!(
                ok(AnalysisInputs::contracts_only(&contracts)),
                Err(HostPairingError::ForkWithoutHost)
            );
            assert_eq!(
                ok(AnalysisInputs::contracts_only(&plain).with_hosts(&index)),
                Err(HostPairingError::HostWithoutView)
            );
            assert_eq!(
                ok(AnalysisInputs::contracts_only(&baseline).with_hosts(&index)),
                Err(HostPairingError::HostWithoutView)
            );
            let mut later = index.clone();
            later.blocked_imports.insert(("f.ts".into(), "x".into()));
            assert_eq!(
                ok(AnalysisInputs::contracts_only(&contracts).with_hosts(&later)),
                Err(HostPairingError::SelectionMismatch)
            );
            assert_eq!(
                ok(AnalysisInputs::contracts_only(&browser).with_hosts(&later)),
                Err(HostPairingError::SelectionMismatch)
            );
            // A host mutation that is not a selection field is not a mismatch.
            let mut unproven = index.clone();
            unproven.unproven_semantics.insert("a.ts".into());
            assert_eq!(
                ok(AnalysisInputs::contracts_only(&contracts).with_hosts(&unproven)),
                Ok(())
            );
        }
    }
}
