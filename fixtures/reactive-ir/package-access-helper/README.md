# Package source: `access` over an exactly known accessor (ADR 0338)

Claim: a helper that branches on its parameter's type or arity is decided for a call whose argument is a getter the selected dialect has audited. `@solid-primitives/utils`'s `access = (v) => typeof v === "function" && !v.length ? v() : v` reads a `createSignal` or `createMemo` getter on every path, because both getters are `typeof "function"` with `length` 0 (`read.bind(null, node)` over `function read(el)`, `@solidjs/signals@2.0.0-rc.13` `dist/dev.js` `accessor` and `dist/prod/signals.js`; the dialect records the sources in `Dialect::accessor_getter_length`). Called in a component body, that read is untracked: `STRICT_READ_UNTRACKED`, confirmed in the rc.13 dev runtime (runtime-oracle run in Chrome on the first three positives and the negatives: one dev diagnostic at each of the three calls and none at the tracked, plain and parameterized ones; the oracle's own native run cannot read package source, so the case is not kept there). Anything the engine cannot prove stays uncertifiable.

- `BadSignal`, `BadMemo` (positive): `access(value)` and `access(doubled)` in the component body. `strict-read-untracked` violation, through the package source.
- `BadAccessWith` (positive): `accessWith(value)` calls any function argument.
- `BadLocalCallsEveryFunction` (positive): a project function named `access` is followed by its own body (`local-calls.ts` calls every function), not by the name.
- `GoodTracked`, `GoodMemoCompute` (negative): the same call in a tracked JSX expression and in a memo's compute.
- `GoodPlain` (negative): a number and a `string` property are returned as they are.
- `GoodStore` (negative): a store is an object, not a getter.
- `GoodWithParameters` (negative): a function with a parameter has `length` 1; `access` returns it without calling it.
- `GoodLocalCallsWithParameters` (negative): a project `access` that calls only a function of positive `length` (`local-access.ts`) never calls a getter, so nothing is read.
- `UncertifiableMemoToString`, `UncertifiableSlice`, `UncertifiableReverse`, `UncertifiableSliceZero`, `UncertifiableMemoBind`, `UncertifiableReverseTwice`, `UncertifiableHeldTuple` (uncertifiable, SC9012): a member call on a primitive's result (`createMemo(...).toString()`, `createSignal(1).slice(0)`, `createMemo(fn).bind(null)`, `.reverse().reverse()`, or on a `const pair = createSignal(1)` binding) is that value's own method, never the primitive, so `call_primitive_name` does not resolve the primitive. What it returns has no provenance the engine can state: `.slice(0)` keeps the getter in the first slot (rc.13 emits STRICT_READ_UNTRACKED for `access(value)`, which silence would have certified), `.slice(1)` and `.reverse()` hold the setter, `.toString()` is a string. `primitive_result_methods` raises a `ReactiveDispatchUnresolved` obligation (`primitive-result-method`) at every such call.
- `GoodMemberCallElsewhere` (negative): a member call on a value that is not a primitive's result raises nothing.
- `UncertifiableRedefinedLength`, `UncertifiablePassedElsewhere` (uncertifiable): a function's `length` is configurable, so a getter handed to `Object.defineProperty` or to anything but the helper being decided has no proven length; the fact needs every reference to be a call of the getter or an argument of the decided helper whose other uses of that parameter cannot run (the package's `: v` return is unreachable for a getter of length 0).
- `BadCalledAndRepeated` (positive): calling the getter and handing it to the same helper twice keep the fact.
- `UncertifiableHoistedRedefinition` (uncertifiable): the helper redefines `length` in a hoisted function declaration written after its `return`; a declaration's body runs whenever it is called, so a site in it is never unreachable for coming after a `return` (the ADR 0280 walk).
- `exported-getter.tsx`, `exported-mut.ts`, `exported-index.tsx` (uncertifiable): an exported getter (`export const [value] = createSignal(1)`) has no static reference besides its declaration, yet other code reaches it through the module namespace (`Object.values(m)`, `m[key]`, `import()`) and redefines `length` first, as `exported-mut.ts` does. A binding the module exports, by declaration or by specifier, proves no length.
- `variants/patched-bind` (uncertifiable): the project replaces `Function.prototype.bind` and gives every bound function `length` 1, which is how every rc.13 getter is built, so the length fact is withheld project-wide. A variant, since the veto covers the whole project.
- `Eval.tsx` `UncertifiableEval` (uncertifiable): a direct `eval` in the declaring file reaches the binding with no static reference. Kept in its own file, since the refusal is for the whole file.
- `MissedHoistedCallee` (known miss, silent): a helper that reads its parameter inside a hoisted function it calls is not followed (nested function declarations are not summarised), so the read is not reported; open-work.md.
- `UncertifiableAlias` (uncertifiable, internal): `const alias = value` is not the binding the primitive created, so the argument proves nothing about its arity and the read stays one the call is not proven to perform.

Only an exact, unwritten binding whose references are all calls or arguments of the decided helper of the accessor slot of `createSignal`/`createOptimistic` (`const [value] = ...`) or of `createMemo` proves the value; a function literal, an alias, a props value, a store, a setter or an import proves nothing, so the branch stays undecided.

## Package bytes

`node_modules/@solid-primitives/utils` is the published `@solid-primitives/utils@7.0.0-next.4`, copied unchanged: `package.json`, `LICENSE`, `dist/index.js`, `dist/index.d.ts`, `dist/types.js`, `dist/types.d.ts`. Integrity (from the `pnpm-lock.yaml` of the corpus app it was copied from): `sha512-9KyRe6eu4hdhxxGOyS9vAa2s/T8FZ1sMJtk3UuV3W1Gi0f4if+o1n7S1M6l/MMjn8+m3+I3VB1KzFa9gNDkDsg==`. SHA-256 of the files as copied:

```
59dd297c1ff4a90620cd3f27aa9b0a650404e8178513d6501af5ec1c3f8370f8  dist/index.js
f6ffec2694503edf41efb59dc9a4adcb62bff71074d1210ee94201b1026b6644  dist/types.js
c464f6ae1e837bbfc3103b2c59cc920de983e147e676329893308bc962165193  dist/index.d.ts
0fd2aeeaa012dac436b3bc4e0b3248667094de1f947a6f79fcadc9ff0640bf53  dist/types.d.ts
f72b16817e60816623cea5d22869fd641ec3bdb07728bb4b0cc4a511ecc8af0b  package.json
492f24f38fcfb6a0b2d550c47fb26164124df40ec0a95c079d43e19d54137d70  LICENSE
```

The manifest keeps its `exports` map, whose `@solid-primitives/source` condition names `src/`, which the package does not ship; the runtime source is its `default` target `dist/index.js`. `.solid-checker/runtime.json` declares a browser development host and selects the package as a dependency source (the arity does not differ between builds: the dev and prod getters are both `read.bind(null, node)`, and the server getters are arrows with no parameter).

## tsc

`solid-js.d.ts` is `dependency-source-package-accessor`'s stub plus the plain `createStore` signature copied from `@solidjs/signals@2.0.0-rc.13` `store/index.d.ts`. The stub omits names the package's declarations import from `solid-js`, so this fixture's tsconfig sets `skipLibCheck`. The files `App.tsx`, `local-access.ts` and `local-calls.ts` pass `tsc --noEmit` (TypeScript 6.0.3, `strict`, `jsxImportSource: @solidjs/web`) against the published `solid-js`, `@solidjs/web` and `@solidjs/signals` 2.0.0-rc.13 typings and these package declarations, so no finding duplicates a type error.
