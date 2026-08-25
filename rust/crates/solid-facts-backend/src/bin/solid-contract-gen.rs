//! Generates dialect review contracts and export indexes from an installed
//! package.
//!
//! A review contract has two halves and only one of them can be derived.
//!
//! The **export set** is a syntactic fact about the package's declarations,
//! and this reads it with `solid_facts::ast` — the same oxc parser the checker
//! uses on user code — following `export *` and `export { x } from` chains.
//! It replaces a regex over `.d.ts` text, which is what the first version of
//! this did and which cannot see the difference between an export and the word
//! "export" in a comment.
//!
//! The **reactive semantics** cannot be derived, and the tables below say so
//! by carrying their evidence. Whether `resolve` opens a `createRoot`, whether
//! `flush` establishes an owner, whether `deep` returns a live store or a
//! snapshot: every one of those is a reading of the runtime's implementation,
//! and every one of them is invisible in the type signature. Three were got
//! wrong from signatures alone before being read from `@solidjs/signals`.
//!
//! The CLI's own `--emit-contract` is the wrong tool here for a structural
//! reason: it derives a package's semantics by tracing its exports back into
//! Solid primitives it already knows, and that knowledge *is* the bundled
//! contract. Pointed at solid-js it finds nothing to trace to.
//!
//! ```text
//! solid-contract-gen --package <dir> --dialect <solid-v1/solid-js|solid-v2/solid-js|solid-v2/solidjs-web> \
//!     --out <path> [--check]
//! ```

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

/// What a callback argument does, in the contract's vocabulary: `tracked`
/// re-runs on dependency change, `deferred` runs later than the call, `inline`
/// runs immediately in the caller's scope.
struct Callback(usize, &'static str);

/// A returned reactive value the contract can describe. Only single values —
/// a tuple like `createSignal`'s is answered by `Dialect::creates_reactive_source`.
struct Returns(&'static str, &'static str);

struct Semantics {
    /// `Some` is a reviewed finite callback domain; `None` is schema-v1's
    /// honest unknown sentinel. The latter is required for variadic APIs and
    /// for exports whose condition-selected implementations disagree, because
    /// this flat review artifact cannot carry entrypoint variants.
    callbacks: Option<&'static [Callback]>,
    returns: Option<Returns>,
}

const fn cb(callbacks: &'static [Callback]) -> Semantics {
    Semantics {
        callbacks: Some(callbacks),
        returns: None,
    }
}

const fn unknown_callbacks() -> Semantics {
    Semantics {
        callbacks: None,
        returns: None,
    }
}

const fn ret(kind: &'static str, label: &'static str) -> Semantics {
    Semantics {
        callbacks: Some(&[]),
        returns: Some(Returns(kind, label)),
    }
}

const fn both(
    callbacks: &'static [Callback],
    kind: &'static str,
    label: &'static str,
) -> Semantics {
    Semantics {
        callbacks: Some(callbacks),
        returns: Some(Returns(kind, label)),
    }
}

const fn unknown_callbacks_and_return(kind: &'static str, label: &'static str) -> Semantics {
    Semantics {
        callbacks: None,
        returns: Some(Returns(kind, label)),
    }
}

/// One declaration entry point: the module specifier a user writes, and the
/// `.d.ts` that specifier resolves to.
///
/// The specifier is carried rather than inferred because the export index is
/// keyed by it, and it is optional because the two outputs need different
/// sets. A file the package does not publish under any specifier still
/// declares names the *compiler* emits calls to, and the contract has to model
/// those; the index must not claim they are importable.
struct Entry {
    module: Option<&'static str>,
    path: &'static str,
}

const fn entry(module: &'static str, path: &'static str) -> Entry {
    Entry {
        module: Some(module),
        path,
    }
}

/// A declaration file no specifier resolves to: contract yes, index no.
const fn internal(path: &'static str) -> Entry {
    Entry { module: None, path }
}

/// One contract artifact this generator can produce: a package at a pinned
/// major, with its declaration entry points and reviewed semantics.
///
/// The target id starts with its stable dialect id and ends with a package
/// slug. One dialect can own several package artifacts without inventing a
/// second, unrelated identifier namespace.
struct ContractTarget {
    /// Contract file stem, and the `--dialect` name.
    id: &'static str,
    package: &'static str,
    /// Declaration entry points, relative to the package root. Every subpath,
    /// because the contract is keyed by export name and a name provided only
    /// by a subpath is still an export of the package.
    ///
    /// Under-listing subpaths is not a conservative omission, it is a false
    /// positive generator: a name the index has under `.` and not under
    /// `/storage` reads as "imported from the wrong module" when it is
    /// imported from `/storage`.
    entries: &'static [Entry],
    /// Names whose declaration syntax occupies value position even though the
    /// package publishes no runtime binding. Runtime probes are authoritative
    /// for these ambient namespaces and type re-exports.
    type_only: &'static [&'static str],
    /// Exports that are values rather than callables.
    values: &'static [&'static str],
    semantics: &'static [(&'static str, Semantics)],
    /// Required so a stale table cannot silently describe a package that no
    /// longer has the name.
    major: &'static str,
}

/// Solid 2.0. Sourced by reading `solid-js@2.0.0-rc.0` and its bundled
/// `@solidjs/signals` implementation, not the signatures — see the module docs.
static SOLID_2: ContractTarget = ContractTarget {
    id: "solid-v2/solid-js",
    package: "solid-js",
    major: "2.",
    // `solid-js@2` publishes its runtime root and the development refresh
    // helpers. `component.d.ts` and
    // `flow.d.ts` used to be listed too and are reached by the root's
    // `export *` anyway; `client/index.d.ts` was listed and has never existed.
    //
    // `core.d.ts` stays, as internal. The root re-exports four names from it
    // by name, so the walk stops there — but the two it leaves behind,
    // `devComponent` and `IS_DEV`, are what the JSX transform emits calls to,
    // and the contract has to carry `devComponent`'s callback shape. Nobody
    // can import either, so the index does not list them.
    entries: &[
        entry("solid-js", "types/index.d.ts"),
        entry("solid-js/refresh", "types/refresh/index.d.ts"),
        internal("types/client/core.d.ts"),
    ],
    type_only: &[],
    // Verified `declare const` or `declare class` in solid-js or
    // @solidjs/signals. `enableExternalSource` and `enforceLoadingBoundary`
    // look like values and are `declare function` — they are declared in
    // @solidjs/signals, which an earlier grep over solid-js/types/ did not
    // reach, so the hand-written contract called them values and this table
    // copied it.
    values: &[
        "$PROXY",
        "$RAW",
        "$TRACK",
        "$REFRESH",
        "$DEVCOMP",
        "DEV",
        "IS_DEV",
        "sharedConfig",
        "storePath",
        "NoHydrateContext",
        "NotReadyError",
    ],
    semantics: &[
        (
            "createMemo",
            both(&[Callback(0, "tracked")], "accessor", "memo result"),
        ),
        (
            "createEffect",
            cb(&[Callback(0, "tracked"), Callback(1, "deferred")]),
        ),
        (
            "createRenderEffect",
            cb(&[Callback(0, "tracked"), Callback(1, "deferred")]),
        ),
        ("createTrackedEffect", cb(&[Callback(0, "tracked")])),
        (
            "createProjection",
            both(&[Callback(0, "tracked")], "store-path", "projection result"),
        ),
        ("onSettled", cb(&[Callback(0, "deferred")])),
        ("createRoot", cb(&[Callback(0, "inline")])),
        ("runWithOwner", cb(&[Callback(1, "inline")])),
        ("children", ret("accessor", "resolved children")),
        (
            "mapArray",
            both(&[Callback(1, "tracked")], "accessor", "mapped array"),
        ),
        ("repeat", cb(&[Callback(1, "tracked")])),
        ("createReaction", cb(&[Callback(0, "deferred")])),
        // Both own the scope their body runs in and return an accessor. The
        // fallback argument is a callback too, invoked only when the boundary
        // trips.
        (
            "createErrorBoundary",
            both(
                &[Callback(0, "tracked"), Callback(1, "deferred")],
                "accessor",
                "guarded value or error fallback",
            ),
        ),
        (
            "createLoadingBoundary",
            both(
                &[Callback(0, "tracked"), Callback(1, "deferred")],
                "accessor",
                "guarded value or loading fallback",
            ),
        ),
        // resolve(fn) wraps its thunk in createRoot -- the signature does not
        // suggest it.
        ("resolve", cb(&[Callback(0, "deferred")])),
        ("lazy", cb(&[Callback(0, "deferred")])),
        // Scope wrappers that run their thunk immediately in the caller's
        // scope. latest/isPending catch NotReadyError but do not clear
        // tracking, so reads inside subscribe -- same inline row as untrack.
        ("untrack", cb(&[Callback(0, "inline")])),
        ("flush", cb(&[Callback(0, "inline")])),
        ("latest", cb(&[Callback(0, "inline")])),
        ("isPending", cb(&[Callback(0, "inline")])),
        // The handler runs when the action is dispatched, never at creation.
        ("action", cb(&[Callback(0, "deferred")])),
        ("createRevealOrder", cb(&[Callback(0, "inline")])),
        ("createComponent", cb(&[Callback(0, "inline")])),
        ("devComponent", cb(&[Callback(0, "inline")])),
        ("ssrScope", cb(&[Callback(0, "inline")])),
        // Public but intended for framework-generated server-component
        // boundaries. The browser build invokes it inline; the server build
        // creates a transparent owner first. The dialect reviews it as an
        // internal callback-taker rather than a user-facing primitive.
        ("runInServerComponentScope", cb(&[Callback(0, "inline")])),
    ],
};

/// Solid 1.x. Table ported from the `1.x` branch's own generator, where it was
/// hand-authored against this package version and reviewed.
///
/// Returning a cleanup is a 2.0 idea: 1.x threads an effect callback's return
/// value to the next run as `prev`.
static SOLID_1X: ContractTarget = ContractTarget {
    id: "solid-v1/solid-js",
    package: "solid-js",
    major: "1.",
    entries: &[
        entry("solid-js", "types/index.d.ts"),
        entry("solid-js/store", "store/types/index.d.ts"),
        entry("solid-js/web", "web/types/index.d.ts"),
        entry("solid-js/universal", "universal/types/index.d.ts"),
    ],
    type_only: &[],
    // Anything absent here is emitted as a function. The eight web constant
    // tables below are `const` declarations whose runtime values are objects,
    // Sets and Symbols; calling them a function is a claim about the package
    // that its own builds refute, which the runtime surface now cross-checks.
    values: &[
        "$PROXY",
        "$TRACK",
        "$RAW",
        "$DEVCOMP",
        "DEV",
        "sharedConfig",
        "isServer",
        "isDev",
        "Aliases",
        "ChildProperties",
        "DelegatedEvents",
        "DOMElements",
        "Properties",
        "RequestContext",
        "SVGElements",
        "SVGNamespace",
    ],
    semantics: &[
        (
            "createMemo",
            both(&[Callback(0, "tracked")], "accessor", "memo result"),
        ),
        ("createEffect", cb(&[Callback(0, "tracked")])),
        ("createRenderEffect", cb(&[Callback(0, "tracked")])),
        ("createComputed", cb(&[Callback(0, "tracked")])),
        (
            "createDeferred",
            both(&[Callback(0, "tracked")], "accessor", "deferred value"),
        ),
        ("createReaction", cb(&[Callback(0, "deferred")])),
        (
            "createSelector",
            both(
                &[Callback(0, "tracked"), Callback(1, "inline")],
                "accessor",
                "selector result",
            ),
        ),
        (
            "children",
            both(&[Callback(0, "tracked")], "accessor", "resolved children"),
        ),
        (
            "mapArray",
            both(
                &[Callback(0, "tracked"), Callback(1, "deferred")],
                "accessor",
                "mapped array",
            ),
        ),
        (
            "indexArray",
            both(
                &[Callback(0, "tracked"), Callback(1, "deferred")],
                "accessor",
                "mapped array",
            ),
        ),
        (
            "from",
            both(
                &[Callback(0, "inline")],
                "accessor",
                "external source value",
            ),
        ),
        ("createRoot", cb(&[Callback(0, "inline")])),
        ("runWithOwner", cb(&[Callback(1, "inline")])),
        ("untrack", cb(&[Callback(0, "inline")])),
        ("batch", cb(&[Callback(0, "inline")])),
        ("onCleanup", cb(&[Callback(0, "deferred")])),
        ("onMount", cb(&[Callback(0, "deferred")])),
        ("onError", cb(&[Callback(0, "deferred")])),
        (
            "catchError",
            cb(&[Callback(0, "inline"), Callback(1, "deferred")]),
        ),
        // `createResource(source, fetcher)` tracks parameter 0 and invokes
        // parameter 1 outside the caller, while `createResource(fetcher)`
        // treats parameter 0 as that fetcher. Schema v1 cannot select a
        // callback map by overload, so a finite row set would certify the
        // other call shape incorrectly.
        ("createResource", unknown_callbacks()),
        ("startTransition", cb(&[Callback(0, "inline")])),
        ("lazy", cb(&[Callback(0, "deferred")])),
        ("createComponent", cb(&[Callback(0, "inline")])),
        // Every function-valued source is memoized. The parameter domain is
        // variadic, so schema v1 cannot state the complete callback set.
        ("mergeProps", unknown_callbacks()),
        ("requestCallback", cb(&[Callback(0, "deferred")])),
        ("getNextElement", cb(&[Callback(0, "inline")])),
        ("use", cb(&[Callback(0, "inline")])),
        // Returns the store itself -- a single value the contract can describe,
        // unlike createStore's tuple.
        ("createMutable", ret("store-path", "mutable store")),
        ("produce", cb(&[Callback(0, "inline")])),
        ("modifyMutable", cb(&[Callback(1, "inline")])),
        ("render", cb(&[Callback(0, "inline")])),
        ("hydrate", cb(&[Callback(0, "inline")])),
        ("effect", cb(&[Callback(0, "tracked")])),
        (
            "memo",
            both(&[Callback(0, "tracked")], "accessor", "memo result"),
        ),
        ("createDynamic", cb(&[Callback(0, "tracked")])),
    ],
};

/// `@solidjs/web`, 2.0's DOM package.
static SOLIDJS_WEB: ContractTarget = ContractTarget {
    id: "solid-v2/solidjs-web",
    package: "@solidjs/web",
    major: "2.",
    // Every subpath the package's `exports` map publishes. The JSX runtime
    // entries declare only types, but they are public import locations in RC.0
    // and own the renderer-specific JSX namespace, so the type-position index
    // must carry them too. `dist/types/index.d.ts` used to be listed here and
    // has never existed in the published package.
    //
    // `./server-functions` and `./frames` each resolve to client or server
    // declarations depending on the condition, so both are listed under the
    // condition-independent specifier. The union is the right answer for an
    // import-location check: a name reachable under either condition is
    // reachable from that specifier.
    entries: &[
        entry("@solidjs/web", "types/index.d.ts"),
        entry("@solidjs/web/jsx-runtime", "types/jsx.d.ts"),
        entry("@solidjs/web/jsx-dev-runtime", "types/jsx.d.ts"),
        entry("@solidjs/web/storage", "storage/types/index.d.ts"),
        entry(
            "@solidjs/web/serialization",
            "serialization/types/index.d.ts",
        ),
        entry(
            "@solidjs/web/serialization/decode",
            "serialization/types/serializer-decode.d.ts",
        ),
        entry(
            "@solidjs/web/server-functions",
            "types/server-functions/client.d.ts",
        ),
        entry(
            "@solidjs/web/server-functions",
            "types/server-functions/server.d.ts",
        ),
        entry(
            "@solidjs/web/server-functions/client",
            "types/server-functions/client.d.ts",
        ),
        entry(
            "@solidjs/web/server-functions/server",
            "types/server-functions/server.d.ts",
        ),
        entry(
            "@solidjs/web/server-functions/rich-args",
            "types/server-functions/rich-args.d.ts",
        ),
        entry("@solidjs/web/frames", "types/frames/client.d.ts"),
        entry("@solidjs/web/frames", "types/frames/server.d.ts"),
        entry("@solidjs/web/frames/client", "types/frames/client.d.ts"),
        entry("@solidjs/web/frames/server", "types/frames/server.d.ts"),
    ],
    // Both names are type-only in the published runtime even though their
    // ambient/re-export declaration syntax is classified in value position.
    type_only: &["JSX", "RequestEventLocals"],
    // Verified runtime values across every RC.0 entrypoint. This is kept
    // separate from the syntactic value-position index: TypeScript functions,
    // classes, and constants all occupy value position, while the contract's
    // `kind` distinguishes callable exports from inert data.
    values: &[
        "ChildProperties",
        "DEFAULT_WEB_PLUGINS",
        "DelegatedEvents",
        "DOMElements",
        "DOMWithState",
        "ERROR_HEADER",
        "FLASH_COOKIE",
        "FRAME_APPLIED_EVENT",
        "FRAME_STREAM_HEADER",
        "FUNCTION_HEADER",
        "GENERIC_SERVER_ERROR_MESSAGE",
        "HREF",
        "INSTANCE_HEADER",
        "isDev",
        "isServer",
        "MathMLElements",
        "Namespaces",
        "OpaqueReference",
        "RawTextElements",
        "RequestContext",
        "ResponseEnvelope",
        "REVALIDATE_HEADER",
        "SAFE_ERROR",
        "SERVER_COMPONENT_BOOTSTRAP",
        "ServerComponentPlugin",
        "SINGLE_FLIGHT_HEADER",
        "SVGElements",
        "VoidElements",
    ],
    semantics: &[
        ("render", cb(&[Callback(0, "inline")])),
        ("hydrate", cb(&[Callback(0, "inline")])),
        ("applyRef", cb(&[Callback(0, "inline")])),
        ("createComponent", cb(&[Callback(0, "inline")])),
        // Browser `dynamic` owns a tracked memo. The root server helper's memo
        // is eager, while the JSX runtime's `{ lazy: true }` keeps it deferred;
        // one name-level row cannot represent both public entrypoints.
        ("dynamic", unknown_callbacks()),
        // Compiler-runtime helpers the JSX transform emits calls to; the
        // dialect reviews them as unmodelled callback-takers. Their browser
        // and server executions differ, so the flat artifact must not pick one.
        ("effect", unknown_callbacks()),
        (
            "memo",
            unknown_callbacks_and_return("accessor", "memo result"),
        ),
        // Browser calls the hydration template; the server binding is a
        // client-only throwing stub.
        ("getNextElement", unknown_callbacks()),
        // Variadic: every function source is memoized, so no finite parameter
        // list can close the callback domain.
        ("mergeProps", unknown_callbacks()),
        // Browser stubs versus server implementations that invoke callbacks.
        ("renderToString", unknown_callbacks()),
        ("ssrElement", unknown_callbacks()),
        ("untrack", cb(&[Callback(0, "inline")])),
        ("frameTransformResult", cb(&[Callback(1, "inline")])),
        ("serverComponentResponse", cb(&[Callback(0, "inline")])),
        // The storage module is fixed, but its `isServer` import follows the
        // host conditions: browser builds throw before invoking the callback.
        ("provideRequestEvent", unknown_callbacks()),
        // Browser default is immediate, `{ lazy: true }` defers until the
        // wrapper renders, and the server never invokes the loader. The flat
        // schema cannot express that condition, so `deferred` is the safe
        // checker role: reachable, but never a reactive subscriber.
        ("clientOnly", cb(&[Callback(0, "deferred")])),
        // The browser implementation passes this thunk to the compiler's
        // `effect` helper; SSR registers it for evaluation by the renderer.
        ("useHead", cb(&[Callback(0, "tracked")])),
    ],
};

static TARGETS: &[&ContractTarget] = &[&SOLID_2, &SOLID_1X, &SOLIDJS_WEB];

/// The names one declaration entry exports, split by position.
#[derive(Default)]
struct Exports {
    values: BTreeSet<String>,
    types: BTreeSet<String>,
}

fn correct_runtime_type_only(exports: &mut Exports, names: &[&str]) {
    for name in names {
        if exports.values.remove(*name) {
            exports.types.insert((*name).to_owned());
        }
    }
}

/// Every name a declaration entry exports, following re-export chains.
///
/// Parsed, not matched: `solid_facts::ast` is oxc, so `export { x }` inside a
/// comment or a string is not an export and a multi-line specifier list is.
///
/// Types are collected alongside values because the import-location index
/// needs them: `import type { Store } from "solid-js"` does not resolve in
/// 1.x either, and a value-only index cannot say so.
///
/// A resolution failure is a hard error, never a skip. This walk *is* the
/// export set: a missing, unreadable, or unparseable file swallowed here
/// regenerates a silently truncated contract with exit 0, and `--check` then
/// certifies the truncation as in sync. The `names.is_empty()` and
/// stale-semantics checks downstream stay as secondary guards, but they only
/// catch truncations large enough to lose a reviewed name.
fn exported_names(
    entry: &Path,
    out: &mut Exports,
    seen: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    let canonical = entry
        .canonicalize()
        .map_err(|error| format!("no declaration file at {}: {error}", entry.display()))?;
    if !seen.insert(canonical.clone()) {
        return Ok(());
    }
    let source = fs::read_to_string(&canonical)
        .map_err(|error| format!("read {}: {error}", canonical.display()))?;
    let facts = solid_facts::ast::extract(canonical.to_string_lossy(), &source)
        .map_err(|error| format!("parse {}: {error}", canonical.display()))?;
    for export in &facts.exports {
        // Two different flags, and in a `.d.ts` only one of them is usable.
        //
        // oxc marks every ambient `export declare ...` statement `type_only`,
        // because a declaration file emits no runtime values — so the
        // statement flag says "type" for `export declare function lazy` just
        // as loudly as for `export interface Component`. The *specifier* flag
        // is the one that discriminates: `false` for functions and consts,
        // `true` for type aliases and interfaces.
        //
        // For re-exports the statement flag is right and the specifier flag is
        // not: `export type { A } from "./x"` marks the statement and leaves
        // the specifier alone.
        //
        // So: inline declarations are judged per specifier, re-exports per
        // statement. Reading the statement flag for both is what made the
        // first version of this drop every value export in the package.
        for declared in &export.declarations {
            let side = if declared.type_only {
                &mut out.types
            } else {
                &mut out.values
            };
            side.insert(declared.exported.to_string());
        }
        for specifier in &export.specifiers {
            let side = if export.type_only || specifier.type_only {
                &mut out.types
            } else {
                &mut out.values
            };
            side.insert(specifier.exported.to_string());
        }
        // Only `export * from "./x"` needs the target walked, and only for the
        // names it does not list. A named re-export was captured above, and
        // walking its target too attributes every *other* name in that file to
        // this module: `solid-js/store` re-exports three names from
        // `./store.js` and would otherwise inherit all of it. Harmless in a
        // flat contract, wrong in an index keyed by module.
        if export.kind == solid_facts::ast::ExportKind::All
            && !export.type_only
            && let Some(module) = &export.module
            && module.starts_with('.')
        {
            let base = canonical.parent().unwrap_or(Path::new("."));
            let stem = [".js", ".mjs", ".cjs", ".mts", ".cts"]
                .iter()
                .find_map(|extension| module.strip_suffix(extension))
                .unwrap_or(module);
            let candidates = [
                base.join(format!("{stem}.d.ts")),
                base.join(format!("{stem}.d.mts")),
                base.join(format!("{stem}.d.cts")),
                base.join(stem).join("index.d.ts"),
            ];
            // Every existing candidate is walked -- a `.d.ts` and a
            // directory index can coexist and each contribute names. None
            // existing is the same silent-truncation class as an unreadable
            // entry point, so it is answered the same way.
            let mut resolved = false;
            for candidate in candidates.iter().filter(|candidate| candidate.is_file()) {
                resolved = true;
                exported_names(candidate, out, seen)?;
            }
            if !resolved {
                return Err(format!(
                    "cannot resolve `export * from \"{module}\"` in {}: tried {}",
                    canonical.display(),
                    candidates
                        .iter()
                        .map(|candidate| candidate.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
    }
    Ok(())
}

/// A JSON string literal for `text`, escaped. Export names come out of a
/// parser, and `export { x as "a\"b" }` is a legal declaration whose name
/// concatenated raw would emit invalid JSON.
fn json_string(text: &str) -> String {
    serde_json::to_string(text).expect("a string always serializes")
}

fn render(target: &ContractTarget, version: &str, names: &BTreeSet<String>) -> String {
    let semantics: BTreeMap<_, _> = target
        .semantics
        .iter()
        .map(|(name, value)| (*name, value))
        .collect();
    let values: BTreeSet<_> = target.values.iter().copied().collect();

    let mut sorted: Vec<_> = names.iter().collect();
    sorted.sort_by_key(|name| name.trim_start_matches('$').to_lowercase());

    let mut body = String::new();
    for (index, name) in sorted.iter().enumerate() {
        let entry = semantics.get(name.as_str());
        let is_value = values.contains(name.as_str()) && entry.is_none();
        body.push_str(&format!(
            "    {}: {{\n      \"kind\": \"",
            json_string(name)
        ));
        body.push_str(if is_value { "value" } else { "function" });
        body.push('"');
        if let Some(Semantics {
            returns: Some(Returns(kind, label)),
            ..
        }) = entry
        {
            body.push_str(&format!(
                ",\n      \"returns\": {{\n        \"kind\": \"{kind}\",\n        \"label\": \"{label}\"\n      }}"
            ));
        }
        if let Some(Semantics {
            callbacks: None, ..
        }) = entry
        {
            body.push_str(",\n      \"callbacks\": {\n        \"status\": \"unknown\"\n      }");
        } else if let Some(Semantics {
            callbacks: Some(callbacks),
            ..
        }) = entry
            && !callbacks.is_empty()
        {
            body.push_str(",\n      \"callbacks\": [\n");
            for (position, Callback(parameter, execution)) in callbacks.iter().enumerate() {
                body.push_str(&format!(
                    "        {{\n          \"parameter\": {parameter},\n          \"execution\": \"{execution}\"\n        }}"
                ));
                body.push_str(if position + 1 == callbacks.len() {
                    "\n"
                } else {
                    ",\n"
                });
            }
            body.push_str("      ]");
        }
        body.push_str("\n    }");
        body.push_str(if index + 1 == sorted.len() {
            "\n"
        } else {
            ",\n"
        });
    }

    format!(
        "{{\n  \"schemaVersion\": 1,\n  \"package\": {{\n    \"name\": {},\n    \"version\": {}\n  }},\n  \"compilerFactsProtocol\": 1,\n  \"exports\": {{\n{body}  }},\n  \"evidence\": {{\n    \"kind\": \"reviewed\",\n    \"generator\": \"solid-contract-gen\"\n  }}\n}}\n",
        json_string(target.package),
        json_string(version)
    )
}

/// Whether a parsed name is one a user could write. Filters out the `default`
/// keyword and anything oxc hands back from a string-named export.
fn declarable(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '$')
}

/// Name to the modules that export it, one entry per position.
#[derive(Default)]
struct Index {
    values: BTreeMap<String, BTreeSet<String>>,
    types: BTreeMap<String, BTreeSet<String>>,
}

fn render_table(name: &str, doc: &str, table: &BTreeMap<String, BTreeSet<String>>) -> String {
    let mut body = String::new();
    for (export, modules) in table {
        // `{:?}` renders a Rust string literal, escaped -- the export names
        // are parser output, and this file is compiled.
        let modules: Vec<_> = modules.iter().map(|module| format!("{module:?}")).collect();
        body.push_str(&format!("    ({export:?}, &[{}]),\n", modules.join(", ")));
    }
    // `rustfmt::skip` is load bearing, not tidiness. Without it `cargo fmt`
    // explodes the multi-module rows across five lines each, `--check` then
    // reports the file stale, and regenerating puts it back -- the two
    // commands undo each other forever.
    format!("{doc}#[rustfmt::skip]\npub static {name}: &[(&str, &[&str])] = &[\n{body}];\n")
}

/// The export index as Rust, for `solid-dialect` to `include!`.
///
/// A generated source file rather than a JSON the dialect parses at startup:
/// `solid-dialect` has no reader, sits below every other crate, and the answer
/// is a constant. The cost is that it is checked in, which is why `--check`
/// covers it — a hand-edit is the same failure as a stale contract.
fn render_index(target: &ContractTarget, version: &str, index: &Index) -> String {
    format!(
        "//! Where `{}@{version}` exports each name, keyed by the module specifier a\n\
         //! user writes in an import.\n\
         //!\n\
         //! GENERATED by `solid-contract-gen`. Do not edit; run `make contracts`.\n\
         //!\n\
         //! Sorted by name so lookup can binary search. A name appears under every\n\
         //! module that exports it — 1.x's `solid-js/web` re-exports nine control-flow\n\
         //! components from `solid-js`, and importing one of them from either module\n\
         //! resolves.\n\
         \n\
         {}\n{}",
        target.package,
        render_table(
            "VALUES",
            "/// Names exported in value position.\n",
            &index.values
        ),
        render_table(
            "TYPES",
            "/// Names exported in type position. Disjoint from [`VALUES`] only by\n\
             /// accident: a class is both, and appears in each.\n",
            &index.types
        ),
    )
}

fn main() -> ExitCode {
    let mut package = None;
    let mut id = None;
    let mut out = None;
    let mut index_out = None;
    let mut check = false;
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--package" => package = args.next(),
            "--dialect" => id = args.next(),
            "--out" => out = args.next(),
            "--index-out" => index_out = args.next(),
            "--check" => check = true,
            other => {
                eprintln!("unexpected argument {other}");
                return ExitCode::from(2);
            }
        }
    }
    let (Some(package), Some(id), Some(out), Some(index_out)) = (package, id, out, index_out)
    else {
        eprintln!(
            "usage: solid-contract-gen --package <dir> --dialect <{}> --out <path> --index-out <path> [--check]",
            TARGETS
                .iter()
                .map(|target| target.id)
                .collect::<Vec<_>>()
                .join("|")
        );
        return ExitCode::from(2);
    };
    let Some(target) = TARGETS.iter().find(|target| target.id == id) else {
        eprintln!("unknown dialect {id}");
        return ExitCode::from(2);
    };
    let root = PathBuf::from(&package);

    let manifest = match fs::read_to_string(root.join("package.json")) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("no package at {package}: {error}");
            return ExitCode::from(2);
        }
    };
    let manifest: serde_json::Value = match serde_json::from_str(&manifest) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("{package}/package.json is not JSON: {error}");
            return ExitCode::from(2);
        }
    };
    let Some(version) = manifest.get("version").and_then(serde_json::Value::as_str) else {
        eprintln!("{package}/package.json has no version");
        return ExitCode::from(2);
    };
    if !version.starts_with(target.major) {
        eprintln!(
            "{package} is {} {version}, not {}x",
            target.package, target.major
        );
        return ExitCode::FAILURE;
    }

    // A fresh `seen` per entry, deliberately. Sharing it across modules made
    // the walk order decide the answer: `solid-js/web` re-exports nine names
    // from `solid-js`, and a `seen` carried over from the `solid-js` entry
    // skips that file and leaves the web module missing all nine.
    let mut index = Index::default();
    let mut names = BTreeSet::new();
    for entry in target.entries {
        let mut exports = Exports::default();
        if let Err(error) =
            exported_names(&root.join(entry.path), &mut exports, &mut BTreeSet::new())
        {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
        correct_runtime_type_only(&mut exports, target.type_only);
        // The contract's export set is every value name from every entry; the
        // index only records the ones a specifier reaches.
        names.extend(
            exports
                .values
                .iter()
                .filter(|name| declarable(name))
                .cloned(),
        );
        let Some(module) = entry.module else {
            continue;
        };
        for (side, table) in [
            (&exports.values, &mut index.values),
            (&exports.types, &mut index.types),
        ] {
            for name in side.iter().filter(|name| declarable(name)) {
                table
                    .entry(name.clone())
                    .or_default()
                    .insert(module.to_owned());
            }
        }
    }
    if names.is_empty() {
        eprintln!("no exports parsed from {package}; the declaration layout changed");
        return ExitCode::FAILURE;
    }

    // A table naming an export the package does not have is stale, and a stale
    // entry is invisible: it simply never matches.
    let stale: Vec<_> = target
        .semantics
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !names.contains(*name))
        .collect();
    if !stale.is_empty() {
        eprintln!(
            "the {id} semantics table names exports {} {version} does not have: {}",
            target.package,
            stale.join(", ")
        );
        return ExitCode::FAILURE;
    }

    let outputs = [
        (out, render(target, version, &names)),
        (index_out, render_index(target, version, &index)),
    ];
    let published: BTreeSet<_> = target
        .entries
        .iter()
        .filter_map(|entry| entry.module)
        .collect();
    if check {
        let stale: Vec<_> = outputs
            .iter()
            .filter(|(path, rendered)| {
                !fs::read_to_string(path).is_ok_and(|current| &current == rendered)
            })
            .map(|(path, _)| path.as_str())
            .collect();
        if !stale.is_empty() {
            eprintln!(
                "{} is stale -- rerun solid-contract-gen without --check",
                stale.join(" and ")
            );
            return ExitCode::FAILURE;
        }
        println!(
            "{id}: in sync ({} exports over {} module(s), {version})",
            names.len(),
            published.len()
        );
        return ExitCode::SUCCESS;
    }
    for (path, rendered) in &outputs {
        if let Err(error) = fs::write(path, rendered) {
            eprintln!("write {path}: {error}");
            return ExitCode::FAILURE;
        }
        println!("wrote {path}");
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A restructured package layout must fail the run, not shrink the
    /// contract: `main` turns any `exported_names` error into a non-zero
    /// exit in both generate and `--check` modes.
    #[test]
    fn resolution_failures_are_errors_not_omissions() {
        let root =
            std::env::temp_dir().join(format!("solid-contract-gen-loud-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();

        // A missing entry point.
        let missing = exported_names(
            &root.join("missing.d.ts"),
            &mut Exports::default(),
            &mut BTreeSet::new(),
        );
        assert!(missing.is_err());

        // An `export *` whose target no candidate stem resolves.
        let entry = root.join("index.d.ts");
        std::fs::write(&entry, "export * from \"./gone.js\";\n").unwrap();
        let unresolved =
            exported_names(&entry, &mut Exports::default(), &mut BTreeSet::new()).unwrap_err();
        assert!(unresolved.contains("export * from"), "{unresolved}");

        // A resolvable chain collects names, `.d.mts` stems included.
        std::fs::write(
            &entry,
            "export * from \"./inner.mjs\";\nexport * from \"./modern.mts\";\n",
        )
        .unwrap();
        std::fs::write(
            root.join("inner.d.ts"),
            "export declare function first(): void;\n",
        )
        .unwrap();
        std::fs::write(
            root.join("modern.d.mts"),
            "export declare function second(): void;\n",
        )
        .unwrap();
        let mut exports = Exports::default();
        exported_names(&entry, &mut exports, &mut BTreeSet::new()).unwrap();
        assert!(exports.values.contains("first"));
        assert!(exports.values.contains("second"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// The emitted JSON and Rust are built by interpolation, so every parsed
    /// name goes through an escaping serializer.
    #[test]
    fn interpolated_names_are_escaped() {
        assert_eq!(json_string("a\"b"), r#""a\"b""#);
        assert_eq!(format!("{:?}", "a\"b"), r#""a\"b""#);
    }

    #[test]
    fn runtime_type_only_corrections_leave_the_type_index_intact() {
        let mut exports = Exports {
            values: BTreeSet::from(["JSX".into(), "render".into()]),
            types: BTreeSet::from(["Component".into()]),
        };

        correct_runtime_type_only(&mut exports, &["JSX", "RequestEventLocals"]);

        assert_eq!(exports.values, BTreeSet::from(["render".into()]));
        assert_eq!(
            exports.types,
            BTreeSet::from(["Component".into(), "JSX".into()])
        );
    }
}
