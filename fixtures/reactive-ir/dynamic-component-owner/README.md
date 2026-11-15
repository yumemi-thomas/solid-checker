# dynamic-component-owner

**Claim (ADR 0992).** `Dynamic(props)` is `createComponent(dynamic(() => props.component), omit(props, "component"))`, and `dynamicCore`'s component calls the function it gets in `createMemo(() => { … untrack(() => component(props)) })` (`solid-js@2.0.0-rc.21` `dist/solid.dev.js:1544`, `@solidjs/web` `dist/web.dev.js:2451`): the dialect says a function that is the exact value of `Dynamic`'s `component` is rendered under an owner (`Dialect::component_entry("Dynamic", Prop("component"))`, rc.21 triple only). So the owner emission takes `<Dynamic component={X} />` for a render of `X`, and the package hand-off (ADR 0976's `unread-package-may-run`) does not take that value for one handed to a package nothing here reads: the dialect's component runs it in a memo of its own. `dynamic()` itself stays unanswered: its static form calls the component with no computation around it (`staticDynamic`, `:1570`).

| component | outcome | why |
|---|---|---|
| `Chosen`, `<Dynamic component={Chosen} />` | silent | the render is its one entry |
| `Spaced`, `<Web.Dynamic component={Spaced} />` | SC4001 uncertifiable | a dotted tag is not resolved by the value-flow index (ADR 0138): open |
| `Picked`, `component={pick()}` with `pick = createMemo(() => Picked)` | SC3001 uncertifiable | the owner emission closes the layer (the flow is vouched, ADR 0138), the package hand-off follows the value out of the memo's arrow and keeps its row |
| `Kept`: also stored and called from an effect's apply | SC4001 and SC3001 uncertifiable | the stored reference is an entry no render shows |
| `Loose`: handed to a project component's `component` prop | the rows it had | a name proves nothing: that `component` is only a prop |
| `<Dynamic component="p" />` | no function enters | a string source takes the tag arm |

`node_modules/@solidjs/web/index.d.ts` carries `ValidComponent`, `ComponentProps`, `DynamicProps` and `Dynamic` byte-faithful from `@solidjs/web@2.0.0-rc.21` `types/index.d.ts:46-54, 177`. The program is closed (`.solid-checker/runtime.json`).
