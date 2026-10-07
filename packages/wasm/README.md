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

The Cargo crate enables `dialect-v2`, which is the default and currently the
only dialect (ADR 0110). A host shipping a version-specific payload can disable
default features and enable a subset; requests naming an omitted dialect
receive the normal unknown-id error. CI compiles the single-dialect variant so
shared infrastructure cannot grow an accidental dependency on a compiler or
catalog it does not declare.

Package behavior enters WASM only through `acceptedContracts`. Each entry pairs
a stable main schema-version-1 document's exact JSON text, its proof-issued
receipt's exact JSON text, and the
host's full Phase-7 `ResolvedImport`: exact package identity, independently
selected runtime and declarations, resolution traces, export identities, and
canonical dependency closure. Rust validates and normalizes all three before
analysis sees a semantic claim.

The document and receipt are strings rather than parsed JSON values because a
receipt binds the document's exact bytes; parsing and reserializing would
silently change that wire identity.

The field is optional only because a project may have no accepted external
behavior. Omitting an import never falls back to package-name matching; that
import remains uncertifiable. Hosts should use the same artifact-acquisition
adapter as the Node CLI and must not manufacture closure digests from contract
bytes.

`acceptedContracts` is the only way package behavior enters. The compiled-in
certified tier this build once carried is retired (ADR 0228), and the authored
tier is not served here: it admits on the Solid runtime installed beside the
package, which a host with no filesystem cannot show. `installedPackages`,
`exportConditions` and `bundledContracts` are still accepted, so an older host's
request decodes, and have no effect.

Rule configuration is not yet transported through the WASM `CheckRequest`.
The adapter cannot read `.solid-checker/rule-options.json` and exposes no rule
override fields, so it uses catalog defaults: all `prefer-*` rules run. Unlike
CLI, daemon, and ESLint callers, WASM callers cannot yet opt out; that requires
a future rule-options channel rather than an adapter-only list. The fact demand
planner follows the defaults and therefore requests the array-shape facts used
by `prefer-for`.
