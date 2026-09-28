# callee-callback-timing

**Claim.** A read in a function literal written in a component body and handed
to a *project* function -- directly, or as an element of an array or object
literal argument -- is **uncertifiable** under `SC1001` unless the callee's own
body calls that parameter during the call, in which case it stays a proven
violation (`execution_role::callee_callback_timing`,
`ReactiveRead::callee_callback_timing`).

The lexical fallback places code by where it is written, so every such literal
used to take the component body's role and be reported as a proven untracked
read. That holds only if the literal runs during the body. Handed to a project
function, it runs wherever that function runs it: during the call, from a
closure it returns or keeps, or never. The one invocation the facts prove is
the direct one: the literal *is* the argument, and the synchronous callee's
body (not a closure it creates) calls the parameter by symbol. A callee with no
body in the project -- a package seen through its declarations, or a callee
nothing resolves -- proves it only through an accepted contract's `inline` row. `@kobalte/core`
had 89 `SC1001` violations of this kind, `chain([...])` handlers among them,
and `@solidjs/router` `createOutlet(() => ...)`.

Measured on the published `solid-js`/`@solidjs/signals`/`@solidjs/web`
`2.0.0-rc.9` bytes (byte-identical to the audited archives), compiled with
`babel-preset-solid@2.0.0-rc.2` and run under jsdom, each component mounted
with `render(() => createComponent(...))`:

- `InvokedDuringCall`, `InvokedCrossFile`, `InvokedThroughNamespace`,
  `InvokedByMethod` and `EagerElements` raise `STRICT_READ_UNTRACKED` while mounting (dev);
- `ChainInHandler`, `KeptCrossFile`, `Ignored` and `AfterAwait` raise nothing
  while mounting, dev or prod; `localChain`'s invoker run from a click handler
  raises nothing either.

| Case | Finding | Why |
| --- | --- | --- |
| `localChain([() => n()])`, invoker run from a handler | `SC1001` uncertifiable | an array element's invocation has no fact |
| `runLater(() => n())` (cross-file), returns a closure that calls it | `SC1001` uncertifiable | the callee's body does not call it during the call |
| `ignore(() => n())`, never calls it | `SC1001` uncertifiable | never invoked, and nothing proves that either |
| an async callee that calls it after an `await` | `SC1001` uncertifiable | an async body may call it after the call returns |
| `eager([() => n()])`, which calls the elements during the call | `SC1001` uncertifiable, **not proven** | a true defect, but element iteration has no fact here |
| `now(() => n())`, whose body calls it | `SC1001` violation | the callee calls the parameter during the call |
| the same through a cross-file import, a namespace import, and a project object's method | `SC1001` violation | the callee is resolved by symbol either way |
| `subscribe(() => n())` from a package seen only through declarations, with no contract (`External.tsx`) | `SC1001` uncertifiable | no body and no accepted `inline` row say it runs the literal during the call |

Remaining approximations: a literal stored in a local array or object
(`const arr = [() => n()]`, `const obj = { f: () => n() }`) is not an argument
and keeps its lexical role; writes in such literals keep theirs too (the flag is
a read hole), and so do pending async reads (`SC5001`).

**Stub.** `solid-js.d.ts` holds `createSignal` verbatim from the rc.9 typings,
as its header lists. `App.tsx` and `helpers.ts` type-check cleanly against the
stub and against the real rc.9 installs, and `External.tsx` against the stub
and its ambient `ext-lib.d.ts`, which has no runtime package (`tsc --noEmit`, TypeScript 5.9.3,
`strict`, `jsxImportSource: "@solidjs/web"`, `skipLibCheck` because
`solid-js@2.0.0-rc.9`'s own `types/index.d.ts` fails to resolve five of its
re-exports).
