# Package source: `access` over an exactly known accessor (ADR 0338)

Claim: a helper that branches on its parameter's type or arity is decided for a call whose argument is a getter the selected dialect has audited. `@solid-primitives/utils`'s `access = (v) => typeof v === "function" && !v.length ? v() : v` reads a `createSignal` or `createMemo` getter on every path, because both getters are `typeof "function"` with `length` 0 (`read.bind(null, node)` over `function read(el)`, `@solidjs/signals@2.0.0-rc.13` `dist/dev.js` `accessor` and `dist/prod/signals.js`; the dialect records the sources in `Dialect::accessor_getter_length`). Called in a component body, that read is untracked: `STRICT_READ_UNTRACKED`, confirmed in the rc.13 dev runtime.

- `BadSignal`, `BadMemo` (positive): `access(value)` and `access(doubled)` in the component body. `strict-read-untracked` violation, through the package source.
- `BadAccessWith` (positive): `accessWith(value)` calls any function argument.
- `BadLocalCallsEveryFunction` (positive): a project function named `access` is followed by its own body (`local-calls.ts` calls every function), not by the name.
- `GoodTracked`, `GoodMemoCompute` (negative): the same call in a tracked JSX expression and in a memo's compute.
- `GoodPlain` (negative): a number and a `string` property are returned as they are.
- `UncertifiableMutableMember` (SC9012): `config.label` of a mutable object may hold a getter.
- `GoodStore` (negative): a store is an object, not a getter.
- `GoodWithParameters` (negative): a function with a parameter has `length` 1; `access` returns it without calling it.
- `GoodLocalCallsWithParameters` (negative): a project `access` that calls only a function of positive `length` (`local-access.ts`) never calls a getter, so nothing is read.
- `UncertifiableMemoToString`, `…Slice`, `…Reverse`, `…SliceZero`, `…MemoBind`, `…ReverseTwice`, `…HeldTuple` (SC9012): a member call on a primitive's result (`.toString()`, `.slice(0)`, `.bind(null)`, `.reverse()`, also on a held tuple) is the value's own method, so its return has no stated provenance (`.slice(0)` keeps the getter, which silence would have certified).
- `GoodMemberCallElsewhere` (negative): a member call on a value that is not a primitive's result raises nothing; `String(1).slice(0)` is a primitive.
- `Unproven.tsx` (ADR 0338, inverted): `access` calls its argument unless it is proven not a function, so an argument the engine can prove neither a getter nor a non-function keeps an SC9012 obligation (`parameter-call-argument-unproven`). The `Uncertifiable*` components reach `access` with a getter through a spread, rest, held alias, `Array.from`, `slice`, `bind`, `Reflect.apply` or a helper's return or hop. `GoodProjectFunction`, `GoodLiteral` and `GoodPrimitiveTyped` are proven non-getters and stay clean.
- `UncertifiableRedefinedLength`, `UncertifiablePassedElsewhere` (uncertifiable): `length` is configurable, so a getter handed to `Object.defineProperty` or anything but the decided helper proves no length; every reference must be a call of it or an argument of that helper (whose `: v` return is unreachable for length 0).
- `BadCalledAndRepeated` (positive): calling the getter and handing it to the same helper twice keep the fact.
- `UncertifiableHoistedRedefinition` (uncertifiable): the helper redefines `length` in a hoisted function declaration written after its `return`; a declaration's body runs whenever it is called, so a site in it is never unreachable for coming after a `return` (the ADR 0280 walk).
- `exported-getter.tsx`, `exported-mut.ts`, `exported-index.tsx` (uncertifiable): other code can reach an exported getter through the module namespace (`Object.values(m)`, `m[key]`, `import()`) and redefine `length`, as `exported-mut.ts` does, so an exported binding proves no length.
- `variants/patched-bind` (uncertifiable): the project replaces `Function.prototype.bind` and gives every bound function `length` 1, which is how every rc.13 getter is built, so the length fact is withheld project-wide.
- `Eval.tsx` `UncertifiableEval` (uncertifiable): a direct `eval` in the declaring file reaches the binding with no static reference.
- `MissedHoistedCallee` (known miss, silent): a helper that reads its parameter inside a hoisted function it calls is not followed (nested function declarations are not summarised), so the read is not reported; open-work.md.
- `UncertifiableAlias` (uncertifiable, internal): `const alias = value` is not the binding the primitive created, so the argument proves nothing about its arity and the read stays one the call is not proven to perform.

Only an exact, unwritten binding whose references are all calls or arguments of the decided helper of the accessor slot of `createSignal`/`createOptimistic` (`const [value] = ...`) or of `createMemo` proves the value; a function literal, an alias, a props value, a store, a setter or an import proves nothing, so the branch stays undecided.

## Package bytes

`node_modules/@solid-primitives/utils` is the published `@solid-primitives/utils@7.0.0-next.7`, copied unchanged: `package.json`, `LICENSE`, `dist/index.js`, `dist/index.d.ts`, `dist/types.js`, `dist/types.d.ts`. Integrity: `sha512-t1ZYgNililGfuHBJ34iT/m3b75MxNOSyjmPrm2MVhNnUM76hBziULCteA6YQ0laIb6THBv8gRvssHK2BxMYw3Q==`. `access` and `accessWith` are next.4's bytes. SHA-256 of the files as copied:

```
206bace61b53891f9649564492471cd26518202b1a306f2c169973e3ac47b1d2  dist/index.js
f6ffec2694503edf41efb59dc9a4adcb62bff71074d1210ee94201b1026b6644  dist/types.js
e7cc6afd3aa8ad55e58414a8cc5c7c1afc6928820ad024a6672be53e1551f761  dist/index.d.ts
5082aca07cb7b28ff15a81f35244faf941e4fc53e335322b2befe5e9d079530b  dist/types.d.ts
82982ef3dc4b3222539fcaa6f5b5619400da74144dcce1556caa63e361d30d53  package.json
492f24f38fcfb6a0b2d550c47fb26164124df40ec0a95c079d43e19d54137d70  LICENSE
```

The `exports` map's `@solid-primitives/source` condition names unshipped `src/`; the runtime source is its `default`, `dist/index.js`. `.solid-checker/runtime.json` declares a browser development host and selects the package as a dependency source (getter arity is 0 in every build).

## tsc

`solid-js.d.ts` is `dependency-source-package-accessor`'s stub plus `createStore` from `@solidjs/signals@2.0.0-rc.13` `store/index.d.ts`; it omits names the package's declarations import, hence `skipLibCheck`. `App.tsx`, `local-access.ts` and `local-calls.ts` pass strict `tsc --noEmit` (TypeScript 6.0.3) against the published 2.0.0-rc.13 typings and these declarations, so no finding duplicates a type error.
