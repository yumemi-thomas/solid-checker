# valid-use-server

`SC7006` · **error** · violation

A `"use server"` directive the compiler ignores or rejects, or a module-level
directive file with an export, re-export or module-scope call it mishandles.

## What it does

The directive pass belongs to the build plugin, not to `@solidjs/web`. Its
behavior is read from `@solidjs/compiler@2.0.0-rc.17` as pinned at the
semantic-facts fork revision (`packages/compiler/src/directives/`), RFC 10 and
`@solidjs/web@2.0.0-rc.17`'s server-functions runtime. The compiler version a
project's build actually runs is not observable, so every claim below holds on
that audited compiler, and each says what an older one does.

Placement (a function-level or module-level directive):

- **A string statement after another statement, in a block, or in a loop**
  (`const t = "users"; "use server";`, `if (id) { "use server"; }`). The pass
  reads a directive only from a prologue (`transform.rs:897-901`
  `body_has_directive` over `body.directives`, `:148-151` for the module), so
  this is an ordinary expression and the function stays on the client. A
  module directive after an `import` is the same.
- **A template literal** `` `use server`; ``: never a directive, even in a
  directive file.
- **A directive in an object method, getter, setter, class method,
  constructor or class accessor.** Never extracted; rc.17 rejects it with a
  compile error (`validate.rs`, rc.8 changeset), earlier compilers ignore it
  and the body keeps running in the browser. A function-valued property
  (`{ f: async function () { "use server" } }`), a block-bodied arrow and a
  class field's arrow are extracted and are not reported.

Not read, mirroring the compiler and upstream: a directive inside a function
nested in an extracted server function (`validate.rs:247-258`), and
function-level placement in a module-level directive file, where the whole
module is server code. Unlike upstream, a stray string statement in the
extracted function's own block is silent too (it "changes nothing"; ownership
case `stray-in-extracted-function/001`).

In a module whose prologue contains `"use server"`:

- **A re-export** (`export { x } from`, `export * from`) is a compile error
  since rc.8 (`transform.rs:1377-1412`; earlier it was missing from the client
  build). Upstream accepts it; the divergence is ownership case
  `re-export/001`.
- **An export that is not a function** (a literal, template, array or object,
  `export default { … }`): `registerServerReference` throws at module
  evaluation, so the server fails at boot
  (`@solidjs/web` `server-functions/dist/server.js:1567-1571`).
- **A module-scope call to a client-side declaration wrapper** (`GET`, `live`,
  `withMeta`, `query`, `action`, `liveQuery`): the client module is rebuilt as
  bare references (`transform.rs:8-11`), so the wrapper's behavior never
  reaches the client (RFC 10: "client-transport declarations like `GET` are
  meaningless _inside_ a module-level file"). The built-in names are matched
  by the callee's **exact import binding** from `solid-js`, `@solidjs/web`,
  `@solidjs/router` or `@solidjs/start` (or a subpath of one), resolved by the
  binder: a project function spelled `query`, a namespace member call and an
  import from another package are silent, and a renamed import
  (`query as q`) is reported under its local name. A call inside a function is
  server-side code and is not reported.

A **wrapped export** (`export const getUser = withValidation(schema, fn)`,
`export default withDelay(fn, 400)`) is **not** a defect. Since rc.4 the
server registers each export's evaluated value and the compiler never inspects
its shape (`transform.rs:411-420`; the compiler's own `module-wrapped-exports`
fixture; RFC 10, "Module-level directives invert this"). The sentence of RFC 10
this rule first encoded (a wrapped export "silently dropped from the client
build") described rc.0-rc.2 and was superseded: rc.3 made it a compile error,
rc.4 registered by value. It is no longer reported on any release (the
compiler version is not observable, so the rule fails closed). Such an export,
and any alias or call whose value the facts do not show to be a function, is an
**uncertifiable** result.

### The `clientWrappers` option

`.solid-checker/rule-options.json`, under `valid-use-server` (or its former
name `server-function-module-directive`):

```json
{ "schemaVersion": 1, "rules": { "valid-use-server": { "clientWrappers": ["defineLoader", "define*", "/^use[A-Z]/"] } } }
```

Upstream's grammar, entry by entry: `/pattern/` (longer than two characters) is
an **unanchored** regular expression; an entry with `*` matches names built
from its literal parts with any run of `[a-zA-Z0-9_$]` between them, anchored;
anything else is exact. An entry that does not parse as a regular expression is
compared exactly. The regular-expression arm is a subset of JavaScript's
(literals, `.`, classes, `^ $`, groups, `|`, `* + ? {n,m}`; no lookaround,
backreference or `\b`), so a pattern outside it compares exactly where upstream
would match: that can only withhold a report. These names are matched against
the callee's **local spelling**, with no import: they are the project's own
statement that the callee is a client wrapper, the one place this checker
matches by name. They apply to module-scope calls in a module-level directive
file, beside the built-in list. Any other key under the rule is refused.

## Why is this bad?

A misplaced or unextractable directive leaves code the author meant to keep off
the client on the client (or fails the build); a re-export or non-function
export fails the compile or the server boot; a client wrapper in a directive
file silently loses its behavior (a `GET` call goes over POST).

## Examples

Examples of **incorrect** code for this rule:

```ts
export async function getUser(id: string) {
  const table = "users";
  "use server"; // ordinary expression: the function stays on the client
  return db.get(table, id);
}

const api = {
  async getUser() {
    "use server"; // never extracted; rc.17: compile error
  },
};
```

```ts
"use server";
import { GET } from "@solidjs/web/server-functions";

export const getUser = GET(async (id: string) => db.users.find(id)); // GET's behavior is lost
export const LIMIT = 10; // not a function: the server throws at boot
export * from "./more-functions"; // re-exports are a compile error
```

Examples of **correct** code for this rule:

```ts
import { GET } from "@solidjs/web/server-functions";

// Function-level directive: the wrapper call round-trips in both builds.
export const getUser = GET(async (id: string) => {
  "use server";
  return db.users.find(id);
});
```

```ts
"use server";

export async function addTodo(title: string) { /* … */ }
export const removeTodo = async (id: string) => { /* … */ };
// A server-side wrapper is registered by value:
export const save = withValidation(schema, async (todo: Todo) => { /* … */ });
```

## How to fix

Put the directive first in the function body or module, as a plain string, in
a function declaration, function expression or block-bodied arrow. Keep client
wrappers (`GET`, `query`, …) around a function-level directive or at the
consumption site, and move non-function exports and re-exports to a module
without the directive.

## When it does not fire

- No module-level directive: only function-level placement is read.
- Type-only exports are erased at build time.
- The rule belongs to the 2.0 catalog only: Solid 1.x has no core server
  functions.

## Related

- [server-function-rich-argument](server-function-rich-argument.md) — the
  transport half of the server-function contract
