//! Stable diagnostic identities. Analysis decides whether a rule applies;
//! this catalog owns its externally visible code, name, and severity.

use solid_reactive_ir::RuleMetadata;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rule {
    StrictReadUntracked,
    ReactiveReadAfterAwait,
    // The decomposed upstream-`reactivity` rules shared with the 1.x
    // dialect: the defects — an accessor used where a value was meant, a
    // readonly proxy written through, a listener bound to a call's result, a
    // derivation nothing tracks, a source handed to an undescribed callee —
    // are version-independent, so both catalogs carry them under the same SC
    // codes and suppressions survive a migration. `no-async-tracked-scope`
    // stays 1.x-only: 2.0 models async computations as a feature
    // (SC5001 and SC5003 own that surface).
    UncalledAccessor,
    ExpectedFunctionGotExpression,
    NoStoreMutationOutsideSetter,
    ReactiveDispatchUnresolved,
    ComponentPropsDestructure,
    ComponentReturnsConditionally,
    NoWriteInPureComputation,
    ActionCalledInOwnedScope,
    ResolveInTrackedScope,
    UntilInTrackedScope,
    FlushInAction,
    StaticDynamicAsyncSource,
    FlushInEffectCallback,
    LeafOwnerForbiddenCall,
    MissingOwner,
    MissingContextProvider,
    PendingAsyncUnsuspendableRead,
    AsyncOutsideLoadingBoundary,
    PrimitiveInDirectiveApplication,
    MissingEffectFunction,
    SyncNodeReceivedAsync,
    HttpResponseAfterFlush,
    ValidUseServer,
    ServerFunctionRichArgument,
    PackageBehaviorUnavailable,
    /// The only identity in this catalog the rules engine never produces.
    ///
    /// Analysis decides whether every other rule applies; this one is decided
    /// *before* analysis, by dialect detection, and is emitted straight to the
    /// reporting path (`main.rs`, at the selection site). It lives in the
    /// catalog anyway because it is an externally visible diagnostic identity:
    /// adapters resolve its code and severity here, `docs/rules/` documents
    /// it, and a suppression naming it has to be a known rule. See ADR 0110.
    UnsupportedSolidRuntime,
    /// Decided by dialect detection too, but *beside* the analysis rather than
    /// instead of it: the installed release of a carried major is one this
    /// vocabulary was not audited on. The backend appends it to the analysis
    /// result (`diagnostics.rs`); the rules engine never produces it.
    UnauditedSolidRelease,
    /// Decided by fact building: the dialect compiler refused a file, so it
    /// carries no compiler facts (ADR 0360). The backend appends it beside the
    /// analysis (`diagnostics.rs`); the rules engine never produces it.
    CompilerRefusedSource,
    /// Decided by suppression comments (ADR 0387): a `solid-checker/*` name in
    /// an `eslint-disable` or `solid-checker-disable` comment that names no
    /// rule of this checker. The backend appends it beside the analysis
    /// (`diagnostics.rs`); the rules engine never produces it.
    UnknownSuppressionRule,
    JsxNoDuplicateProps,
    PreferFor,
    PreferShow,
}

/// Base URL of the per-rule documentation pages in `docs/rules/`.
pub const DOCS_BASE_URL: &str = solid_reactive_ir::DOCS_BASE_URL;

/// The documentation page for a diagnostic, addressed by its externally
/// visible rule name so adapters that only carry the name (LSP, snapshots)
/// can link without a catalog lookup.
#[must_use]
pub fn docs_url(rule_name: &str) -> String {
    format!("{DOCS_BASE_URL}/{rule_name}.md")
}

impl Rule {
    pub const ALL: [Self; 34] = [
        Self::StrictReadUntracked,
        Self::ReactiveReadAfterAwait,
        Self::UncalledAccessor,
        Self::ExpectedFunctionGotExpression,
        Self::NoStoreMutationOutsideSetter,
        Self::ReactiveDispatchUnresolved,
        Self::ComponentPropsDestructure,
        Self::ComponentReturnsConditionally,
        Self::NoWriteInPureComputation,
        Self::ActionCalledInOwnedScope,
        Self::ResolveInTrackedScope,
        Self::UntilInTrackedScope,
        Self::FlushInAction,
        Self::StaticDynamicAsyncSource,
        Self::FlushInEffectCallback,
        Self::LeafOwnerForbiddenCall,
        Self::MissingOwner,
        Self::MissingContextProvider,
        Self::PendingAsyncUnsuspendableRead,
        Self::AsyncOutsideLoadingBoundary,
        Self::PrimitiveInDirectiveApplication,
        Self::MissingEffectFunction,
        Self::SyncNodeReceivedAsync,
        Self::HttpResponseAfterFlush,
        Self::ValidUseServer,
        Self::ServerFunctionRichArgument,
        Self::PackageBehaviorUnavailable,
        Self::UnsupportedSolidRuntime,
        Self::UnauditedSolidRelease,
        Self::CompilerRefusedSource,
        Self::UnknownSuppressionRule,
        Self::JsxNoDuplicateProps,
        Self::PreferFor,
        Self::PreferShow,
    ];

    #[must_use]
    pub const fn metadata(self) -> RuleMetadata {
        let (code, name, severity, uncertifiable) = match self {
            Self::StrictReadUntracked => ("SC1001", "strict-read-untracked", "warning", false),
            Self::ReactiveReadAfterAwait => ("SC1002", "reactive-read-after-await", "error", false),
            Self::UncalledAccessor => ("SC1005", "uncalled-accessor", "warning", false),
            Self::ExpectedFunctionGotExpression => {
                ("SC1007", "reactive-handler-frozen", "warning", false)
            }
            Self::NoStoreMutationOutsideSetter => (
                "SC2003",
                "no-store-mutation-outside-setter",
                "warning",
                false,
            ),
            // ADR 0398: SC9012 and SC9005 are obligations, not rules. SC9012
            // also carries what SC9011 (`reactive-source-uncaptured`) was, as
            // the context `package-callee`; no key configures either.
            Self::ReactiveDispatchUnresolved => {
                ("SC9012", "reactive-dispatch-unresolved", "warning", true)
            }
            Self::ComponentPropsDestructure => ("SC1003", "no-destructure", "error", false),
            Self::ComponentReturnsConditionally => {
                ("SC1004", "components-return-once", "error", false)
            }
            Self::NoWriteInPureComputation => {
                ("SC2001", "no-write-in-pure-computation", "error", false)
            }
            Self::ActionCalledInOwnedScope => {
                ("SC2002", "action-called-in-owned-scope", "error", false)
            }
            // `resolve` and `until` are one configurable rule with two codes
            // (ADR 0384): the name is the identity a project configures, the
            // code names the defect, so a finding says which primitive it is
            // about (SC2004 `resolve`, SC2005 `until`) and one name enables
            // or disables both. The rc.0 dev guard of `resolve` throws on an
            // active observer (probed), so the proven tracked-scope form
            // mirrors it as an error; production has no guard and silently
            // takes a one-shot snapshot.
            Self::ResolveInTrackedScope => (
                "SC2004",
                "no-await-primitive-in-tracked-scope",
                "error",
                false,
            ),
            // rc.9's `until` carries resolve's observer guard verbatim
            // (`@solidjs/signals@2.0.0-rc.9` `dist/dev.js:2718-2722`) and the
            // same dev-only throw, so it takes the next code in the family
            // and the same severity.
            Self::UntilInTrackedScope => (
                "SC2005",
                "no-await-primitive-in-tracked-scope",
                "error",
                false,
            ),
            // `@solidjs/signals@2.0.0-rc.8` added the `FLUSH_IN_ACTION` dev
            // throw to `flush` inside an action step (rc.9
            // `dist/dev-shared.js:2210-2219`): an error like the other
            // runtime-mirrored throws in the family.
            Self::FlushInAction => ("SC2006", "flush-in-action", "error", false),
            // `flush()` run by an effect's apply callback is a no-op in every
            // client build, and the dev build usually also *warns*
            // (`@solidjs/signals` `FLUSH_IN_EFFECT_CALLBACK`, severity "warn",
            // rc.13 `dist/dev-shared.js:2687-2698`; not after a nested
            // synchronous render effect): nothing throws, the writes land in
            // the running flush's continuation. A proven defect at the
            // runtime's own severity, hence a warning.
            Self::FlushInEffectCallback => ("SC2008", "flush-in-effect-callback", "warning", false),
            // rc.9's static `dynamic` form refuses a promise-valued source:
            // the dev builds throw at the call (`@solidjs/web@2.0.0-rc.9`
            // `dist/web.dev.js:2249`, `dist/server.dev.js:3979`) and the
            // production builds render nothing. A proven violation with the
            // family's dev-throw severity.
            Self::StaticDynamicAsyncSource => {
                ("SC2007", "static-dynamic-async-source", "error", false)
            }
            Self::LeafOwnerForbiddenCall => ("SC3001", "leaf-owner-forbidden-call", "error", false),
            // Settled-cleanup findings override this family default to error:
            // the rc.0 dev runtime throws SETTLED_CLEANUP_UNOWNED, while the
            // other missing-owner operations leak silently.
            Self::MissingOwner => ("SC4001", "missing-owner", "warning", false),
            // ADR 0352: a read under an owner of a context no provider supplies
            // throws `ContextNotFoundError` in the dev and production builds, a
            // crash at the read, so it is an error like SC2006's dev throw.
            Self::MissingContextProvider => ("SC4002", "missing-context-provider", "error", false),
            // Leaf-owner reads override this family default to warning so the
            // merged rule preserves the runtime-mirroring severity split.
            Self::PendingAsyncUnsuspendableRead => {
                ("SC5001", "pending-async-unsuspendable-read", "error", false)
            }
            Self::AsyncOutsideLoadingBoundary => {
                ("SC5003", "async-outside-loading-boundary", "warning", false)
            }
            // The apply callback runs with no owner (`@solidjs/web` rc.0's
            // `ref()` is `runWithOwner(null, ...)`), so an owner-attaching
            // primitive created there is the SC4001-family defect: a real,
            // per-element leak the dev runtime answers with a NO_OWNER_*
            // *warning* (probed), never a throw. The catalog mirrors that —
            // warning severity, violation kind (the leak is proven).
            Self::PrimitiveInDirectiveApplication => (
                "SC6001",
                "primitive-in-directive-application",
                "warning",
                false,
            ),
            Self::MissingEffectFunction => ("SC7001", "missing-effect-function", "error", false),
            Self::SyncNodeReceivedAsync => {
                ("SC7002", "sync-computation-received-async", "error", false)
            }
            // SC7003 and SC9003 each carry two rule names on purpose: the
            // code identifies the defect (an invalid or unresolved target),
            // while the name identifies the surface it was found on
            // (`refresh` versus `affects`).
            // The post-flush drop is real but request-time ordering decides
            // whether it occurs, so this is an uncertifiable hazard.
            Self::HttpResponseAfterFlush => {
                ("SC7005", "http-response-after-flush", "warning", true)
            }
            // A misplaced or unextractable directive leaves server code on
            // the client (or fails the build), and a directive file's
            // re-export or non-function export fails the compile or the
            // server boot (ADR 0385), so this is an error.
            Self::ValidUseServer => ("SC7006", "valid-use-server", "error", false),
            // The default transport throws at the call site (probed, rc.0
            // server-functions client), so the proven form is an error.
            Self::ServerFunctionRichArgument => {
                ("SC7007", "server-function-rich-argument", "error", false)
            }
            Self::PackageBehaviorUnavailable => {
                ("SC9005", "package-behavior-unavailable", "error", true)
            }
            // Uncertifiable, not a violation: the project's own code is not
            // the defect. The checker cannot model the runtime it installs,
            // so it states that and proves nothing else -- reporting a
            // violation here would assert something about source it never
            // analyzed. Error severity because the alternative is a silent
            // analysis under the wrong language.
            Self::UnsupportedSolidRuntime => ("SC9013", "unsupported-solid-runtime", "error", true),
            // Uncertifiable for the same reason as SC9013 -- the claim is about
            // the installed runtime, not the project's source -- but a warning:
            // the analysis ran, and every other finding beside it stands.
            Self::UnauditedSolidRelease => ("SC9014", "unaudited-solid-release", "warning", true),
            // ADR 0399: a refusal the dialect proves a defect of the source (a
            // removed 1.x attribute `tsc` accepts) is an error, one per file.
            // Every other refusal keeps ADR 0360's project-level notice (a
            // warning, uncertifiable): the file has no compiler facts, so
            // nothing about it is proven, and the findings that do not depend
            // on it stand. The backend builds both (`compiler_refused_finding`),
            // so this entry is the violation's identity.
            Self::CompilerRefusedSource => ("SC9015", "compiler-refused-source", "error", false),
            // Uncertifiable: a name nothing resolves suppresses nothing, so the
            // comment is not a claim about any site. Project-level and a warning
            // like SC9015, because the findings beside it stand.
            Self::UnknownSuppressionRule => ("SC9016", "unknown-suppression-rule", "warning", true),
            Self::JsxNoDuplicateProps => ("SC8003", "jsx-no-duplicate-props", "error", false),
            Self::PreferFor => ("SC8014", "prefer-for", "error", false),
            Self::PreferShow => ("SC8015", "prefer-show", "warning", false),
        };
        let preference = matches!(self, Self::PreferFor | Self::PreferShow);
        RuleMetadata {
            code,
            name,
            severity,
            uncertifiable,
            default_enabled: true,
            configurable: !matches!(
                self,
                Self::PackageBehaviorUnavailable | Self::ReactiveDispatchUnresolved
            ),
            presets: if preference { &["preferences"] } else { &[] },
        }
    }

    #[must_use]
    pub fn from_identity(code: &str, name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|rule| rule.metadata().code == code && rule.metadata().name == name)
    }
}

/// The catalog as the npm plugin consumes it: one JSON entry per rule, in
/// catalog order. Generated into `packages/cli/lib/rules-solid-v2.json` by the test
/// below; `SOLID_RULES_UPDATE=1 cargo test -p solid-v2-rules` rewrites it, a plain
/// test run fails on drift. The JS surface must never hand-maintain rule
/// facts the catalog already owns.
#[must_use]
pub fn manifest_json() -> String {
    solid_reactive_ir::rule_manifest_json(
        solid_reactive_ir::RuleManifestIdentity {
            dialect: "solid-v2",
            config: "v2",
            namespace: "",
        },
        DOCS_BASE_URL,
        Rule::ALL.into_iter().map(Rule::metadata),
    )
}

#[cfg(test)]
mod tests {
    /// The npm plugin reads the catalog from a checked-in JSON file; this is
    /// what keeps that file the catalog. `SOLID_RULES_UPDATE=1` rewrites it.
    #[test]
    fn the_shipped_manifest_is_the_catalog() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../packages/cli/lib/rules-solid-v2.json");
        let expected = super::manifest_json();
        if std::env::var_os("SOLID_RULES_UPDATE").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &expected).unwrap();
            return;
        }
        let shipped = std::fs::read_to_string(&path).unwrap_or_default();
        assert_eq!(
            shipped, expected,
            "packages/cli/lib/rules-solid-v2.json has drifted from the catalog; run SOLID_RULES_UPDATE=1 cargo test -p solid-v2-rules to rewrite it"
        );
    }

    use std::collections::HashSet;

    use super::Rule;

    #[test]
    fn diagnostic_identities_are_unique_and_well_formed() {
        let identities = Rule::ALL
            .into_iter()
            .map(|rule| {
                let metadata = rule.metadata();
                assert!(metadata.code.starts_with("SC"));
                assert_eq!(metadata.code.len(), 6);
                assert!(!metadata.name.is_empty());
                (metadata.code, metadata.name)
            })
            .collect::<HashSet<_>>();
        assert_eq!(identities.len(), Rule::ALL.len());
    }

    /// ADR 0384: the name is the configurable identity, so codes that share
    /// one must agree on everything a configuration or a preset can see.
    #[test]
    fn codes_sharing_a_name_share_their_configuration() {
        for rule in Rule::ALL {
            let metadata = rule.metadata();
            for other in Rule::ALL {
                let other = other.metadata();
                if other.name == metadata.name {
                    assert_eq!(
                        (
                            other.severity,
                            other.uncertifiable,
                            other.default_enabled,
                            other.presets
                        ),
                        (
                            metadata.severity,
                            metadata.uncertifiable,
                            metadata.default_enabled,
                            metadata.presets
                        ),
                        "{} and {} share the name {} and must share its configuration",
                        metadata.code,
                        other.code,
                        metadata.name
                    );
                }
            }
        }
    }

    #[test]
    fn every_rule_has_a_documentation_page() {
        let docs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../docs/rules");
        solid_reactive_ir::assert_rules_have_documentation(
            &docs,
            Rule::ALL.into_iter().map(|rule| rule.metadata().name),
        );
    }

    /// Every identity the reactive IR emits as a *static violation* on the
    /// 2.0 static-API surface must resolve here — a miss panics in `solve`.
    #[test]
    fn every_v2_static_violation_identity_resolves() {
        for (code, name) in [
            ("SC2004", "no-await-primitive-in-tracked-scope"),
            ("SC2005", "no-await-primitive-in-tracked-scope"),
            ("SC2006", "flush-in-action"),
            ("SC2008", "flush-in-effect-callback"),
            ("SC2007", "static-dynamic-async-source"),
            ("SC7002", "sync-computation-received-async"),
            ("SC7005", "http-response-after-flush"),
            ("SC7006", "valid-use-server"),
            ("SC7007", "server-function-rich-argument"),
            ("SC8003", "jsx-no-duplicate-props"),
            ("SC8014", "prefer-for"),
            ("SC8015", "prefer-show"),
        ] {
            assert!(
                Rule::from_identity(code, name).is_some(),
                "IR static-violation identity {code}/{name} does not resolve in the v2 catalog"
            );
        }
        // The refresh/affects target identities were removed on 2026-08-17:
        // `Refreshable<T>` brands the target *in the type system*, so every
        // invalid target is TS2345 and every "cannot prove the brand"
        // obligation asks a question the type already answers. Asserted absent
        // so a reintroduction has to argue with this comment.
        for (code, name) in [
            ("SC7003", "invalid-refresh-target"),
            ("SC7003", "invalid-affects-target"),
            ("SC7004", "affects-keys-on-accessor"),
            ("SC9003", "refresh-target-unresolved"),
            ("SC9003", "affects-target-unresolved"),
        ] {
            assert!(
                Rule::from_identity(code, name).is_none(),
                "{code}/{name} duplicates a TypeScript diagnostic and was removed"
            );
        }
    }

    #[test]
    fn runtime_mirrored_severities_match_solid_two() {
        assert_eq!(
            Rule::AsyncOutsideLoadingBoundary.metadata().severity,
            "warning"
        );
        assert_eq!(
            Rule::PendingAsyncUnsuspendableRead.metadata().severity,
            "error"
        );
        // The server throw for a bare ssrSource: "client" read outside a
        // Loading boundary is unconditional (rc.0 dist/server.js), so the
        // static rule mirrors it as an error.
        // SETTLED_CLEANUP_UNOWNED is a dev *throw* (rc.0 dev bundle emits an
        // error diagnostic and throws), not a warning: an onSettled callback
        // returning a cleanup in an unowned scope halts in dev and drops the
        // cleanup in production. The catalog mirrors the throw as an error.
        assert_eq!(Rule::MissingOwner.metadata().severity, "warning");
        // resolve() under an active observer is a dev *throw* ("Cannot call
        // resolve inside a reactive scope", probed on the rc.0 signals dev
        // bundle), mirrored as an error like the other owned/tracked-scope
        // throws.
        assert_eq!(Rule::ResolveInTrackedScope.metadata().severity, "error");
        // until() carries the same dev throw ("Cannot call until inside a
        // reactive scope", rc.9 signals dev bundle).
        assert_eq!(Rule::UntilInTrackedScope.metadata().severity, "error");
        // flush() in an action step throws FLUSH_IN_ACTION in dev (rc.8 and
        // rc.9 signals dev bundles, probed).
        assert_eq!(Rule::FlushInAction.metadata().severity, "error");
        // flush() in an effect's apply callback is a no-op the dev build warns
        // about (FLUSH_IN_EFFECT_CALLBACK, severity "warn", rc.13 signals).
        assert_eq!(Rule::FlushInEffectCallback.metadata().severity, "warning");
        // A promise-valued static dynamic() source is a dev *throw*
        // ("dynamic(): a static source must resolve synchronously, not to a
        // promise", rc.9 web.dev.js and server.dev.js).
        assert_eq!(Rule::StaticDynamicAsyncSource.metadata().severity, "error");
        // The rich-argument transport throw is unconditional at the default
        // client (probed) — error; the post-flush header drop only occurs
        // when the boundary settles after the shell flush — warning.
        assert_eq!(
            Rule::ServerFunctionRichArgument.metadata().severity,
            "error"
        );
        assert_eq!(Rule::HttpResponseAfterFlush.metadata().severity, "warning");
        assert!(Rule::HttpResponseAfterFlush.metadata().uncertifiable);
    }
}
