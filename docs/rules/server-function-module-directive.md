# server-function-module-directive

`SC7006` · **error** · violation

A module-level `"use server"` directive with an export that is not a direct
function declaration — a wrapped export, a non-function default export, or a
re-export.

## What it does

In a module whose directive prologue contains `"use server"`, flags every
export the server-function compiler provably does not turn into a client
reference:

- `export const x = wrapper(async () => …)` — a call-expression initializer
  (`GET(…)`, `withMeta(…)`, any wrapper);
- `export default wrapper(fn)` — a non-function default-export expression
  that is a call;
- `export { x } from "./other"` and `export * from "./other"` — re-exports.

**Premise: doc-derived.** The directive pass lives in the build plugin, which
is not part of the pinned `@solidjs/web@2.0.0-rc.0` package, so the RFC text
is the specification this rule encodes — and it is explicit. RFC 10
§Compiler implications: *"One pre-existing bug to fix regardless: with a
module-level `"use server"` directive, a wrapped export (`export const x =
wrapper(async () => ...)`) is silently dropped from the client build — only
direct function exports become references. This gets more visible with
`GET(fn)` as the blessed idiom. Minimum: a diagnostic; better: extract the
inner function and preserve the call in client output."* This rule is that
minimum diagnostic, on the checker's side of the fence.

The pinned runtime corroborates the premise from its own side:
`@solidjs/web@2.0.0-rc.0`'s `server-functions/dist/server.js` throws `Export
from a 'use server' module must be a function` in `createServerReference` —
a non-function value cannot become a reference even where the pipeline does
hand it over (code-read).

## Why is this bad?

The client build silently loses the export: the module's other functions
become fetch-backed references, while the wrapped one becomes `undefined` on
the client. The failure appears far from its cause — an import that
typechecks, then a runtime "x is not a function" in the browser — and gets
more likely as `GET(fn)` becomes the blessed method-declaration idiom.

## Examples

Examples of **incorrect** code for this rule:

```ts
"use server";
import { GET } from "@solidjs/web/server-functions";

// Dropped from the client build: only direct function exports become
// references under a module-level directive.
export const getUser = GET(async (id: string) => db.users.find(id));

export * from "./more-functions"; // re-exports are dropped too
```

Examples of **correct** code for this rule:

```ts
// No module-level directive: the function-level directive round-trips the
// wrapper call in both builds (verified compiler behavior, RFC 10).
import { GET } from "@solidjs/web/server-functions";

export const getUser = GET(async (id: string) => {
  "use server";
  return db.users.find(id);
});
```

```ts
"use server";

// Direct function exports become references — declarations and direct
// function expressions alike.
export async function addTodo(title: string) { /* … */ }
export const removeTodo = async (id: string) => { /* … */ };
```

## How to fix

Move the directive into each function body and keep the wrapper at the export
site — function-level directives round-trip wrapper calls, so `export const
getData = GET(async (id) => { "use server"; … })` works in both builds — or
keep the module-level directive and export only plain functions, wrapping
them where they are imported.

## When it does not fire

- **Direct function exports** — `export function f() {}`, `export async
  function f() {}`, `export const f = () => {}` (function-expression
  initializers) — are the supported shape.
- **Identifier aliases and plain values** (`export const x = localFn`,
  `export const LIMIT = 5`) route to silence: the RFC names the wrapped-call
  and re-export forms, and an alias's fate is not provable from this package's
  artifacts, so nothing is claimed.
- **Type-only exports** are erased at build time and never at risk.
- **Function-level directives** put nothing at risk — a module without a
  module-level `"use server"` prologue is never inspected.
- The rule belongs to the 2.0 catalog only: Solid 1.x has no core server
  functions.

## Related

- [server-function-rich-argument](server-function-rich-argument.md) — the
  transport half of the server-function contract
