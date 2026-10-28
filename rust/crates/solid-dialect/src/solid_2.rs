//! Solid 2.0.
//!
//! Every table here started as an extraction of what `solid-reactive-ir`
//! hardcoded before ADR 0006, not a fresh reading of the 2.0 docs; the
//! engine has since been wired onto this crate, so these tables are now the
//! only place the answers live. Provenance stays recorded per table so a
//! change is checked against the runtime it describes, not against memory.
//!
//! Entries added after the original extraction come from the exact published
//! packages (see [`TABLE`]); each needs the same treatment -- a cited source
//! and a fixture or focused regression test.

use crate::{
    Boundary, CallbackOwner, CleanupRule, ContextRole, Dialect, Execution, OwnerRequirementRole,
    Primitive, ReactiveRole, ResultSlot, TrackedCallbackTiming, Version, lookup, reverse,
};

mod releases;

#[path = "../../../dialects/solid-v2/runtime_configuration.rs"]
mod runtime_configuration;

/// Solid 2.0, answering for one installation of its three archives.
///
/// Almost every answer is the same on every release this vocabulary was read
/// on, so the releases are carried as data rather than as more types: one field
/// per answer the reviewed releases disagree on, each decided by the resolved
/// release of the package that declares it (`releases.rs`, which builds the
/// value from the resolved `solid-js`, `@solidjs/signals` and `@solidjs/web`).
/// `Solid2` the value is the conservative vocabulary
/// ([`Solid2::CONSERVATIVE`]); [`Solid2::RC3`] and [`Solid2::RC9`] are those
/// triples', and [`Solid2::RC13`] carries the new audited store-key premise. The audited vocabulary, the
/// conservative one variant ids are spelled against ([`Solid2::CONSERVATIVE`])
/// and the one a project with no `solid-js` is analyzed under
/// ([`Solid2::DEFAULTED`]) are three names, not one (`releases.rs`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Solid2 {
    /// Strict store-key witnesses read on exact @solidjs/signals rc.13.
    /// Other owners/releases remain unaudited for this new premise.
    store_strict_keys_audited: bool,
    /// B1, from `@solidjs/signals`.
    store_root: releases::StoreRootTyping,
    /// B3: whether this release's `omit` has the predicate form, a lone function
    /// argument the returned view invokes
    /// ([`Solid2::callback_runs_on_result_access`]).
    ///
    /// A runtime difference, not only a typing one:
    ///
    /// - `@solidjs/signals@2.0.0-rc.3`'s `omit(props, ...keys)` never invokes
    ///   a key. Every build only tests membership, `keys.includes(property)`
    ///   in the view's `get`/`has`/`keys` traps and in the non-`Proxy` copy,
    ///   and `new Set(keys)` above four keys: `dist/dev.js:9334-9369` (sha256
    ///   `cc68ed0f…1a79`), `dist/prod/store/utils.js:169-199`
    ///   (`ff62f6f1…acb9`) and `dist/node.cjs:7816` (`bc0e35d3…5c1c`) of the
    ///   published rc.3 `@solidjs/signals` archive.
    ///   A function passed there is a key that matches no property.
    /// - `2.0.0-rc.9` selects the predicate by
    ///   `keys.length === 1 && typeof keys[0] === "function"`
    ///   (`dist/dev.js:4380`) and calls it on every read of the view.
    ///
    /// TypeScript separates the two only where types exist (rc.3 rejects a
    /// function key with TS2345; rc.9 accepts it). In an untyped artifact
    /// nothing in the call does, so the answer comes from the release of the
    /// package that implements `omit`, the resolved `@solidjs/signals`
    /// (`solid-js` only re-exports it): `true` on rc.9 alone, since no
    /// runtime before it tests `typeof keys[0]` (the rc.1-rc.8 review § 3.3).
    /// A signals release nobody compared, or none resolved, keeps `false`,
    /// under its `SC9014` notice.
    omit_predicate_form: bool,
    /// B4: whether `until` is a name this installation exports, from
    /// `solid-js` and `@solidjs/signals` together. `false` answers `None` for
    /// the name everywhere the vocabulary is asked about it, so nothing reaches
    /// the `until` rows or SC2005.
    until: bool,
    /// N4 of the rc.9 review: whether `flush` throws `FLUSH_IN_ACTION` in dev
    /// while an action step is on the stack, from `@solidjs/signals`.
    ///
    /// `2.0.0-rc.8` added the guard (`dist/dev-shared.js:1904-1913`; rc.9
    /// `:2210-2219`): `if (actionStepDepth > 0) throw new Error("[FLUSH_IN_ACTION]
    /// …")` opens `flush`, before its `fn` argument is read, so `flush()` and
    /// `flush(fn)` both throw. `actionStepDepth` is raised only by `action`'s
    /// `step`, around `it.next(v)`/`it.throw(v)` (rc.8 `dist/dev.js:1682-1690`,
    /// rc.9 `:2016-2024`). The production and observe builds take the same
    /// branch and return `fn?.()` without draining (rc.9
    /// `dist/prod/core/scheduler.js:1182-1184`), so nothing throws there. No
    /// release before rc.8 has `actionStepDepth` at all (the rc.1-rc.8 review
    /// § 3, probe R). `true` only for a signals release some review read at
    /// rc.8 or later; an unread or unresolved one keeps `false`, which states
    /// nothing, under its `SC9014` notice.
    flush_in_action: bool,
    /// N6, from `@solidjs/signals`: whether its dev `flush` reports
    /// `FLUSH_IN_EFFECT_CALLBACK` when called from an effect's apply callback
    /// (`dist/dev-shared.js:2687-2698` on rc.13; rc.1 `dist/dev.js:1120`, rc.3
    /// `:1810`, rc.6 `:2286`, rc.8 `dist/dev-shared.js:1935`, rc.9 `:2241`).
    /// rc.0 has no `inEffectCallback` marker. `true` only for a signals
    /// release some review read at rc.1 or later; an unread or unresolved one
    /// keeps `false`.
    flush_in_effect_callback: bool,
    /// B2, from `@solidjs/web`.
    dynamic_options: releases::DynamicOptions,
    /// N3, from `@solidjs/signals`: whether its dev store-setter guard
    /// exempts a root owner ([`Dialect::store_setter_guard_exempts_roots`]).
    store_setter_roots: releases::StoreSetterRootGuard,
    /// N5, from `@solidjs/signals`: whether an optimistic-store setter meets
    /// the owned-scope write guard at all
    /// ([`Dialect::optimistic_store_setter_guarded`]).
    optimistic_store_setter: releases::OptimisticStoreSetterGuard,
    /// ADR 0292, from `solid-js`: whether `lazy` renders the loaded `default`
    /// through `createComponent` ([`Dialect::renders_loaded_default_argument`]).
    ///
    /// A runtime difference. `solid-js@2.0.0-rc.13`'s wrapper renders
    /// `createComponent(Comp, props)` (`dist/solid.dev.js:1425`), whose
    /// `observedComponent` runs the body as `untrack(() => Comp(props),
    /// label)` (`:59`), the strict-read window a JSX-rendered component body
    /// has. rc.3 and rc.9 call `untrack(() => Comp(props))` themselves, with
    /// no label (rc.9 `dist/solid.dev.js:1175-1179`), so the loaded body opens
    /// no strict-read window there. `true` on rc.13 alone; every other
    /// release, unread and unresolved included, keeps `false`.
    lazy_renders_component: bool,
    /// ADR 0353, from `@solidjs/web`: whether `render` and `hydrate` run their
    /// `code` callback under the root they create
    /// ([`Dialect::callback_runs_in_created_root`]). `true` on rc.13 and
    /// rc.17: `render` calls `code()` inside `createRoot(dispose => …)`
    /// (rc.13 `dist/web.dev.js:488-495`, rc.17 `:551-558`), and `hydrate`
    /// reaches that `render` on every path (rc.13 `:1521-1620`, rc.17
    /// `:1626-1735`, possibly after it returns). No other `@solidjs/web`
    /// release was read for this, so each keeps `false`.
    render_creates_root: bool,
    /// ADR 0358, from `@solidjs/web`: which spread keys `assignProp`
    /// installs as listeners (`releases::SpreadEventKeys`).
    spread_event_keys: releases::SpreadEventKeys,
    /// From `@solidjs/signals`: whether the setter `createOptimistic` returns
    /// meets the dev owned-scope write guard
    /// ([`Dialect::optimistic_signal_setter_guarded`]). rc.13 returns
    /// `setSignal.bind(null, node)`; rc.17 returns `optimisticWrite.bind(null,
    /// node)` (`dist/dev.js:2423-2432`), which only queues a guess
    /// (`:294-306`): measured legal in a root body and a memo compute on rc.17,
    /// `REACTIVE_WRITE_IN_OWNED_SCOPE` on rc.13. `true` on read releases
    /// through rc.13; rc.17, unread and unresolved keep `false`.
    optimistic_signal_setter_guarded: bool,
    /// From `@solidjs/signals`: whether a pending accessor read inside
    /// `latest(fn)` still throws `PENDING_ASYNC_UNTRACKED_READ` in a dev
    /// strict-read window. rc.13's `read()` exempts only `isPending`
    /// (`dist/dev-shared.js:6386`); rc.17 hands every `latest`/`isPending`
    /// read to `verdictValue` first (`dist/dev-shared.js:5258-5281`,
    /// `dist/dev.js:1349-1358`), which warns but does not throw (measured).
    /// `true` on read releases through rc.13; rc.17, unread and unresolved
    /// keep `false`, which reports nothing.
    latest_rejects_pending_read: bool,
    /// From `@solidjs/web`: whether `hydrate` calls `code` before it returns
    /// on every path. rc.17 defers it behind `DOMContentLoaded` or a
    /// microtask while the document is loading (`dist/web.dev.js:1631-1645`),
    /// so no inline execution is stated there, though its root still is
    /// (`render_creates_root`); read releases through rc.13 keep `true`.
    hydrate_code_inline: bool,
    /// From `@solidjs/signals`: what the dev owned-scope write guards do while
    /// the runtime disposes an owner ([`Dialect::write_guard_in_disposal`]).
    /// rc.17 adds `!disposalDepth` to `setSignal`, `markRefresh` and
    /// `devGuardStoreSetterWrite` (the rc.17 review C4): `Rejects` on read
    /// releases through rc.13, `StandsDown` on rc.17, `Unknown` for signals
    /// nobody read or that did not resolve.
    disposal_write_guard: crate::WriteGuardInDisposal,
    /// From all three packages: whether rc.17's callback-taking exports
    /// `dynamicComponent`, `dispatchAsInteraction`, `createDeferred`,
    /// `lazyModule` and `onHydrationEnd` are exports, modelled as read on rc.17
    /// ([`Solid2::call_form`], [`Solid2::callback_owners`]). `true` only when
    /// `solid-js`, `@solidjs/signals` and `@solidjs/web` all resolve to it:
    /// `dispatchAsInteraction`'s dev build calls signals' `withInteraction`,
    /// `solid-js`'s client `createDeferred` is signals', and the hydration drain
    /// `onHydrationEnd` waits for is reached from web's `hydrate`.
    rc17_callback_exports: bool,
}

impl Solid2 {
    /// [`Primitive::DynamicComponent`]'s form for one call. `dynamicComponent`
    /// is `dynamicCore(source, options, tagArm)` itself (`solid-js@2.0.0-rc.17`
    /// `dist/solid.dev.js:1444`, `:1545`; `solid.js:1382`, `:1476`;
    /// `solid.observe.js:1417`, `:1511`; `server.js:2374`, `:2436`), the core
    /// rc.17's `dynamic` forwards to (`@solidjs/web` `dist/web.dev.js:2466-2467`),
    /// so its first two arguments select what `dynamic`'s do: `if
    /// (options?.static) return staticDynamic(untrack(source), tagArm);`
    /// (`dist/solid.dev.js:1445`; `if (options?.static) {` in
    /// `dist/server.js:2375`). What it lacks is `dynamic`'s tag arm, so a
    /// string source renders nothing (`case "string": return tagArm ?
    /// tagArm(component, props) : undefined;`, `:1524`; `staticDynamic` ends
    /// `return () => undefined;`, `:1542-1543`) where `dynamic` creates the
    /// element. No form's answer is about what the returned component renders,
    /// and [`Dialect::intrinsic_tag_renderers`] names only the `Dynamic` tag,
    /// so the forms are `dynamic`'s. Probed on the dev, observe and prod client
    /// builds beside `dynamic`: the default source ran 0 times at the call, once
    /// at render under an owner that is not the caller's, and again on a write;
    /// the static source ran once at the call under the caller's owner, never
    /// again, and threw `REACTIVE_WRITE_IN_OWNED_SCOPE` (dev) on a write in a
    /// root; a static thenable threw in dev and rendered `undefined` in prod;
    /// a string rendered nothing. A third argument is the tag arm, which the
    /// runtime calls with each string source (probed); the typings declare two
    /// parameters, and a call passing one states nothing.
    fn dynamic_component_form(
        &self,
        option: &dyn Fn(usize, &str) -> crate::OptionLiteral,
    ) -> Primitive {
        use crate::OptionLiteral;
        // Unreachable through `primitive`, which names it only where it is
        // exported; the bare name answers nothing.
        if !self.rc17_callback_exports {
            return Primitive::DynamicComponent;
        }
        if option(2, "") != OptionLiteral::Absent {
            return Primitive::DynamicUnknownForm;
        }
        match option(1, "static") {
            OptionLiteral::Absent | OptionLiteral::False => Primitive::Dynamic,
            OptionLiteral::True => Primitive::DynamicStatic,
            OptionLiteral::Unknown => Primitive::DynamicUnknownForm,
        }
    }
}

/// The Solid 2 language's own vocabulary, [`Solid2::CONSERVATIVE`], spelled
/// like the unit struct it used to be so the value keeps its name everywhere
/// the engine and its tests hold it. It is not the audited release's: the
/// engine's tests pin the conservative answers, and a release-dependent
/// answer is tested on the variant that gives it.
/// A braced struct lives only in the type namespace, which leaves the value
/// namespace free for this constant.
#[allow(non_upper_case_globals)]
pub const Solid2: Solid2 = Solid2::CONSERVATIVE;

/// Source: the pre-ADR-0006 hardcoded name list `solid-reactive-ir` used to
/// carry (26 names), plus the four the namespace-import expansion adds in
/// `solid-reactive-ir/src/symbols.rs` — `merge`, `refresh`, `affects` from
/// `solid-js`, and `dynamic` from `@solidjs/*`. The engine now asks this
/// table; the old list no longer exists.
///
/// `runWithOwner` is the one name here that came from neither. It is extracted
/// from `solid-js@2.0.0-rc.0`, which re-exports it from `@solidjs/signals`
/// as `runWithOwner<T>(owner: Owner | null, fn: () => T): T` — the same
/// signature 1.x has. The engine had always recognised it by spelling whatever
/// the dialect, so before this it resolved to `PrimitiveName::Other` under 2.0
/// and the vocabulary claimed a name 2.0 exports did not exist.
const TABLE: &[(&str, Primitive)] = &[
    ("action", Primitive::Action),
    ("affects", Primitive::Affects),
    ("children", Primitive::Children),
    ("clientOnly", Primitive::ClientOnly),
    ("createContext", Primitive::CreateContext),
    // New in `solid-js@2.0.0-rc.17`, gated like `until` (`Solid2::exports`):
    // the client builds wrap `@solidjs/signals`' `createDeferred` exactly as
    // `createMemo` is wrapped for hydration (`dist/solid.dev.js:975-976`,
    // `solid.js:932-933`, `solid.observe.js:949-950`); the server builds
    // alias it, `const createDeferred = createMemo;` (`dist/server.js:2035`,
    // `server.dev.js:2205`, `server.observe.js:2135`).
    ("createDeferred", Primitive::CreateDeferred),
    ("createEffect", Primitive::CreateEffect),
    ("createErrorBoundary", Primitive::CreateErrorBoundary),
    ("createLoadingBoundary", Primitive::CreateLoadingBoundary),
    ("createMemo", Primitive::CreateMemo),
    ("createOptimistic", Primitive::CreateOptimistic),
    ("createOptimisticStore", Primitive::CreateOptimisticStore),
    ("createOwner", Primitive::CreateOwner),
    ("createProjection", Primitive::CreateProjection),
    ("createReaction", Primitive::CreateReaction),
    ("createRenderEffect", Primitive::CreateRenderEffect),
    ("createRevealOrder", Primitive::CreateRevealOrder),
    ("createRoot", Primitive::CreateRoot),
    ("createSignal", Primitive::CreateSignal),
    ("createStore", Primitive::CreateStore),
    ("createTrackedEffect", Primitive::CreateTrackedEffect),
    ("deep", Primitive::Deep),
    // New in `@solidjs/web@2.0.0-rc.17` (`dist/web.dev.js:653`, `web.js:484`,
    // `web.observe.js:510`, `server.js:4423`, `server.dev.js:4766`,
    // `server.observe.js:4578`); gated like `until` (`Solid2::exports`).
    ("dispatchAsInteraction", Primitive::DispatchAsInteraction),
    ("dynamic", Primitive::Dynamic),
    // New in `solid-js@2.0.0-rc.17`: `const dynamicComponent = dynamicCore;`
    // (`dist/solid.dev.js:1545`, `solid.js:1476`, `solid.observe.js:1511`,
    // `server.js:2436`, `server.dev.js:2658`, `server.observe.js:2550`),
    // re-exported unchanged by every `@solidjs/web` build (`dist/web.dev.js:2`).
    ("dynamicComponent", Primitive::DynamicComponent),
    ("Errored", Primitive::Errored),
    ("flush", Primitive::Flush),
    ("For", Primitive::For),
    ("getOwner", Primitive::GetOwner),
    ("httpHeader", Primitive::HttpHeader),
    ("httpStatus", Primitive::HttpStatus),
    ("hydrate", Primitive::Hydrate),
    ("isPending", Primitive::IsPending),
    ("latest", Primitive::Latest),
    ("lazy", Primitive::Lazy),
    // New in `solid-js@2.0.0-rc.17` (`dist/solid.dev.js:1408`, `solid.js:1347`,
    // `solid.observe.js:1381`, `server.js:2268`, `server.dev.js:2474`,
    // `server.observe.js:2373`); gated like `until`.
    ("lazyModule", Primitive::LazyModule),
    ("Loading", Primitive::Loading),
    ("mapArray", Primitive::MapArray),
    ("Match", Primitive::Match),
    ("merge", Primitive::Merge),
    ("omit", Primitive::Omit),
    ("onCleanup", Primitive::OnCleanup),
    // Exported from `solid-js@2.0.0-rc.17` (`dist/solid.dev.js:385`,
    // `solid.js:345`, `solid.observe.js:362`, `server.js:2731`,
    // `server.dev.js:3065`, `server.observe.js:2904`); gated like `until`.
    ("onHydrationEnd", Primitive::OnHydrationEnd),
    ("onSettled", Primitive::OnSettled),
    ("reconcile", Primitive::Reconcile),
    ("refresh", Primitive::Refresh),
    ("render", Primitive::Render),
    ("Repeat", Primitive::Repeat),
    ("repeat", Primitive::RepeatMap),
    ("resolve", Primitive::Resolve),
    ("runWithOwner", Primitive::RunWithOwner),
    ("Show", Primitive::Show),
    ("snapshot", Primitive::Snapshot),
    ("Switch", Primitive::Switch),
    // New in `2.0.0-rc.5`: `until<T>(fn: () => T, options?: UntilOptions):
    // Promise<Truthy<T>>` (`@solidjs/signals` `dist/types/signals.d.ts:601`
    // on rc.5, `:608` on rc.9), re-exported from the `solid-js` root
    // (`types/index.d.ts:1`) from the same release. rc.3's typings do not
    // export it (TS2305), so the row answers only where the installation has
    // it (`Solid2::until`); everywhere else the name is unknown.
    ("until", Primitive::Until),
    ("untrack", Primitive::Untrack),
    ("useContext", Primitive::UseContext),
    ("useHead", Primitive::UseHead),
];

/// Every name this dialect exports, derived from [`TABLE`] rather than
/// mirrored beside it. The mirror was a second list to keep in step, and
/// keeping two lists in step by hand is the defect this crate exists to
/// remove one level down.
#[cfg(test)]
pub(crate) fn names() -> Vec<&'static str> {
    TABLE.iter().map(|(name, _)| *name).collect()
}

/// The [`TABLE`] names only some installations export (`Solid2::exports`).
#[cfg(test)]
pub(crate) const RELEASE_GATED_NAMES: &[&str] = &[
    "until",
    "dynamicComponent",
    "dispatchAsInteraction",
    "createDeferred",
    "lazyModule",
    "onHydrationEnd",
];

impl Dialect for Solid2 {
    fn compiler_lowers_event_attribute(&self, name: &str) -> bool {
        // The pinned rc.17 compiler's `is_event_name`
        // (`packages/compiler/src/shared/constants.rs:272-275`): `on` followed
        // by an ASCII uppercase letter. `onclick` and `on-foo` are attributes.
        let bytes = name.as_bytes();
        bytes.len() > 2 && bytes.starts_with(b"on") && bytes[2].is_ascii_uppercase()
    }
    fn spread_key_is_event(&self, key: &str) -> Option<bool> {
        match self.spread_event_keys {
            releases::SpreadEventKeys::Unaudited => None,
            releases::SpreadEventKeys::AnyOn => Some(key.starts_with("on") && !key.contains(':')),
            // rc.17 `assignProp`: `!hasNamespace && prop.startsWith("on") &&
            // (c = prop.charCodeAt(2)) > 64 && c < 91` (`dist/web.dev.js:1943`).
            releases::SpreadEventKeys::OnUppercase => Some(
                !key.contains(':')
                    && key.len() > 2
                    && key.starts_with("on")
                    && key.as_bytes()[2].is_ascii_uppercase(),
            ),
        }
    }
    fn document_dispatch_reaches_no_element_handler(&self) -> bool {
        // Read on the rc.17 bytes (`dist/web.js` `eventHandler`): it walks
        // `e.composedPath()` from the event's target and calls only
        // `node["$$" + type]` handlers, which `addEvent` sets on elements.
        // Only the release whose `eventHandler` was read answers.
        matches!(
            self.spread_event_keys,
            releases::SpreadEventKeys::OnUppercase
        )
    }
    fn optimistic_signal_setter_guarded(&self) -> bool {
        self.optimistic_signal_setter_guarded
    }
    fn host_boolean_exports(&self) -> &'static [crate::HostBooleanExport] {
        // `const isServer = false;`/`true;` in each target, exported, and an
        // unchanged `exports["."]`: rc.13, and rc.17 (`web.js:2100`,
        // `web.dev.js:2412`, `web.observe.js:2151`, `server.js:4432`,
        // `server.dev.js:4775`, `server.observe.js:4587`).
        const TARGETS: &[&str] = &[
            "dist/web.js",
            "dist/web.dev.js",
            "dist/web.observe.js",
            "dist/server.js",
            "dist/server.dev.js",
            "dist/server.observe.js",
        ];
        &[
            crate::HostBooleanExport {
                module: "@solidjs/web",
                export: "isServer",
                version: "2.0.0-rc.13",
                client: false,
                server: true,
                runtime_targets: TARGETS,
            },
            crate::HostBooleanExport {
                module: "@solidjs/web",
                export: "isServer",
                version: "2.0.0-rc.17",
                client: false,
                server: true,
                runtime_targets: TARGETS,
            },
        ]
    }

    fn runtime_configuration_api(
        &self,
        path: &str,
        start: u64,
        end: u64,
    ) -> Option<crate::RuntimeConfigurationApi> {
        runtime_configuration::api(path, start, end)
    }

    fn hydration_host_member_read_is_harmless(&self, member: &str, truthiness_only: bool) -> bool {
        runtime_configuration::harmless_host_read(member, truthiness_only)
    }

    // rc.9 dev-shared.js core read() explicitly excludes pendingCheckActive
    // from the strict pending-read safeguard; verdict.isPending supplies it.
    // Store proxy traps have a different guard, so this is accessor-only.
    // rc.17 sends `latest` reads to `verdictValue` too (`latest_rejects_pending_read`).
    fn callback_handles_pending_accessor_read(&self, primitive: Primitive, index: usize) -> bool {
        index == 0
            && (primitive == Primitive::IsPending
                || (primitive == Primitive::Latest && !self.latest_rejects_pending_read))
    }

    fn version(&self) -> Version {
        Version::V2
    }

    fn direct_jsx_return_is_component(&self) -> bool {
        true
    }

    /// 2.0 folds the store APIs into core and moves the DOM package out.
    /// The web subpaths are owned too: `export_modules` answers with them
    /// for the generated export tables' rows, and a module this dialect
    /// reports exports from is a module it must own.
    fn modules(&self) -> &'static [&'static str] {
        &[
            "solid-js",
            "solid-js/internal",
            "solid-js/refresh",
            "@solidjs/web",
            "@solidjs/web/frames",
            "@solidjs/web/frames/client",
            "@solidjs/web/frames/server",
            "@solidjs/web/jsx-dev-runtime",
            "@solidjs/web/jsx-runtime",
            "@solidjs/web/serialization",
            "@solidjs/web/serialization/decode",
            "@solidjs/web/server-functions",
            "@solidjs/web/server-functions/client",
            "@solidjs/web/server-functions/rich-args",
            "@solidjs/web/server-functions/server",
            "@solidjs/web/storage",
        ]
    }

    /// 2.0 splits the definitions across three archives: the reactive core
    /// lives in `@solidjs/signals`, the DOM primitives in `@solidjs/web`, and
    /// `solid-js` re-exports the core under the dialect's public spellings
    /// while declaring some of its own. The other `@solidjs/*` packages
    /// (`router`, `meta`, `start`, `element`) are consumers of these three and
    /// are deliberately absent.
    fn primitive_defining_packages(&self) -> &'static [&'static str] {
        &["solid-js", "@solidjs/signals", "@solidjs/web"]
    }

    fn ecosystem_scopes(&self) -> &'static [&'static str] {
        &["@solidjs/"]
    }

    fn primitive(&self, name: &str) -> Option<Primitive> {
        static INDEX: crate::NameIndex = crate::NameIndex::new();
        lookup(&INDEX, &[TABLE], name).filter(|primitive| self.exports(*primitive))
    }

    /// The two `dynamic` call forms have no row of their own in [`TABLE`]: a
    /// form is reached from the name, never the other way round, so both
    /// spell as the export they are a call of.
    fn name_of(&self, primitive: Primitive) -> Option<&'static str> {
        match primitive {
            Primitive::DynamicStatic | Primitive::DynamicUnknownForm => Some("dynamic"),
            _ if !self.exports(primitive) => None,
            _ => reverse(TABLE, primitive),
        }
    }

    /// `dynamic(source, options?)` is the one 2.0 export whose runtime is
    /// chosen by an option. `@solidjs/web@2.0.0-rc.9` opens every build of it
    /// with `if (options?.static) …` (`dist/web.dev.js:2199`,
    /// `dist/web.js:2034`, `dist/web.observe.js:2056`, `dist/server.js:3729`,
    /// `dist/server.dev.js:3977`, `dist/server.observe.js:3817`), a
    /// truthiness test, so:
    ///
    /// - a literal `static: true` is [`Primitive::DynamicStatic`];
    /// - no options, a `null`/`undefined` literal, an exact literal without
    ///   `static`, or `static: false` is the default runtime, unchanged: the
    ///   first statement is skipped and the lazy tracked memo is built exactly
    ///   as rc.3 builds it;
    /// - anything the syntax does not prove is
    ///   [`Primitive::DynamicUnknownForm`], which states nothing. Keeping the
    ///   default model there instead would record `tracked` in a callback
    ///   summary, and claim a created owner, for a source the runtime may run
    ///   once, untracked, under the caller's owner.
    ///
    /// Answered from the resolved `@solidjs/web` (`Solid2::dynamic_options`),
    /// because only rc.9's runtime reads the option. On rc.0-rc.8 every call
    /// is the default form, whatever it passes: rc.0-rc.6 declare
    /// `dynamic(source)` with one parameter (TS2554 on a second), and rc.7 and
    /// rc.8 accept `DynamicOptions { deferStream }` (TS2353 on `static`) that
    /// the client bundle never reads. Modelling rc.9's static form there would
    /// state rc.9 behaviour -- no owner, an untracked source -- about a runtime
    /// that creates the memo and its owner as always (the rc.1-rc.8 review
    /// § 6). On a web release nobody read, an option-bearing call is
    /// [`Primitive::DynamicUnknownForm`], which states nothing.
    fn call_form(
        &self,
        primitive: Primitive,
        option: &dyn Fn(usize, &str) -> crate::OptionLiteral,
    ) -> Primitive {
        use crate::OptionLiteral;
        if primitive == Primitive::DynamicComponent {
            return self.dynamic_component_form(option);
        }
        if primitive != Primitive::Dynamic {
            return primitive;
        }
        match (self.dynamic_options, option(1, "static")) {
            (releases::DynamicOptions::Ignored, _)
            | (_, OptionLiteral::Absent | OptionLiteral::False) => Primitive::Dynamic,
            (releases::DynamicOptions::StaticForm, OptionLiteral::True) => Primitive::DynamicStatic,
            (releases::DynamicOptions::StaticForm, OptionLiteral::Unknown)
            | (releases::DynamicOptions::Unread, OptionLiteral::True | OptionLiteral::Unknown) => {
                Primitive::DynamicUnknownForm
            }
        }
    }

    /// Source: `solid-reactive-ir/src/execution_role.rs`, the argument-index
    /// match. `createEffect`/`createRenderEffect` take `(compute, apply)`, so
    /// the tracked callback is at index 1.
    fn callback_positions(&self, primitive: Primitive) -> &'static [usize] {
        match primitive {
            // createEffect/createRenderEffect take (compute, apply);
            // runWithOwner takes (owner, fn).
            // dispatchAsInteraction(event, fn).
            Primitive::CreateEffect
            | Primitive::CreateRenderEffect
            | Primitive::RunWithOwner
            | Primitive::DispatchAsInteraction => &[1],
            // createErrorBoundary(fn, fallback) and createLoadingBoundary(fn,
            // fallback): argument 0 is the tracked body, argument 1 renders
            // when it throws or suspends.
            Primitive::CreateErrorBoundary | Primitive::CreateLoadingBoundary => &[0, 1],
            // repeat(count, mapFn) and mapArray(list, mapFn) both map at 1.
            Primitive::RepeatMap | Primitive::MapArray => &[1],
            Primitive::CreateMemo
            | Primitive::CreateDeferred
            | Primitive::CreateTrackedEffect
            | Primitive::CreateSignal
            | Primitive::CreateStore
            | Primitive::CreateProjection
            | Primitive::CreateOptimistic
            | Primitive::CreateOptimisticStore
            | Primitive::Dynamic
            // rc.17: `lazyModule(fn, moduleUrl?)`, `onHydrationEnd(callback)`.
            | Primitive::LazyModule
            | Primitive::OnHydrationEnd
            // Both `dynamic` call forms keep the source at 0; the form
            // changes how it runs, not where it sits.
            | Primitive::DynamicStatic
            | Primitive::DynamicUnknownForm
            | Primitive::ClientOnly
            | Primitive::Flush
            | Primitive::Untrack
            | Primitive::OnSettled
            | Primitive::CreateReaction
            | Primitive::Action
            // latest(fn), isPending(fn), resolve(fn), until(fn, options?)
            // each take one thunk.
            | Primitive::Latest
            | Primitive::IsPending
            | Primitive::Resolve
            | Primitive::Until
            | Primitive::Lazy
            | Primitive::UseHead
            // ADR 0323: `children(fn)` is a lazy memo over `fn`.
            | Primitive::Children
            | Primitive::CreateRevealOrder => &[0],
            _ => &[],
        }
    }

    /// Source: the published signal/runtime implementations, transcribed at
    /// the primitive boundary. Tracked computes such as `createMemo`,
    /// `createTrackedEffect`, derived state/store factories, and `dynamic`
    /// are deliberately absent: putting them here would erase their tracked
    /// read obligations. `createRoot` and `createRevealOrder` are present
    /// because each clears tracking while establishing an owner.
    ///
    /// `runWithOwner` is deferred for the same reason `untrack` is, and on the
    /// same evidence: `@solidjs/signals`' implementation sets `tracking =
    /// false` around the call. It swaps the owner, not the observer, so a read
    /// inside it does not subscribe. 1.x classifies it the same way.
    ///
    /// `latest(fn)` and `isPending(fn)` are deliberately absent. Each catches
    /// `NotReadyError` around `fn` — that is what they change — but neither
    /// clears tracking, so reads inside them subscribe in the caller's scope
    /// exactly as a bare `fn()` would. Listing them here would erase those
    /// read obligations; their `callback_executions` rows say the same thing.
    ///
    /// `flush(fn)` is absent for the same reason. It runs `fn()` inline
    /// between a `syncDepth` increment and the drain and touches neither
    /// `context` nor the listener (`@solidjs/signals@2.0.0-rc.0`
    /// `dist/dev.js:1085-1099`, rc.3 `dist/dev.js:1788-1802`, rc.9
    /// `dist/dev-shared.js:2202-2230`), so its callback inherits the caller's
    /// read role exactly as [`Solid2::callback_preserves_owner_write_context`]
    /// already makes it inherit the caller's write role. Probed on every
    /// published rc.0-rc.9 client build, dev and prod: `createMemo(() =>
    /// flush(() => count()))` re-runs when `count` is written, as the bare
    /// read does and `untrack` does not; `getObserver()` inside `flush(fn)` is
    /// the memo's; an effect compute reading through `flush(fn)` re-runs; and
    /// `flush(() => count())` in a component body raises
    /// `STRICT_READ_UNTRACKED` (dev) exactly as `count()` there does.
    fn runs_callback_deferred(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::CreateRoot
                | Primitive::CreateRevealOrder
                | Primitive::Untrack
                | Primitive::OnSettled
                | Primitive::CreateReaction
                | Primitive::Action
                | Primitive::ClientOnly
                | Primitive::RunWithOwner
                // resolve(fn) returns a Promise -- the thunk's reads settle
                // outside the current computation.
                | Primitive::Resolve
                // until(fn) is resolve's shape (see `callback_executions`):
                // its predicate is the compute of a user effect under a fresh
                // root, and the caller has no observer for it to subscribe.
                | Primitive::Until
                | Primitive::Lazy
                // rc.17's `lazyModule` calls `fn` only from the returned
                // `load`, and `onHydrationEnd` only from a microtask or the
                // hydration drain; neither runs it during the call
                // (`callback_executions`).
                | Primitive::LazyModule
                | Primitive::OnHydrationEnd
                // dynamic(source, { static: true }) is
                // `staticDynamic(untrack(source))`: the signals `untrack`,
                // which clears the listener around the call.
                | Primitive::DynamicStatic
        )
    }

    /// Source: the rc.0 write guard, which exempts children-forbidden
    /// scopes — `!(context._config & CONFIG_CHILDREN_FORBIDDEN)` at the
    /// setter (`dev.js:3154-3172`), `refresh` (`:3316-3331`), and action
    /// (`:4312-4400`) throw sites, with the runtime comment "leaf imperative
    /// scopes (tracked effects, onSettled) stay legal". Empirically probed:
    /// writes, `refresh`, and action calls inside `createTrackedEffect` and
    /// owner-backed `onSettled` succeed on the published rc.0 bundle.
    fn leaf_scopes_allow_writes(&self) -> bool {
        true
    }

    /// Source: `untrack` clears `tracking` but not `context`
    /// (`@solidjs/signals@2.0.0-rc.17` `dist/dev-shared.js:5099-5115`, rc.0
    /// `dev.js:2928-2942`), and the write guard keys on `context`. Probed: a setter,
    /// `refresh`, or action call inside `untrack(...)` within a memo,
    /// component body, or effect compute throws
    /// `REACTIVE_WRITE_IN_OWNED_SCOPE` / `ACTION_CALLED_IN_OWNED_SCOPE`,
    /// while the identical `untrack` call in an event handler succeeds. So
    /// for write legality `untrack` is transparent to its call site. (The
    /// official RFC text claims `untrack` blocks allow writes; the rc.0
    /// runtime contradicts it — the runtime wins.)
    ///
    /// `dynamic(source, { static: true })` runs its source through that same
    /// `untrack` (`solid-js@2.0.0-rc.17` `dist/solid.dev.js:1445`, in
    /// `dynamicCore`: `staticDynamic(untrack(source), tagArm)`; rc.9
    /// `@solidjs/web` `dist/web.dev.js:2199`), with no owner of its own, so a write in a static source is exactly as legal as
    /// at the `dynamic` call.
    ///
    /// `flush(fn)` runs `fn()` inline between a `syncDepth` increment and the
    /// drain, touching neither `context` nor the listener
    /// (`@solidjs/signals@2.0.0-rc.17` `dist/dev-shared.js:2646-2668`, rc.0
    /// `dist/dev.js:1085-1099`, rc.3
    /// `dist/dev.js:1788-1802`, rc.9 `dist/dev-shared.js:2202-2230`). Probed
    /// on every published rc.0-rc.9 dev client build: `getOwner()`
    /// inside `flush(fn)` is the caller's owner, and a signal setter inside
    /// `flush(fn)` throws `REACTIVE_WRITE_IN_OWNED_SCOPE` in a memo compute,
    /// a component body and a `createRoot` body, inline or passed by name,
    /// while the same call at module scope succeeds.
    ///
    /// `dispatchAsInteraction(event, fn)` is `return fn();` in the production
    /// and server builds (`@solidjs/web@2.0.0-rc.17` `dist/web.js:484`,
    /// `dist/server.js:4423`) and `OBSERVE.attribution.withInteraction({…},
    /// fn)` in dev and observe (`dist/web.dev.js:653-660`,
    /// `dist/web.observe.js:510-517`), which is `fn()` with no engine and
    /// `interactionStart(ref)`, `fn()`, `interactionEnd(returned)` with one
    /// (`@solidjs/signals@2.0.0-rc.17` `dist/dev-shared.js:335-347`; the
    /// engine's hooks, `dist/dev.attribution.js:199-220`, only push and pop
    /// its own interaction frame). Probed on the dev build, with and without
    /// the attribution engine enabled: `getOwner()` inside is the caller's, a
    /// signal setter inside throws `REACTIVE_WRITE_IN_OWNED_SCOPE` in a
    /// `createRoot` body and a memo compute exactly as the bare write does,
    /// and the same call outside any owner succeeds.
    fn callback_preserves_owner_write_context(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::Untrack
                | Primitive::DynamicStatic
                | Primitive::Flush
                | Primitive::DispatchAsInteraction
        )
    }

    /// Source: `onCleanup(fn)` pushes `fn` on the current owner's `_disposal`
    /// (`@solidjs/signals@2.0.0-rc.17` `dist/dev-shared.js:1415-1421`), and
    /// the engine calls that list only from `runDisposal` (`:1302-1330`) with
    /// `enterDisposal()` around it (`:1314`), on the owner's disposal and on a
    /// computation's rerun alike. An effect apply's returned cleanup is no
    /// registration: a rerun calls it outside that bracket (probed: a signal
    /// setter in it throws), and only the final teardown raises the depth for
    /// it (`:1290`).
    fn callback_runs_only_in_disposal(&self, primitive: Primitive, argument: usize) -> bool {
        primitive == Primitive::OnCleanup && argument == 0
    }

    /// Source: rc.17's `onCleanup` returns `cleanup(fn)` (`@solidjs/signals@2.0.0-rc.17`
    /// `dist/dev.js:2377-2405`, `dist/prod/signals.js:65-67`), and `cleanup`
    /// returns `fn` with or without an owner (`dist/dev-shared.js:1415-1421`,
    /// `dist/prod/core/owner.js:252-256`); `solid-js` re-exports it
    /// (`dist/solid.dev.js:2`) and its server builds return `fn` too
    /// (`dist/server.dev.js:229-234`). Probed: `const h = onCleanup(() =>
    /// setV(1)); h();` in a memo throws `REACTIVE_WRITE_IN_OWNED_SCOPE`.
    ///
    /// Only rc.17's bytes were read for this, so the answer keys on the same
    /// read `@solidjs/signals` release as `write_guard_in_disposal`: its
    /// `StandsDown` is exactly `Release::Read(RC17)` (`releases.rs`). Every
    /// other release, read for other questions or not, answers `Unknown`. No
    /// vocabulary digit of its own: one would double the variant table.
    fn disposal_callback_returned(
        &self,
        primitive: Primitive,
        argument: usize,
    ) -> crate::DisposalCallbackReturn {
        if primitive == Primitive::OnCleanup
            && argument == 0
            && self.disposal_write_guard == crate::WriteGuardInDisposal::StandsDown
        {
            crate::DisposalCallbackReturn::Returned
        } else {
            crate::DisposalCallbackReturn::Unknown
        }
    }

    /// Source: a rerun of an effect calls the cleanup its apply returned
    /// outside `runDisposal`'s bracket (probed on rc.17: a signal setter in it
    /// throws), and only the final teardown raises the depth for it
    /// (`@solidjs/signals@2.0.0-rc.17` `dist/dev-shared.js:1290`). The
    /// settled and reaction primitives' cleanups are not read for this.
    fn returned_cleanup_runs_outside_disposal(
        &self,
        primitive: Primitive,
        argument: usize,
    ) -> bool {
        matches!(
            (primitive, argument),
            (Primitive::CreateEffect | Primitive::CreateRenderEffect, 1)
                | (Primitive::CreateTrackedEffect, 0)
        )
    }

    /// Answered from the resolved `@solidjs/signals` (`releases.rs`): rc.17
    /// adds `!disposalDepth` to `setSignal` (`dist/dev-shared.js:5620-5626`),
    /// `markRefresh` (`:5801-5806`) and `devGuardStoreSetterWrite`
    /// (`dist/dev.js:5811-5812`); rc.13 and every earlier read release have no
    /// such test (the rc.17 review C4: a setter in a cleanup throws on rc.13,
    /// is legal on rc.17). A signals release nobody read, or that did not
    /// resolve, answers `Unknown`.
    fn write_guard_in_disposal(&self) -> crate::WriteGuardInDisposal {
        self.disposal_write_guard
    }

    /// Source: `createRoot(init)` is `createOwner()` then `runWithOwner(owner,
    /// init)` (`@solidjs/signals@2.0.0-rc.3` `dist/dev.js:2275-2281` for
    /// `createOwner`, which marks every owner it makes `_root: true`; the rc.9
    /// review § 3.3 finds the `createRoot` slice byte-identical, and rc.9's
    /// own `solid-js` export only forwards to it). So `init` runs during the
    /// call with that root as the ambient `context` the write guards read.
    ///
    /// Probed on the published rc.0-rc.9 triples, dev client builds: a signal
    /// setter, and `refresh(memo)`, directly in a `createRoot` body throw
    /// `REACTIVE_WRITE_IN_OWNED_SCOPE` on every release, whether the root is at
    /// module scope, in a component body, in a memo compute or in an effect
    /// apply. Prod builds carry no guard and throw nothing.
    ///
    /// `createRoot`, and `render` and `hydrate` on the audited `@solidjs/web`
    /// rc.13 and rc.17 (`render_creates_root`). Their `code` argument is called
    /// inside the `createRoot` that `render` makes (`@solidjs/web@2.0.0-rc.13`
    /// `dist/web.dev.js:488-495`, rc.17 `:551-558`), and `hydrate` reaches that
    /// `render` on every path (rc.13 `:1521-1620`, rc.17 `:1626-1735`), so the
    /// write guard reads the same root there. On rc.17 that holds on the path
    /// that runs `code` after `hydrate` returns too (`:1631-1645` calls
    /// `hydrate` again from the listener), so the root is claimed while the
    /// timing is not (`hydrate_code_inline`). Only rc.13 and rc.17 were read
    /// for `render`, so no other release claims it. Not
    /// `runWithOwner(owner, fn)`, which also runs `fn` inline under an owner,
    /// but a supplied owner can be a leaf (`getOwner()` in a tracked effect),
    /// where every write is legal. `createRevealOrder` makes a root too, but
    /// was not probed.
    fn callback_runs_in_created_root(&self, primitive: Primitive, argument: usize) -> bool {
        argument == 0
            && (primitive == Primitive::CreateRoot
                || (self.render_creates_root
                    && matches!(primitive, Primitive::Render | Primitive::Hydrate)))
    }

    /// ADR 0353. Off rc.13, `render`'s and `hydrate`'s `code` callback is not
    /// audited as a root body ([`Solid2::render_creates_root`]), so a write
    /// directly in it is unplaced rather than legal: a call at module scope is
    /// legal only if no root wraps the callback, and nothing read says so.
    fn callback_root_is_unaudited(&self, primitive: Primitive, argument: usize) -> bool {
        argument == 0
            && !self.render_creates_root
            && matches!(primitive, Primitive::Render | Primitive::Hydrate)
    }

    /// ADR 0323 (review round 10). `createRoot(init)` is `runWithOwner(createOwner(),
    /// () => init(dispose))` (`@solidjs/signals@2.0.0-rc.13`
    /// `dist/dev-shared.js:3478-3481`) and `runWithOwner` swaps `context` and
    /// sets `tracking = false` around `fn()` and nothing else
    /// (`dist/dev-shared.js:6919-6947`); `strictRead` is never touched, and a
    /// read warns whenever it is set (`dist/dev-shared.js:6511-6516`). A
    /// component body runs under `untrack(() => Comp(props), label)`, which sets
    /// it for the synchronous call (`:5863-5888`). So a read in either callback
    /// warns exactly where the same read written at the call would, and
    /// nowhere else (an event handler or a timer has no window). `untrack(fn)`
    /// without a label *sets* `strictRead = false`, so it is the explicit
    /// untracked intent and stays out of this set. `flush` clears neither and
    /// is already modelled as inheriting the caller's role; `batch` is not an
    /// export of this vocabulary. 1.x had no strict-read window at all. The
    /// other primitives the dialect lists as `runs_callback_deferred` were read
    /// the same way: `onSettled`, `action` and `clientOnly` run their callback
    /// from a queue or an invocation, never at the call; `resolve` and `until`
    /// evaluate it as an effect compute (`recompute` clears `strictRead`,
    /// `dist/dev-shared.js:4890-4896`); and
    /// `createReaction`'s invalidation callback and `lazy`'s loader are
    /// reporting slots already, because the runtime runs them in a window of
    /// its own.
    fn callback_inherits_calling_window(&self, primitive: Primitive, argument: usize) -> bool {
        matches!(
            (primitive, argument),
            (Primitive::CreateRoot, 0)
                | (Primitive::RunWithOwner, 1)
                // `createOwner()` then `runWithOwner(owner, () => { const value
                // = fn(); ... })` (`@solidjs/signals@2.0.0-rc.13` `dist/dev.js:10170-10179`).
                | (Primitive::CreateRevealOrder, 0)
        )
    }

    /// Source: `solid-js`'s dev `createComponent` is `devComponent`, which runs
    /// the component as `createRoot(() => { …; return untrack(() =>
    /// Comp(props)) }, { transparent: true })` (`solid-js@2.0.0-rc.3`
    /// `dist/dev.js:35-53` and `:1116-1118`; rc.0 the same shape; rc.9 renames
    /// it `observedComponent`, `dist/solid.dev.js:35-58` and `:1148-1150`).
    /// `untrack` keeps the owner, so the owner a write directly in the body
    /// meets is that root (`_root: true`, probed on rc.3 and rc.9). The prod
    /// build calls `untrack(() => Comp(props))` with no root, and carries no
    /// write guard either.
    ///
    /// Probed on every published rc.0-rc.9 dev client build, a store setter
    /// directly in a component body (inline, through `untrack` or `flush(fn)`,
    /// through a helper, and in a nested component) answers exactly as one in
    /// a `createRoot` body: `createStore`'s throws on rc.0 and rc.9 and not on
    /// rc.1-rc.8, `createOptimisticStore`'s throws on rc.9 only. A signal,
    /// optimistic-signal or action call there throws on every release.
    fn component_body_runs_under_root(&self) -> bool {
        true
    }

    /// Answered from the resolved `@solidjs/signals` (`releases.rs`, N3 of the
    /// rc.9 review). rc.1-rc.8's `devGuardStoreSetterWrite` opens with
    /// `if (context && !context._root && …)`; rc.9 drops `!context._root`
    /// (`dist/dev-shared.js:5878`, citing #3500), so `false` there only. rc.0
    /// has no setter-entry guard, and its `createStore` and
    /// `createOptimisticStore` setters disagree under a root (probed), so it
    /// keeps `true`, which claims nothing.
    fn store_setter_guard_exempts_roots(&self) -> bool {
        self.store_setter_roots == releases::StoreSetterRootGuard::Exempt
    }

    /// Answered from the resolved `@solidjs/signals` (`releases.rs`, N5):
    /// `false` on rc.0 only, whose optimistic store writes take the engine's
    /// path and meet no owned-scope guard (probed: legal in a memo and an
    /// effect compute on rc.0, `REACTIVE_WRITE_IN_OWNED_SCOPE` on rc.1-rc.9).
    fn optimistic_store_setter_guarded(&self) -> bool {
        self.optimistic_store_setter == releases::OptimisticStoreSetterGuard::Guarded
    }

    /// Source: rc.0 `onSettled` (`dev.js:4855-4893`). Called under a live
    /// children-capable owner it becomes `createTrackedEffect(() =>
    /// untrack(cb))` — a leaf owner where the leaf-scope rules apply. Called
    /// out-of-band (event handler, no owner, inside another leaf) the
    /// callback is enqueued as a plain function: `onCleanup` inside it warns
    /// `NO_OWNER_CLEANUP` instead of throwing, primitives do not throw, and
    /// `flush()` is a silent no-op (all probed). `createTrackedEffect` is a
    /// leaf owner unconditionally and stays out of this list.
    fn leaf_owner_requires_owned_call_site(&self, primitive: Primitive) -> bool {
        primitive == Primitive::OnSettled
    }

    /// The owner is a synchronous dynamic scope: `@solidjs/signals` sets it
    /// around each computation and `runWithOwner` and restores it before
    /// returning, so no owner is current on an empty stack. Probed on
    /// `solid-js`/`@solidjs/signals` 2.0.0-rc.3 and rc.9, dev and prod builds,
    /// in Chromium 153, for every one of the fourteen reviewed fresh-stack
    /// schedulers (`queueMicrotask`, `setTimeout`, `setInterval`,
    /// `requestAnimationFrame`, `requestIdleCallback`, `Promise.then`/`catch`/
    /// `finally`, `scheduler.postTask`, and the `IntersectionObserver`,
    /// `ResizeObserver`, `MutationObserver`, `PerformanceObserver` and
    /// `ReportingObserver` callbacks), each scheduled inside a `createRoot`,
    /// inside a `createMemo` compute and at module scope: `getOwner()` is
    /// `null` in the callback; an `onCleanup` there raises `NO_OWNER_CLEANUP`
    /// (dev) and never runs on the root's disposal; a `createEffect` there
    /// raises `NO_OWNER_EFFECT` (dev); an `onSettled` returning a cleanup
    /// raises `SETTLED_CLEANUP_UNOWNED` (dev) and the cleanup never runs.
    /// Control: a `createRoot` created in the callback owns its `onCleanup`,
    /// which runs on that root's disposal.
    ///
    /// The same answer covers code after an `await` (`owners::AwaitContinuations`):
    /// it resumes from a promise continuation, on the same empty stack. Probed
    /// on the published `@solidjs/signals` 2.0.0-rc.0, rc.3, rc.6, rc.8 and
    /// rc.9 dev builds and the rc.9 prod build, under Node 24: inside a
    /// `createRoot`, `getOwner()` is non-null before the `await` and `null`
    /// after it; an `onCleanup` after it raises `NO_OWNER_CLEANUP` (dev) and
    /// does not run when the root is disposed, while one before it does; a
    /// `createEffect` after it raises `NO_OWNER_EFFECT` (dev).
    fn fresh_stack_callback_owner(&self) -> Option<CallbackOwner> {
        Some(CallbackOwner::None)
    }

    /// `createStore` returns `Readonly<T>` over the root record
    /// (`@solidjs/signals@2.0.0-rc.0` and `rc.3`), so a write to one of its own
    /// properties is TS2540 and belongs to TypeScript. Nested records and props
    /// objects are not readonly and stay this checker's.
    ///
    /// From `@solidjs/signals@2.0.0-rc.7` it declares `Store<T> = T`: the
    /// write type-checks and the runtime still drops it, so a vocabulary built
    /// from such a signals answers `false` and SC2003 reports it
    /// (`releases.rs`).
    fn store_root_properties_are_readonly(&self) -> bool {
        self.store_root == releases::StoreRootTyping::Readonly
    }

    fn release_owners(&self) -> &'static [crate::ReleaseOwner] {
        releases::OWNERS
    }

    /// ADR 0324. The audited `@solidjs/router` releases, each read in full on
    /// the installed bytes (`dist/index.js` for the default condition, and for
    /// the `solid` condition `dist/index.jsx` re-exporting `routing.js` and
    /// `utils.js`):
    ///
    /// | release | `dist/index.js` sha256 | `routing.js` | `utils.js` |
    /// | --- | --- | --- | --- |
    /// | `2.0.0-next.26` | `b657bc8d…baa35` | `639f16fd…153ea` | `562b280a…7e2a` |
    /// | `2.0.0-next.41` | `23d0fe1c…36de2` | `a07c96fa…f2042` | `7ae70266…19db0` |
    ///
    /// What was read: `createMatcher` assigns `match.params[key] = locSegment`
    /// (a segment of `location.split("/")`) and a splat's remainder string
    /// (`index.js:76,85`; `utils.js:81,91`); `mergeParams` is an
    /// `Object.assign` of those (`index.js:1040-1046`; `routing.js:483-489`);
    /// `useParams()` is `useRoute().params` (`index.js:700-702`;
    /// `routing.js:163-165`), `createMemoObject` of that merge unless
    /// `utils.paramsWrapper` replaces it (`index.js:1306`; `routing.js:725-728`,
    /// `954`), a Proxy whose `get` is a memo of `fn()[property]`
    /// (`index.js:113`; `utils.js:124-`). The declared type is
    /// `Record<string, string | undefined>` (`types.d.ts:27`). next.41 passes
    /// each segment and the splat remainder through `decodeParam`, a
    /// `decodeURIComponent` that keeps a malformed escape raw (a string
    /// either way; `index.js:142,151,162`; `utils.js:140,150,161`);
    /// `mergeParams` `index.js:1340`, `routing.js:470`; `useParams`
    /// `index.js:1013`, `routing.js:167` (its argument unread); the wrapper
    /// `index.js:1595`, `routing.js:682-685`, `869`; the Proxy `index.js:177`,
    /// `utils.js:183`; `types.d.ts:41`. It also calls `config.links(router,
    /// basePath)` with the whole router context (`claims.js:42-44`), so a
    /// config naming `links` withholds the premise on every release. Versions
    /// before `2.0.0-next.26` (`next.16` to `next.19` in the corpus, and
    /// `0.15.3`) were not read and are not claimed.
    fn router_params_are_url_strings(&self, version: &str) -> bool {
        releases::AUDITED_ROUTER_RELEASES.contains(&version)
    }

    fn audited_router_releases(&self) -> &'static [&'static str] {
        releases::AUDITED_ROUTER_RELEASES
    }

    fn audited_router_digest(&self, version: &str) -> Option<&'static str> {
        releases::AUDITED_ROUTER_DIGESTS
            .iter()
            .find_map(|(audited, digest)| (*audited == version).then_some(*digest))
    }

    /// ADR 0335. The audited `@solidjs/router` releases for route
    /// definitions: `2.0.0-next.16`, `next.18`, `next.26` and `next.41`, each read on
    /// `dist/` (the `solid` condition's `index.jsx` and its modules, and the
    /// `dist/index.js` bundle). `createRoutes` copies a definition's
    /// `component` into the route record (`routing.js:328-332` next.16,
    /// `363-367` next.18, `381-385` next.26, `370-376` next.41); its only
    /// render is `createComponent(component, …)` in the outlet
    /// (`routing.js:810-811`, `882-883`, `968-969`, `902`); `preloadRoute`
    /// and `createRouteContext` read `component.preload` only, and from
    /// next.41 the unexported `SERVER_ROUTE` brand (`serverRouteShared.js`),
    /// which only `serverRouteComponent` values carry and whose `render` and
    /// `call` never call the component. next.41's new exports (`pendingLinks`,
    /// the preload strategies, `useInRouter`, `formAction`,
    /// `serverRouteComponent`) hand out no record, and `keysFor` returns
    /// strings. `createBranches` recurses into `children`
    /// arrays the same way. Nothing else reads `component`, and nothing calls
    /// it. Project code can reach it as a value (`useRouteMatches()()[i].route`,
    /// `instance.routes`), which the premise's key-read veto closes. next.41
    /// calls `config.links(router, basePath)` with the router context and its
    /// matches, so a config naming `links` withholds the premise.
    fn audited_route_config_digest(&self, version: &str) -> Option<&'static str> {
        releases::AUDITED_ROUTE_CONFIG_DIGESTS
            .iter()
            .find_map(|(audited, digest)| (*audited == version).then_some(*digest))
    }

    /// ADR 0334. The audited `@tanstack/solid-router` releases, read on all
    /// three builds (`dist/source` for the `solid` condition, `dist/esm`,
    /// `dist/cjs` by call pattern) and the `@tanstack/router-core` each loads:
    ///
    /// | release | router-core |
    /// | --- | --- |
    /// | `2.0.0-beta.29` | `1.171.15` |
    /// | `2.0.0-rc.0` | `1.171.16` |
    /// | `2.0.0-rc.4` | `1.171.22` |
    /// | `2.0.0-rc.6` | `1.171.22` |
    /// | `2.0.0-rc.8` | `1.171.22` |
    /// | `2.0.0-rc.9` | `1.171.22` |
    ///
    /// What was read: every factory (`createFileRoute`, `createRoute`,
    /// `createRootRoute`, `createRootRouteWithContext`, `createLazyRoute`,
    /// `createLazyFileRoute`; `route.jsx`, `fileRoute.js`) stores its options
    /// as `route.options` and reads none of the four keys. `Match.jsx` reads
    /// `options.component` into a memo rendered as
    /// `<Dynamic component={OutComponent()}/>`; `pendingComponent`,
    /// `errorComponent` and `notFoundComponent` are rendered as `<X/>` or
    /// through `Dynamic` (`CatchBoundary.jsx` renders its `errorComponent`
    /// prop through `Dynamic`; `renderRouteNotFound.jsx` as `<X {...data}/>`),
    /// or tested for truth. router-core reads them only for
    /// `route.options[type]?.preload?.()`, truth tests, and
    /// `Object.assign(route.options, lazyRoute.options)`. None of the four
    /// values is called.
    ///
    /// rc.8 was audited by diffing its `npm pack` against rc.6 on all three
    /// builds: every file above is byte-identical and the same router-core
    /// is loaded; only `RouterProvider`, `Transitioner`, `link` and `router`
    /// changed (an `isServer ?? router.isServer` test, navigation
    /// attribution, intent handlers installed through getters), `Asset`'s
    /// compiled output differs on `esm`/`cjs` (a `mergeProps` dropped), and
    /// the new `flightData` module reads a server-function response's
    /// headers. None reads one of the four keys.
    ///
    /// rc.9 was audited the same way against rc.8, with the same router-core:
    /// the route-option reads are byte-identical on `dist/source`; changed are
    /// `RouterProvider`, `Transitioner`, `router` and `useBlocker` (observe
    /// navigation attribution, through the new `observe` module, which reads
    /// a matched route's `fullPath` and params), `ClientOnly`, `awaited`,
    /// `registryTransfer`, `useMatch`, and on `esm`/`cjs` the compiled
    /// `Asset`, `link` and `CatchBoundary` (`$$click` to `_$$click`). None
    /// reads one of the four keys.
    fn audited_route_components(&self, version: &str) -> Option<crate::AuditedRouteRelease> {
        releases::AUDITED_ROUTE_RELEASES
            .iter()
            .find(|release| release.version == version)
            .copied()
    }

    fn audited_installation(&self) -> &'static [(&'static str, &'static str)] {
        releases::AUDITED_INSTALLATION
    }

    fn review_installation(
        &self,
        installed: &[crate::InstalledRelease<'_>],
    ) -> crate::InstallationReview {
        releases::review(installed)
    }

    fn variant_key(&self) -> Option<&'static str> {
        self.key()
    }

    fn variants(&self) -> &'static [&'static dyn Dialect] {
        &releases::OTHER_VARIANTS
    }

    fn variant_by_key(&self, key: &str) -> Option<&'static dyn Dialect> {
        Solid2::of_key(key).map(|vocabulary| vocabulary as &'static dyn Dialect)
    }

    fn defaulted_vocabulary(&self) -> Option<&'static dyn Dialect> {
        Some(Solid2::DEFAULTED.interned())
    }

    /// Source: rc.0 store setters put the store into the Writing set for the
    /// duration of the draft callback, so `setStore(d => { store.value = 7 })`
    /// commits through the original proxy (probed: the write lands after
    /// `flush()`; the same write through *another* store's proxy inside that
    /// callback is silently dropped, and outside any setter it is silently
    /// dropped too).
    fn store_setter_callback_enables_proxy_writes(&self) -> bool {
        true
    }

    /// Audited rc.13 @solidjs/signals store Get: the strict-read warning
    /// tests typeof key == "string" and excludes "then" before reading a
    /// tracked node. Symbol/protocol keys are not strict-read witnesses.
    fn store_key_warns_strict_read(&self, key: &str) -> bool {
        self.store_strict_keys_audited && key != "then"
    }

    /// ADR 0293. rc.9 and rc.13 `@solidjs/signals` store Get test
    /// `typeof key === "string" && key !== "then"` before warning (rc.9
    /// `dist/dev.js:6229-6235`, rc.13 `dist/dev.js:6660-6675`): a promise
    /// resolution probes `.then`, and that is not a read the user wrote.
    /// rc.3's gate has no `then` clause (`dist/dev.js:7662`), but no
    /// vocabulary carries that difference, so every release withholds `then`:
    /// only a violation is lost there, never one invented.
    fn store_key_may_skip_strict_read(&self, key: &str) -> bool {
        key == "then"
    }

    /// Source: rc.0 `devComponent` (`solid-js` dev entry `dev.js:35-50`)
    /// wraps the body in `untrack(() => Comp(props), '<Name>')`, so
    /// `STRICT_READ_UNTRACKED` fires only when a prop getter reads reactive
    /// state during that window. Probed: a component receiving
    /// `{ title: "Hello" }` reads `props.title` in its body with no warning;
    /// the same component receiving `{ get title() { return sig() } }`
    /// warns. Which of the two a prop is is decided entirely by the callers.
    fn props_require_caller_proof(&self) -> bool {
        true
    }

    /// The 2.0 `reactive-read-after-await` page claims store-path and props
    /// member reads; dependency collection genuinely ends at the first await
    /// for those exactly as for accessor calls (`@solidjs/signals` tracks via
    /// the ambient listener, which the resumed continuation no longer has).
    fn reports_member_reads_after_await(&self) -> bool {
        true
    }

    /// Source: RFC 10 — Solid 2.0 moves the `"use server"` directive and the
    /// `@solidjs/web/server-functions` runtime into core, and the pinned
    /// `@solidjs/web@2.0.0-rc.0` ships that runtime (`server-functions/dist`,
    /// probed). 1.x server functions belonged to SolidStart, not this
    /// vocabulary.
    fn models_server_functions(&self) -> bool {
        true
    }

    /// Source: `solid-reactive-ir/src/lib.rs` `read_is_under_loading` and
    /// `jsx_element_is_loading`. 2.0's error boundary is `Errored`.
    fn boundary_kind(&self, tag: &str) -> Option<Boundary> {
        match tag {
            "Loading" => Some(Boundary::Async),
            "Errored" => Some(Boundary::Error),
            _ => None,
        }
    }

    /// Source: the `solid-js@2.0.0-rc.0` and `@solidjs/web@2.0.0-rc.0`
    /// implementations, read
    /// rather than inferred. What matters is whether the body creates an owner
    /// before invoking the callback:
    ///
    /// - `createRoot(init)` is `createOwner()` then `runWithOwner`. Creates.
    /// - `resolve(fn)` wraps `fn` in `createRoot` too, which its signature
    ///   does not suggest -- it reads as a plain thunk-taker.
    /// - `flush(fn)`, `untrack(fn)`, `latest(fn)`, `isPending(fn)` call
    ///   `fn()` directly. No owner is created, so the callback is exactly as
    ///   owned as the call site: Inherits, not None. (What each wraps the
    ///   call in differs — `untrack` clears tracking, `latest`/`isPending`
    ///   only catch `NotReadyError` and leave reads subscribing — but that is
    ///   `runs_callback_deferred`'s question, not this one's.)
    /// - `createEffect(compute, apply)` owns at 0 and runs `apply` unowned,
    ///   which is why these are argument positions and not
    ///   [`Solid2::callback_positions`].
    ///
    /// Anything absent is unmodelled, not ownerless.
    fn callback_owners(&self, primitive: Primitive) -> &'static [(usize, CallbackOwner)] {
        match primitive {
            Primitive::CreateRoot
            | Primitive::CreateMemo
            // `createDeferred` is `accessor(computed(prev => { node =
            // getOwner(); …; r = fn(prev); … }, options))`
            // (`@solidjs/signals@2.0.0-rc.17` `dist/dev.js:8897-8909`,
            // `dist/prod/deferred.js:127-138`, `dist/observe/deferred.js:129-140`):
            // `fn` runs as the compute of the node it creates. Probed beside
            // `createMemo` on the dev, observe and prod client builds and both
            // server builds (where it is `createMemo`): `getOwner()` inside is
            // a new owner whose parent is the caller's, a `createEffect` there
            // raises no `NO_OWNER_EFFECT`, and an `onCleanup` there runs when
            // the enclosing root is disposed.
            | Primitive::CreateDeferred
            | Primitive::CreateSignal
            | Primitive::CreateStore
            | Primitive::CreateProjection
            | Primitive::CreateOptimistic
            | Primitive::CreateOptimisticStore
            // ADR 0323: the memo `children` creates owns `fn`'s runs.
            | Primitive::Children
            | Primitive::Resolve => &[(0, CallbackOwner::Creates)],
            // `@solidjs/signals@2.0.0-rc.9` `until` (`dist/dev.js:2717-2785`,
            // `dist/prod/signals.js:530`): `new Promise(… createRoot(dispose =>
            // { …; effect(fn, …) }))`, the predicate is the compute of an
            // effect created under that fresh root -- except when
            // `options.signal` is already aborted, where it rejects before
            // `createRoot` and never runs `fn` at all (`:2734`). Either way no owner of
            // the caller's is used.
            Primitive::Until => &[(0, CallbackOwner::Creates)],
            // The supplied owner is nullable. The call-site classifier
            // sharpens this to Creates or None when its value is proven.
            Primitive::RunWithOwner => &[(1, CallbackOwner::Conditional)],
            // createOwner() then runWithOwner(owner, () => fn()).
            Primitive::CreateRevealOrder => &[(0, CallbackOwner::Creates)],
            // The loader is invoked from the wrapper component body.
            Primitive::Lazy => &[(0, CallbackOwner::Inherits)],
            // `lazyModule(fn)` returns `read = load`, and `load` is `if (p)
            // return p; const cur = p = fn(); …` (`solid-js@2.0.0-rc.17`
            // `dist/solid.dev.js:1408-1431`, `solid.js:1347-1368`,
            // `server.js:2268-2272`), so `fn` runs on the stack of whoever
            // first calls the result or its `preload`, and nothing is set up
            // around it. Owned like `lazy`'s loader, at those invocations
            // (`callback_requires_return_invocation`); both cache the first
            // promise, so only the first invocation runs it. Probed on the
            // dev, observe and prod client builds and both server builds: not
            // run at the call; run once, at the first call of the result, with
            // that caller's `getOwner()` and `getObserver()` (a client memo
            // calling it re-ran on a write to a signal `fn` read, on that
            // first run only); in dev a module-scope call left a
            // `createEffect` in `fn` raising `NO_OWNER_EFFECT` and a call in a
            // root body made a write in `fn` throw
            // `REACTIVE_WRITE_IN_OWNED_SCOPE`. The server's `peek` also calls
            // `load` (`dist/server.js:2284-2289`), the client's never does; no
            // invocation site counts `peek`.
            Primitive::LazyModule => &[(0, CallbackOwner::Inherits)],
            // `onHydrationEnd(callback)` is `queueMicrotask(callback)` unless a
            // hydration is in progress, and then a push drained by
            // `drainHydrationCallbacks` (`solid-js@2.0.0-rc.17`
            // `dist/solid.dev.js:385-404`, `solid.js:345-364`); the server
            // builds only `queueMicrotask(callback)` (`dist/server.js:2731-2733`).
            // The drain runs where hydration ends, which `hydrate`'s own
            // `finally { sharedConfig.hydrating = false; }` reaches
            // (`@solidjs/web@2.0.0-rc.17` `dist/web.dev.js:1732-1735`), on
            // its caller's stack. Probed on the dev, observe and prod client
            // builds: registered outside a hydration (module scope, a root, a
            // memo compute, a rendered component) `getOwner()` in the callback
            // is `null`, `onCleanup` raises `NO_OWNER_CLEANUP`, `createEffect`
            // `NO_OWNER_EFFECT` (dev) and a write is legal; registered in a
            // component `hydrate` renders, it ran before `hydrate` returned,
            // with no owner when `hydrate` was called at module scope and with
            // the enclosing root as owner (a write threw) when it was called
            // in a `createRoot` body. The server builds: `null` everywhere.
            Primitive::OnHydrationEnd => &[(0, CallbackOwner::Conditional)],
            // Both boundaries own the scope their body runs in, and render
            // their fallback under it.
            Primitive::CreateErrorBoundary | Primitive::CreateLoadingBoundary => {
                &[(0, CallbackOwner::Creates), (1, CallbackOwner::Creates)]
            }
            // `createEffect` queues its apply (`effect()` enqueues every
            // `EFFECT_USER` run), so the apply always runs from the flush,
            // where no owner is current.
            Primitive::CreateEffect => &[(0, CallbackOwner::Creates), (1, CallbackOwner::None)],
            // `createRenderEffect` does not. `effect()` ends
            // `recompute(node, true); !options?.defer && (… || options?.schedule
            // ? node._queue.enqueue(…) : runEffect(node))`
            // (`@solidjs/signals@2.0.0-rc.3` `dist/dev.js:5285-5289`, the same
            // in `dist/prod/core/effect.js:16-17`; rc.9 `dist/dev.js:1612-1620`),
            // and `runEffect` sets neither `context` nor `tracking`, so the
            // first apply runs before the call returns under the caller's
            // owner. Probed on both prereleases, dev and prod: `getOwner()` in
            // the first apply is the enclosing root, a render effect created
            // there raises no `NO_OWNER_EFFECT`, and a setter there throws
            // `REACTIVE_WRITE_IN_OWNED_SCOPE` (dev builds) under a root or a
            // component.
            // Every later run comes from the flush, where `getOwner()` is
            // `null` and a render effect created there raises
            // `NO_OWNER_EFFECT`.
            Primitive::CreateRenderEffect => &[
                (0, CallbackOwner::Creates),
                (1, CallbackOwner::InheritsFirstRun),
            ],
            // `createReaction` is deliberately absent from this arm: 1.x
            // runs its invalidation callback as a leaf owner, but the
            // RC.0 runtime allocates the reaction a computation like
            // `createEffect` does, so 2.0 does not end the ownership chain
            // there. The `dialect-solid-2` fixture
            // pair pins the difference.
            Primitive::CreateTrackedEffect | Primitive::OnSettled => &[(0, CallbackOwner::Leaf)],
            // Not a leaf (1.x's model — the `dialect-solid-2` fixture pins
            // that difference), but not owned either: the RC.0 runtime
            // invokes the invalidation callback with no owner, and
            // `onCleanup` inside it emits `NO_OWNER_CLEANUP`.
            Primitive::CreateReaction => &[(0, CallbackOwner::None)],
            // Client builds either call the loader at declaration time or on
            // first render; server builds never call it.
            Primitive::ClientOnly => &[(0, CallbackOwner::Conditional)],
            // The browser implementation wraps the thunk in `effect`; SSR
            // registers it for evaluation under the renderer's scope.
            Primitive::UseHead => &[(0, CallbackOwner::Creates)],
            // Both build a row owner -- `_owner: createOwner()` in
            // @solidjs/signals -- so a primitive created in a row callback is
            // disposed with the row rather than leaking.
            Primitive::MapArray | Primitive::RepeatMap => &[(1, CallbackOwner::Creates)],
            Primitive::Flush | Primitive::Untrack | Primitive::Latest | Primitive::IsPending => {
                &[(0, CallbackOwner::Inherits)]
            }
            // `fn()` at the call, every build (see
            // `callback_preserves_owner_write_context`): probed, `getOwner()`
            // inside is the caller's in a root, a memo compute and at module
            // scope (`null`), dev, observe and prod.
            Primitive::DispatchAsInteraction => &[(1, CallbackOwner::Inherits)],
            // Both mount entry points wrap the application callback in the
            // root owner they create (`render` is `createRoot` plus insert).
            Primitive::Render | Primitive::Hydrate => &[(0, CallbackOwner::Creates)],
            // `dynamic(() => Comp)` renders the resolved component under the
            // component owner the wrapper creates, so primitives created in
            // the thunk are disposed with the dynamic node.
            Primitive::Dynamic => &[(0, CallbackOwner::Creates)],
            // The static form builds no memo and no wrapper computation: the
            // source is `untrack(source)` at the `dynamic` call itself
            // (`@solidjs/web@2.0.0-rc.9` `dist/web.dev.js:2199`,
            // `dist/server.js:3730`), under whatever owner the caller has.
            Primitive::DynamicStatic => &[(0, CallbackOwner::Inherits)],
            // `DynamicUnknownForm` is absent: its two runtimes disagree
            // (Creates versus Inherits), and absent is "unmodelled".
            _ => &[],
        }
    }

    /// Source: `solid-js@2.0.0-rc.0`'s `types/client/flow.d.ts`, read from
    /// the installed package, which spells the three `<For>` forms out:
    ///
    /// ```text
    /// keyed?: true          children: (item: T[number],           index: Accessor<number>)
    /// keyed: false          children: (item: Accessor<T[number]>, index: number)
    /// keyed: (item) => any  children: (item: Accessor<T[number]>, index: Accessor<number>)
    /// ```
    ///
    /// `<Repeat>` is 2.0's answer to `<Index>` and is not the same shape: its
    /// children take `(index: number)`, a plain number, so it has no accessor
    /// parameter at all.
    /// The dynamic-flag form claims nothing anywhere: RFC 03 says to "avoid
    /// dynamic boolean `keyed` values with function children" precisely
    /// because the callback shape is mode-specific — a truthy flag hands the
    /// callback raw values where the falsy overload hands accessors.
    /// Claiming either shape would fabricate a source for the other, so the
    /// table refuses, mirroring the 1.x dialect's stance on its boolean
    /// `keyed={expr}` (`Show`/`Match` there). `CustomKey` only reaches here
    /// when the key expression is proven a function.
    fn children_accessor_parameters(
        &self,
        primitive: Primitive,
        key: crate::KeyForm,
    ) -> &'static [usize] {
        match primitive {
            // `Show`/`Match` take a boolean `keyed` only (rc.0 flow.d.ts has
            // no key-function overload for them); a function value would be
            // truthy at runtime and select the raw-value overload, so the
            // proven-function form also claims no accessor.
            Primitive::Show | Primitive::Match => match key {
                crate::KeyForm::Keyed | crate::KeyForm::CustomKey | crate::KeyForm::DynamicFlag => {
                    &[]
                }
                crate::KeyForm::Unkeyed | crate::KeyForm::Absent => &[0],
            },
            Primitive::For => match key {
                crate::KeyForm::CustomKey => &[0, 1],
                crate::KeyForm::Unkeyed => &[0],
                crate::KeyForm::Absent | crate::KeyForm::Keyed => &[1],
                crate::KeyForm::DynamicFlag => &[],
            },
            _ => &[],
        }
    }

    /// `createStore` and `createOptimisticStore` return `[store, setStore]`;
    /// `createProjection` returns the store itself, which the result-slot
    /// table also describes.
    fn returns_store(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::CreateStore | Primitive::CreateOptimisticStore | Primitive::CreateProjection
        )
    }

    /// `createRoot` returns `runWithOwner(owner, () => init(dispose))`.
    /// Source: rc.9 signals-negative-rows audit § 3.1; rc.13 runtime audit
    /// row `createRoot` records both the core and hydration bodies unchanged.
    /// This says nothing about the dispose parameter's value or lifetime.
    fn returned_callback_value(&self, primitive: Primitive) -> Option<usize> {
        (primitive == Primitive::CreateRoot).then_some(0)
    }

    /// Source: the published declarations, read from the exact package
    /// artifacts of the audited prerelease. In
    /// `solid-js@2.0.0-rc.3/types/server/signals.d.ts`:
    /// `createSignal<T>(value: Exclude<T, Function>, options?): Signal<T>`,
    /// `createMemo<T>(compute, options?): SourceAccessor<T>`, and
    /// `type SourceAccessor<T> = Refreshable<SignalAccessor<T>>`. `Signal` is
    /// re-exported from `@solidjs/signals@2.0.0-rc.3`, whose
    /// `dist/types/signals.d.ts` declares
    /// `type Signal<T> = [get: SourceAccessor<T>, set: Setter<T>]`. rc.3 is
    /// the prerelease this repository's fixtures and ecosystem corpus pin; the
    /// same three declarations were checked in rc.0 and rc.5 and are
    /// unchanged. `SourceAccessor` is one of the names
    /// [`Dialect::type_role`] already classifies as an accessor, so these rows
    /// restate an audited 2.0 declaration.
    ///
    /// A single-value `returns` summary is *not* the authority for this
    /// question: it cannot express a tuple, so its silence for `createSignal`
    /// is not the 2.0 vocabulary's negative claim.
    ///
    /// `createOptimistic` also returns a `Signal<T>` and `createProjection` a
    /// store; both stay absent until a proof needs them and their own review
    /// lands.
    fn reactive_result_slot(&self, primitive: Primitive, slot: ResultSlot) -> Option<ReactiveRole> {
        match (primitive, slot) {
            (Primitive::CreateSignal, ResultSlot::TupleItem(0)) => Some(ReactiveRole::Accessor),
            (Primitive::CreateSignal, ResultSlot::TupleItem(1)) => Some(ReactiveRole::Setter),
            // `createDeferred` returns `accessor(computed(…))`, as `createMemo`
            // does (`accessor_getter_length`).
            (Primitive::CreateMemo | Primitive::CreateDeferred, ResultSlot::Whole) => {
                Some(ReactiveRole::Accessor)
            }
            _ => None,
        }
    }

    /// ADR 0338. Source: `@solidjs/signals@2.0.0-rc.13`. Every getter these
    /// primitives return is `accessor(node)`, which is
    /// `read.bind(null, node)`: `dist/dev.js` `accessor` (line 2341; callers
    /// `createSignal` 2350 and 2354, `createMemo` 2357, `createOptimistic`
    /// 2931 and 2935) and `dist/prod/signals.js` `accessor` (line 68). `read`
    /// is declared `function read(el)` (`dist/dev-shared.js` line 6333,
    /// `dist/prod/core/core.js` line 1494), so a function of length 1 bound
    /// to one argument has `length` 0, in both builds. `solid-js@2.0.0-rc.13`
    /// wraps the client primitives only to choose between them
    /// (`dist/solid.dev.js` `hydratedCreateSignal`, `createOptimistic`) and
    /// returns what they return; its server build (`dist/server.js`
    /// `createSignal`, `createMemo`, `createSyncMemo`, `createOptimistic`)
    /// returns arrow functions with no parameter, also length 0. A
    /// `createSignal(fn)` getter and a `createOptimistic` getter are the same.
    fn accessor_getter_length(&self, primitive: Primitive, slot: ResultSlot) -> Option<u32> {
        matches!(
            (primitive, slot),
            (
                Primitive::CreateSignal | Primitive::CreateOptimistic,
                ResultSlot::TupleItem(0)
            ) | (
                Primitive::CreateMemo | Primitive::CreateDeferred,
                ResultSlot::Whole
            )
        )
        .then_some(0)
    }

    /// ADR 0183. Source: `@solidjs/signals@2.0.0-rc.9`, `dist/dev-shared.js`
    /// `computed` and `setupComputedNode`: `createMemo(fn, options)` builds a
    /// computed node and ends with `!options?.lazy && recompute(self, true)`,
    /// so with no options argument `fn` runs during the call, once, with the
    /// new node as the current owner. The node is children-capable
    /// (`CONFIG_CHILDREN_FORBIDDEN` is set only on leaf owners), so a signal
    /// write there trips `setSignal`'s owned-scope guard. Probed on the rc.9 dev
    /// and prod builds under Node 24, inside a `createRoot`: a memo with no
    /// options ran its compute once at creation; one with `{ lazy: true }` did
    /// not run; a compute that writes a signal raised
    /// `REACTIVE_WRITE_IN_OWNED_SCOPE` in dev, rethrown when the memo was read
    /// (prod has no guard). Only the one-argument call is stated: an options
    /// argument can carry `lazy`.
    ///
    /// `createEffect(compute, effect)`'s compute is the same: probed the same
    /// way, it ran once during the call (the effect function is queued), and a
    /// write in it is caught and logged by the effect rather than thrown. Only
    /// the two-argument call is stated, so no options argument is in play.
    ///
    /// rc.17's `createDeferred(fn)` is `accessor(computed(wrapper, undefined))`
    /// whose wrapper calls `fn(prev)` before anything can suspend it
    /// (`@solidjs/signals@2.0.0-rc.17` `dist/dev.js:8897-8909`). Probed beside
    /// `createMemo` on the dev, observe and prod client builds: `fn` ran once
    /// at the call, did not with `{ lazy: true }`, and a write in it threw
    /// `REACTIVE_WRITE_IN_OWNED_SCOPE` (dev) when the accessor was read.
    fn eager_owned_computation_slot(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> bool {
        matches!(
            (primitive, argument, argument_count),
            (Primitive::CreateMemo | Primitive::CreateDeferred, 0, 1)
                | (Primitive::CreateEffect, 0, 2)
        )
    }

    /// Source: the match this replaced in `solid-reactive-ir/src/static_api.rs`,
    /// which was 2.0-shaped and correct here. Unchanged on purpose — the point
    /// of moving it was 1.x, where every one of these is a different number.
    ///
    /// `createStore` and `createOptimisticStore` have two forms. The plain
    /// `createStore(value, options?)` puts options at 1 and the derived
    /// `createStore(fn, initial, options?)` at 2; only the derived form takes a
    /// compute, and the rule that asks this is about computes.
    fn options_argument(&self, primitive: Primitive) -> Option<usize> {
        match primitive {
            Primitive::CreateMemo
            | Primitive::CreateDeferred
            | Primitive::CreateSignal
            | Primitive::CreateOptimistic
            | Primitive::CreateTrackedEffect => Some(1),
            Primitive::CreateStore
            | Primitive::CreateProjection
            | Primitive::CreateOptimisticStore
            | Primitive::CreateEffect
            | Primitive::CreateRenderEffect => Some(2),
            _ => None,
        }
    }

    /// Spelled out per the trait's rule that an options slot alone is not
    /// evidence for a particular option key. Only the signal-family
    /// constructors route `options.sync` into their node's `CONFIG_SYNC`:
    /// the store family (`createStore(fn, …)`, `createProjection`,
    /// `createOptimisticStore`) rebuilds its node options with only
    /// `loadingValue`/`name` (`@solidjs/signals@2.0.0-rc.0` dev bundle,
    /// `createProjectionInternal`), `sync` is absent from their option
    /// types, and probing confirms a `sync: true` async store derive never
    /// emits `SYNC_NODE_RECEIVED_ASYNC` — the option is inert there, so a
    /// rule keyed on it would flag runtime-legal code.
    fn supports_sync_option(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::CreateMemo
                | Primitive::CreateSignal
                | Primitive::CreateOptimistic
                | Primitive::CreateTrackedEffect
                | Primitive::CreateEffect
                | Primitive::CreateRenderEffect
        )
    }

    /// `merge(...sources)` replaces each function source with
    /// `createMemo(source)` (`@solidjs/signals@2.0.0-rc.9` `dist/dev.js:4290`).
    fn wraps_function_arguments_in_memo(&self, primitive: Primitive) -> bool {
        primitive == Primitive::Merge
    }

    /// Reuses the audited slot and schedule facts below. Deferred slots retain
    /// their exact trigger; registration alone cannot establish delivery.
    fn browser_callback_trigger(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> Option<crate::BrowserCallbackTrigger> {
        use crate::BrowserCallbackTrigger as Trigger;
        self.callback_execution_at(primitive, argument, argument_count)?;
        // The public MemoOptions excludes lazy, but JS and an assertion can
        // still pass it to computed(compute, options). No option object is
        // proved by an argument-count-only question.
        if matches!(primitive, Primitive::CreateMemo | Primitive::CreateDeferred)
            && argument == 0
            && argument_count > 1
        {
            return Some(Trigger::ComputationDemand);
        }
        if self.synchronous_callback_slot(primitive, argument, argument_count) {
            return Some(Trigger::DuringCall);
        }
        match (primitive, argument) {
            (
                Primitive::CreateMemo
                | Primitive::CreateDeferred
                | Primitive::CreateEffect
                | Primitive::CreateRenderEffect,
                0,
            ) if self.tracked_callback_timing(primitive, argument, argument_count)
                == Some(TrackedCallbackTiming::DuringCall) =>
            {
                Some(Trigger::DuringCall)
            }
            (Primitive::CreateEffect | Primitive::CreateRenderEffect, 1)
            | (Primitive::CreateTrackedEffect, 0) => Some(Trigger::EffectFlush),
            (Primitive::OnCleanup, 0) => Some(Trigger::OwnerDisposal),
            (Primitive::OnSettled, 0) => Some(Trigger::Settlement),
            _ => None,
        }
    }

    fn browser_children_trigger(
        &self,
        primitive: Primitive,
    ) -> Option<crate::BrowserCallbackTrigger> {
        (self.renders_children_through_callback(primitive)
            || matches!(primitive, Primitive::Loading | Primitive::Errored))
        .then_some(crate::BrowserCallbackTrigger::RenderSelection)
    }

    fn browser_callback_member_trigger(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
        member: &str,
    ) -> Option<crate::BrowserCallbackTrigger> {
        (matches!(
            primitive,
            Primitive::CreateEffect | Primitive::CreateRenderEffect
        ) && argument == 1
            && argument_count >= 2
            && matches!(member, "effect" | "error"))
        .then_some(if member == "error" {
            crate::BrowserCallbackTrigger::ComputationError
        } else {
            crate::BrowserCallbackTrigger::EffectFlush
        })
    }

    fn effect_apply_defer_options(&self, primitive: Primitive, argument: usize) -> Option<usize> {
        (matches!(
            primitive,
            Primitive::CreateEffect | Primitive::CreateRenderEffect
        ) && argument == 1)
            .then_some(2)
    }

    fn browser_children_dead(
        &self,
        primitive: Primitive,
        when_false: bool,
        each_empty: bool,
        count_zero: bool,
    ) -> bool {
        (when_false && matches!(primitive, Primitive::Show | Primitive::Match))
            || (each_empty && primitive == Primitive::For)
            || (count_zero && primitive == Primitive::Repeat)
    }

    /// The four `createX(fn, ...)` derived forms were read from the runtime
    /// bundled by `solid-js@2.0.0-rc.0`: `createSignal`, `createOptimistic`,
    /// `createStore` and `createOptimisticStore` all branch on
    /// `typeof first === "function"` and build a computed from it, so
    /// argument 0 is a tracked compute exactly when a function is passed.
    ///
    /// `latest` and `isPending` are `Inline` and it is worth saying why, since
    /// reads inside them do subscribe: they run immediately in the caller's
    /// scope and never re-run on their own. `Tracked` here would mean "this
    /// primitive re-runs it", which neither does.
    ///
    /// `flush` is `Inline` too, and since the engine's call graph follows
    /// these execution rows, a callback handed to `flush` is reachable.
    /// `flushSync(fn)` does invoke `fn`, so the row is the truthful one; its
    /// callbacks are deferred scopes, so that reachability changes no read
    /// or owner diagnostic.
    ///
    /// The effect apply at argument 1 is `Deferred` for both effect
    /// constructors, and for `createRenderEffect` that is an attribution
    /// answer, not a schedule. Its first apply runs *during* the call
    /// (see [`Solid2::callback_owners`]), with the caller's `tracking` still
    /// set, so a read there does subscribe an enclosing tracked computation
    /// (probed: a memo whose compute creates the render effect re-runs when a
    /// signal read only in that apply changes). What `Deferred` keeps true is
    /// the part the strict-read rule mirrors: `runEffect` opens the
    /// `"an effect callback"` strict-read window on every run, the first
    /// included, and a read in any later run subscribes nothing. The first
    /// run's owner, and with it the fact that its reads are the caller's, is
    /// stated by `callback_owners` (`CallbackOwner::InheritsFirstRun`, which
    /// the engine reads to keep those reads in the caller's summary); neither
    /// is read off this word.
    ///
    /// This row also decides [`Dialect::runs_callback_synchronously`], so the
    /// evidence is worth stating exactly: `@solidjs/signals`' `flush(fn)` is
    /// `syncDepth++; try { return fn() } finally { flush(); syncDepth-- }`
    /// (2.0.0-rc dev bundle), so the callback is invoked and its value returned
    /// **during** the call; moving it to `Deferred` would claim the callback
    /// has not run when the call returns, which the runtime contradicts.
    /// `onCleanup(fn)` only registers `fn` on the current owner, or returns it
    /// when there is none (`@solidjs/signals@2.0.0-rc.9` `dist/dev.js:2174`
    /// and `cleanup`, `dist/dev-shared.js:2855`; `dist/prod/signals.js:57`).
    /// It never calls `fn` during the call.
    fn callback_executions(&self, primitive: Primitive) -> &'static [(usize, Execution)] {
        match primitive {
            Primitive::CreateMemo
            // rc.17: a memo's compute (`callback_owners`). Probed: a signal
            // `fn` reads re-ran it on a write, and `getObserver()` inside is
            // the node `getOwner()` is.
            | Primitive::CreateDeferred
            | Primitive::CreateProjection
            | Primitive::CreateTrackedEffect
            | Primitive::CreateSignal
            | Primitive::CreateStore
            | Primitive::CreateOptimistic
            | Primitive::CreateOptimisticStore
            | Primitive::Dynamic
            | Primitive::UseHead
            // ADR 0323: `children(fn)` is `createMemo(fn, { lazy: true })`
            // (`solid-js@2.0.0-rc.13` `dist/solid.js:21-24`,
            // `dist/solid.dev.js:20-23`; the same bytes in rc.0, rc.6, rc.8
            // and rc.9), so `fn` is a computation of its own and a read in it
            // subscribes that memo, never the body that wrote the literal.
            // Lazy: it first runs when the result is read, not during the
            // call, so no `TrackedCallbackTiming` is stated for it.
            | Primitive::Children => &[(0, Execution::Tracked)],
            Primitive::CreateEffect | Primitive::CreateRenderEffect => {
                &[(0, Execution::Tracked), (1, Execution::Deferred)]
            }
            Primitive::CreateErrorBoundary | Primitive::CreateLoadingBoundary => {
                &[(0, Execution::Tracked), (1, Execution::Tracked)]
            }
            Primitive::MapArray => &[(1, Execution::Tracked)],
            Primitive::RepeatMap => &[(0, Execution::Tracked), (1, Execution::Inline)],
            // `onCleanup(fn)` registers `fn` on the current owner and returns;
            // the runtime runs it when that owner is disposed or re-runs,
            // never inside the strict-read window of the body that registered
            // it (probed on 2.0.0-rc.9, dev, from a component body: a read in
            // the callback raises no `STRICT_READ_UNTRACKED`). It is attributed
            // `Deferred`, as `onSettled` is. The execution *word* is stated
            // separately (`callback_execution_word_at`) and is unchanged.
            Primitive::OnCleanup
            | Primitive::CreateReaction
            | Primitive::OnSettled
            | Primitive::Resolve
            // The same attribution answer as `resolve`, on the same reading:
            // the predicate subscribes the effect `until` creates, never the
            // caller (who has no observer, or the call throws).
            | Primitive::Until
            | Primitive::Lazy
            // Not run during the call (`callback_owners`): `lazyModule`'s `fn`
            // from the returned `load`, as `lazy`'s loader; `onHydrationEnd`'s
            // callback from a microtask or the hydration drain.
            | Primitive::LazyModule
            | Primitive::OnHydrationEnd
            | Primitive::Action
            | Primitive::ClientOnly => &[(0, Execution::Deferred)],
            Primitive::CreateRoot
            | Primitive::CreateRevealOrder
            | Primitive::Flush
            | Primitive::Untrack
            // `staticDynamic(untrack(source))`: invoked once, before
            // `dynamic` returns, and never again (`@solidjs/web@2.0.0-rc.9`
            // `dist/web.dev.js:2199`, `dist/web.js:2034`).
            | Primitive::DynamicStatic
            | Primitive::Latest
            | Primitive::IsPending => &[(0, Execution::Inline)],
            Primitive::RunWithOwner => &[(1, Execution::Inline)],
            // `dispatchAsInteraction(event, fn)` returns `fn()`'s value, once,
            // before it returns, and touches neither the owner nor the listener
            // (probed on dev, observe and prod: a memo whose compute reads a
            // signal only inside it re-runs when the signal is written, and
            // `getObserver()` inside is the memo's).
            Primitive::DispatchAsInteraction => &[(1, Execution::Inline)],
            // `render(() => <App/>, el)` and `hydrate` invoke the code
            // callback once, immediately, under the root they create.
            // Source: `solidjs-web.json`'s callback rows, the same shape 1.x
            // models for its `solid-js/web` pair. rc.17's `hydrate` may defer
            // it (`hydrate_code_inline`), so no execution is stated there.
            Primitive::Render => &[(0, Execution::Inline)],
            Primitive::Hydrate if self.hydrate_code_inline => &[(0, Execution::Inline)],
            _ => &[],
        }
    }

    /// Read from `@solidjs/signals@2.0.0-rc.0` `dist/dev.js`, the bundle the
    /// rc.0 install resolves — line numbers
    /// are that file's. Every answer below was also measured against that
    /// bundle under `--conditions browser` with the probe worker's own
    /// observation shape.
    ///
    /// One line decides most of it: `setupComputedNode` ends with
    /// `!options?.lazy && recompute(self, true)` (`:2845`), so any node built by
    /// `computed(fn, options)` (`:2707-2757`) whose options do not set `lazy`
    /// runs its compute *during* the creating call. That covers:
    ///
    /// - `createMemo` (`:4558-4560`, `accessor(computed(compute, options))`) —
    ///   the public `MemoOptions` has no `lazy` member, so this is
    ///   unconditional. 2.0's memo is **not** pull-based on creation;
    /// - `createSignal(fn)` (`:4548-4552`, the derived overload's
    ///   `computed(first, second)`);
    /// - `createOptimistic(fn)` (`:4778-4790` → `optimisticComputed`,
    ///   `:2888-2892`, which is `computed` plus one field);
    /// - `createProjection` (`:5634-5675`, `node = computed(() => { … }, …)` at
    ///   `:5670` with options that carry only `loadingValue`/`name`).
    ///
    /// `createEffect` (`:4561-4581`) and `createRenderEffect` (`:4610-4612`)
    /// both go through `effect()` (`:4107-4121`), which calls
    /// `recompute(node, true)` unconditionally before queueing the *effect*
    /// function. So the tracked **compute** at argument 0 runs during the call
    /// in 2.0 — the opposite of 1.x's `createEffect`, and the headline dialect
    /// difference on this axis. (Argument 1 is [`Execution::Deferred`], which
    /// is not this method's domain.)
    ///
    /// `createTrackedEffect` (`:4642-4644` → `trackedEffect`, `:4253-4309`) is
    /// the one deferring member: it builds its computed with `lazy: true` and
    /// ends with `node._queue.enqueue(EFFECT_USER, run)` (`:4294`), so nothing
    /// runs before the creating call returns.
    ///
    /// Deliberately unestablished: `createStore` and `createOptimisticStore`
    /// (their derived overloads did not accept the probe's call shape, so no
    /// measurement backs a claim), `dynamic`, `useHead`, the two boundary
    /// primitives and `mapArray`/`repeat`. The callback summary leaves the
    /// exact callback leaf open for those rather than assuming they follow `computed`.
    /// 2.0's execution words, moved off `interproc.rs`'s hardcoded
    /// table by ADR 0111.
    ///
    /// `createEffect` defers to the attribution answer: argument 0 is the
    /// tracked compute and argument 1 the effect function, which `effect()`
    /// always enqueues (`options.user` selects `EFFECT_USER`), so `deferred`
    /// is a true promise about it. Everything else is stated directly,
    /// because the execution word is not derivable from the attribution one --
    /// `createRenderEffect`'s apply is attributed `Deferred` and has no
    /// execution word.
    ///
    /// `createRenderEffect`'s apply has **no** word, and that is the answer
    /// the bytes support rather than a gap. `deferred` promises the callback
    /// has not run when the export returns, and the plain two-argument call
    /// runs it before returning (`@solidjs/signals@2.0.0-rc.3`
    /// `dist/dev.js:5285-5289`; probed on rc.3 and rc.9, dev and prod, inside
    /// no owner, a root, a memo compute, a render-effect compute and a
    /// component body under `render`: `compute,apply,returned` every time).
    /// `inline` would be false too: the same call leaves the first apply for
    /// later when `options.defer` (skipped) or `options.schedule` (queued)
    /// is set, when the compute returns a promise or reads a source that is
    /// still pending (probed: the apply runs after the source settles), and
    /// on rc.9 when the first pass was staged into a live transaction
    /// (`dist/dev.js:1617`); and every later run comes from the flush. A
    /// callback leaf left open is the fail-closed reading of both.
    ///
    /// Three arms of the old shared table are absent rather than ported:
    /// `createResource`, `on` and `mergeProps` are Solid 1.x names this
    /// dialect does not carry.
    fn callback_execution_word_at(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> Option<Execution> {
        match (primitive, argument) {
            (Primitive::CreateEffect, _) | (Primitive::CreateRenderEffect, 0) => {
                self.callback_execution_at(primitive, argument, argument_count)
            }
            (
                Primitive::CreateMemo
                | Primitive::CreateDeferred
                | Primitive::CreateTrackedEffect
                | Primitive::CreateSignal
                | Primitive::CreateStore
                | Primitive::CreateProjection
                | Primitive::CreateOptimistic
                | Primitive::CreateOptimisticStore
                | Primitive::Dynamic,
                0,
            ) => Some(Execution::Tracked),
            // `onHydrationEnd` queues its callback or pushes it for the drain,
            // and returns (`solid-js@2.0.0-rc.17` `dist/solid.dev.js:385-392`,
            // `dist/server.js:2731-2733`): probed, it never ran before the call
            // returned. `lazyModule` has no word, as `lazy` has none: its `fn`
            // runs when the result is called, which may be during the very
            // call that receives it.
            (
                Primitive::OnSettled
                | Primitive::Action
                | Primitive::CreateReaction
                | Primitive::OnCleanup
                | Primitive::OnHydrationEnd,
                0,
            ) => Some(Execution::Deferred),
            // The static `dynamic` form is `untrack(source)` at the call
            // (`@solidjs/web@2.0.0-rc.9` `dist/web.dev.js:2199`,
            // `dist/server.js:3730`): the source has run and returned before
            // `dynamic` does, so it is `inline`, with `untrack`'s clearing
            // travelling separately exactly as it does for `untrack`.
            (
                Primitive::CreateRoot
                | Primitive::Untrack
                | Primitive::Flush
                | Primitive::DynamicStatic,
                0,
            )
            | (Primitive::RunWithOwner | Primitive::DispatchAsInteraction, 1) => {
                Some(Execution::Inline)
            }
            // Deliberately unanswered, so a forwarded parameter opens the
            // unknown-callback sentinel:
            //
            // - `DynamicUnknownForm`: `tracked` or `inline` depending on a
            //   value the syntax does not prove.
            // - `until`: its predicate runs during the call (the effect's
            //   first compute), again after it whenever a dependency changes
            //   until one run is truthy, and not at all when `options.signal`
            //   is already aborted (`@solidjs/signals@2.0.0-rc.9`
            //   `dist/dev.js:2734`). No one word is that; `resolve`,
            //   the same shape, is unanswered for the same reason.
            _ => None,
        }
    }

    fn tracked_callback_timing(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> Option<TrackedCallbackTiming> {
        if self.callback_execution_at(primitive, argument, argument_count)
            != Some(Execution::Tracked)
        {
            return None;
        }
        match primitive {
            Primitive::CreateMemo
            | Primitive::CreateDeferred
            | Primitive::CreateSignal
            | Primitive::CreateOptimistic
            | Primitive::CreateProjection
            | Primitive::CreateEffect
            | Primitive::CreateRenderEffect => Some(TrackedCallbackTiming::DuringCall),
            Primitive::CreateTrackedEffect => Some(TrackedCallbackTiming::AfterCall),
            _ => None,
        }
    }

    /// `render`/`hydrate` run their code callback once and never again, and
    /// `lazy`'s loader runs once from the wrapper component body, so a
    /// reactive read in any of them is a likely dependency bug — the same
    /// three entry points 1.x reports. `runWithOwner` swaps the owner while
    /// `@solidjs/signals` sets `tracking = false` around the call (see
    /// [`Solid2::runs_callback_deferred`]), so reads inside it register
    /// nothing either. `mapArray`/`repeat` are present for their map function
    /// (ADR 0443): rc.17 runs it under `runWithOwner`, so it is untracked, not
    /// tracked as this comment used to say (`callback_runs_untracked_without_window`).
    /// `createEffect(compute, apply)` and `createRenderEffect(compute, apply)`
    /// are the two with an apply slot; `createTrackedEffect(compute, options?)`
    /// has none. Matches the `(1, Execution::Deferred)` row in
    /// `callback_executions`, which is the same fact seen from the attribution
    /// side.
    fn apply_callback_argument(&self, primitive: Primitive) -> Option<usize> {
        matches!(
            primitive,
            Primitive::CreateEffect | Primitive::CreateRenderEffect
        )
        .then_some(1)
    }

    fn reports_untracked_reads_at(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> bool {
        let _ = argument_count;
        // `createReaction`'s invalidation callback runs untracked and
        // one-shot; the RC.0 runtime emits `STRICT_READ_UNTRACKED` for a
        // reactive read inside it, so the checker reports the same.
        (matches!(
            primitive,
            Primitive::Hydrate | Primitive::Lazy | Primitive::Render | Primitive::CreateReaction
        ) && argument == 0)
            || (primitive == Primitive::RunWithOwner && argument == 1)
            // ADR 0443: the map function of `mapArray` and `repeat` runs with
            // tracking off (`callback_runs_untracked_without_window`).
            || (matches!(primitive, Primitive::MapArray | Primitive::RepeatMap) && argument == 1)
    }

    /// `mapArray(list, map, { name })` and `repeat(count, map, { name })` wrap
    /// the map function in `setStrictRead(options.name)` (rc.13
    /// `@solidjs/signals` `mapArray`, `repeat`), so any options argument may
    /// open a labelled window around it.
    fn callback_may_open_strict_window(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> bool {
        argument == 1
            && argument_count > 2
            && matches!(primitive, Primitive::MapArray | Primitive::RepeatMap)
    }

    /// ADR 0443. The map function of `mapArray(list, map, options?)` and
    /// `repeat(count, map, options?)` runs with tracking off and no strict-read
    /// window of its own unless `options.name` is passed, so a reactive read in
    /// it subscribes nothing and raises nothing: the mapper runs under
    /// `runWithOwner`, which sets `tracking = false` and leaves `strictRead`
    /// alone (`@solidjs/signals@2.0.0-rc.17` `dist/dev-shared.js:5779`), inside
    /// the map computation whose recompute clears `strictRead`
    /// (`dist/dev-shared.js:4196-4200`). The wrapper that opens the window sits
    /// at `dist/dev.js:3093-3102` (`mapArray`) and `3623-3633` (`repeat`), and
    /// only `options?.name` selects it. The mapper itself is called at
    /// `dist/dev.js:3347`, `3362-3377`, `3406` (`mapArray`) and `3661`, `3694`
    /// (`repeat`). Probed on the rc.17 browser dev builds, module scope and
    /// component body, with and without `name`: `observers(source)` is `[]`, a
    /// write leaves the mapper's run count and output unchanged, and only the
    /// named calls raise `STRICT_READ_UNTRACKED`.
    fn callback_runs_untracked_without_window(
        &self,
        primitive: Primitive,
        argument: usize,
    ) -> bool {
        argument == 1 && matches!(primitive, Primitive::MapArray | Primitive::RepeatMap)
    }

    /// ADR 0443. `runWithOwner(owner, fn)` sets `tracking = false` around
    /// `fn()` and leaves `strictRead` as the call had it (`dist/dev-shared.js:5776-5785`
    /// at rc.17, `runWithOwner` and `getObserver`), so `fn` runs in the
    /// window of the call and untracked. Inside a tracked computation (an
    /// effect or memo compute, a JSX expression) the call has no window, so a
    /// read in `fn` neither subscribes nor warns. Probed on rc.17: a read in
    /// `createEffect(() => runWithOwner(owner, () => log(signal())))` ran
    /// once, left `observers(signal)` empty, raised nothing, and a write did
    /// not re-run it, in a root and in a component body.
    ///
    /// Not `createRoot`: it is `runWithOwner(createOwner(), …)` by the code
    /// read at `dist/dev-shared.js:3478-3481`, but no probe reached a read in
    /// one inside a computation, and a root there may be deliberate.
    fn callback_clears_tracking_in_calling_window(
        &self,
        primitive: Primitive,
        argument: usize,
    ) -> bool {
        primitive == Primitive::RunWithOwner && argument == 1
    }

    /// The callbacks nothing runs until the returned value is used:
    /// `createReaction`'s invalidation callback arms only once the returned
    /// tracker is called, `lazy`'s loader waits for the wrapper component to
    /// render, and `mapArray(list, mapFn)`/`repeat(count, mapFn)` pull both
    /// their source and their map function through the returned accessor.
    /// rc.17's `lazyModule(fn)` is `lazy`'s loader without the component:
    /// `fn` runs only from the returned `load` (`callback_owners`).
    fn callback_requires_return_invocation(&self, primitive: Primitive, argument: usize) -> bool {
        (argument == 0
            && matches!(
                primitive,
                Primitive::CreateReaction | Primitive::Lazy | Primitive::LazyModule
            ))
            || (argument <= 1 && primitive == Primitive::MapArray)
            || (argument == 1 && primitive == Primitive::RepeatMap)
    }

    /// `const track = createReaction(onInvalidate); track(() => read())` —
    /// the tracker argument is a tracked computation, the same two-stage
    /// shape as 1.x. Source: the 2.0 cheatsheet's "one-shot tracked
    /// callback" entry and the RC.0 runtime.
    fn returned_callback_execution_at(
        &self,
        primitive: Primitive,
        result_slot: Option<usize>,
        argument: usize,
        argument_count: usize,
    ) -> Option<Execution> {
        match (primitive, result_slot, argument, argument_count) {
            (Primitive::CreateReaction, None, 0, 1..) => Some(Execution::Tracked),
            _ => None,
        }
    }

    fn returned_callback_owner_at(
        &self,
        primitive: Primitive,
        result_slot: Option<usize>,
        argument: usize,
        argument_count: usize,
    ) -> Option<CallbackOwner> {
        match (primitive, result_slot, argument, argument_count) {
            (Primitive::CreateReaction, None, 0, 1..) => Some(CallbackOwner::Creates),
            _ => None,
        }
    }

    /// async boundary `Loading` and the error boundary `Errored`.
    fn boundary_name(&self, boundary: Boundary) -> &'static str {
        match boundary {
            Boundary::Async => "Loading",
            Boundary::Error => "Errored",
        }
    }

    /// Source: `solid-reactive-ir/src/cleanup.rs`, both arms of the match —
    /// the unconditional list and the four that depend on the first argument
    /// being a function.
    ///
    /// `createReaction` is a correction to that extraction, not part of it:
    /// the RC.0 runtime allocates a computation for the reaction when it
    /// is called, exactly as `createEffect` does, so creating one in a leaf
    /// or cleanup scope leaks it. Its `creates_directive_owner` row already
    /// recorded the disposal obligation this arm was missing.
    fn cleanup_rule(&self, primitive: Primitive) -> CleanupRule {
        match primitive {
            Primitive::OnCleanup
            | Primitive::Flush
            | Primitive::CreateMemo
            | Primitive::CreateDeferred
            | Primitive::CreateEffect
            | Primitive::CreateRenderEffect
            | Primitive::CreateReaction
            | Primitive::CreateTrackedEffect
            | Primitive::CreateProjection
            | Primitive::CreateRoot
            | Primitive::CreateOwner
            | Primitive::MapArray
            | Primitive::RepeatMap
            | Primitive::CreateRevealOrder
            | Primitive::CreateErrorBoundary
            | Primitive::CreateLoadingBoundary
            | Primitive::UseHead
            | Primitive::Children => CleanupRule::Always,
            Primitive::CreateSignal
            | Primitive::CreateStore
            | Primitive::CreateOptimistic
            | Primitive::CreateOptimisticStore => CleanupRule::WhenFirstArgumentIsFunction,
            _ => CleanupRule::Never,
        }
    }

    /// Source: the `accepts_cleanup_return` list in
    /// `solid-reactive-ir/src/cleanup.rs`, extracted unchanged.
    fn accepts_cleanup_return(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::OnSettled
                | Primitive::CreateTrackedEffect
                | Primitive::CreateReaction
                | Primitive::CreateEffect
                | Primitive::CreateRenderEffect
        )
    }

    /// Source: the control-flow component lists `solid-reactive-ir`
    /// hardcoded before ADR 0006, extracted unchanged; this table is now the
    /// only place they live.
    /// Source: `solid-js@2.0.0-rc.13` `dist/solid.dev.js:1476` (`Show`) and
    /// `:1523` (`Match`): `typeof child === "function" && child.length > 0`
    /// selects the callback path; otherwise the function itself is the child.
    fn children_callback_requires_parameters(&self, primitive: Primitive) -> bool {
        matches!(primitive, Primitive::Show | Primitive::Match)
    }

    fn renders_children_through_callback(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::For
                | Primitive::Repeat
                | Primitive::Show
                | Primitive::Match
                | Primitive::Switch
        )
    }

    /// Source: the source-discovery gate in `solid-reactive-ir/src/lib.rs`,
    /// extracted unchanged. Every one returns a tuple or a store a single-value
    /// `returns` summary cannot describe.
    fn creates_reactive_source(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::CreateSignal
                | Primitive::CreateMemo
                | Primitive::CreateDeferred
                | Primitive::CreateStore
                | Primitive::CreateProjection
                | Primitive::CreateOptimistic
                | Primitive::CreateOptimisticStore
        )
    }

    /// Source: `@solidjs/signals@2.0.0-rc.9` `enforceLoadingBoundary`, probed
    /// in dev: `ASYNC_OUTSIDE_LOADING_BOUNDARY` is emitted for a *render
    /// effect* whose read is pending with no `Loading` ancestor during the
    /// synchronous `render()` window, and never for a memo or a user effect's
    /// compute. `createRenderEffect` is the one user-callable render effect;
    /// compiled JSX positions are classified from compiler facts instead.
    fn computation_read_is_render(&self, primitive: Primitive) -> bool {
        primitive == Primitive::CreateRenderEffect
    }

    /// Source: `@solidjs/web@2.0.0-rc.9` exports `render(code, element)` and
    /// `hydrate(fn, node)`, whose first argument is the root of the mounted
    /// tree and which have no other caller.
    fn mounts_component_tree(&self, primitive: Primitive) -> bool {
        matches!(primitive, Primitive::Render | Primitive::Hydrate)
    }

    /// Source: `solid-reactive-ir/src/directives.rs` `is_created_primitive`.
    /// 2.0 renamed `mergeProps` to `merge`. `omit` and `pick`, the
    /// `splitProps` replacements, are deliberately absent for the same reason
    /// 1.x leaves `splitProps` out: their result is a tuple, not the merged
    /// object.
    fn merges_props_reactivity(&self, primitive: Primitive) -> bool {
        primitive == Primitive::Merge
    }

    /// 2.0 replaced `splitProps` with `omit`; `store/utils.d.ts` declares
    /// `omit(props: T, ...keys: K)`, the same props-plus-key-lists shape.
    /// rc.9's predicate overload is the one argument this does not cover; see
    /// [`Solid2::callback_runs_on_result_access`].
    fn splits_props(&self, primitive: Primitive) -> bool {
        primitive == Primitive::Omit
    }

    /// `@solidjs/signals@2.0.0-rc.9` adds
    /// `omit(props: T, hidden: (key: keyof T & (string | symbol)) => boolean):
    /// Partial<T>` (`dist/types/store/utils.d.ts:251`), and the runtime picks
    /// that form by
    /// `keys.length === 1 && typeof keys[0] === "function"` (`dist/dev.js:4380`,
    /// the same test at `dist/prod/store/utils.js:982` and
    /// `dist/observe/store/utils.js:984`; `solid-js` re-exports this `omit`,
    /// server build included, `dist/server.js:2`). So only a call with exactly
    /// two arguments can carry a predicate, and it is argument 1.
    ///
    /// When it runs, read from the same bytes:
    ///
    /// - **Wherever `Proxy` exists** (`SUPPORTS_PROXY = typeof Proxy ===
    ///   "function"`, `dist/dev-shared.js:273`), `omit` only stores it:
    ///   `new Proxy(new OmitView(source, kind, hidden), omitTraps)`
    ///   (`dist/dev.js:4406`). `isHidden(view, key)` then calls it on every
    ///   `get`, `has` and key enumeration of the returned view
    ///   (`omitTraps`, `:4178-4241`; `isHidden`, `:3495-3498`), and on every
    ///   walk of the view by `merge` and the spread helpers (`:3557-3703`,
    ///   `:3826-3870`, `:3984`, `:4071`, `:4113-4128`). Nothing around those
    ///   calls touches the listener, so the predicate's reads subscribe
    ///   whatever computation is reading the view, and its writes run under
    ///   that reader's owner.
    /// - **Without `Proxy`**, it is called once per own property name during
    ///   the `omit` call itself, in the caller's scope (`:4408-4427`).
    ///
    /// Neither is a schedule the `Execution` vocabulary can state, and the
    /// first is the one every supported runtime takes.
    ///
    /// Answered from the resolved `@solidjs/signals`
    /// (`Solid2::omit_predicate_form`). rc.3's `omit` has no predicate form:
    /// `typeof keys[0]` is never read and every build only tests membership
    /// (`keys.includes(key)`, rc.3 `dist/dev.js:9334-9369`), and its
    /// declaration rejects a function there (TS2345). So the audited
    /// vocabulary answers `false` -- for typed code that is TypeScript's
    /// boundary, and for an untyped artifact it is what rc.3's bytes do -- and
    /// only a vocabulary built from signals rc.9 answers the predicate slot.
    fn callback_runs_on_result_access(
        &self,
        primitive: Primitive,
        argument: usize,
        argument_count: usize,
    ) -> bool {
        self.omit_predicate_form
            && primitive == Primitive::Omit
            && argument == 1
            && argument_count == 2
    }

    /// `action(genFn)`'s generator, argument 0. Every release the reviews read
    /// drives it the same way: `const it = genFn(...args)` inside the returned
    /// wrapper's promise executor, then `step()`, which runs `it.next(v)` (or
    /// `it.throw(v)` for a rejected yielded thenable) and settles or schedules
    /// the next step from the result (`@solidjs/signals@2.0.0-rc.9`
    /// `dist/dev.js:1961-2066`). From rc.8 `step` brackets that call with
    /// `enterActionStep()`/`exitActionStep()`; the stepping itself is older,
    /// and answering it on every release is what lets the throw below be the
    /// one release-keyed fact.
    fn callback_runs_as_action_steps(&self, primitive: Primitive, argument: usize) -> bool {
        primitive == Primitive::Action && argument == 0
    }

    /// `createComponent(Comp, props)`, argument 0, on every release the
    /// reviews read. The export is `solid-js`'s; `@solidjs/web` re-exports it
    /// unchanged in every build (`export { …, createComponent, … } from
    /// 'solid-js'`, rc.3 and rc.9 `dist/web.js:2`, `dist/server.js:2`, and the
    /// dev and observe builds). Each `solid-js` build calls `Comp` exactly once,
    /// synchronously, before it returns, and retains nothing it later invokes:
    ///
    /// | build | rc.0-rc.9 | owner `Comp` runs under |
    /// | --- | --- | --- |
    /// | client prod (`solid.js`; rc.9 `:1122-1124`, rc.3 `:1095-1097`) | `untrack(() => Comp(props \|\| {}))` | the caller's |
    /// | client dev (`dev.js`/`solid.dev.js`; rc.3 `devComponent` `:35-53`, rc.9 `observedComponent` `:35-57`) | `createRoot(() => untrack(() => Comp(props)), { transparent: true })` | a transparent child root of the caller's |
    /// | client observe (rc.8, rc.9 `solid.observe.js:36-46`) | as dev, without the dev checks | as dev |
    /// | server prod (`server.js`; rc.9 `:1644-1646`, rc.3 `:1466-1468`), and rc.7/rc.8's server dev and observe | `Comp(props \|\| {})` | the caller's |
    /// | server dev/observe (rc.9 `server.dev.js:1748-1755`, `server.observe.js:1721-1725`) | `runWithOwner(createComponentOwner(…), () => Comp(props \|\| {}))`, or `Comp(props \|\| {})` with no owner | a child of the caller's |
    ///
    /// `createRoot` runs its function in place (`runWithOwner(owner, …)` on
    /// the caller's stack), so no build defers the call. Every one of the ten
    /// releases' builds was read for the three shapes above.
    /// The dev builds also store `Comp` on the owner (`owner._component = { fn:
    /// Comp, … }`) and tag it with `$DEVCOMP`; no build of `solid-js`,
    /// `@solidjs/signals` or `@solidjs/web` at rc.3 or rc.9 reads
    /// `_component` back, so neither is a second entry. Probed on the
    /// published triples (`createComponent` inside a memo, then a write to a
    /// signal `Comp` read): every one of rc.3's four builds and rc.9's six ran
    /// `Comp` once, between the statements before and after the call, and
    /// never again.
    fn renders_component_argument(&self, name: &str) -> Option<usize> {
        (name == "createComponent").then_some(0)
    }

    /// ADR 0292, read on the published `solid-js@2.0.0-rc.13` builds
    /// (`dist/solid.js`, `dist/solid.observe.js` and `dist/solid.dev.js`, which
    /// adds only a throw for a string second argument; `dist/server.js`
    /// selects the same member):
    ///
    /// ```js
    /// function lazy(fn, options, moduleUrl) {
    ///   const exportName = options?.export;
    ///   ...
    ///   const load = () => { ... const cur = p = fn();
    ///     cur.then(mod => { comp = () => exportName ? mod[exportName] : mod.default; }, ...);
    ///     return cur; };
    ///   const wrap = props => { ...
    ///     local = createMemo$1(() => load().then(mod => exportName ? mod[exportName] : mod.default));
    ///     ...
    ///     return createMemo$1(() => (Comp = (comp || local)()) ? createComponent(Comp, props) : "", { sync: true }); };
    ///   wrap.preload = load;
    ///   ...
    ///   return wrap; }
    /// ```
    ///
    /// `fn` is called only by `load`, which the wrapper and `.preload()`
    /// reach; the selected member is `default` exactly when no `options`
    /// object is passed, and it is rendered through `createComponent`. With a
    /// second argument the selected member is data, so only the one-argument
    /// form answers. Earlier releases render the member without
    /// `createComponent` (`Solid2::lazy_renders_component`), so only a
    /// vocabulary read on rc.13 answers at all.
    fn renders_loaded_default_argument(
        &self,
        primitive: Primitive,
        arguments: usize,
    ) -> Option<usize> {
        (self.lazy_renders_component && primitive == Primitive::Lazy && arguments == 1).then_some(0)
    }

    /// ADR 0153, read on every bundle of the audited `solid-js` archives
    /// (rc.3 and rc.9: `dist/solid.js`, `dist/dev.js`, `dist/server.js`, and
    /// rc.9's observe variants) and on `@solidjs/signals` rc.3, rc.6 and rc.9
    /// (`dist/prod/core/context.js`):
    ///
    /// ```js
    /// function createContext(defaultValue, options) {
    ///   const id = Symbol(options && options.name || "");
    ///   function provider(props) {
    ///     return createRoot(() => {
    ///       setContext(provider, props.value);
    ///       return children(() => props.children);
    ///     });
    ///   }
    ///   provider.id = id;
    ///   provider.defaultValue = defaultValue;
    ///   return provider;
    /// }
    /// function useContext(context) { return getContext(context); }
    /// function createComponent(Comp, props) { return untrack(() => Comp(props || {})); }
    /// ```
    ///
    /// `getContext(e, t = getOwner())` throws `NoOwnerError` without an owner,
    /// answers the owner's map at `e.id` (rc.9 `t.ze[e.id]`, rc.3 `hasContext`
    /// then `t.we[e.id]`) or else `e.defaultValue`, and throws
    /// `ContextNotFoundError` when that is `undefined`; the server bundle's
    /// `useContext` rethrows the same error. `setContext(e, t, r = getOwner())`
    /// stores `t`, or `e.defaultValue` when `t` is `undefined`, under `e.id` in a
    /// copy of the owner's map. The server `createComponent` is
    /// `Comp(props || {})` without `untrack`. So a context made with no
    /// arguments reads only values stored under its fresh symbol, and storing
    /// one takes the context object itself.
    fn context_role(&self, name: &str) -> Option<ContextRole> {
        match name {
            "createContext" => Some(ContextRole::Create),
            "useContext" => Some(ContextRole::Read),
            "createComponent" => Some(ContextRole::Render),
            _ => None,
        }
    }

    /// rc.13's `getContext` (the read `useContext` wraps, `solid-js`
    /// `dist/solid.dev.js`) checks the owner before anything else and throws
    /// `NoOwnerError`, whatever default the context has (ADR 0352). The
    /// `ContextNotFoundError` half is a different claim (a missing provider),
    /// so it is not a role here.
    ///
    /// rc.17's `Loading` and `Errored` (`solid-js` `dist/solid.dev.js`) call
    /// `createLoadingBoundary`/`createErrorBoundary`, whose `createBoundary`
    /// (`@solidjs/signals` `dist/dev.js`) warns `NO_OWNER_BOUNDARY` when
    /// `getOwner()` is null; the hydrated wrappers pass straight through with
    /// no owner (`noHydrationId`). JSX `<Loading>` reaches them through
    /// `createComponent`, whose dev build runs the component in a transparent
    /// root, so the role is the direct call's (ADR 0462).
    fn owner_requirement_role(&self, primitive: Primitive) -> Option<OwnerRequirementRole> {
        match primitive {
            Primitive::UseContext => Some(OwnerRequirementRole::ContextRead),
            Primitive::Loading | Primitive::Errored => Some(OwnerRequirementRole::Boundary),
            _ => crate::dialect::shared_owner_requirement_role(primitive),
        }
    }

    /// `Dynamic`'s `component`, on every release the reviews read. `Dynamic`
    /// is `@solidjs/web`'s, the same function in every build of rc.3 and rc.9
    /// (rc.3 `web.js:1865`, `dev.js:1935`, `server.js:3208`; rc.9 `web.js:2099`,
    /// `web.dev.js:2271`, `web.observe.js:2121`, `server.js:3764`,
    /// `server.dev.js:4013`, `server.observe.js:3852`):
    ///
    /// ```js
    /// function Dynamic(props) {
    ///   const Comp = dynamic(() => props.component);
    ///   return createComponent(Comp, omit(props, "component"));
    /// }
    /// ```
    ///
    /// `dynamic(source)` keeps the value in a memo it creates (`cached`) and
    /// returns a component that, rendered, creates one more memo reading it:
    /// a function is called there as `component(props)` (or its
    /// `Symbol.for("solid.component-binding")` target, when it carries one),
    /// a string builds an element, and nothing else is done with it. The value
    /// reaches no other invocation: `bindingOf` and the thenable test read a
    /// property of it, and the dev and observe builds' `Object.assign(component,
    /// { [$DEVCOMP]: true })` writes one. The `props` it receives are
    /// `omit(props, "component")`. `createComponent` runs `Dynamic`, and then
    /// `Comp`, in place (ADR 0136), so the component is invoked only inside
    /// computations the render creates.
    ///
    /// Probed on the published triples: `createComponent(Dynamic, { get
    /// component() { return Selected(); } })` with `Selected = createMemo(() =>
    /// flag() ? A : B)`, inside a root. In all six rc.9 builds and rc.3's
    /// four, `A` ran once, between the statements before and after the call,
    /// with no `component` in its props, under an owner below the render's
    /// (one level in prod, three in dev and observe; rc.3's client prod owners
    /// do not expose the link). The client builds ran `B` once when `flag` was
    /// written and flushed, under the same owner depth; the server builds,
    /// which do not re-run, never did. A data property (`{ component: A }`)
    /// behaved the same.
    fn component_prop_renderers(&self) -> &'static [(&'static str, &'static str)] {
        &[("Dynamic", "component")]
    }

    /// ADR 0313: `<Dynamic component="button" …/>`. Read on the audited
    /// `@solidjs/web@2.0.0-rc.13` client builds: `Dynamic(props)` renders
    /// `createComponent(dynamic(() => props.component), omit(props,
    /// "component"))` (`dist/web.dev.js:2403`), and `dynamic`'s returned
    /// component answers a string with `staticElement(component, props)`
    /// (`:2375`), which creates the element and runs `spread(el, props)`
    /// (`:2396-2399`) -- the helper a compiled JSX spread onto an intrinsic
    /// element calls. Its `assignProp` turns an un-namespaced `on…` key into an
    /// event listener (`:1803`). The production and observe builds are the same
    /// code (`dist/web.js:2137`, `:2154-2157`, `:1578`;
    /// `dist/web.observe.js:2185`, `:2202-2205`, `:1626`). Earlier releases were
    /// not re-read for this answer; they take the audited release's under their
    /// `SC9014` notice. A server build creates no element and dispatches no
    /// event.
    fn intrinsic_tag_renderers(&self) -> &'static [(&'static str, &'static str)] {
        &[("Dynamic", "component")]
    }

    /// `createMemo(compute)`, one argument, `compute` taking no parameters. The
    /// value `compute` returns is the node's value, and the node is reachable
    /// from program code only through the accessor `createMemo` returns
    /// (`accessor(computed(compute, undefined))` in `@solidjs/signals`, rc.9
    /// `prod/signals.js:77`; the accessor is `read.bind(null, node)`, and its
    /// `$REFRESH` slot re-runs `compute`). With no options there is no
    /// `loadingValue`, `equals` or `ssrSource`; a parameterless compute cannot
    /// read its previous value. `solid-js`'s client `createMemo` is the
    /// hydration wrapper (rc.9 `solid.js:596-601`, `770-772`): while hydrating
    /// it may yield a serialized value instead of calling `compute`, which is
    /// another value, never a second path for this one. The server
    /// `createMemo` runs `compute` in place.
    fn accessor_yields_only_its_compute(&self, primitive: Primitive) -> bool {
        primitive == Primitive::CreateMemo
    }

    /// `flush` inside an action step: the `FLUSH_IN_ACTION` dev throw, on the
    /// releases that have it (`Solid2::flush_in_action`).
    fn throws_inside_action_step(&self, primitive: Primitive) -> bool {
        self.flush_in_action && primitive == Primitive::Flush
    }

    /// `createEffect(compute, apply)`'s apply, argument 1: queued on the effect
    /// queue and run by `runEffect` inside the draining flush, between
    /// `setEffectCallback(true)` and `setEffectCallback(false)`
    /// (`@solidjs/signals@2.0.0-rc.13` `dist/dev.js:1876`, `:1906`). Every
    /// creation path of a user effect enqueues the run (`dist/dev.js:1737-1740`),
    /// so the draining flush is always the one on the stack. `createRenderEffect`
    /// is not: unless `schedule` is set its first run is the synchronous
    /// `runEffect(node, LANE_RUN)` at creation, with no flush in progress.
    fn callback_runs_in_effect_phase(&self, primitive: Primitive, argument: usize) -> bool {
        primitive == Primitive::CreateEffect && argument == 1
    }

    /// Once a flush is running, `flush` returns without draining, after
    /// reporting `FLUSH_IN_EFFECT_CALLBACK` if `inEffectCallback` is up (rc.13
    /// `dist/dev-shared.js:2680-2698`); the production build returns at the
    /// same branch without the report (`dist/prod/core/scheduler.js`). The
    /// marker is one global boolean that a nested synchronous `runEffect`
    /// lowers (`dist/dev.js:1906`), so the no-op is the claim and the report
    /// is not. The marker and the report are in every read release from rc.1;
    /// rc.0 has neither (`Solid2::flush_in_effect_callback`).
    fn warns_inside_effect_phase(&self, primitive: Primitive) -> bool {
        self.flush_in_effect_callback && primitive == Primitive::Flush
    }

    /// `@solidjs/signals@2.0.0-rc.17` `dist/dev-shared.js:3947-3957`
    /// (`assertNotInEffectCallback`: `inEffectCallback && context === null`
    /// throws `PRIMITIVE_IN_EFFECT_CALLBACK`) is called by `createOwner`
    /// (`:1496`, which `createRoot` calls first, `:1537-1538`) and by
    /// `setupComputedNode` (`:4927`), which every computation reaches at its
    /// creation: `computed` (`:4689`: `createMemo`, `dist/dev.js:2433-2434`;
    /// function-form `createSignal`, `:2412-2414`; `createProjection` and
    /// function-form `createStore`, `:6744-6793`, `:8438-8440`) and
    /// `createEffectNode` (`:4891`: `createEffect` and `createRenderEffect`
    /// through `effect`, `dist/dev.js:1851-1858`). Value-form state creates no
    /// computation. Read on rc.17 only; the release answer it rests on is
    /// `disposal_write_guard`, which is `StandsDown` exactly when the read
    /// signals release is rc.17 (`releases.rs`, tested beside it).
    fn throws_creating_in_effect_callback(
        &self,
        primitive: Primitive,
        first_argument_is_function: bool,
    ) -> bool {
        self.disposal_write_guard == crate::WriteGuardInDisposal::StandsDown
            && match primitive {
                Primitive::CreateMemo
                | Primitive::CreateEffect
                | Primitive::CreateRenderEffect
                | Primitive::CreateRoot
                | Primitive::CreateProjection => true,
                Primitive::CreateSignal | Primitive::CreateStore => first_argument_is_function,
                _ => false,
            }
    }

    /// `createSignal` and `createOptimistic` both return
    /// `Signal<T> = [get: SourceAccessor<T>, set: Setter<T>]`; `createStore`
    /// and `createOptimisticStore` both return
    /// `[get: Store<T>, set: StoreSetter<T>]`. `createProjection` returns the
    /// store itself, so it is deliberately absent, and `createResource` is not
    /// 2.0 vocabulary at all.
    fn returns_reactive_tuple(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::CreateSignal
                | Primitive::CreateStore
                | Primitive::CreateOptimistic
                | Primitive::CreateOptimisticStore
        )
    }

    fn creates_directive_owner(&self, primitive: Primitive) -> bool {
        matches!(
            primitive,
            Primitive::CreateSignal
                | Primitive::CreateMemo
                | Primitive::CreateDeferred
                | Primitive::CreateStore
                | Primitive::CreateProjection
                | Primitive::CreateOptimistic
                | Primitive::CreateOptimisticStore
                | Primitive::CreateEffect
                | Primitive::CreateRenderEffect
                | Primitive::CreateTrackedEffect
                | Primitive::CreateReaction
                | Primitive::CreateRoot
                | Primitive::CreateOwner
                | Primitive::CreateErrorBoundary
                | Primitive::CreateLoadingBoundary
                | Primitive::UseHead
        )
    }

    /// Source: `solid-js@2.0.0-rc.0`'s `types/index.d.ts`, which re-exports
    /// the whole vocabulary from the package root. 2.0 folded the store API
    /// into core, so there is no `solid-js/store`; the one primitive that
    /// lives elsewhere is `dynamic`, from the web package.
    /// Two packages, unlike 1.x: 2.0 split the DOM out into `@solidjs/web`,
    /// which has subpaths of its own. A name in both — `render` is not, but
    /// the shape allows it — reports both, and importing it from either
    /// resolves.
    fn export_modules(&self, name: &str, position: crate::ExportPosition) -> Vec<&'static str> {
        if (name == "until" && !self.until)
            || (matches!(
                name,
                "dynamicComponent"
                    | "dispatchAsInteraction"
                    | "createDeferred"
                    | "lazyModule"
                    | "onHydrationEnd"
            ) && !self.rc17_callback_exports)
        {
            return Vec::new();
        }
        let mut found = crate::exports::modules(
            crate::exports::solid_v2_solid_js::VALUES,
            crate::exports::solid_v2_solid_js::TYPES,
            name,
            position,
        );
        for module in crate::exports::modules(
            crate::exports::solid_v2_solidjs_web::VALUES,
            crate::exports::solid_v2_solidjs_web::TYPES,
            name,
            position,
        ) {
            if !found.contains(&module) {
                found.push(module);
            }
        }
        found
    }

    /// Source: `solid-reactive-ir/src/symbols.rs`, `add_solid_import_names`,
    /// which before ADR 0006 matched `"solid-js"` and `"@solidjs/web"` each
    /// exactly. `dynamic` is an `@solidjs/web` root export; a third-party
    /// `@solidjs/*` package exporting a same-spelled `dynamic` is not this
    /// primitive and must not resolve as it.
    fn namespace_import_primitives(&self, module: &str) -> &'static [&'static str] {
        if module == "solid-js" {
            NAMESPACE_SOLID_JS
        } else if module == "solid-js/internal" {
            NAMESPACE_SOLID_JS_INTERNAL
        } else if module == "@solidjs/web" {
            NAMESPACE_SOLIDJS_WEB
        } else {
            &[]
        }
    }
}

/// The names a `solid-js/internal` namespace import exposes: the three
/// boundary primitives `solid-js@2.0.0-rc.13` declares there and no longer at
/// the root (ADR 0194).
const NAMESPACE_SOLID_JS_INTERNAL: &[&str] = &[
    "createErrorBoundary",
    "createLoadingBoundary",
    "createRevealOrder",
];

/// The names a `solid-js` namespace import exposes.
///
/// The invariant, which `solid_1x.rs` enforced too before the 1.x dialect was
/// retired: every modelled primitive
/// the module exports must keep its namespace spelling, so a primitive
/// cannot be reachable as `import { x }` but invisible as `Solid.x`. The
/// list used to stop short of that (19 names, `children`/`For`/`Repeat`
/// among them, resolved as nothing through a namespace import — the
/// `namespace-import-v2` fixture pins the difference); the
/// `every_modelled_export_resolves_through_its_namespace_module` test below
/// now derives the expectation from [`TABLE`] and the export census, exactly
/// as the 1.x dialect does.
const NAMESPACE_SOLID_JS: &[&str] = &[
    "Errored",
    "For",
    "Loading",
    "Match",
    "Repeat",
    "Show",
    "Switch",
    "action",
    "affects",
    "children",
    "createContext",
    "createDeferred",
    "createEffect",
    "createErrorBoundary",
    "createLoadingBoundary",
    "createMemo",
    "createOptimistic",
    "createOptimisticStore",
    "createOwner",
    "createProjection",
    "createReaction",
    "createRenderEffect",
    "createRevealOrder",
    "createRoot",
    "createSignal",
    "createStore",
    "createTrackedEffect",
    "deep",
    "dynamicComponent",
    "flush",
    "getOwner",
    "isPending",
    "latest",
    "lazy",
    "lazyModule",
    "mapArray",
    "merge",
    "omit",
    "onCleanup",
    "onHydrationEnd",
    "onSettled",
    "reconcile",
    "refresh",
    "repeat",
    "resolve",
    "runWithOwner",
    "snapshot",
    "until",
    "untrack",
    "useContext",
];

/// The names an `@solidjs/web` namespace import exposes, under the same
/// census-derived invariant as [`NAMESPACE_SOLID_JS`]: the entry points plus
/// the control-flow and owner helpers the package re-exports.
const NAMESPACE_SOLIDJS_WEB: &[&str] = &[
    "Errored",
    "For",
    "Loading",
    "Match",
    "Repeat",
    "Show",
    "Switch",
    "clientOnly",
    "dispatchAsInteraction",
    "dynamic",
    "dynamicComponent",
    "getOwner",
    "httpHeader",
    "httpStatus",
    "hydrate",
    "render",
    "untrack",
    "useHead",
];

#[cfg(test)]
mod tests;
