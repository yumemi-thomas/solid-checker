# lazy-entry-owner

**Claim (ADR 0992).** `lazy(loader)` renders the export it names under an owner: `wrap(props)` returns `createMemo(() => (Comp = (comp || local)()) ? createComponent(Comp, props) : "", { sync: true })`, with `comp` the loaded `mod.default` (or `mod[options.export]`) (`solid-js@2.0.0-rc.21` `dist/solid.dev.js:1409-1446`). The Solid 2 dialect says so (`Dialect::component_entry("lazy", LoadedExport)`: `RenderedUnderOwner`, read on the rc.21 triple only). The owner emission (ADR 0970) therefore takes such a render for a component's entry, as a tag's, and an owner operation or a render effect in the component keeps no SC4001 `owner-callers-unenumerated` row for it. The rule is the namespace census's own reading of the load, narrowed (`NamespaceHandOuts::named_only_by_lazy_render`): the loader is exactly `() => import("…")` of one project module (the facts' `Lazy` consumer), `lazy` is Solid's by its import binding, the export is `default` or `options.export` and a function nothing rebinds (a function declaration, or a `const`), and no other read names it. The package half (redirecting `import("pkg")`, the `-pk-lazy` placement) is the proof batch's.

| component | outcome | why |
|---|---|---|
| `Page` (`export default`), `lazy(() => import("./Page"))` | silent | the render is its one entry |
| `Named`, `lazy(() => import("./Named"), { export: "Named" })` | silent | the options name the export |
| `Picked`: `const loader = pick(); lazy(loader)` | SC4001 (and SC3001) uncertifiable | the loader is no literal import: the module's namespace escapes with no name |
| `ThenShape`: `lazy(() => import("./ThenShape").then((m) => ({ default: m.ThenShape })))` | the same | the shape the corpus writes (viviana-ui-web, solid-groove), not the literal one; docs/open-work.md records it |
| `Aliased.tsx`: `import { lazy as load }` and `S.lazy(...)` | the same | the facts name a `lazy` read only for the identifier `lazy`; widening it is a facts-schema change (docs/open-work.md) |
| `Mine`: a local function named `lazy` | the same | the import binding decides, not the name |
| `Reassigned`: `export let Late = …; Late = …` | the same | the module's export is live and `lazy` reads it once the promise settles |
| `Also`: loaded by `lazy` and read through a namespace as a value | the same | another read names it |

`node_modules/solid-js/index.d.ts` carries `lazy` and `Component` byte-faithful from `solid-js@2.0.0-rc.21` `types/client/component.d.ts:6, 118-129`. The program is closed (`.solid-checker/runtime.json`). The SC1002 at the `.then` handler and the SC5003 rows at the `lazy` tags are other claims and unchanged.
