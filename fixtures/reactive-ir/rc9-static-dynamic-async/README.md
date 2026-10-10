# rc9-static-dynamic-async

`dynamic(source, { static: true })` on `@solidjs/web@2.0.0-rc.9` calls the
source once, untracked, before `dynamic` returns, and renders what it returned
with no memo in between. A source that returns a Promise is therefore never
rendered:

- the dev builds throw at the call, on any value with a callable `then`
  (`dist/web.dev.js:2249`, `dist/server.dev.js:3979`):
  `dynamic(): a static source must resolve synchronously, not to a promise`;
- the production and observe builds have no guard and fall through to
  `() => undefined`, a component that renders nothing
  (`dist/web.js:2080-2090`, `dist/web.observe.js:2102-2112`,
  `dist/server.js:3729-3733`, `dist/server.observe.js:3817-3821`).

A hand-run of all six bundles under Node confirmed both halves for an async
arrow, `Promise.resolve`, `new Promise` and a plain thenable.

The published typings do not see it: the source is
`() => T | Promise<T> | null | undefined | false` and `DynamicOptions.static`
is a plain `boolean` (`types/index.d.ts:82-97`), with no overload that ties
them. `tsc --noEmit` is clean on this `App.tsx` against its stub and against
the published rc.9 triple (strict, bundler resolution,
`jsxImportSource: "@solidjs/web"`). A source typed `PromiseLike<T>` is TS2322
there, so a non-Promise thenable is TypeScript's and is not a case here.

Expected SC2007 `static-dynamic-async-source`, a violation, at each proven
source:

- `AsyncArrow`, `AsyncFunctionExpression`, `ParenthesizedAsync`,
  `NamespaceAsync` — an inline `async` function;
- `ResolvedPromise`, `ConstructedPromise` — an expression-bodied arrow whose
  returned call the compiler resolves to the standard library's
  `PromiseConstructor.resolve` or its construct signature;
- `NamedAsync`, `ConstAsync` — an identifier Type Facts resolve to a
  same-file async `function` declaration or `const` async arrow.

Silent:

- `SyncStatic` — the static form with a synchronous source;
- `AsyncDefault`, `AsyncFalse`, `AsyncDeferStream` — the default form, whose
  memo settles an async source;
- `unknownFlag` — `{ static: flag }` is the form that states nothing;
- `shadowedPromise` — a local `Promise` is not the standard library's, and
  this source returns synchronously;
- `fromParameter`, `LetAsync` — a caller-supplied or reassignable source;
- `BlockBodiedResolve` — an approximation: a block-bodied non-async source is
  not followed, although this one does return a Promise.

Also SC9014 once, at the solid-js manifest: rc.9 is not the audited release.

`solid-js.d.ts` says which declarations are byte-faithful. `node_modules/`
holds the three package manifests at `2.0.0-rc.9`; the static form is
answered from the resolved `@solidjs/web`. The rc.3 counterpart is
`release-triple-static-dynamic-async-rc3`.
