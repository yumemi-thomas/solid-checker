# static-dynamic-async-source

`SC2007` · **error** · violation

`dynamic(source, { static: true })` is called with a source that provably
returns a Promise.

## What it does

Flags a `dynamic` call from `@solidjs/web` (`2.0.0-rc.9` on; see "When it does
not fire") whose options argument is a literal with `static: true` and whose
source is proven to return a Promise:

- an `async` function, written inline or named by an identifier that resolves
  to a same-file `async function` declaration or a `const` initialized with an
  async function;
- an expression-bodied arrow whose returned expression is a call the compiler
  resolves to the standard library's `Promise.resolve(...)` or
  `new Promise(...)`.

**Premise: read from the published bytes** of `@solidjs/web@2.0.0-rc.9`.
Every build opens `dynamic` with `if (options?.static) …` and calls the source
once, untracked, before returning. The dev builds then refuse a thenable
(`dist/web.dev.js:2249`, and the same line at `dist/server.dev.js:3979`):

```js
function staticDynamic(component) {
  if (component && typeof component.then === "function") throw new Error("dynamic(): a static source must resolve synchronously, not to a promise");
  ...
```

The production and observe builds have no guard: a Promise is neither a
function nor a string, so they return `() => undefined`, a component that
renders nothing (`dist/web.js:2080-2090`, `dist/web.observe.js:2102-2112`,
`dist/server.js:3729-3733`, `dist/server.observe.js:3817-3821`). A hand-run of
all six bundles under Node confirmed both outcomes for an async arrow,
`Promise.resolve`, `new Promise` and a plain thenable.

**TypeScript does not report it.** The published signature is

```ts
export declare function dynamic<T extends ValidComponent>(source: () => T | Promise<T> | null | undefined | false, options?: DynamicOptions): Component<ComponentProps<T>>;
```

with `DynamicOptions.static?: boolean` (`types/index.d.ts:82-97`): nothing ties
the source's type to the option, so every source this rule proves is
`tsc --noEmit`-clean against the published rc.9 typings. The one spelling the
type does reject — a source typed `PromiseLike<T>` rather than `Promise<T>`,
which is TS2322 — is not reported here.

## Why is this bad?

The static form exists for a source that cannot change: it skips the memo the
default form builds, and with it the machinery that waits for a Promise. A
promise-valued source is the one thing that machinery was for. In dev the call
throws at module or component evaluation; in production the resolved
component is never rendered and nothing says why.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const Editor = dynamic(async () => (await import("./Editor")).default, { static: true });

const Chart = dynamic(() => Promise.resolve(BarChart), { static: true });
```

Examples of **correct** code for this rule:

```tsx
// The default form settles the source in its memo; render it under <Loading>.
const Editor = dynamic(async () => (await import("./Editor")).default);

// A static source that is synchronous.
const Chart = dynamic(() => BarChart, { static: true });
```

## How to fix

Drop `static: true` so `dynamic()` settles the source in its memo, and render
the result under a `<Loading>` boundary. If the component really cannot
change, resolve it before calling `dynamic()` and pass a synchronous source.

## When it does not fire

- **Before rc.9.** The static form arrives in `@solidjs/web@2.0.0-rc.9`. On
  rc.0-rc.6 `dynamic` takes one parameter (a second is TS2554) and on rc.7 and
  rc.8 `DynamicOptions` has no `static` (TS2353); no build before rc.9 reads
  the option, so an async source is settled by the memo as usual.
  `fixtures/reactive-ir/release-triple-static-dynamic-async-rc3` pins it
  silent. On a web release nobody compared, an option-bearing call is the form
  that states nothing.
- **An unproven option.** `{ static: flag }`, a spread, or an options variable
  may take either runtime, so no claim is made.
- **An unproven source.** A block-bodied non-async function (even one whose
  every `return` is `Promise.resolve(...)`), a parameter, an import, a `let`
  binding, a call chained off a Promise (`Promise.resolve(x).then(f)`), and a
  `Promise` that is not the standard library's all stay silent. This is an
  approximation, not a claim that they are synchronous.

## Related

- [sync-computation-received-async](sync-computation-received-async.md) — the
  same "asserted synchronous, given a Promise" defect on `sync: true`
  computations
