# v1/no-proxy-apis

`SC8009` · **error** · violation

Code depends on JavaScript `Proxy` while targeting an environment where Proxy
support is intentionally excluded.

## What it does

Reports runtime imports from `solid-js/store`, direct `new Proxy` and `Proxy.revocable`
calls, `mergeProps` inputs that may require lazy proxying, and call/member
expressions inside JSX spreads that make Solid proxy the spread result. The rule
models the compatibility policy of eslint-plugin-solid's `no-proxy-apis`; it is
not a claim that these APIs are unsafe on modern engines.

Direct Proxy calls use the compiler-selected standard-library declaration; a
project class or function that shadows `Proxy` stays silent. For `mergeProps`,
the audited 1.9 client runtime enters its Proxy path when a source is callable
or carries Solid's private `$PROXY` marker. Inline or locally resolved functions
therefore produce violations, while an exact plain object literal is certified
safe. A closed literal with accessors is safe as well when every key is
statically known: accessors do not make the object callable or introduce a
hidden `$PROXY` marker. Imports, parameters, spreads, call results, member reads, and other
unresolved values may be plain objects or Proxy-triggering values and produce
an **uncertifiable** obligation. Identifier names such as `props` are never
used as evidence.

An explicit `import type` declaration is proven erased and stays silent.
Side-effect imports and bindings referenced by runtime expressions are proven
to execute and report violations. An ordinary import with no runtime binding
reference is **uncertifiable**: default TypeScript emit erases it, while
`verbatimModuleSyntax` preserves it, and the current fact domains do not carry
that effective emit option.

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
