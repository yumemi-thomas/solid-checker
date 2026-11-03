# context-default-undefined

**Claim (ADR 0575).** rc.17 `getContext` reads `context.defaultValue` when no provider stored a value and throws `ContextNotFoundError` when it is `undefined`; a provider of `undefined` stores the default. So `createContext(undefined)` is default-less and `<Ctx value={undefined}>` is no provider.

`App.tsx` violations (`missing-context-provider`): `BodyRead`, `WrappedRead` (`undefined as …`), `NamespaceRead` (`S.createContext`/`S.useContext`), `AliasRead` (aliased import, a second argument), `JsxRead`, `ProviderOfUndefined` (the only provider passes `undefined`). Silent: `DefaultReads` (`null`, `0`, a `const` holding an object) and `Provided`. Obligation (SC4002 uncertifiable): `UnprovenRead`, a default (`maybeDefault()`) the syntax does not prove is not `undefined`.

**Claim (ADR 0577).** `HandlerMixed.tsx`: a function called in the body and handed to `onClick` reads a provider-free context: the body's run throws `ContextNotFoundError` (SC4002 violation), and the handler's run, which would throw `NoOwnerError`, is not proven to be reached (SC4001 obligation). With a default (`MixedWithDefault`) the body's run returns it and SC4001 stays a violation.

Not covered: a shadowed `undefined` (tsc TS2397 refuses `const undefined`), and a `createContext(undefined)` behind a project wrapper (the binding's initializer is not the call: not a candidate, as for `createContext()`). Premises: the closed program is the `private` manifest. Runtime: fuzzer cells `mx-recall` CONTEXT_NOT_FOUND `default-undefined` and `handler-mixed`.
