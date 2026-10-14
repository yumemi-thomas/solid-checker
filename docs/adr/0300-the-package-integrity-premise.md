# ADR 0300: The package integrity premise

- Status: accepted owner decision; implemented (2026-10-14).
- Owners: `accessor_provenance::Resolver`, the dependency getter read join in
  `interproc`, and diagnostic snapshot premises.
- Extends ADRs 0273, 0284, 0289, 0291 and 0297.

## Decision

"Package integrity premise", for dependency-source mode (ADRs 0284, 0289, 0291, 0297): code outside the analyzed program — packages that are not selected for source analysis, and anything else the checker cannot see — neither provides a selected package's exported contexts nor rewrites (assigns, `defineProperty`, `Object.assign`, deletes, prototype-patches) objects a selected package creates. This is exactly what an accepted package contract already assumes, so it claims nothing contract mode does not. Everything the checker CAN see still vetoes: project code and selected package code that provides the context or writes the object (ADR 0273 member stability veto, ADR 0284 decision 8 scoping, ADR 0289's re-provided case) must still refuse.

Every visible provider of an exact, default-less selected-package context must
supply a proven getter value. A visible spread or copy is judged as that value;
it is never substituted with the package's original value. Unknown providers,
open literals, mutable bindings and missing exact identities refuse proof.

An exact source helper may receive the context only when every parameter use
is a useContext read, directly or through another such current helper. A
provider, write, alias or opaque handoff refuses. Exact exports/re-exports are
publication, not provision. A factory value is proved when its expression
runs; a preceding guard does not establish a different successful value.

Named member and descriptor writes remain global vetoes. Project wildcard
writes are considered for every object; package wildcards retain ADR 0284
decision 8's package scope. Within either scope, an exact target proof may discharge a
wildcard only when every local identity input comes from other fresh literal
allocation sites. `Object.assign` forwards its exact target; unknown targets,
parameter targets, other calls, exported bindings, eval and unsupported writes
refuse. A dynamic-key write/delete whose receiver derives to the object also
vetoes, including through aliases.

A getter proof requires a closed own-property shape and an exact runtime
accessor call at getter entry. It supplies reactive origin only. The existing
execution analysis decides whether the project Get is untracked and whether
that execution is proven. ADR 0289's may-derivation remains SC9012 when any
proof requirement refuses. Project-created getter objects and helper-only
getter reads retain their earlier limits.

Reports state this premise beside standard runtime configuration, in JSON's
`premises` and every text format. An absent visible veto establishes neither
premise. A runtime that uses an excluded provider or write violates the premise.

## Evidence

The dependency-source integrity fixtures cover multiple visible package
providers, unrelated wildcard targets, untracked and tracked Gets, a project
spread provider, project descriptor replacement, selected-package writes,
dynamic-key alias deletion, and visible versus unseen non-selected providers.
The original re-provided regression selects the wrapper so its inert provider
remains visible. The original unknown-target wildcard fixture stays SC9012.

TypeScript 5.9.3 accepts the read, spread-provider and descriptor-replacement
cases against published `solid-js@2.0.0-rc.13`, `@solidjs/web@2.0.0-rc.13` and
`@solidjs/router@2.0.0-next.26` declarations. No type diagnostic is duplicated.

The 49-project rc.13 browser sweep moves 562/3,248 violations/uncertifiable to
565/3,245, with no errors or removed violation. Two unique sites are added:
kui TopicsRoute.tsx:425:75 (in two projects) and donegeon WaitlistRoute.tsx:33:108.
Both pass TypeScript with real typings. Both directly evaluate a package
search getter over a memo during untracked setup, before passing its string
to a signal initializer. Intentional seeds need explicit untrack under the
same strict-read model; this proof establishes that untracked runtime read.

Hand review used kui router 2.0.0-next.26's routing.js:514-516 and
routers/factory.jsx:227, and donegeon router 2.0.0-next.16's
routing.js:461-463 and routers/factory.jsx:124. Three other unique sites
remain uncertifiable about execution (SC9012 becomes SC1001 uncertifiable).
