# dynamic-string-tag-events

**Claim (ADR 0313).** The ADR 0199 event-only proof follows a callback prop through `<Dynamic {...others} component={...}/>` when `component` is proven a string literal for the chain (rc.13's `Dynamic` renders that intrinsic and `assignProp` attaches `on...` keys as listeners); component/computed `as`, unknown spreads, getters, a project `Dynamic`, non-`on...` props and consumers calling the prop while handing `props` to `omit` are uncertifiable. `Dynamic` is copied byte for byte from `@solidjs/web@2.0.0-rc.13` `types/index.d.ts`.
