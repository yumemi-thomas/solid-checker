# solid-checker-wasm

WebAssembly entry point for StackBlitz, WebContainers, and browser workers.
Unlike the CLI, `checkSync` and `planSync` never start a child process or read
source files from disk. The Node and Bun owned check is described under
[Owned check](#owned-check).

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

## Owned check

`solid-checker-wasm/owned.mjs` checks a project from its files alone, with no
host-supplied Type Facts. It is available on Node and Bun only, with no
browser entry. One worker thread runs two WebAssembly reactors over one
in-memory copy of the project:
- the canonical Type Facts producer, the same Go source as the native
  `solid-typefacts` built for `wasip1`;
- the checker.

The package ships both in `owned/`, with a manifest of their digests. No
source checkout, native binary or child process is involved.

```js
import { createOwnedChecker, mountDirectory } from "solid-checker-wasm/owned.mjs"

const root = "/workspace/app"
const checker = await createOwnedChecker({ mount: mountDirectory(root) })
const result = await checker.check({ projectId: `${root}/tsconfig.json` })
// { ok: true, snapshot, notes, json, transport } or { ok: false, error, json, transport }
await checker.close()
```

**What it answers.** A check answers exactly what the native ordinary check
answers with `--no-package-contracts` over the same files. Native defaults
and contract consumption are unchanged. No package contract is read from any
tier, so an import a tier covers stays an open obligation. The request takes
the native options: `runtime`, `dependencySources`, `noDependencySource`,
`windowRealm`, `dialect`, `presets`, `enableRules`. `projectId` must be
absolute, and is canonicalized against the mount exactly as native
`--project` is.

**The mount** is a frozen copy:
- regular files, directories and symbolic links at absolute normalized POSIX
  paths under one root;
- every parent a declared directory;
- every link target resolving inside the root.

Everything is validated before anything is written, and file bytes are copied
when `createOwnedChecker` is called. `mountDirectory` snapshots a directory by
its real path, records links without following them, and throws on an
unreadable entry, any other kind of entry, or a link leaving the root.

The mount has these limits:
- **Ancestors:** nothing above the root exists in the mount, so a native
  check's discovery above it has no counterpart.
- **Links:** links leaving the root, such as pnpm stores, are refused.
- **Modes:** modes and permissions are not represented.
- **Later changes:** a change on disk after the mount is taken is not seen.
  Create a new checker to see it.

**Lifecycle.** Checks run one at a time, in call order. A request's JSON is
fixed when `check` is called.
- **Refusal:** a refusal (`ok: false`) leaves the checker usable.
- **Poison:** a worker error or exit, a trap, or a producer program left open
  rejects the current check and every queued one, stops the worker, and
  rejects every later check. Nothing is restarted.
- **Close:** `close()` rejects pending checks and stops the worker.

**Trust.** The digest check detects a corrupted or mismatched artifact in a
package trusted as a unit; it does not authenticate it. The producer
handshake checks compatibility only. Neither reactor can read the host disk:
both see only the mount. Certification is not served.

**Tested:**
- Node 24.11.1 and Bun 1.4.0 on macOS arm64, through `make wasm-parity`
  (scripts/wasm-parity/README.md);
- a smoke subset in CI.

**Not yet established:** Windows, browsers, other runtime versions, bounded
memory, and `rule-options.json`. A check's producer memory grows with reuse.
