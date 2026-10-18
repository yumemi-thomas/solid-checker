# ADR 0313: A string tag renders an intrinsic element

- Status: accepted and implemented (2026-10-17).
- Owners: `Dialect::intrinsic_tag_renderers` (solid-dialect), `intrinsic_tag_prop`
  (`solid-reactive-ir/src/indexes.rs`), the props-shape walk in
  `execution_role::prop_reaches_only_events` (ADR 0199, 0218), and
  `unredirected_packages` (`solid-facts-backend/src/dependency_sources.rs`).
- Relation: extends ADR 0218's event-only proof through one more hop;
  narrows ADR 0297's "namespace import keeps its package on contracts", which
  ADR 0312 made moot.

## Context

A callback prop handed to a wrapper component (`<Button onClick={() => … count() …}>`)
is read untracked in the component's render window only if something calls it
there. ADR 0218 proves it is not, when every use of the prop is an event
attribute of an intrinsic element or a spread/prop of a project component that
satisfies the same proof. Wrapper libraries break the chain at one hop:
Kobalte's `Polymorphic` renders `<Dynamic {...others} component={props.as}/>`,
and `Dynamic` is not a project function. On the openbot corpus 48 findings
(Button, IconButton, …) were uncertifiable for that reason alone.

Separately, one `import * as X from "@kobalte/core/switch"` kept all of
`@kobalte/core` off source analysis (ADR 0297 retained a package reached through
a namespace import so the namespace kept its contract). Contracts are gone, so
the retention only cost every named import of the package its source.

## Decision

1. **`Dynamic` with a string `component` is an intrinsic element.** The dialect
   states, per release family, which export renders the element a string prop
   names and assigns every other prop as a spread onto that element does
   (`intrinsic_tag_renderers`). Solid 2: `Dynamic` / `component`, read on
   `@solidjs/web@2.0.0-rc.13` (dev, prod and observe builds); earlier releases
   take the audited answer under their SC9014 notice. An un-namespaced `on…` key
   becomes an event listener and is never called by the render.
2. **Only when the string is proven.** The engine asks only of a component whose
   every declaration sits in a dialect package under that export name, and only
   when the `component` value at that element is a string literal for this
   rendering chain. The proof carries, per hop, the keys a props object can hold
   and whether each value is a string literal: a project element's props are its
   attributes, spreads of tracked views and children; `omit` by string-literal
   keys drops keys; `merge` of views and complete data-property literals takes
   the later source's keys; a `const` alias or type assertion keeps them. An
   unknown spread, a getter or spread in a merged literal, a computed key, a
   namespaced attribute, a non-literal value or an omit predicate leaves the key
   unproven, and the finding stays uncertifiable. A later literal attribute
   settles the key over an earlier unknown spread; an unknown spread after it
   opens it again.
3. **A namespace import no longer retains its package.** The import itself keeps
   its own fail-closed boundary obligation (`NamespaceValue`, ADR 0290 round 1),
   and its members are not redirected, so nothing proves a call through it.

## Consequences

- Corpus (49 projects, browser/development/csr): uncertifiable 2,763 → 2,713, all
  removals SC1001 (42), SC9005 (5), SC9012 (3); violations unchanged at 553;
  no finding added. openbot 48, jibe-run 2.
- Still uncertifiable: Kobalte's own callbacks (`onSelect`, `onChange` run inside
  its handler closures), namespace-imported components, a `Button` that
  destructures `...others`, a `const Button = ButtonRoot` alias, any `as` that is
  not a literal.
- Known and unchanged: a namespace import written inside selected package source
  yields no finding at a project call that reaches it (before and after this
  ADR); fixing that needs namespace-member redirects.

## Evidence

Fixture `dynamic-string-tag-events` (clean chains, six refusals, one violation;
source passes `tsc --noEmit` against the published rc.13 typings); coverage of
298 projects with only the new snapshot added; `solid-dialect`, `solid-reactive-ir`
and `solid-facts-backend` tests; workspace clippy with `-D warnings`.
