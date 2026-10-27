# test-runner-bun

Claim (ADR 0438): a callback handed directly to `bun:test`'s `describe` and `test` (declared in `bun-types/test.d.ts`) runs from the runner's scheduler with no owner and no strict-read window, so a synchronous dispatch in it does not withhold the event proofs: the handler's effect creation is a violation (SC4001); its write and read are clean.

Published bytes: `node_modules/bun-types` is `bun-types@1.4.4`, integrity `sha512-yjrJh5oGX7mMPKGd19YXyIjmEuj4G4zNC30bs7PM9Az7NQ3ndF3j6hbeRdvJTRQEhJ/FYQwW0a2moweEi2hMxQ==`; only `package.json` and `test.d.ts` are copied, unchanged (`bun-test.d.ts` is a reference that brings the module declaration into the program, as the package's own `index.d.ts` does):

```
f8e1ba23b5fc8cd1ffcf5c3f159a5d03b3341da33945e4edee0ef02dc8eddf3d  package.json
b7aa2808c01a7207d8aeef3343ac1f3bbf563da2a28baf8634c48b606c34020e  test.d.ts
```

`variants/local`: the same test imports `describe` and `it` (as `test`) from a project module (`./runner`) instead; the callee resolves to a project declaration, so the dispatch keeps the veto and the handler's findings are uncertifiable.

The `solid-js` stubs carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
