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

Rule configuration is not yet transported through the WASM `CheckRequest`.
The adapter cannot read `.solid-checker/rule-options.json` and exposes no
`presets` or `enableRules` fields, so it uses catalog defaults: all three style
preferences remain disabled. Unlike CLI, daemon, and ESLint callers, WASM
callers cannot yet opt into them; that requires a future rule-options channel
rather than an adapter-only list. The fact demand planner follows that
effective enablement, so WASM also omits the array-shape queries used only by
`prefer-for`.
