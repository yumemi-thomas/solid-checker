# solid-checker-wasm

WebAssembly entry point for StackBlitz, WebContainers, and browser workers.
Unlike the CLI, this API never starts a child process or reads source files
from disk.

```ts
import { checkSync } from "solid-checker-wasm"

const snapshot = JSON.parse(checkSync(JSON.stringify({
  projectId: "/workspace/example/tsconfig.json",
  generation: 1,
  sources,
  typeFacts
})))
```

`sources` contains `{ path, source, compilerOptions? }` objects. `typeFacts` is
the TypeFacts v3 closure for those exact sources and generation. Keeping the
TypeScript host outside the Rust module lets StackBlitz use a browser-native
TypeScript engine without process spawning; the Rust module still runs the
same Oxc, Solid compiler, reactive IR, and solver path as the native CLI.

The Cargo crate enables both `dialect-v1` and `dialect-v2` by default. A host
shipping a version-specific payload can disable default features and enable
only one; requests naming an omitted dialect receive the normal unknown-id
error. CI compiles both single-dialect variants so shared infrastructure cannot
grow an accidental dependency on either compiler or catalog.

Package contracts are bound to the installed package an import specifier
actually resolves to, and this entry point has no Type Facts session of its own
with which to ask the compiler where that is. The optional `resolvedImports`
field carries the host's own answer, and a host that supplies it gets
identity-bound contracts: a specifier that resolves outside the installed
directory a contract was classified against -- a tsconfig `paths` entry mapping
a bare specifier onto project source while a package of that name is installed
beside it -- has the contract refused, and the import becomes uncertifiable
exactly as an unknown package's would.

Two things about a supplied row are checked rather than trusted, because a
wrong row is otherwise indistinguishable from a project whose contracts
genuinely do not apply. `startByte`/`endByte` are UTF-8 **byte** offsets into
the source this same request carries, and the source at that span must read as
the specifier — TypeScript reports positions in UTF-16 code units, so a host
that forwards them unconverted is correct for ASCII and silently wrong after
the first non-ASCII character. And `resolvedPath` is empty exactly when
`resolution` is `"unresolved"`; a row claiming `unresolved` is *accepted* by
contract binding, so labelling resolutions the host did not perform is the one
mistake here that would fail open. Either violation is a hard error naming the
row, not a refused contract.

A request that omits the field binds package contracts by specifier name, which
is what this adapter has always done. That is a stated limitation of the
adapter, not a weaker analysis of the same request: nothing is silently
upgraded, and nothing about the older behavior changes. When the field *is*
supplied it is all-or-nothing per specifier -- a file it omits has no answer,
and a contract is refused there rather than quietly falling back to the name,
because a partially trusted answer is the one shape that could certify from a
resolution nobody reported.

Rule configuration is not yet transported through the WASM `CheckRequest`.
The adapter cannot read `.solid-checker/rule-options.json` and exposes no rule
override fields, so it uses catalog defaults: all `prefer-*` rules run. Unlike
CLI, daemon, and ESLint callers, WASM callers cannot yet opt out; that requires
a future rule-options channel rather than an adapter-only list. The fact demand
planner follows the defaults and therefore requests the array-shape facts used
by `prefer-for`.
