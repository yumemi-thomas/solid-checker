# server-function-rich-argument

`SC7007` · **error** · violation

A `Date` / `Map` / `Set` / `RegExp` / typed-array argument handed to a server
function while nothing in the project installs an argument serializer.

## What it does

At call sites of proven server functions — functions whose body carries a
`"use server"` directive, or functions imported from a module with a
module-level `"use server"` directive — flags arguments whose resolved
TypeScript type is (or contains at top level) one of the types the default
transport cannot carry: `Date`, `Map`/`ReadonlyMap`, `Set`/`ReadonlySet`,
`RegExp`, or a typed array. The rule goes silent project-wide once
`enableRichArguments` is imported (from
`@solidjs/web/server-functions/rich-args`) or a `serializeArgs` is configured
through `configureServerFunctionsClient`.

**Premise: probe-confirmed** on the pinned
`@solidjs/web@2.0.0-rc.0` server-functions client
(`server-functions/dist/client.js`). By default argument lists travel as
plain JSON — there is no serializer in the client bundle — and
`isJSONSafe` (`client.js:141-170`) accepts only finite primitives, plain
objects, and acyclic arrays. Every probed rich value — a `Date`, `Map`,
`Set`, `RegExp`, `Float64Array`/`Int32Array`, a cyclic object, a class
instance, and a `Date`/`Map`/`Set` nested inside an object or array — makes
the call reject with the directed error at `client.js:395-401`: *"Server
function arguments are sent as JSON by default and these arguments are not
JSON-serializable. Call enableRichArguments() …"*. With
`enableRichArguments()`'s one-line body applied
(`serializeArgs: args => serializeString(args, getServerFunctionsCodec())`,
`dist/rich-args.js`), the same calls encode successfully. The documented
constrained set — "Dates, Maps, Sets, typed arrays, cyclic structures" (RFC
10 and the `rich-args` entry's own doc comment) — is what the rule encodes;
`RegExp` rides the doc's "etc." and is included because the probe proves it
throws (its prototype fails the plain-object check).

## Why is this bad?

The declared TypeScript signature accepts the value — the type system is
fiction at this boundary — so the defect is invisible until the call runs in
a browser and rejects. In-process SSR calls do **not** go through the
transport, so the code can even appear to work server-side and fail only for
real users.

## Examples

Examples of **incorrect** code for this rule (no `enableRichArguments`
anywhere in the project):

```ts
// api.ts
export async function saveEvent(when: Date, tags: Set<string>) {
  "use server";
  await db.insert({ when, tags });
}
```

```tsx
// Client call site: both arguments throw at the transport.
const when = new Date();
const tags = new Set(["a"]);
await saveEvent(when, tags);
```

Examples of **correct** code for this rule:

```ts
// One-time opt-in at client startup — installs the codec's write half
// (~5 KB gz) and every rich argument travels faithfully:
import { enableRichArguments } from "@solidjs/web/server-functions/rich-args";
enableRichArguments();
```

```ts
// Or keep the transport plain and convert at the call site:
await saveEvent(when.toISOString(), Array.from(tags));
```

```ts
// A lone binary body has a natural HTTP encoding and never throws:
await uploadChunk(bytes); // bytes: Uint8Array — sent as the request body
```

## How to fix

Call `enableRichArguments()` once at client startup, or convert the argument
to a JSON-safe shape at the call site (`date.toISOString()`,
`Array.from(map)`, plain objects and arrays of finite primitives).

## When it does not fire

- **`enableRichArguments` imported anywhere** (a value import of the
  `rich-args` entry — importing the entry is the opt-in), or
  **`configureServerFunctionsClient({ serializeArgs })`** configured: both
  install a serializer and remove the throw project-wide (probed), so the
  rule is silent for the whole project.
- **Unresolvable argument types route to silence, not uncertifiable.** Only
  identifier arguments carry demanded type facts; an inline `new Date()`, a
  spread, or an argument whose type cannot be resolved is not reported — an
  unproven rich type is not a proven throw, and this rule only claims proven
  ones.
- **Nested rich types stay silent** for the same reason the rule is precise:
  matching is top-level only (union/intersection members and `[]` array
  element types). `{ when: Date }` throws at runtime (probed) but is not
  claimed statically; the top-level set is what the docs constrain.
- **Natural HTTP encodings.** A `Uint8Array` as the only argument — or in
  trailing position after JSON-safe leading arguments — is sent as a request
  body, not as JSON (probed: it reaches `fetch`), so those positions are
  silent. Other typed arrays (`Float64Array`, `Int32Array`, …) have no
  natural encoding and throw in every position (probed).
- **Server-side call sites.** A call inside a `"use server"` module or inside
  a function that itself carries the directive runs in-process on the server
  and never crosses the client transport (RFC 10).
- The rule belongs to the 2.0 catalog only: Solid 1.x has no core server
  functions.

## Related

- [server-function-module-directive](server-function-module-directive.md) —
  the compiler half of the server-function contract
