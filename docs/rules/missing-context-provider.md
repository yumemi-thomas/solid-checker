# missing-context-provider

`SC4002` · **error** by default · violation

A `useContext(Ctx)` read runs under an owner, and `Ctx` was created with no
default, and no provider for `Ctx` exists in the program. Solid throws
`ContextNotFoundError` there (rc.13 `getContext`), so the read never returns a
value. The throw is the same in the development and production builds.

## What it does

The rule reports a read only when the whole program is known and no part of it
can provide the context:

- The program is closed: its boundary is declared closed, or its manifest
  publishes nothing. An open program can be given a provider by code the
  analysis cannot see, so a read the same census finds no provider for there
  is an uncertifiable obligation, never a violation (ADR 0517).
- The context is a project binding, `const Ctx = createContext()` with no
  argument or with `undefined` (rc.17 treats an `undefined` default as none;
  ADR 0575), that is never written. A default the syntax does not prove is a
  value (`createContext(maybe())`) makes the read an uncertifiable obligation.
- No `<Ctx value={…}>` element names the binding (one passing the global
  `undefined`, with no spread, provides nothing: ADR 0575), and no other reference to it
  escapes. An escape is any use that could hand the binding to code that
  provides it: an object literal, a call argument, a return, an assignment.
  Import and export specifiers do not escape; each importer's own uses are
  checked where they occur.
- The program has no namespace import or export, no `export *`, and no module
  load (`require`, `import()`), since each of those reaches a binding without
  naming it.

The read must run under an owner on every path: a component body, a computation
a component creates, or a function called from one. A function a reviewed
standard-library carrier runs before it returns (`[1].forEach(hf)`, `map`,
`sort`, `Array.from`'s map function, `Reflect.apply`) or a tagged template's tag
counts as called there (ADR 0561) where it provably runs it (a non-empty array
literal, `Reflect.apply`; ADR 0563); a same-named project or local `forEach`
does not, and an empty or unknown-length receiver leaves SC4001's obligation.
An arrow written as a prop of a same-file component that only ever calls the
prop also counts as called, at each call written in the component's own body or JSX
(`<Kid run={() => useContext(Ctx)} />` with `props.run()` or `({ run }) => run()`
in `Kid`; ADR 0562). A forwarded, spread, defaulted or copied prop, a call in
the child's handler or `onSettled` callback, and a child in another file are
not followed. A read in a function stored on a member (`o.m = …`,
`this.m = …`, a factory's returned object, an object literal's slot) or handed
to a project wrapper and called later stays SC4001's obligation. A read with no owner is
`missing-owner` (SC4001), which throws `NoOwnerError` instead. A read whose owner
is uncertain is SC4001's uncertifiable result. A read in a function that runs
both under an owner and with none (called in the body and handed to `onClick`)
is reported here, since the owned run throws first, and SC4001's claim there is
an obligation (ADR 0577). A read in a lazy memo nothing proves is read
(ADR 0576), or in code no execution reaches (ADRs 0458, 0578), is uncertifiable.

A default makes the read a value rather than a throw. A context with a provider
above the reader is silent, since the provider is what the rule looks for.

## Examples

Incorrect (closed program, `ThemeContext` has no provider anywhere):

```tsx
import { createContext, useContext } from "solid-js";

const ThemeContext = createContext<"light" | "dark">();

export function Panel() {
  const theme = useContext(ThemeContext); // SC4002: no provider
  return <i>{theme}</i>;
}
```

Correct:

```tsx
export function App() {
  return (
    <ThemeContext value="dark">
      <Panel />
    </ThemeContext>
  );
}
```

## How to fix

Render a provider for the context above the component that reads it, so that the
reader sits inside the provider's children in the rendered tree. If a fallback
value is meant, create the context with a default, `createContext("light")`.

## Premises and limits

The claim is about the closed program, so it needs no runtime premise beyond
rc.13's `getContext`. A context that is provided somewhere, escapes, is
written, or is read through a member or an alias is open (ADR 0713): its read
under an owner is an uncertifiable obligation, blocker
`provider-not-proven-above`, unless every render of the reading function sits
under `<Ctx value>`: in its children with no function between, through an
intrinsic element, a hook, a component nested inside, or a wrapper whose one
`return` is `<Ctx value>{props.children}</Ctx>`, with every reference to each
such function a call or render site (not exported in an open program). A
sibling subtree, a second render outside, a conditional provider and a render
prop callback (`<For>{() => <K />}</For>`) are not proven. Package contexts (a kobalte dialog hook outside
its `<Dialog>`, a router hook outside its router) are provided by package code,
which is not this program, so they are not decided here either. Both are in
`docs/open-work.md`.

## A stored throw needs a read that reports it

A throw here stored on a computation without a proven read that reports it is `computation-error-unobserved` ([ADR 0893](../decisions.md#adr-0893), as in [static-dynamic-async-source](static-dynamic-async-source.md)).

## Related

- [missing-owner](missing-owner.md): the same read with no owner, which throws `NoOwnerError`
- [package-behavior-unavailable](package-behavior-unavailable.md): package source that is not analyzed
