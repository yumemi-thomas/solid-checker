# A history that a dependency builds (ADR 0324 refusal)

`router.ts` passes `fancyHistory()` (`node_modules/fancy-history`, returning `{ ...memoryHistory(), utils: { ...h.utils, paramsWrapper } }`). The premise needs every router creation's history to be a stock history called directly, so it is not applied and the site `App.tsx` settles under a stock history stays open (`SC9012`). Router is the published `@solidjs/router@2.0.0-next.26` byte for byte (MIT; digest `e67af539…04d6`). Declarations as in `router-params`.
