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
  analysis cannot see, so it makes no claim.
- The context is a project binding, `const Ctx = createContext()` with no
  argument, that is never written.
- No `<Ctx value={…}>` element names the binding, and no other reference to it
  escapes. An escape is any use that could hand the binding to code that
  provides it: an object literal, a call argument, a return, an assignment.
  Import and export specifiers do not escape; each importer's own uses are
  checked where they occur.
- The program has no namespace import or export, no `export *`, and no module
  load (`require`, `import()`), since each of those reaches a binding without
  naming it.

The read must run under an owner on every path: a component body, a computation
a component creates, or a function called from one. A read with no owner is
`missing-owner` (SC4001), which throws `NoOwnerError` instead. A read whose owner
is uncertain is SC4001's uncertifiable result.

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
rc.13's `getContext`. What the rule does not decide is a provider that exists
but is not above the reader: a closed component tree, with every render site
known, would be needed for that. Package contexts (a kobalte dialog hook outside
its `<Dialog>`, a router hook outside its router) are provided by package code,
which is not this program, so they are not decided here either. Both are in
`docs/open-work.md`.

## Related

- [missing-owner](missing-owner.md): the same read with no owner, which throws `NoOwnerError`
- [package-behavior-unavailable](package-behavior-unavailable.md): package source that is not analyzed
