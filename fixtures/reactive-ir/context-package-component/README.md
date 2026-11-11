# context-package-component

**Claim (ADR 0949, ADR 0943).** A package component a project renders with a JSX tag, whose source reads a context the package created with no default, is an SC4002 **obligation at the element** (uncertifiable, `provider-not-proven-above`, never a violation: the package may provide its own context), not silent. ADR 0290 attributed a package's requirement only at a project *call*; a render is no call (the component's owner context is a seed, ADR 0869's root of its own, not an edge), so the project was certified clean while Solid throws `ContextNotFoundError` (CONTEXT_NOT_FOUND).

`App.tsx`, each exported component renders one:

- Obligation, at the tag: `Direct` (`<Panel>`), `Renamed` (`import { Panel as Aliased }`), `Namespace` (`<ui.Panel>`, a namespace import's member), `ViaLocalAlias` (`const LocalAlias = Panel; <LocalAlias>`, an exact alias, ADR 0331: the alias binding's reference is an obligation too), and `Inner`'s `<Panel>` (rendered by `Nested`: the element is the entry, `<Inner/>` is the project's own component).
- Silent: `Defaulted` (`Quiet` reads a context created with a default), `NoRead` (`Plain` reads no context), `TypeOnly` (`typeof Panel` and `import type` run nothing).

The tag is resolved exactly (the call graph's own resolution, `SemanticLookup::function_call_sites`, and the exact local alias); no name is matched. The walk goes back from the read through every owner edge, each call and tag site, and a nested function's enclosing function, through package functions only; a project element that renders a function it reached is the site. A project function no owner edge reaches that may run (`SiteReach::function_runs`) runs its entries; one proven never to run runs none.

Package: `node_modules/dep-ui-context` is this fixture's own (`dist/index.js` reads `UiContext = createContext()` with no default in `Panel`; `Quiet` reads `createContext(1)`; `Plain` reads none), with a `solid-js` peer dependency, selected as dependency source by `.solid-checker/runtime.json`. `solid-js.d.ts` is the stub of `context-package-no-default` (the published rc.13 typings, abbreviated), with `lazy` and `Dynamic` added for the siblings.

Chrome (rc.21 development build, the runtime oracle; the cases of ADR 0949's matrix are the same shapes with a package of the same form): `<Panel>`, `<Aliased>`, `<ui.Panel>`, `<LocalAlias>` and a nested render each throw ContextNotFoundError; `<Quiet>` and `<Plain>` throw nothing. Measured at 29b3f9312 (the checker certified every positive) and again with ADR 0949.

Not covered: a provider the project renders above the element (`<Shell><Panel/></Shell>` where the package's `Shell` provides the context around its children): not followed into the package, so the element stays an obligation (accepted cost, `docs/open-work.md`).
