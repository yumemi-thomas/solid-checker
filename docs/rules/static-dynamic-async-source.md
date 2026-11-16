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
`Promise.resolve`, `new Promise` and a plain thenable. From rc.17 the same
code is `solid-js`'s `dynamicCore`, which `@solidjs/web`'s `dynamic` forwards to
(`solid-js@2.0.0-rc.17` `dist/solid.dev.js:1445` and `:1530`,
`dist/server.dev.js:2587-2589`); under web rc.17 the static form is modelled
only with `solid-js` rc.17.

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
- **A synchronous source.** A function whose closed return type holds no Promise
  (ADR 0654). A source with no such proof is not silent, see below.

## Uncertifiable

A static source not proven synchronous, blocker `static-source-unproven` (ADR 0654): a
parameter, an import, a `let`, a return typed `any`, `unknown` or a type parameter
(Type Facts `returnOpen`), or a return that may be a Promise or not. An unproven option
(`{ static: flag }`, a spread, an options variable) may take either runtime, so a source
not proven synchronous is uncertifiable there too, and a proven Promise as well. A
block body or a `.then` chain whose return type is a Promise on every path is a violation.

- **A site that never runs.** A call in a function only a `defer: true` apply
  with no compute source calls is silent (ADR 0590).

A throw inside a generator stepped by an async caller rejects that caller's flight. The thenable source proves the runtime guard, but no proof follows the rejected flight to an observed read or error boundary. These sites retain a named `async-flight-error-unobserved` obligation (ADR 0852), including in-place callback layers.

## Related

- [sync-computation-received-async](sync-computation-received-async.md) — the
  same "asserted synchronous, given a Promise" defect on `sync: true`
  computations

## A stored throw needs a read that reports it

`resolve`, `until`, a static `dynamic` source and `useContext` throw with no diagnostic. In a memo or another storing compute the error stays on the node until a read rethrows it. With no proven read that reports it (an effect compute with no `error` arm, intrinsic JSX, a handler, a component body) the row is `computation-error-unobserved`; one entry that reports keeps the violation (ADR 0893). A class constructor or method with no entry found, written in such a compute, is read as the site would be (ADR 0914).
A throw that leaves an `async` function rejects the promise its call returned: it is a violation only where that promise is dropped (an expression statement, `void`, a `then` chain with no rejection arm), awaited, or returned to a call that drops it; a promise that is caught, assigned or handed on leaves the row `computation-error-unobserved` ([ADR 1008](../decisions.md#adr-1008)).
A function the render runs in place reports where its element does ([ADR 1006](../decisions.md#adr-1006)): one an intrinsic `ref` applies (the literal, an array element, a conditional's branch, the literal an exact same-file factory call returns) and the function child of `For`, `Repeat`, `Show` or `Match`; a boundary above the element still takes the throw.

## Every call ends (ADR 0989)

Each `dynamic` call in its static form, or in a form the options do not decide, is a site (`dynamic-static`, DYNAMIC_STATIC_PROMISE). A source proven to answer synchronously (a closed return type with no thenable, a sync or async generator: the static form throws on a thenable only) ends it (`source-synchronous`); a source not proven synchronous stays an obligation.
