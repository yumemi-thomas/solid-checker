//! ADR 0334: the route-components premise.
//!
//! An audited `@tanstack/solid-router` release only renders the `component`,
//! `pendingComponent`, `errorComponent` and `notFoundComponent` values of a
//! route's options (`<Dynamic component={…}/>`, `<X/>`) and never calls one
//! (`Dialect::audited_route_components`). A function written as one of those
//! values of the options literal handed to an exact router factory import is
//! therefore rendered there, which accounts for that reference in the
//! component census (`indexes/components.rs`, ADR 0331).
//!
//! The premise applies to a project only when every runtime import of the
//! router resolves, through the compiler's attested resolution, to an audited
//! release whose installed bytes and whose `@tanstack/router-core` are the
//! audited ones, no manifest records a replacement of either, and no project
//! file reads one of the four keys back (a member `.component`, a
//! `['component']` element, a destructured `component`): read back, the value
//! can be called by the project, which the census does not see.

use std::collections::HashSet;

use solid_dialect::Dialect;
use solid_facts::ast::{FunctionKind, ImportKind};
use solid_facts::core::Span;
use solid_facts::{FileFacts, ProjectFacts, SpecifierAttestation};

use crate::router_params::{ROUTER_PACKAGE, RouterInstallEvidence};

/// The specifier of the audited package.
pub const ROUTE_PACKAGE: &str = "@tanstack/solid-router";

/// The package the router stores and loads route options with.
pub const ROUTE_CORE_PACKAGE: &str = "@tanstack/router-core";

/// The compiler's references to the binding named at a span, as
/// `(path, span)`; `None` when it knows no symbol there.
pub type BindingReferences<'a> = dyn Fn(&FileFacts, Span) -> Option<Vec<(String, Span)>> + 'a;

/// The route option keys whose values the audited releases only render.
const COMPONENT_KEYS: [&str; 4] = [
    "component",
    "pendingComponent",
    "errorComponent",
    "notFoundComponent",
];

/// Factories called with the options directly: `createRoute(options)`.
const DIRECT_FACTORIES: [&str; 2] = ["createRoute", "createRootRoute"];

/// Factories whose call returns the function taking the options:
/// `createFileRoute(path)(options)`.
const CURRIED_FACTORIES: [&str; 4] = [
    "createFileRoute",
    "createLazyFileRoute",
    "createLazyRoute",
    "createRootRouteWithContext",
];

/// What the backend read of one resolved `@tanstack/solid-router` install.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RouteInstall {
    /// Digest of the router's installed bytes (`scripts/router-digest.mjs`).
    pub router_digest: Option<String>,
    /// The directory of the router-core install, as the analysis paths
    /// spell it: its files are audited code, not project reads.
    pub core_directory: Option<String>,
    /// The `@tanstack/router-core` Node resolves from the router's real
    /// directory: its version and the digest of its bytes.
    pub core: Option<(String, String)>,
}

/// The option values the premises account for, as `(file path, value span)`:
/// TanStack route options, and `@solidjs/router` route definitions
/// ([`router_config_values`]). Empty where neither applies.
///
/// `references` answers the compiler's references to the binding named at a
/// span, as `(path, span)`, or `None` when it knows no symbol there;
/// `namespace_exposed` the modules reached as a namespace (`None`: unknown);
/// `prototype_written` whether an analyzed file writes through a `prototype`
/// reflectively (ADR 0292's veto).
#[must_use]
pub fn route_component_values(
    facts: &ProjectFacts,
    dialect: &dyn Dialect,
    install: &RouterInstallEvidence,
    references: &BindingReferences<'_>,
    namespace_exposed: Option<&HashSet<String>>,
    prototype_written: bool,
) -> HashSet<(String, Span)> {
    let mut values = tanstack_values(facts, dialect, install);
    if !prototype_written {
        values.extend(router_config_values(
            facts,
            dialect,
            install,
            references,
            namespace_exposed,
        ));
    }
    values
}

fn tanstack_values(
    facts: &ProjectFacts,
    dialect: &dyn Dialect,
    install: &RouterInstallEvidence,
) -> HashSet<(String, Span)> {
    let mut values = HashSet::new();
    let Some(audited) = audited_directories(facts, dialect, install) else {
        return values;
    };
    for file in unaudited_files(facts, &audited) {
        let factories = factory_locals(file);
        if factories.direct.is_empty() && factories.curried.is_empty() {
            continue;
        }
        let ast = &*file.ast;
        let resolves_to = |reference: Span, locals: &[Span]| {
            ast.reference_declarations
                .binary_search_by_key(&reference, |(span, _)| *span)
                .ok()
                .is_some_and(|at| locals.contains(&ast.reference_declarations[at].1))
        };
        let factory_call = |call: &solid_facts::ast::CallFact, locals: &[Span]| {
            call.direct_callee && !call.construct && resolves_to(call.callee, locals)
        };
        for call in &ast.calls {
            let direct = factory_call(call, &factories.direct);
            let curried = !call.construct
                && ast.calls.iter().any(|inner| {
                    inner.span == ast.peel_ts_sugar_span(call.callee)
                        && factory_call(inner, &factories.curried)
                });
            if !direct && !curried {
                continue;
            }
            let [options] = call.arguments.as_slice() else {
                continue;
            };
            if options.spread || !options.exact_object_literal {
                continue;
            }
            for member in &options.literal_members {
                if COMPONENT_KEYS.contains(&member.key.as_str()) {
                    values.insert((file.path.to_string(), member.value));
                    values.insert((file.path.to_string(), ast.peel_ts_sugar_span(member.value)));
                }
            }
        }
    }
    values
}

/// The directories of the audited installs, when the premise applies to
/// `facts`: every runtime import of the router reaches audited bytes, nothing
/// records a replacement, and no other analyzed file (the project's own or
/// another package's) reads a component key back.
fn audited_directories(
    facts: &ProjectFacts,
    dialect: &dyn Dialect,
    install: &RouterInstallEvidence,
) -> Option<Vec<String>> {
    if install.route_replacement_recorded {
        return None;
    }
    let mut directories = Vec::new();
    for file in &facts.files {
        for import in &file.ast.imports {
            if import.module.as_str() != ROUTE_PACKAGE
                || import.type_only
                || import.bindings.iter().all(|binding| binding.type_only)
            {
                continue;
            }
            directories.extend(reaches_audited_bytes(
                facts, file, import, dialect, install,
            )?);
        }
    }
    directories.sort();
    directories.dedup();
    (!directories.is_empty() && !unaudited_files(facts, &directories).any(reads_component_key))
        .then_some(directories)
}

fn reaches_audited_bytes(
    facts: &ProjectFacts,
    file: &FileFacts,
    import: &solid_facts::ast::ImportFact,
    dialect: &dyn Dialect,
    install: &RouterInstallEvidence,
) -> Option<[String; 2]> {
    let imports = facts.resolved_imports.as_ref()?;
    let SpecifierAttestation::Attested(row) =
        imports.specifier(file.path.as_str(), import.span, ROUTE_PACKAGE)
    else {
        return None;
    };
    // One package by both the nearest manifest and the resolver's own record,
    // loaded from `node_modules` unpatched, as the bundler loads it.
    if row.resolution != solid_facts::ImportResolution::NodeModules
        || row.resolved_path.contains("patch_hash=")
        || row.package_name.as_deref() != Some(ROUTE_PACKAGE)
        || row.resolver_package_name.as_deref() != Some(ROUTE_PACKAGE)
        || row
            .resolver_package_version
            .as_deref()
            .is_some_and(|version| row.package_version.as_deref() != Some(version))
        || !crate::router_params::runtime_agrees(
            facts,
            file,
            import,
            &row.resolved_path,
            ROUTE_PACKAGE,
        )
    {
        return None;
    }
    let audited = dialect.audited_route_components(row.package_version.as_deref()?)?;
    let manifest = row.package_manifest.as_deref()?;
    let installed = install.route_installs.get(manifest)?;
    let core_matches = installed.core.as_ref().is_some_and(|(version, digest)| {
        version == audited.core_version && digest == audited.core_digest
    });
    if installed.router_digest.as_deref() != Some(audited.router_digest) || !core_matches {
        return None;
    }
    let router = manifest.strip_suffix("package.json")?.to_owned();
    let core = format!(
        "{}/",
        installed.core_directory.as_deref()?.trim_end_matches('/')
    );
    Some([router, core])
}

/// Every analyzed file outside the audited installs: the project's own and
/// any other package's.
fn unaudited_files<'a>(
    facts: &'a ProjectFacts,
    audited: &'a [String],
) -> impl Iterator<Item = &'a FileFacts> {
    facts.files.iter().filter(move |file| {
        !audited
            .iter()
            .any(|directory| file.path.as_str().starts_with(directory.as_str()))
    })
}

/// Whether `file` reads one of the component keys off some object: a member
/// `.component`, an element `['component']`, or a destructured `component`
/// in any binding or assignment pattern, nested or defaulted. Keys compare
/// decoded; a member name spelled with an escape counts as a read.
fn reads_component_key(file: &FileFacts) -> bool {
    let ast = &*file.ast;
    let is_key = |text: &str| COMPONENT_KEYS.contains(&text);
    ast.members.iter().any(|member| {
        file.source
            .get(member.property.start as usize..member.property.end as usize)
            .is_none_or(|text| text.contains('\\') || is_key(text))
    }) || ast
        .literal_computed_members
        .iter()
        .any(|member| is_key(member.key.as_str()))
        || ast
            .bindings
            .iter()
            .chain(ast.functions.iter().flat_map(|function| &function.parameters))
            .flat_map(|binding| &binding.object_slots)
            .any(|slot| is_key(slot.property.as_str()))
        || ast.pattern_elements.iter().any(|element| {
            matches!(&element.slot, solid_facts::ast::PatternSlot::Key(key) if is_key(key.as_str()))
        })
}

/// The local names a file gives the router's factories.
struct FactoryLocals {
    direct: Vec<Span>,
    curried: Vec<Span>,
}

fn factory_locals(file: &FileFacts) -> FactoryLocals {
    let mut locals = FactoryLocals {
        direct: Vec::new(),
        curried: Vec::new(),
    };
    for import in &file.ast.imports {
        if import.module.as_str() != ROUTE_PACKAGE || import.type_only {
            continue;
        }
        for binding in import.bindings.iter().filter(|binding| !binding.type_only) {
            if binding.kind != ImportKind::Named {
                continue;
            }
            match binding.imported.as_deref() {
                Some(name) if DIRECT_FACTORIES.contains(&name) => {
                    locals.direct.push(binding.local.span);
                }
                Some(name) if CURRIED_FACTORIES.contains(&name) => {
                    locals.curried.push(binding.local.span);
                }
                _ => {}
            }
        }
    }
    locals
}

/// ADR 0335: the `component` values of the route definitions written in the
/// `routes` of a `createRouter({ routes: [...] })` call, an exact named import
/// of an audited `@solidjs/router` release, recursively through each
/// definition's `children` array. The audited releases render a definition's
/// `component` with `createComponent` and never call it
/// (`Dialect::audited_route_config_digest`). Only literals count: a definition
/// that is not an object literal written in the array, or an array that is
/// not written at its key, is not followed.
fn router_config_values(
    facts: &ProjectFacts,
    dialect: &dyn Dialect,
    install: &RouterInstallEvidence,
    references: &BindingReferences<'_>,
    namespace_exposed: Option<&HashSet<String>>,
) -> HashSet<(String, Span)> {
    let mut values = HashSet::new();
    let Some(audited) = router_directories(facts, dialect, install, references, namespace_exposed)
    else {
        return values;
    };
    for file in unaudited_files(facts, &audited) {
        let locals = router_creation_locals(file);
        if locals.is_empty() {
            continue;
        }
        let ast = &*file.ast;
        for call in &ast.calls {
            let creates = call.direct_callee
                && !call.construct
                && ast
                    .reference_declarations
                    .binary_search_by_key(&call.callee, |(span, _)| *span)
                    .ok()
                    .is_some_and(|at| locals.contains(&ast.reference_declarations[at].1));
            let [config] = call.arguments.as_slice() else {
                continue;
            };
            if !creates || config.spread || !config.exact_object_literal {
                continue;
            }
            // The router calls `config.transformUrl(...)` on the config
            // (`routers/factory.jsx:188`), so the config holds no callable
            // that could bind `this` to it and its `routes`.
            let routes_literal = |property: &solid_facts::ast::ObjectPropertyFact| {
                ast.array_literal_elements
                    .iter()
                    .any(|array| array.span == ast.peel_ts_sugar_span(property.value))
            };
            let Some(own) = this_free_properties(
                file,
                ast.peel_ts_sugar_span(config.span),
                &|key, property| (key == "routes").then(|| routes_literal(property)),
            ) else {
                continue;
            };
            for property in own {
                if file.source_text(property.key) == Some("routes") {
                    definitions(file, ast.peel_ts_sugar_span(property.value), 0, &mut values);
                }
            }
        }
    }
    values
}

/// Collect the `component` values of the object-literal definitions written
/// in the array literal at `array`, and of their `children` arrays. A
/// definition counts only when it is itself a closed object literal (its span
/// is a literal's: no spread, computed key, duplicate, setter or prototype
/// member), so its `component` data property is the final value the router
/// reads; with static keys only, a property inside another property's value
/// is that value's, not the definition's.
///
/// The router reads a definition's keys and calls `route.preload(...)` on its
/// route record (`routers/components.jsx:133`, `server.js:150`), so a getter,
/// a method or a `this`-binding function there receives the definition or a
/// record holding it. A definition counts, with its subtree, only when it has
/// no getter or method and every key but `component` holds a non-callable
/// literal or an arrow function (`children` an array literal or an arrow).
fn definitions(file: &FileFacts, array: Span, depth: u32, values: &mut HashSet<(String, Span)>) {
    let ast = &*file.ast;
    let Some(literal) = ast
        .array_literal_elements
        .iter()
        .find(|literal| literal.span == array)
    else {
        return;
    };
    if depth > 32 {
        return;
    }
    let peeled = |span: Span| ast.peel_ts_sugar_span(span);
    for element in &literal.elements {
        if element.kind != solid_facts::ast::ArrayElementKind::Value {
            continue;
        }
        let Some(own) =
            this_free_properties(file, peeled(element.span), &|key, property| match key {
                "component" => Some(true),
                "children" => Some(
                    arrow(file, property.value)
                        || ast
                            .array_literal_elements
                            .iter()
                            .any(|array| array.span == peeled(property.value)),
                ),
                _ => None,
            })
        else {
            continue;
        };
        for property in own {
            match file.source_text(property.key) {
                Some("component") => {
                    values.insert((file.path.to_string(), property.value));
                    values.insert((file.path.to_string(), peeled(property.value)));
                }
                Some("children") => {
                    definitions(file, peeled(property.value), depth + 1, values);
                }
                _ => {}
            }
        }
    }
}

/// Whether `span` is an arrow function: it has no `this` of its own.
fn arrow(file: &FileFacts, span: Span) -> bool {
    let span = file.ast.peel_ts_sugar_span(span);
    file.ast
        .functions
        .iter()
        .any(|function| function.span == span && function.kind == FunctionKind::Arrow)
}

/// The own properties of the object literal at `object`, when nothing in it
/// can bind `this` to it: the literal is closed (no spread, computed key,
/// duplicate, setter or prototype member; with static keys only, a property
/// inside another property's value is that value's), has no getter, every
/// property is a data property and not a method, and every value is a
/// non-callable literal or an arrow function, except where `special`
/// answers for a key. `None` otherwise.
fn this_free_properties<'f>(
    file: &'f FileFacts,
    object: Span,
    special: &dyn Fn(&str, &solid_facts::ast::ObjectPropertyFact) -> Option<bool>,
) -> Option<Vec<&'f solid_facts::ast::ObjectPropertyFact>> {
    let ast = &*file.ast;
    let shape = ast
        .object_get_shapes
        .iter()
        .find(|shape| shape.span == object && shape.closed)?;
    if shape
        .properties
        .iter()
        .any(|property| property.getter.is_some())
    {
        return None;
    }
    let inside = |span: Span| object.contains(span) && span != object;
    let own = ast
        .object_properties
        .iter()
        .filter(|property| inside(property.span))
        .filter(|property| {
            !ast.object_properties
                .iter()
                .any(|outer| inside(outer.span) && outer.value.contains(property.span))
        })
        .collect::<Vec<_>>();
    let safe = own.len() == shape.properties.len()
        && own.iter().all(|property| {
            property.data
                && !property.method
                && !property.computed
                && file.source_text(property.key).is_some_and(|key| {
                    special(key, property).unwrap_or_else(|| {
                        property.value_kind.is_proven_noncallable() || arrow(file, property.value)
                    })
                })
        });
    safe.then_some(own)
}

/// The directory of every audited `@solidjs/router` install, when the
/// premise applies: every runtime import of the router reaches an audited
/// release's bytes, nothing records a replacement, and no analyzed file
/// outside those installs can obtain a route definition
/// ([`definitions_exposed`]).
fn router_directories(
    facts: &ProjectFacts,
    dialect: &dyn Dialect,
    install: &RouterInstallEvidence,
    references: &BindingReferences<'_>,
    namespace_exposed: Option<&HashSet<String>>,
) -> Option<Vec<String>> {
    if install.replacement_recorded {
        return None;
    }
    let imports = facts.resolved_imports.as_ref()?;
    let mut directories = Vec::new();
    for file in &facts.files {
        for import in &file.ast.imports {
            if import.module.as_str() != ROUTER_PACKAGE
                || import.type_only
                || import.bindings.iter().all(|binding| binding.type_only)
            {
                continue;
            }
            let SpecifierAttestation::Attested(row) =
                imports.specifier(file.path.as_str(), import.span, ROUTER_PACKAGE)
            else {
                return None;
            };
            if row.resolution != solid_facts::ImportResolution::NodeModules
                || row.resolved_path.contains("patch_hash=")
                || row.package_name.as_deref() != Some(ROUTER_PACKAGE)
                || row.resolver_package_name.as_deref() != Some(ROUTER_PACKAGE)
                || row
                    .resolver_package_version
                    .as_deref()
                    .is_some_and(|version| row.package_version.as_deref() != Some(version))
                || !crate::router_params::runtime_agrees(
                    facts,
                    file,
                    import,
                    &row.resolved_path,
                    ROUTER_PACKAGE,
                )
            {
                return None;
            }
            let audited = dialect.audited_route_config_digest(row.package_version.as_deref()?)?;
            let manifest = row.package_manifest.as_deref()?;
            if install.digests.get(manifest).map(String::as_str) != Some(audited) {
                return None;
            }
            directories.push(manifest.strip_suffix("package.json")?.to_owned());
        }
    }
    directories.sort();
    directories.dedup();
    (!directories.is_empty()
        && !definitions_exposed(facts, &directories, references, namespace_exposed?))
    .then_some(directories)
}

/// The router's exports that hand project code route records or
/// definitions in the audited releases: `useRouteMatches()` returns matches
/// whose `route` holds `key` (the definition) and `component`, and
/// `useContext(RouterContext)` holds the same matches. Route props
/// (`params`, `location`, `data`, `children`), `preload` arguments and every
/// other hook hold none.
const EXPOSING_EXPORTS: [&str; 2] = ["useRouteMatches", "RouterContext"];

/// Whether an analyzed file outside the audited installs can obtain a route
/// definition, so a `component` read off some value could be one. A
/// definition written in `createRouter`'s literal is held only by the
/// router, which exposes it through [`EXPOSING_EXPORTS`] and the instance's
/// `routes` and `config`. So the router may be imported only by name, never
/// those exports, never as a namespace, default, deep path, dynamic load or
/// re-export; and every `createRouter` call must be the initializer of a
/// `const` whose every reference is its declaration, its module surface or a
/// JSX tag (`<Router/>`), in a module no file reaches as a namespace
/// (`namespace_exposed`; `Routes["Router"]` names no reference). Any
/// non-literal load or module hazard already withheld the premise.
fn definitions_exposed(
    facts: &ProjectFacts,
    audited: &[String],
    references: &BindingReferences<'_>,
    namespace_exposed: &HashSet<String>,
) -> bool {
    let names_router = |module: &str| {
        module == ROUTER_PACKAGE || module.starts_with(concat!("@solidjs/router", "/"))
    };
    for file in unaudited_files(facts, audited) {
        let ast = &*file.ast;
        // Vite's `import.meta.glob` loads modules no load fact names.
        if file.source.contains("import.meta.glob") {
            return true;
        }
        // The config and definitions are checked for own callables only; an
        // inherited one (`Object.prototype.transformUrl = …`) binds `this` to
        // them as well. Any prototype spelling withholds the premise, beside
        // the reflective writes `prototype_written` covers.
        if ast.members.iter().any(|member| {
            matches!(
                file.source_text(member.property),
                Some("prototype" | "__proto__" | "setPrototypeOf")
            )
        }) || ast.object_properties.iter().any(|property| {
            file.source_text(property.key)
                .is_some_and(|key| key.trim_matches(['"', '\'']) == "__proto__")
        }) {
            return true;
        }
        if ast
            .exports
            .iter()
            .any(|export| !export.type_only && export.module.as_deref().is_some_and(names_router))
            || ast
                .module_loads
                .iter()
                .any(|load| load.specifier.as_deref().is_none_or(names_router))
            || ast
                .import_equals
                .iter()
                .any(|import| !import.type_only && names_router(import.module.as_str()))
        {
            return true;
        }
        let mut creators = Vec::new();
        for import in &ast.imports {
            if import.type_only || !names_router(import.module.as_str()) {
                continue;
            }
            for binding in import.bindings.iter().filter(|binding| !binding.type_only) {
                if import.module.as_str() != ROUTER_PACKAGE || binding.kind != ImportKind::Named {
                    return true;
                }
                match binding.imported.as_deref() {
                    Some(name) if EXPOSING_EXPORTS.contains(&name) => return true,
                    Some("createRouter") => creators.push(binding.local.span),
                    _ => {}
                }
            }
        }
        if !creators.is_empty() && namespace_exposed.contains(file.path.as_str()) {
            return true;
        }
        for creator in creators {
            for reference in ast
                .reference_declarations
                .iter()
                .filter(|(_, declaration)| *declaration == creator)
                .map(|(reference, _)| *reference)
            {
                let Some(call) = ast
                    .calls
                    .iter()
                    .find(|call| call.callee == reference && call.direct_callee && !call.construct)
                else {
                    return true;
                };
                let Some(name) = ast
                    .bindings
                    .iter()
                    .find(|binding| {
                        binding.immutable
                            && binding.shape == solid_facts::ast::BindingShape::Identifier
                            && binding.call_initializer == Some(call.span)
                    })
                    .and_then(|binding| binding.names.first())
                else {
                    return true;
                };
                let Some(uses) = references(file, name.span) else {
                    return true;
                };
                if !uses.iter().all(|(path, span)| {
                    facts
                        .files
                        .iter()
                        .find(|candidate| candidate.path.as_str() == path)
                        .is_some_and(|candidate| rendered_or_surface(candidate, *span))
                }) {
                    return true;
                }
            }
        }
    }
    false
}

/// Whether `span` in `file` is a binding's declaration or module surface, or
/// the name of a JSX tag (`<Router>` or its closing tag; not `<a.Router>`).
fn rendered_or_surface(file: &FileFacts, span: Span) -> bool {
    crate::indexes::reference_is_declaration_or_module_surface(file, span)
        || file.ast.jsx_elements.iter().any(|element| {
            element.member_object.is_none()
                && (element.name.span == span || element.closing_name == Some(span))
        })
}

/// The local names a file gives the router's `createRouter`.
fn router_creation_locals(file: &FileFacts) -> Vec<Span> {
    file.ast
        .imports
        .iter()
        .filter(|import| import.module.as_str() == ROUTER_PACKAGE && !import.type_only)
        .flat_map(|import| &import.bindings)
        .filter(|binding| {
            !binding.type_only
                && binding.kind == ImportKind::Named
                && binding.imported.as_deref() == Some("createRouter")
        })
        .map(|binding| binding.local.span)
        .collect()
}
