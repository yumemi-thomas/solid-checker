# dynamic-string-tag-events

**Claim (ADR 0313).** The ADR 0199 event-only proof
(`execution_role::prop_reaches_only_events`) follows a callback prop through
`<Dynamic {...others} component={…}/>` when the `component` that element
renders with is proven a string literal for this rendering chain. A string
`component` makes rc.13's `Dynamic` render that intrinsic element and assign
every other prop with `spread`, whose `assignProp` attaches an un-namespaced
`on…` key as an event listener (`Dialect::intrinsic_tag_renderers`). The
literal then runs only on event dispatch, as one written on the element does.

`polymorphic.tsx` has the shape of `@kobalte/core@2.0.0-alpha.0`'s
`Polymorphic` (`omit(props, "as")` spread onto `Dynamic`, `component={props.as}`)
and `ButtonRoot` (`merge({ type: "button" }, props)`, `omit`, then
`<Polymorphic as="button" … {...others}/>`), and of a project `Button` and
`IconButton` over them, like openbot's. The proof carries, per hop, the keys
the props object can hold and whether each value is a string literal: a
project element's props are its attributes, spreads and children; `omit` by
string-literal keys drops keys; `merge` of views and complete data-property
literals takes later sources' keys; a `const` alias or type assertion keeps
them. Anything else (an unknown spread, a getter or spread in a merged literal,
a non-literal value) leaves the key unproven.

| Case | Finding | Why |
| --- | --- | --- |
| `UsesButton`, `UsesIconButton` | none | `as` stays `"button"` through every hop |
| `UsesButtonAsAnchor` | none | the caller's `as="a"` overrides it, still a string |
| `UsesAnchor` | none | `component="a"` is written after the unknown spread |
| `UsesButtonAsComponent` | `SC1001` uncertifiable | `as={Custom}`: `Dynamic` renders a component |
| `UsesButtonAsComputed` | `SC1001` uncertifiable | `as={tag()}` is not a literal |
| `UsesButtonSpread` | `SC1001` uncertifiable | an unknown spread may carry `as` |
| `UsesGetterRoot` | `SC1001` uncertifiable | a getter in the merged literal |
| `UsesShadowRoot` | `SC1001` uncertifiable | a project function named `Dynamic` is not the dialect's |
| `UsesTooltip` | `SC1001` uncertifiable | not an `on…` prop |
| `UsesCallingRoot` | `SC1001` uncertifiable | the consumer calls the prop while it renders, but it also hands `props` to `omit`; a single-source view may be `props` itself, so the body proof refuses (ADR 0326 review round 2). The read stays a proof obligation, not a proven violation |

The reads in `CallingRoot` and the shadowing `Dynamic` belong to other
rules and do not move.

`solid-js.d.ts` is copied from `forwarded-event-prop` with three changes: the
`i` and `span` elements, `button`'s handler taking a `MouseEvent`, and
`Component` copied from `solid-js@2.0.0-rc.13` `types/client/component.d.ts:6`
(its `SolidElement` is the fixture's `JSX.Element`). `node_modules/@solidjs/web/types/index.d.ts`
copies `Dynamic` and its prop types from `@solidjs/web@2.0.0-rc.13`
`types/index.d.ts` byte for byte, with the fixture's global `JSX`; the proof
depends on `Dynamic` being declared in that package under that name.
`@solidjs/signals` is `forwarded-event-prop`'s stub. The three source files
pass `tsc --noEmit` against the published rc.13 `solid-js`, `@solidjs/web` and
`@solidjs/signals` typings without the fixture stubs.

Not covered here: Kobalte's own published source.
