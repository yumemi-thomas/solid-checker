# test-runner-node

Claim (ADR 0396): a callback handed directly to `node:test`'s `describe` and `it` (declared in `@types/node/test.d.ts`, where `describe` is `suite` and `it` is `test`) runs from the runner's scheduler with no owner and no strict-read window, so a synchronous dispatch in it does not withhold the event proofs: the handler's effect creation is a violation (SC4001); its write and read are clean.

Published bytes: `node_modules/@types/node` is `@types/node@26.6.5`, integrity `sha512-uMzsv9stDdofIZBx4UGh4Z9ZgXnGC9seRDscilBcTHAeOFk67MZg/k9bWVfO+bk4qZmG+l52hRnLwUiH2LnJTg==`; only `package.json` and `test.d.ts` are copied, unchanged (`node-test.d.ts` is a reference that brings the module declaration into the program, as the package's own `index.d.ts` does):

```
e9bf302124358d90ec233c509c4f9fb2223129b988da8147b5c1bf835f652e86  package.json
3e221ff0522e6ff95aa74a5211126aaf4d7e85ed8c8ff19290d591ad4bef988f  test.d.ts
```

`variants/local`: the same test imports `describe` and `it` from a project module (`./runner`) instead; the callee resolves to a project declaration, so the dispatch keeps the veto and the handler's findings are uncertifiable.

The `solid-js` stubs carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
