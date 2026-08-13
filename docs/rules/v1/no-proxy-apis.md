# v1/no-proxy-apis

`SC8009` · **error** · violation

Code depends on JavaScript `Proxy` while targeting an environment where Proxy
support is intentionally excluded.

## What it does

Reports imports from `solid-js/store`, direct `new Proxy` and `Proxy.revocable`
calls, `mergeProps` inputs that may require lazy proxying, and call/member
expressions inside JSX spreads that make Solid proxy the spread result. The rule
models the compatibility policy of eslint-plugin-solid's `no-proxy-apis`; it is
not a claim that these APIs are unsafe on modern engines.

## Why is this bad?

Solid stores and some dynamic prop operations rely on native ES2015 Proxy
semantics. Proxy cannot be faithfully polyfilled, so applications shipped to
older or constrained runtimes can fail even when the rest of the bundle
transpiles successfully.

## Examples

Incorrect for a no-Proxy target:

```tsx
import { createStore } from "solid-js/store";
const wrapped = new Proxy(model, traps);
<Widget {...readProps()} />
```

Prefer plain signals, eager objects, and direct props when that target must be
supported.

## How to fix

Remove the proxy-dependent operation or raise the application's minimum runtime
to one with native Proxy support. This project-wide compatibility decision cannot
be inferred from syntax, so no automatic fix is offered.

## Configuration

Modern-only projects should disable the rule explicitly with
`{ "v1/no-proxy-apis": { "enabled": false } }` in
`.solid-checker/rule-options.json`.
