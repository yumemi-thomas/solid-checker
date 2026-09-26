# rc9-dynamic-static

`dynamic(source, options?)` has two runtimes on `@solidjs/web@2.0.0-rc.9`,
and the first statement of every build picks one:

```js
if (options?.static) return staticDynamic(untrack(source));
```

(`dist/web.dev.js:2199`, `dist/web.js:2034`, `dist/web.observe.js:2056`; the
server builds call `untrack(source)` the same way at `dist/server.js:3729-3730`,
`dist/server.dev.js:3977-3978` and `dist/server.observe.js:3817-3818`.) With
`static` truthy the source runs once, untracked, under the caller's owner,
before `dynamic` returns, and no memo is built. Otherwise the rc.3 path runs
unchanged: a lazy tracked `createMemo` over the source.

The dialect models the call form, not just the export
(`Dialect::call_form`, `Primitive::DynamicStatic`):

| options argument | form | source runs | owner |
| --- | --- | --- | --- |
| absent, `undefined`/`null`, exact literal without `static`, `static: false` | default | tracked, lazily, in the memo | created |
| exact literal `static: true` | static | inline, once, untracked (as `untrack`) | the caller's |
| anything else (`options`, `{ static: flag }`, spreads, wrapped literals) | unknown | not modelled | not modelled |

What the cases pin:

- **Owner** — `StaticEffect` and `NamespaceStaticEffect` report the unowned
  effect exactly as `UntrackEffect` does. Under the default model they were
  silent (the effect was placed under the memo `dynamic` does not create).
- **Writes** — `StaticWrite` is silent (no owner, no observer at module
  scope), while `DefaultWrite`, `FalseWrite` and `DeferStreamWrite` keep the
  default form's SC2001, and `BodyStaticWrite` keeps SC2001 because `untrack`
  keeps the component owner the write guard keys on.
- **Reads** — `BodyStaticRead` is silent: `untrack(source)` carries no
  strict-read label, so the runtime issues no `STRICT_READ_UNTRACKED` either.
- **Fail closed** — `unknownOptions` and `unknownFlag` claim nothing. The
  default model would have reported their writes as tracked-compute writes and
  placed their effects under a memo, both false if the value is truthy.

`tsc --noEmit` is clean on this fixture against its stubs, and on the same
`App.tsx` against the published rc.9 typings (`solid-js`, `@solidjs/web`,
`@solidjs/signals` at `2.0.0-rc.9`). Against rc.3's typings every two-argument
`dynamic` call is TS2554, so no rc.3-valid call changes form. The stub in
`solid-js.d.ts` says which declarations are byte-faithful and which are
reduced; `node_modules/solid-js/package.json` selects the 2.0 dialect at
`2.0.0-rc.9`.
