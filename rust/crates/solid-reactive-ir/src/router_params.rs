//! ADR 0324: the router-parameters premise.
//!
//! `useParams()` of an audited `@solidjs/router` release hands out route
//! parameters as URL path-segment strings (or `undefined` for an absent
//! optional one). A project that reads them through the direct
//! `const params = useParams()` binding therefore holds a primitive in
//! `params.<key>`, which the primitive proofs of ADR 0320 consume.
//!
//! This module decides, once per project, whether the premise applies: which
//! releases the project resolves, whether the dialect audited them, and
//! whether the project sets anything the audit did not read
//! (`paramsWrapper`, `queryWrapper`, `wrapParams`, a custom `RouterIntegration`).
//! The release comes from the compiler's attested import resolution, never
//! from a name: an unresolved import or a package that is not the router
//! applies nothing.

use std::collections::BTreeSet;

use solid_dialect::Dialect;
use solid_facts::ast::{ExportKind, ImportKind, MemberFact};
use solid_facts::core::Span;
use solid_facts::{FileFacts, ProjectFacts, SpecifierAttestation};

/// The specifier of the audited package.
pub const ROUTER_PACKAGE: &str = "@solidjs/router";

/// Spellings of what replaces how the router builds parameters or search
/// objects: the integration hooks `paramsWrapper` and `queryWrapper`
/// (`RouterUtils`, `types.d.ts:285-286`), the context's `wrapParams`, and a
/// custom `RouterIntegration` that supplies them. The stock `createRouter`,
/// `Router` and the history helpers set none of them (nothing in the audited
/// package assigns `paramsWrapper`), so a call of `createRouter` alone is no
/// replacement. A mention anywhere in the program outside the router's own
/// files leaves the premise unapplied.
const REPLACEMENT_SPELLINGS: [&str; 5] = [
    "paramsWrapper",
    "queryWrapper",
    "wrapParams",
    "RouterIntegration",
    "RouterContext",
];

/// The histories the audited package exports (`routers/index.js`). Each
/// returns a fresh integration whose `utils` the package itself writes
/// (`routers/history.js:91,131,178` next.26, `:87,127,174` next.41: `go`,
/// `renderPath`, `parsePath`,
/// `beforeLeave`) and none of which is `paramsWrapper`.
pub(crate) const STOCK_HISTORIES: [&str; 3] = ["browserHistory", "hashHistory", "memoryHistory"];

/// What a namespace import of the router may be used for: hooks that read
/// the router's state and build nothing the integration reads. Any other
/// member (`createRouter`, a history, `RouterContext`) leaves the premise
/// unapplied, because the member could be handed an integration.
const NAMESPACE_MEMBERS: [&str; 12] = [
    "useParams",
    "useLocation",
    "useNavigate",
    "useSearchParams",
    "useMatch",
    "useHref",
    "useIsRouting",
    "useResolvedPath",
    "useRouteMatches",
    "useLinkState",
    "usePreloadRoute",
    "int",
];

/// The `createRouter` option the router calls with its whole context
/// (`createRouter({ links })`, from `2.0.0-next.41`): a config naming it
/// withholds both router premises (ADR 0324, 0335).
pub(crate) const LINKS_KEY: &str = "links";

/// The `createRouter` option every audited release calls as a method of
/// Root's props, which hold the router context (ADR 0324).
const PRELOAD_KEY: &str = "preload";

/// The router hook that calls its argument with the router context as
/// `this`: `useAction(action)` returns `(...args) => action.apply({ r:
/// useRouter() }, args)` (`dist/data/action.js:137-139` next.16, `:143-145`
/// next.18 and next.26, `:232-235` next.41).
const USE_ACTION: &str = "useAction";

/// Router exports whose call returns the router's own action wrapper
/// (`toAction`: `function (...args) { return invoke.call(this, …) }`, whose
/// `invoke` keeps `this.r` and calls the project function plainly,
/// `fn(...variables)`). next.41 adds `formAction`; its `action` is a
/// deprecation wrapper calling `formAction.apply(this, args)`
/// (`data/deprecated.js:16`).
const ACTION_FACTORIES: [&str; 2] = ["action", "formAction"];

/// Router hooks that read an action's members and call nothing on it:
/// `useSubmissions(fn, filter)` reads `fn.base` (`data/action.js:131`
/// next.26, `:214` next.41).
const ACTION_READERS: [&str; 1] = ["useSubmissions"];

/// A history's members the project may call without being able to install
/// `utils` on it: `get` reads the location and `set` writes one.
const STOCK_HISTORY_CALLS: [&str; 2] = ["get", "set"];

/// Whether, and why not, the router-parameters premise applies to a project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RouterParamsStatus {
    /// No file imports `useParams` from the router: nothing to decide.
    NotUsed,
    /// Every resolved `useParams` import is an audited release and the
    /// project replaces nothing the audit read.
    Applies { version: String },
    /// A resolved release the dialect did not audit (or an import whose
    /// resolution was not attested): the premise is not applied, and the
    /// project says so in a notice.
    Unaudited {
        version: Option<String>,
        /// The release is audited but the installed bytes are not its bytes.
        bytes_differ: bool,
        /// The first `useParams` import that resolved to it.
        path: String,
        span: solid_facts::core::Span,
    },
    /// The project sets something the audit did not read.
    Replaced { spelling: &'static str },
}

impl RouterParamsStatus {
    /// Whether the premise may be applied.
    #[must_use]
    pub fn applies(&self) -> bool {
        matches!(self, Self::Applies { .. })
    }
}

/// What the backend observed of the installed router beyond the facts: the
/// bytes of each router install the project's imports resolve to, and whether
/// the project's own manifests record a replacement of the package. Read from
/// disk by the backend and never set by a request, so a caller with no disk
/// (a test, the WASM adapter) carries none and the premise stays unapplied.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RouterInstallEvidence {
    /// Some manifest or lockfile above the project records a patch of the
    /// package, a `patches/` entry naming it, or an override or dependency
    /// that is not a plain registry version.
    pub replacement_recorded: bool,
    /// Digest of each resolved router install's bytes
    /// (`scripts/router-digest.mjs`), by the path of its `package.json`.
    pub digests: std::collections::BTreeMap<String, String>,
    /// ADR 0334: what was read of each resolved `@tanstack/solid-router`
    /// install, by the path of its `package.json`.
    pub route_installs: std::collections::BTreeMap<String, crate::route_components::RouteInstall>,
    /// ADR 0334: a manifest, lockfile or patch directory above the project
    /// records a replacement of `@tanstack/solid-router` or
    /// `@tanstack/router-core`.
    pub route_replacement_recorded: bool,
}

/// One runtime import of the router and what it resolved to.
struct Reached {
    version: Option<String>,
    /// The release is audited but the installed bytes are not its bytes (or
    /// could not be read).
    bytes_differ: bool,
    path: String,
    span: Span,
    reads_params: bool,
}

/// The premise's status for `facts` under `dialect`.
///
/// `install` is what the backend read from disk (see
/// [`RouterInstallEvidence`]): the release string is a claim a patch, a fork or
/// an override keeps, so the premise also needs the installed bytes to be the
/// audited ones.
#[must_use]
pub fn router_params_status(
    facts: &ProjectFacts,
    dialect: &dyn Dialect,
    install: &RouterInstallEvidence,
) -> RouterParamsStatus {
    // Every runtime import of the router, whichever binding it names: a copy
    // of the router that only `createRouter` comes from is as much the
    // router `useParams` reads as one `useParams` is imported from.
    let mut reached: Vec<Reached> = Vec::new();
    let mut uses_params = false;
    let mut replaced: Option<&'static str> = None;
    for file in &facts.files {
        for import in &file.ast.imports {
            if import.module.as_str() != ROUTER_PACKAGE
                || import.type_only
                || import.bindings.iter().all(|binding| binding.type_only)
            {
                continue;
            }
            let reads_params = import.bindings.iter().any(|binding| {
                binding.kind != ImportKind::Namespace
                    && binding.imported.as_deref() == Some("useParams")
            });
            uses_params |= reads_params;
            let mut bytes_differ = false;
            let version = facts.resolved_imports.as_ref().and_then(|imports| {
                match imports.specifier(file.path.as_str(), import.span, ROUTER_PACKAGE) {
                    // A `paths` mapping (or any other resolution outside
                    // `node_modules`): the specifier names a project file
                    // here, and no release of the router is in question.
                    SpecifierAttestation::Attested(row)
                        if matches!(
                            row.resolution,
                            solid_facts::ImportResolution::NonRelative
                                | solid_facts::ImportResolution::Relative
                        ) =>
                    {
                        replaced =
                            Some("a router specifier that does not load the installed package");
                        None
                    }
                    SpecifierAttestation::Attested(row)
                        if row.resolution == solid_facts::ImportResolution::NodeModules
                            && row.resolved_path.contains("patch_hash=") =>
                    {
                        replaced = Some("a patched install of the router");
                        None
                    }
                    // The version the check consults is the one in the
                    // manifest nearest the resolved file (`package_version`).
                    // The resolver's own record of the package
                    // (`resolver_package_version`) must not disagree with it:
                    // two answers about which package was loaded is no
                    // answer.
                    SpecifierAttestation::Attested(row)
                        if row.resolution == solid_facts::ImportResolution::NodeModules
                            && row.package_name.as_deref() == Some(ROUTER_PACKAGE)
                            && row.resolver_package_name.as_deref() == Some(ROUTER_PACKAGE)
                            && row
                                .resolver_package_version
                                .as_deref()
                                .is_none_or(|version| {
                                    row.package_version.as_deref() == Some(version)
                                }) =>
                    {
                        if !runtime_agrees(facts, file, import, &row.resolved_path, ROUTER_PACKAGE)
                        {
                            replaced = Some(
                                "a bundler resolution that does not load the installed package",
                            );
                            return None;
                        }
                        let version = row.package_version.as_deref()?;
                        let digest = dialect.audited_router_digest(version);
                        // Only an audited release has bytes to differ from.
                        bytes_differ = digest.is_some_and(|audited| {
                            row.package_manifest.as_deref().and_then(|manifest| {
                                install.digests.get(manifest).map(String::as_str)
                            }) != Some(audited)
                        });
                        Some(version.to_owned())
                    }
                    _ => None,
                }
            });
            reached.push(Reached {
                version,
                bytes_differ,
                path: file.path.to_string(),
                span: import.span,
                reads_params,
            });
        }
    }
    if !uses_params {
        return RouterParamsStatus::NotUsed;
    }
    if let Some(spelling) = replaced {
        return RouterParamsStatus::Replaced { spelling };
    }
    // The notice sits at the first `useParams` import that reached an
    // unaudited release (or bytes that are not the audited ones), else at the
    // first other import of one.
    let unaudited = |entry: &&Reached| {
        entry.bytes_differ
            || entry
                .version
                .as_deref()
                .is_none_or(|version| !dialect.router_params_are_url_strings(version))
    };
    if let Some(entry) = reached
        .iter()
        .filter(unaudited)
        .find(|entry| entry.reads_params)
        .or_else(|| reached.iter().find(unaudited))
    {
        return RouterParamsStatus::Unaudited {
            version: entry.version.clone(),
            bytes_differ: entry.bytes_differ,
            path: entry.path.clone(),
            span: entry.span,
        };
    }
    if install.replacement_recorded {
        return RouterParamsStatus::Replaced {
            spelling: "a recorded patch, fork or override of the router",
        };
    }
    // Every import resolved to an audited release; one may differ from the
    // next in a monorepo, and the premise then needs each of them audited,
    // which was just checked.
    let version = reached
        .first()
        .and_then(|entry| entry.version.clone())
        .unwrap_or_default();
    for file in &facts.files {
        if file
            .path
            .as_str()
            .contains("/node_modules/@solidjs/router/")
        {
            continue;
        }
        for spelling in REPLACEMENT_SPELLINGS {
            if file.source.contains(spelling) {
                return RouterParamsStatus::Replaced { spelling };
            }
        }
    }
    if let Some(reason) = history_not_stock(facts) {
        return RouterParamsStatus::Replaced { spelling: reason };
    }
    if let Some(reason) = router_context_as_this(facts, &|file| {
        file.path
            .as_str()
            .contains("/node_modules/@solidjs/router/")
    }) {
        return RouterParamsStatus::Replaced { spelling: reason };
    }
    RouterParamsStatus::Applies { version }
}

/// Whether the project's own bundler, when it was asked, loads the same
/// package the compiler resolved. A bundler alias that sends the specifier to
/// a project file or another package makes the audited bytes irrelevant.
/// Without the bundler's answer the compiler's is the premise: the standard
/// Vite configuration of AGENTS.md "Dependency source and pins".
pub(crate) fn runtime_agrees(
    facts: &ProjectFacts,
    file: &FileFacts,
    import: &solid_facts::ast::ImportFact,
    attested: &str,
    package: &str,
) -> bool {
    let Some(runtime) = &facts.runtime_resolutions else {
        return true;
    };
    let Some(specifier) = import.specifier_span else {
        return false;
    };
    match runtime.outcome(file.path.as_str(), specifier, package) {
        solid_facts::runtime_resolution::RuntimeOutcome::File {
            path,
            physical_path,
        } => {
            let installed = format!("/node_modules/{package}/");
            let root = |text: &str| text.rfind(&installed).map(|at| text[..at].to_owned());
            let attested_root = root(attested);
            attested_root.is_some()
                && (root(physical_path) == attested_root || root(path) == attested_root)
        }
        _ => false,
    }
}

/// The reason, when some use of the router can hand it an integration the
/// audit did not read.
///
/// The router builds its params object from `utils.paramsWrapper` of the
/// integration `createRouter` takes from `config.history` (`factory.jsx:141`,
/// `index.js:1306` next.26; `:255`, `:1595` next.41). The premise therefore
/// holds only when **every** router creation in the program names no
/// history, or a stock one called directly
/// and never given a `utils` afterwards. Anything else -- a config that is not
/// a closed literal, a history of another origin, a history passed on, a
/// router reached through a re-export, a namespace member, a dynamic load --
/// is a history the checker cannot read, and withholds the premise.
fn history_not_stock(facts: &ProjectFacts) -> Option<&'static str> {
    let mut accepted_values: BTreeSet<(String, Span)> = BTreeSet::new();
    let mut constants: Vec<(usize, Span)> = Vec::new();
    for (index, file) in facts.files.iter().enumerate() {
        if file
            .path
            .as_str()
            .contains("/node_modules/@solidjs/router/")
        {
            continue;
        }
        let ast = &*file.ast;
        let imports = match router_locals(file) {
            Ok(imports) => imports,
            Err(reason) => return Some(reason),
        };
        if ast
            .exports
            .iter()
            .any(|export| !export.type_only && export.module.as_deref() == Some(ROUTER_PACKAGE))
        {
            return Some("a re-export of the router");
        }
        if ast.module_loads.iter().any(|load| {
            load.specifier.as_deref() != Some("") && names_router(load.specifier.as_deref())
        }) || ast
            .import_equals
            .iter()
            .any(|import| !import.type_only && names_router(Some(import.module.as_str())))
        {
            return Some("a dynamic or CommonJS load of the router");
        }
        if ast.imports.iter().any(|import| {
            !import.type_only
                && import.module.as_str() != ROUTER_PACKAGE
                && names_router(Some(import.module.as_str()))
        }) {
            return Some("a deep import of the router");
        }
        for local in &imports.namespaces {
            for reference in references_to(ast, *local) {
                let member = ast.members.iter().find(|member| member.object == reference);
                if !member.is_some_and(|member| {
                    NAMESPACE_MEMBERS.contains(&text(file, member.property).unwrap_or_default())
                }) {
                    return Some("a router namespace member that is not a hook");
                }
            }
        }
        for local in &imports.create {
            for reference in references_to(ast, *local) {
                let Some(call) = ast
                    .calls
                    .iter()
                    .find(|call| call.callee == reference && call.direct_callee && !call.construct)
                else {
                    return Some("a router creation that is not a direct call");
                };
                let Some(config) = call.arguments.first().filter(|argument| {
                    call.arguments.len() == 1
                        && !argument.spread
                        && argument.exact_object_literal
                        && !text(file, argument.span)
                            .unwrap_or_default()
                            .contains("__proto__")
                        && argument.literal_members.len() == argument.property_names.len()
                }) else {
                    return Some("a router creation whose config is not a closed literal");
                };
                // `2.0.0-next.41` calls `config.links(router, basePath)` with the
                // whole router context (`claims.js:42-44`), whose `wrapParams` a
                // computed key could replace. Refused on every release.
                if config
                    .literal_members
                    .iter()
                    .any(|member| member.key.as_str() == LINKS_KEY)
                {
                    return Some("a router config with `links`");
                }
                // Every audited release renders `<Root ... preload={config.preload}>`
                // and calls `props.preload({ params, location, intent })` on Root's
                // props, which hold `routerState` (next.16 `routers/factory.jsx:125`,
                // `components.jsx:12`; next.18 and next.26 `:228`, `:12`; next.41
                // `:295`, `:13`): a non-arrow `preload` receives the router context
                // as `this`, whose `wrapParams` a computed key could replace.
                if config.literal_members.iter().any(|member| {
                    member.key.as_str() == PRELOAD_KEY && !lexical_this(file, member.value)
                }) {
                    return Some("a router config `preload` that is not an arrow function");
                }
                let Some(history) = config
                    .literal_members
                    .iter()
                    .find(|member| member.key.as_str() == "history")
                else {
                    continue;
                };
                let value = ast.peel_ts_sugar_span(history.value);
                if stock_call(file, value, &imports.stock) {
                    continue;
                }
                match history_constant(file, history.value, value, &imports.stock) {
                    Some(declaration) => {
                        accepted_values.insert((file.path.to_string(), value));
                        accepted_values.insert((file.path.to_string(), history.value));
                        constants.push((index, declaration));
                    }
                    None => return Some("a router history that is not a stock history"),
                }
            }
        }
    }
    for (index, declaration) in constants {
        let file = &facts.files[index];
        let ast = &*file.ast;
        if ast.exports.iter().any(|export| {
            export
                .specifiers
                .iter()
                .chain(&export.declarations)
                .any(|specifier| specifier.local.span == declaration)
        }) {
            return Some("a router history that is exported");
        }
        for reference in references_to(ast, declaration) {
            let accepted = accepted_values.contains(&(file.path.to_string(), reference))
                || ast.members.iter().any(|member: &MemberFact| {
                    member.object == reference
                        && STOCK_HISTORY_CALLS
                            .contains(&text(file, member.property).unwrap_or_default())
                        && ast
                            .calls
                            .iter()
                            .any(|call| call.callee == member.span && !call.construct)
                });
            if !accepted {
                return Some("a router history that the project uses beyond get and set");
            }
        }
    }
    None
}

/// ADR 0324, 0335: the reason, when some project value may run with the
/// router context as `this` through `useAction`, which calls
/// `action.apply({ r: useRouter() }, args)` ([`USE_ACTION`]). `this.r` then
/// reaches `wrapParams` (ADR 0324) and `matches()`, which hold every route
/// definition (ADR 0335).
///
/// Every reference to the router's `useAction` (an exact named import under
/// any local name; a namespace member is refused) must be a direct call whose
/// first argument is proven to keep `this` from project code
/// ([`action_keeps_this`]): an arrow, or the router's own action wrapper.
/// Since the router looks `apply` up on the value, a `prototype`,
/// `__proto__`, `setPrototypeOf` or `getPrototypeOf` spelling, a `__proto__`
/// key, or a write of a member named `apply` anywhere withholds it as a
/// tripwire (as ADR 0335's for `Object.prototype`). `skip` names the
/// router's own files.
pub(crate) fn router_context_as_this(
    facts: &ProjectFacts,
    skip: &dyn Fn(&FileFacts) -> bool,
) -> Option<&'static str> {
    let mut called = false;
    for file in facts.files.iter().filter(|file| !skip(file)) {
        let ast = &*file.ast;
        let locals = ActionLocals::of(file);
        for namespace in &locals.namespaces {
            for reference in references_to(ast, *namespace) {
                let member = ast.members.iter().find(|member| member.object == reference);
                if member.is_none_or(|member| {
                    ast.computed_members.binary_search(&member.span).is_ok()
                        || text(file, member.property) == Some(USE_ACTION)
                }) {
                    return Some("a router namespace that may reach `useAction`");
                }
            }
        }
        for local in &locals.use_action {
            for reference in references_to(ast, *local) {
                called = true;
                let Some(call) = ast
                    .calls
                    .iter()
                    .find(|call| call.callee == reference && call.direct_callee && !call.construct)
                else {
                    return Some("a `useAction` that is not a direct call");
                };
                if !call.arguments.first().is_some_and(|argument| {
                    !argument.spread && action_keeps_this(facts, skip, file, &locals, argument.span)
                }) {
                    return Some("a `useAction` argument that may bind `this`");
                }
            }
        }
    }
    if !called {
        return None;
    }
    for file in facts.files.iter().filter(|file| !skip(file)) {
        let ast = &*file.ast;
        if ast.members.iter().any(|member| {
            matches!(
                text(file, member.property),
                Some("prototype" | "__proto__" | "setPrototypeOf" | "getPrototypeOf")
            )
        }) || ast.object_properties.iter().any(|property| {
            text(file, property.key).is_some_and(|key| key.trim_matches(['"', '\'']) == "__proto__")
        }) {
            return Some("a prototype spelling beside `useAction`");
        }
        let targets = ast
            .assignments
            .iter()
            .map(|assignment| assignment.target)
            .chain(ast.iteration_targets.iter().copied());
        for target in targets {
            let target = ast.peel_ts_sugar_span(target);
            if ast.members.iter().any(|member| {
                target.contains(member.span)
                    && (text(file, member.property) == Some("apply")
                        || ast
                            .literal_computed_members
                            .iter()
                            .any(|literal| literal.span == member.span && literal.key == "apply"))
            }) {
                return Some("a write of `apply` beside `useAction`");
            }
        }
    }
    None
}

/// The local names a file gives the router exports `useAction` reads.
struct ActionLocals {
    use_action: Vec<Span>,
    factories: Vec<Span>,
    readers: Vec<Span>,
    namespaces: Vec<Span>,
}

impl ActionLocals {
    fn of(file: &FileFacts) -> Self {
        let mut locals = Self {
            use_action: Vec::new(),
            factories: Vec::new(),
            readers: Vec::new(),
            namespaces: Vec::new(),
        };
        for import in &file.ast.imports {
            if import.module.as_str() != ROUTER_PACKAGE || import.type_only {
                continue;
            }
            for binding in import.bindings.iter().filter(|binding| !binding.type_only) {
                let local = binding.local.span;
                match (binding.kind, binding.imported.as_deref()) {
                    (ImportKind::Namespace | ImportKind::Default, _) => {
                        locals.namespaces.push(local);
                    }
                    (_, Some(USE_ACTION)) => locals.use_action.push(local),
                    (_, Some(name)) if ACTION_FACTORIES.contains(&name) => {
                        locals.factories.push(local);
                    }
                    (_, Some(name)) if ACTION_READERS.contains(&name) => {
                        locals.readers.push(local);
                    }
                    _ => {}
                }
            }
        }
        locals
    }

    /// The direct call at `span` of one of `locals`.
    fn call<'f>(
        file: &'f FileFacts,
        span: Span,
        locals: &[Span],
    ) -> Option<&'f solid_facts::ast::CallFact> {
        file.ast.calls.iter().find(|call| {
            call.span == span
                && call.direct_callee
                && !call.construct
                && declaration_of(file, call.callee)
                    .is_some_and(|declaration| locals.contains(&declaration))
        })
    }
}

/// Whether `span` is an arrow function, which has no `this` of its own.
fn arrow_function(file: &FileFacts, span: Span) -> bool {
    let span = file.ast.peel_ts_sugar_span(span);
    file.ast.functions.iter().any(|function| {
        function.span == span && function.kind == solid_facts::ast::FunctionKind::Arrow
    })
}

/// The `const` binding (one identifier) the reference at `span` names.
fn const_binding(file: &FileFacts, span: Span) -> Option<(&solid_facts::ast::BindingFact, Span)> {
    let declaration = declaration_of(file, file.ast.peel_ts_sugar_span(span))?;
    file.ast
        .bindings
        .iter()
        .find(|binding| {
            binding.shape == solid_facts::ast::BindingShape::Identifier
                && binding.immutable
                && binding.names.iter().any(|name| name.span == declaration)
        })
        .map(|binding| (binding, declaration))
}

/// Whether the value at `span` has a lexical `this`: an arrow, or a `const`
/// bound to one, which no write can rebind.
fn lexical_this(file: &FileFacts, span: Span) -> bool {
    arrow_function(file, span)
        || const_binding(file, span).is_some_and(|(binding, _)| {
            binding
                .initializer
                .is_some_and(|initializer| arrow_function(file, initializer))
        })
}

/// Whether the `useAction` argument at `span` keeps the router context from
/// project code: an inline arrow, an inline direct call of the router's
/// [`ACTION_FACTORIES`], or a `const` initialized by one whose every
/// reference is a [`safe_action_use`]. An unexported `const` in a file with
/// no direct `eval` is read off the binder; an exported one, or a named
/// import of one, is followed program-wide ([`exported_action_keeps_this`]).
/// No project code then holds the value to give it an own `apply`. Anything
/// else is unproven.
fn action_keeps_this(
    facts: &ProjectFacts,
    skip: &dyn Fn(&FileFacts) -> bool,
    file: &FileFacts,
    locals: &ActionLocals,
    span: Span,
) -> bool {
    if own_action(file, locals, span) {
        return true;
    }
    let Some((binding, declaration)) = const_binding(file, span) else {
        return imported_action(facts, file, span).is_some_and(|(declaring, declaration)| {
            exported_action_keeps_this(facts, skip, declaring, declaration)
        });
    };
    if exports_local(file, declaration) {
        return exported_action_keeps_this(facts, skip, file, declaration);
    }
    binding
        .initializer
        .is_some_and(|initializer| own_action(file, locals, initializer))
        && !crate::interproc::property_gets::direct_eval_in(file, Span::new(0, u32::MAX))
        && references_to(&file.ast, declaration)
            .into_iter()
            .all(|reference| reference == declaration || safe_action_use(file, locals, reference))
}

/// An inline arrow or an inline direct call of the router's
/// [`ACTION_FACTORIES`].
fn own_action(file: &FileFacts, locals: &ActionLocals, span: Span) -> bool {
    let span = file.ast.peel_ts_sugar_span(span);
    arrow_function(file, span) || ActionLocals::call(file, span, &locals.factories).is_some()
}

/// Whether the module's own export list names the local `declaration`
/// (`export const save`, `export { save }`, `export default save`).
fn exports_local(file: &FileFacts, declaration: Span) -> bool {
    file.ast.exports.iter().any(|export| {
        export.module.is_none()
            && export
                .specifiers
                .iter()
                .chain(&export.declarations)
                .any(|specifier| {
                    specifier.local.span == declaration
                        || declaration_of(file, specifier.local.span) == Some(declaration)
                })
    })
}

/// Whether the reference at `reference` hands the action to nothing that can
/// give it an own `apply`: the first argument of `useAction` or of an
/// [`ACTION_READERS`] hook, the callee of a call, or the value of an
/// intrinsic element's unnamespaced attribute (`<form action={save}>`).
fn safe_action_use(file: &FileFacts, locals: &ActionLocals, reference: Span) -> bool {
    let ast = &*file.ast;
    ast.calls.iter().any(|call| {
        call.direct_callee
            && !call.construct
            && call.arguments.first().is_some_and(|argument| {
                !argument.spread && ast.peel_ts_sugar_span(argument.span) == reference
            })
            && declaration_of(file, call.callee).is_some_and(|callee| {
                locals.use_action.contains(&callee) || locals.readers.contains(&callee)
            })
    }) || ast
        .calls
        .iter()
        .any(|call| !call.construct && ast.peel_ts_sugar_span(call.callee) == reference)
        || ast.jsx_elements.iter().any(|element| {
            element.member_object.is_none()
                && text(file, element.name.span)
                    .is_some_and(|name| name.starts_with(|c: char| c.is_ascii_lowercase()))
                && element.attributes.iter().any(|attribute| {
                    // A namespaced attribute (`use:x`) may hand the value to
                    // project code.
                    attribute.namespace.is_none()
                        && attribute
                            .expression
                            .is_some_and(|value| ast.peel_ts_sugar_span(value) == reference)
                })
        })
}

/// The declaring file and `const` name span a `useAction` argument names
/// through a named import, resolved twice and required to agree: the
/// compiler's attested resolution of the specifier lands on the declaring
/// module itself (a barrel is not followed), and the import binding's Type
/// Facts symbol, followed through its aliases, has that one declaration.
fn imported_action<'f>(
    facts: &'f ProjectFacts,
    file: &FileFacts,
    span: Span,
) -> Option<(&'f FileFacts, Span)> {
    let local = declaration_of(file, file.ast.peel_ts_sugar_span(span))?;
    let import = file.ast.imports.iter().find(|import| {
        !import.type_only
            && import.bindings.iter().any(|binding| {
                binding.local.span == local
                    && !binding.type_only
                    && binding.kind == ImportKind::Named
            })
    })?;
    let landed = landing(facts, file, import.span, import.module.as_str())?;
    let root = canonical_symbol(facts, file.path.as_str(), local)?;
    let (path, declaration) = sole_declaration(facts, root)?;
    if path != landed {
        return None;
    }
    let declaring = facts
        .files
        .iter()
        .find(|candidate| candidate.path.as_str() == path)?;
    const_binding_at(declaring, declaration).map(|_| (declaring, declaration))
}

/// ADR 0324, 0335: whether an exported `const` action keeps the router
/// context from project code. It must be initialized by an inline arrow or
/// router action call, and every analyzed file must be accounted for: each
/// import of it is a named import whose attested resolution lands on the
/// declaring module, in a file that does not export it on; no file re-exports
/// from that module or reaches it as a namespace, default import, dynamic
/// load or `import =` (an unattested specifier may be it); no file holding it
/// has a direct `eval`; and every reference, the binder's in each file and
/// Type Facts' program-wide census of the symbol through every alias, is a
/// [`safe_action_use`] or the declaring module's own export list.
///
/// The program boundary (ADR 0193) is not consulted. An unseen importer in an
/// open program may pass the action anywhere, which hands out no `this`; it
/// reaches the router context only by giving the router's own action object
/// an own `apply`, rewriting an object a selected package creates, which the
/// package integrity premise (ADR 0300) excludes. An unseen caller of
/// `useAction` with its own function is outside the program either way.
fn exported_action_keeps_this(
    facts: &ProjectFacts,
    skip: &dyn Fn(&FileFacts) -> bool,
    declaring: &FileFacts,
    declaration: Span,
) -> bool {
    let Some(binding) = const_binding_at(declaring, declaration) else {
        return false;
    };
    let declaring_locals = ActionLocals::of(declaring);
    if !binding
        .initializer
        .is_some_and(|initializer| own_action(declaring, &declaring_locals, initializer))
    {
        return false;
    }
    let Some(root) = canonical_symbol(facts, declaring.path.as_str(), declaration) else {
        return false;
    };
    let Some(symbol) = facts.typescript.symbol(root) else {
        return false;
    };
    let module = declaring.path.as_str();
    // The declaring module's own export list (`export { save }`,
    // `export { save as other }`): its module surface.
    let surface = declaring
        .ast
        .exports
        .iter()
        .filter(|export| export.module.is_none() && export.kind != ExportKind::Default)
        .flat_map(|export| export.specifiers.iter().chain(&export.declarations))
        .map(|specifier| specifier.local.span)
        .collect::<Vec<_>>();
    for file in facts.files.iter().filter(|file| !skip(file)) {
        let ast = &*file.ast;
        if file.source.contains("import.meta.glob") {
            return false;
        }
        // ADR 0339's premise: installed code does not load project modules.
        let installed = file.path.as_str().contains("/node_modules/");
        let reaches = |span: Span, specifier: Option<&str>| {
            !installed
                && specifier.is_none_or(|specifier| {
                    landing(facts, file, span, specifier).is_none_or(|path| path == module)
                })
        };
        if ast.exports.iter().any(|export| {
            !export.type_only
                && export.module.is_some()
                && reaches(export.span, export.module.as_deref())
        }) || ast
            .module_loads
            .iter()
            .any(|load| reaches(load.span, load.specifier.as_deref()))
            || ast
                .import_equals
                .iter()
                .any(|import| !import.type_only && reaches(import.span, Some(&import.module)))
            || ast.imports.iter().any(|import| {
                !import.type_only
                    && import.bindings.iter().any(|binding| {
                        !binding.type_only
                            && matches!(binding.kind, ImportKind::Namespace | ImportKind::Default)
                    })
                    && reaches(import.span, Some(import.module.as_str()))
            })
        {
            return false;
        }
        let here = std::ptr::eq(file, declaring);
        let mut names = Vec::new();
        if here {
            names.push(declaration);
        }
        for import in ast.imports.iter().filter(|import| !import.type_only) {
            let landed = landing(facts, file, import.span, import.module.as_str());
            for binding in import.bindings.iter().filter(|binding| !binding.type_only) {
                match canonical_symbol(facts, file.path.as_str(), binding.local.span) {
                    Some(symbol) if symbol == root => {}
                    Some(_) => continue,
                    // No symbol for a binding that may be the module's: unknown.
                    None if landed.is_none_or(|path| path == module) => return false,
                    None => continue,
                }
                if binding.kind != ImportKind::Named
                    || landed != Some(module)
                    || exports_local(file, binding.local.span)
                {
                    return false;
                }
                names.push(binding.local.span);
            }
        }
        if names.is_empty() {
            continue;
        }
        if crate::interproc::property_gets::direct_eval_in(file, Span::new(0, u32::MAX)) {
            return false;
        }
        let locals = ActionLocals::of(file);
        for name in &names {
            if !references_to(ast, *name).into_iter().all(|reference| {
                reference == *name
                    || (here && surface.contains(&reference))
                    || safe_action_use(file, &locals, reference)
            }) {
                return false;
            }
        }
    }
    // The compiler's census: every identifier any analyzed file resolves to
    // the symbol, through every alias. The `useAction` argument is one, so an
    // empty census is one Type Facts did not carry, not an unused action.
    if symbol.references().next().is_none() {
        return false;
    }
    symbol.references().all(|reference| {
        let Some(file) = facts
            .files
            .iter()
            .find(|file| file.path.as_str() == reference.path.as_ref())
        else {
            return false;
        };
        let (Ok(start), Ok(end)) = (
            u32::try_from(reference.start_byte),
            u32::try_from(reference.end_byte),
        ) else {
            return false;
        };
        let span = Span::new(start, end);
        (std::ptr::eq(file, declaring) && (span == declaration || surface.contains(&span)))
            || safe_action_use(file, &ActionLocals::of(file), span)
    })
}

/// The module path the compiler's attested resolution of `specifier`, written
/// in the statement at `span`, lands on; `None` when it is unattested.
fn landing<'f>(
    facts: &'f ProjectFacts,
    file: &FileFacts,
    span: Span,
    specifier: &str,
) -> Option<&'f str> {
    let SpecifierAttestation::Attested(row) =
        facts
            .resolved_imports
            .as_ref()?
            .specifier(file.path.as_str(), span, specifier)
    else {
        return None;
    };
    let path = if row.included_path.is_empty() {
        &row.resolved_path
    } else {
        &row.included_path
    };
    (!path.is_empty()).then_some(path.as_ref())
}

/// The Type Facts symbol of the entity at exactly `span`, followed through its
/// aliases (cycle-safe) to the symbol that declares it.
fn canonical_symbol<'f>(facts: &'f ProjectFacts, path: &str, span: Span) -> Option<&'f str> {
    let entity = facts
        .typescript
        .entities_for_path(path)
        .iter()
        .find(|entity| {
            entity.location.start_byte == u64::from(span.start)
                && entity.location.end_byte == u64::from(span.end)
                && !entity.symbol.is_empty()
        })?;
    let mut current: &str = entity.symbol.as_ref();
    let mut seen = BTreeSet::new();
    loop {
        if !seen.insert(current) {
            return None;
        }
        let symbol = facts.typescript.symbol(current)?;
        if symbol.alias_target().is_empty() {
            return Some(symbol.id());
        }
        current = symbol.alias_target();
    }
}

/// The one declaration of `symbol`, as its file path and name span.
fn sole_declaration<'f>(facts: &'f ProjectFacts, symbol: &str) -> Option<(&'f str, Span)> {
    let [declaration] = facts.typescript.symbol(symbol)?.declarations() else {
        return None;
    };
    Some((
        declaration.location.path.as_ref(),
        Span::new(
            u32::try_from(declaration.location.start_byte).ok()?,
            u32::try_from(declaration.location.end_byte).ok()?,
        ),
    ))
}

/// The `const` binding (one identifier) whose name is at `declaration`.
fn const_binding_at(file: &FileFacts, declaration: Span) -> Option<&solid_facts::ast::BindingFact> {
    file.ast.bindings.iter().find(|binding| {
        binding.shape == solid_facts::ast::BindingShape::Identifier
            && binding.immutable
            && binding.names.iter().any(|name| name.span == declaration)
    })
}

fn names_router(specifier: Option<&str>) -> bool {
    specifier.is_some_and(|specifier| {
        specifier.contains("solidjs/router") || specifier.contains("node_modules")
    })
}

/// The local names a file gives the router's exports.
struct RouterLocals {
    create: Vec<Span>,
    stock: Vec<Span>,
    namespaces: Vec<Span>,
}

fn router_locals(file: &FileFacts) -> Result<RouterLocals, &'static str> {
    let mut locals = RouterLocals {
        create: Vec::new(),
        stock: Vec::new(),
        namespaces: Vec::new(),
    };
    for import in &file.ast.imports {
        if import.module.as_str() != ROUTER_PACKAGE || import.type_only {
            continue;
        }
        for binding in import.bindings.iter().filter(|binding| !binding.type_only) {
            match (binding.kind, binding.imported.as_deref()) {
                (ImportKind::Namespace, _) => locals.namespaces.push(binding.local.span),
                (_, Some("createRouter")) => locals.create.push(binding.local.span),
                (_, Some("RouterContext")) => return Err("RouterContext"),
                (_, Some(name)) if STOCK_HISTORIES.contains(&name) => {
                    locals.stock.push(binding.local.span);
                }
                (_, Some(_)) => {}
                (_, None) => return Err("a default import of the router"),
            }
        }
    }
    Ok(locals)
}

fn text(file: &FileFacts, span: Span) -> Option<&str> {
    file.source.get(span.start as usize..span.end as usize)
}

/// Every identifier reference the binder resolved to `declaration`.
fn references_to(ast: &solid_facts::ast::AstFacts, declaration: Span) -> Vec<Span> {
    ast.reference_declarations
        .iter()
        .filter(|(_, resolved)| *resolved == declaration)
        .map(|(reference, _)| *reference)
        .collect()
}

fn declaration_of(file: &FileFacts, reference: Span) -> Option<Span> {
    let ast = &*file.ast;
    ast.reference_declarations
        .binary_search_by_key(&reference, |(span, _)| *span)
        .ok()
        .map(|at| ast.reference_declarations[at].1)
        .or_else(|| {
            ast.object_properties
                .iter()
                .find(|property| property.value == reference)
                .and_then(|property| property.shorthand_binding)
        })
}

/// Whether `value` is a direct call of a stock history the file imports.
fn stock_call(file: &FileFacts, value: Span, stock: &[Span]) -> bool {
    file.ast.calls.iter().any(|call| {
        call.span == value
            && call.direct_callee
            && !call.construct
            && declaration_of(file, call.callee)
                .is_some_and(|declaration| stock.contains(&declaration))
    })
}

/// The declaration of the `const` a history value names, when it is
/// initialized by a direct call of a stock history.
fn history_constant(file: &FileFacts, written: Span, value: Span, stock: &[Span]) -> Option<Span> {
    let declaration = declaration_of(file, value).or_else(|| declaration_of(file, written))?;
    file.ast
        .bindings
        .iter()
        .find(|binding| {
            binding.shape == solid_facts::ast::BindingShape::Identifier
                && binding.immutable
                && binding.names.iter().any(|name| name.span == declaration)
        })
        .and_then(|binding| binding.call_initializer)
        .filter(|call| stock_call(file, *call, stock))
        .map(|_| declaration)
}
