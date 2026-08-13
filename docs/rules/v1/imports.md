# v1/imports

`SC8002` · **warning** · violation

A named Solid export is imported from a Solid package entry point that does not
actually export it.

## What it does

Checks named value and type imports from modules owned by the selected Solid 1.x
dialect. The valid locations come from the generated export index for the
installed Solid package, including names that are legitimately re-exported from
more than one entry point. Default and namespace imports are not checked, and an
unknown name is left to the unresolved-import tooling rather than guessed at here.

## Why is this bad?

Solid splits browser, server, store, and core APIs across package entry points.
Importing a real API from the wrong entry point can fail at build time or select
the wrong runtime implementation.

## Examples

Incorrect:

```ts
import { createStore } from "solid-js";
```

Correct:

```ts
import { createStore } from "solid-js/store";
```

## How to fix

Move the named import to the module shown in the diagnostic. A safe automatic fix
is offered when the import declaration contains only that one specifier; splitting
a multi-specifier declaration is left to the author so import grouping and ordering
remain intentional.

## Related

- [jsx-no-undef](jsx-no-undef.md) — unresolved identifiers used by JSX
