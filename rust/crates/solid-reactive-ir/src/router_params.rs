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
use solid_facts::ast::{ImportKind, MemberFact};
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
/// (`routers/history.js:91,131,178`: `go`, `renderPath`, `parsePath`,
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
/// `index.js:1306`). The premise therefore holds only when **every** router
/// creation in the program names no history, or a stock one called directly
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
