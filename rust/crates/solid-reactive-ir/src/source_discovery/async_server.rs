//! Async source options and the server-rendering premise.

use super::*;

/// The statically-proven rc.0 async/hydration options declared on one
/// reactive source. All flags default to `false` when there is no options
/// argument (or an explicit `undefined`/`null`), which keeps that case on the
/// fully-proven path.
///
/// Runtime ground truth, probed against `solid-js@2.0.0-rc.0` /
/// `@solidjs/signals@2.0.0-rc.0` (2026-08-15):
///
/// - A computation created with `loadingValue` (or a store-family source with
///   `seedLoadingValue: true`) is born committed. During its first flight,
///   untracked strict reads return the declared value (no
///   `PENDING_ASYNC_UNTRACKED_READ`), tracked reads serve it without
///   suspending (no `Loading` boundary participation, no
///   `ASYNC_OUTSIDE_LOADING_BOUNDARY`), reads inside `createTrackedEffect`
///   neither warn nor throw, and `isPending` reads `false`.
/// - The declared window ends at the first real answer: with a re-ask in
///   flight (input change or refresh), an untracked strict read throws
///   `PENDING_ASYNC_UNTRACKED_READ` and a `createTrackedEffect` read warns
///   `PENDING_ASYNC_FORBIDDEN_SCOPE` and throws — exactly like an undeclared
///   async node. SC5001/SC5002 therefore stay reported on declared sources,
///   with conditional wording.
/// - A bare `ssrSource: "client"` source (no declaration) never runs its
///   compute on the server; *any* read of it during SSR outside a `Loading`
///   fallback flush throws `ssrSource: "client" read during SSR outside a
///   <Loading> boundary` — including reads of a fully synchronous compute —
///   while under a boundary it suspends finally so the fallback is flushed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub(crate) struct AsyncSourceOptions {
    /// The source provably declares `loadingValue` (presence-keyed at
    /// runtime: `"loadingValue" in options`) or `seedLoadingValue: true` in
    /// an exact object literal.
    pub(crate) declared_loading: bool,
    /// The source provably declares `ssrSource: "client"` with no
    /// `loadingValue`/`seedLoadingValue` declaration, on a function-form
    /// call: the server installs a client hole for it.
    pub(crate) ssr_client_bare: bool,
    /// The source is a proven bare `ssrSource: "client"` source, but the
    /// analyzed project does not contain enough evidence to decide whether a
    /// server-rendering entry exists. Absence of an import is not proof that
    /// the application is CSR-only: the server entry may live in another
    /// tsconfig or package.
    pub(crate) server_rendering_unresolved: bool,
    /// An options argument exists that the analyzer cannot read as an exact
    /// object literal, so option-dependent claims cannot be proven either
    /// way.
    pub(crate) opaque: bool,
}

/// Reads the async/hydration option surface of one source-creating call, as
/// far as the options argument is statically readable. Value claims
/// (`seedLoadingValue: true`, `ssrSource: "client"` with nothing declared)
/// require an exact object literal; the presence claim for `loadingValue`
/// survives spreads because a later spread cannot remove the key from the
/// runtime's `in` check.
pub(crate) fn async_source_options(
    file: &FileFacts,
    call: &solid_facts::ast::CallFact,
    primitive: Option<Primitive>,
    dialect: &dyn Dialect,
) -> AsyncSourceOptions {
    use solid_facts::ast::ArgumentValueKind;
    let Some(index) = primitive.and_then(|primitive| dialect.options_argument(primitive)) else {
        return AsyncSourceOptions::default();
    };
    let Some(argument) = call.arguments.get(index) else {
        return AsyncSourceOptions::default();
    };
    if matches!(
        argument.value,
        ArgumentValueKind::Undefined | ArgumentValueKind::Null
    ) {
        return AsyncSourceOptions::default();
    }
    let named = |span, expected: &str| file.source_text(span) == Some(expected);
    let loading_key = argument
        .property_names
        .iter()
        .any(|key| named(*key, "loadingValue"));
    let seed_key = argument
        .property_names
        .iter()
        .any(|key| named(*key, "seedLoadingValue"));
    let seed_true = argument
        .boolean_properties
        .iter()
        .any(|property| named(property.name, "seedLoadingValue") && property.value);
    let seed_false = argument
        .boolean_properties
        .iter()
        .any(|property| named(property.name, "seedLoadingValue") && !property.value);
    let ssr_client = argument
        .string_properties
        .iter()
        .any(|property| named(property.name, "ssrSource") && property.value == "client");
    let declared_loading = loading_key || (argument.exact_object_literal && seed_true);
    // The server installs the client hole only for function-form sources (a
    // value-form `createSignal(0, …)` never runs a compute, so `ssrSource`
    // is inert there); an unresolvable computation argument fails the proof.
    let function_form = matches!(
        call.arguments.first().map(|argument| argument.value),
        Some(ArgumentValueKind::Function | ArgumentValueKind::AsyncFunction)
    );
    let ssr_client_bare = argument.exact_object_literal
        && ssr_client
        && !loading_key
        && (!seed_key || seed_false)
        && function_form;
    AsyncSourceOptions {
        declared_loading,
        ssr_client_bare,
        server_rendering_unresolved: false,
        opaque: !argument.exact_object_literal && !declared_loading,
    }
}

/// The exports whose import proves the project server-renders (or hydrates
/// server-rendered HTML). A bare `ssrSource: "client"` source is only a
/// runtime error on the server path. Named imports prove that path; their
/// absence leaves the rendering mode unresolved because the server entry may
/// live outside the analyzed project. The export names come from the
/// historical `@solidjs/web` audit; no package document is read during analysis.
///
/// **Not asked of the dialect, and it could not answer.** Five of these six
/// are not primitives — only `hydrate` is in the vocabulary — so
/// `export_modules` returns nothing for them and a seam question keyed on the
/// name would silence the rule rather than generalize it. Which exports prove
/// server rendering is this rule's own subject; which *module* they may come
/// from is the dialect's, and that half is asked below.
pub(super) const SERVER_RENDER_IMPORTS: [&str; 6] = [
    "renderToStream",
    "renderToString",
    "renderToFrameStream",
    "renderServerComponent",
    "handleServerFunctionRequest",
    "hydrate",
];

/// Whether the analyzed application server-renders.
///
/// The three outcomes are not two: a project with no visible server entry is
/// not the same fact as a project the user has explicitly selected a
/// client-only rendering mode for. The first is an absence of evidence -- the
/// server entry may live in another tsconfig or package -- and rules must
/// report a proof obligation. The second is evidence, and a rule whose whole
/// premise is "if this application server-renders" has had that premise
/// disproven and must stay silent rather than report an obligation the user
/// has already discharged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ServerRenderingPremise {
    /// An explicit rendering selector, or a visible server-rendering entry
    /// import, proves the application server-renders.
    Renders,
    /// An explicit rendering selector proves it does not.
    ProvenClientOnly,
    /// No selector, and no server-rendering entry point is visible in the
    /// analyzed project -- which does not prove the application is CSR-only.
    Unresolved,
}

impl ServerRenderingPremise {
    /// The premise from the two inputs that decide it: the explicit rendering
    /// selector when the user set one, and otherwise whether a
    /// server-rendering entry point is visible in the analyzed project.
    ///
    /// A selector answers the question outright in both directions. Only
    /// without one does the import survey matter, and then only one way: a
    /// visible entry proves server rendering, while no visible entry proves
    /// nothing at all.
    pub(crate) const fn select(
        rendering: Option<RuntimeRendering>,
        imports_server_entry: bool,
    ) -> Self {
        match rendering {
            Some(RuntimeRendering::StringSsr | RuntimeRendering::StreamingSsr) => Self::Renders,
            Some(RuntimeRendering::Csr) => Self::ProvenClientOnly,
            None if imports_server_entry => Self::Renders,
            None => Self::Unresolved,
        }
    }

    /// Whether server rendering is proven to happen. `ProvenClientOnly` and
    /// `Unresolved` are both "not proven", and callers that need to tell them
    /// apart must match on the enum instead.
    pub(crate) const fn renders(self) -> bool {
        matches!(self, Self::Renders)
    }
}

/// Whether the analyzed project server-renders: an explicit rendering
/// selector when there is one, otherwise whether any analyzed file imports a
/// server rendering entry point from a module this dialect owns.
pub(crate) fn project_server_rendering(
    facts: &ProjectFacts,
    environment: &RuntimeEnvironment,
    dialect: &dyn solid_dialect::Dialect,
) -> ServerRenderingPremise {
    if let Some(rendering) = environment.rendering {
        return ServerRenderingPremise::select(Some(rendering), false);
    }
    let imports_server_entry = facts.files.iter().any(|file| {
        file.ast.imports.iter().any(|import| {
            // `@solidjs/web` and its subpaths, spelled by the dialect rather
            // than by this module. `modules()` already enumerates them, so a
            // dialect that publishes its render entries from somewhere else
            // says so once instead of being matched against a literal here.
            dialect.owns_module(import.module.as_str())
                && !import.type_only
                && import.bindings.iter().any(|binding| {
                    !binding.type_only
                        && binding
                            .imported
                            .as_deref()
                            .is_some_and(|imported| SERVER_RENDER_IMPORTS.contains(&imported))
                })
        })
    });
    ServerRenderingPremise::select(None, imports_server_entry)
}

#[cfg(test)]
mod server_rendering_premise_tests {
    use super::ServerRenderingPremise;
    use crate::RuntimeRendering;

    /// The three states must stay three. Folding `ProvenClientOnly` into
    /// `Unresolved` is what made an explicitly CSR project report an
    /// uncertifiable result whose own message said the premise could not be
    /// proven, and folding it into `Renders` would invent an SSR violation
    /// for an application that has no server.
    #[test]
    fn an_explicit_rendering_selector_decides_the_premise_in_both_directions() {
        for rendering in [RuntimeRendering::StringSsr, RuntimeRendering::StreamingSsr] {
            for imports in [false, true] {
                let premise = ServerRenderingPremise::select(Some(rendering), imports);
                assert_eq!(premise, ServerRenderingPremise::Renders);
                assert!(premise.renders());
            }
        }
        // The selector outranks the survey in the other direction too: an
        // unused `renderToStream` import in a project the user has declared
        // client-only does not resurrect the server.
        for imports in [false, true] {
            let premise = ServerRenderingPremise::select(Some(RuntimeRendering::Csr), imports);
            assert_eq!(premise, ServerRenderingPremise::ProvenClientOnly);
            assert!(!premise.renders());
        }
    }

    /// Without a selector the survey decides one way only. A visible server
    /// entry proves server rendering; no visible entry is an absence of
    /// evidence, not evidence of absence, because the entry may live in
    /// another tsconfig or package.
    #[test]
    fn without_a_selector_a_visible_entry_proves_and_its_absence_does_not() {
        assert_eq!(
            ServerRenderingPremise::select(None, true),
            ServerRenderingPremise::Renders
        );
        let unresolved = ServerRenderingPremise::select(None, false);
        assert_eq!(unresolved, ServerRenderingPremise::Unresolved);
        assert!(!unresolved.renders());
    }
}
