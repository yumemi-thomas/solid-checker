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
filesystem — an in-memory copy of the project (`mount`) or a host directory
read on demand (`directory`):
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
handshake checks compatibility only. With a `mount`, neither reactor can read
the host disk: both see only the mount. With a `directory`, both reach the
host only through the read-only directory store, and only under the
directory. Certification is not served.

### Directory mode

```js
const checker = await createOwnedChecker({ directory: "/workspace/app" })
```

A mount copies a whole tree into memory before the first check, which does
not fit large projects (a check typically reads a few percent of the tree,
`node_modules` included). Directory mode reads only what a check reaches.

- **Options.** `createOwnedChecker` reads its option keys once and the one
  present value once; `mount` and `directory` are exclusive. `directory` must
  be a path string naming a directory other than `/`; it is resolved to its
  real path once, on the calling thread.
- **The store.** Both reactors share one store. Each host read is recorded
  once, keyed by operation and physical path: `lstat` (with the identity
  `dev, ino, mode, nlink, size, mtimeNs, ctimeNs`), `readlink`, a directory
  listing taken between two `lstat`s, and a file's bytes, read once with
  `fstat` before and after. Every later answer comes from these records.
  Paths resolve as a kernel resolves one syscall, with at most 32 link
  substitutions. Listings are ordered by name bytes on every engine.
- **Namespace.** It is the mount's: nothing above the directory exists except
  the directories leading to it, each listing only the next one. Anything
  else outside it is absent to both reactors, with no host read. The one
  exception is the request's project spelling (next item), and it is never
  visible to a reactor's own calls.
- **Project spelling.** Native `--project` is canonicalized by the host's
  `realpath(3)` before anything reads it; the checker does the same through
  WASI, inside the namespace. A spelling that passes through a host place
  outside it -- macOS `/tmp` or `/var`, or a link of the caller's to the
  project -- fails there with `NotFound`. Only then the checker asks the
  store once, through its `solid_owned_host` imports (`owned-projection.mjs`),
  what the spelling names when those outside places are read from the host:
  - outside places reached from the spelling itself, or from links that are
    themselves outside, are read with `lstat` and `readlink` only, recorded and
    revalidated like every other record; a link inside the directory that
    leads outside is still the input fault below;
  - a path that ends inside the directory is used only if every component
    below the directory is an exact entry of its parent's listing, the
    directory's own listing first. Those listings are recorded and revalidated
    too. Native canonicalization needs search permission only, so a listing
    that fails (`EACCES` on a search-only parent, for example) is refused as
    `listing … to confirm its spelling failed`, where native answers;
  - the projected path is native's under the further assumption that the host
    applies no other name canonicalization: case-insensitive names, Unicode
    normalization and firmlinks are not resolved. A listing without the exact
    name is refused as `not spelled as its directory entry`. A case variant
    *inside* the directory is answered as WASI canonicalizes it, which may
    keep the request's spelling where native would not;
  - a path that ends outside the directory is refused as `outside the
    directory root`, a missing or dangling one as `ENOENT`, and `ENOTDIR`,
    `EACCES`, `EPERM` and a link loop as those errors.

  A mount has no host to ask: an outside spelling there stays `ENOENT`.
- **Precondition.** The caller keeps the host namespace from `/` through the
  directory, everything a check reaches, and the host names and link targets
  outside the directory that a project spelling passes through, unchanged
  while the checker lives. The store issues ordinary path-string calls, so it
  is not a sandbox: it cannot stop the operating system following a link
  someone swaps in.
- **What is detected.** After every check, every record the checker ever
  captured is performed again and compared exactly: identity, link target,
  listing, and each recorded error. Any difference rejects that check and
  every later one, and the answer is discarded. When every record validates,
  there was an instant between the check's last read and the start of
  validation at which the host held every recorded byte, identity, target,
  listing and error. This rests on the filesystem changing a file's identity
  whenever it changes. A rewrite restored to the same size, `mtime` and
  `ctime` (for example within one timestamp tick on a filesystem with coarse
  timestamps) is not detected. Stat fields outside the identity (`atime`,
  which reading may itself change; `blksize`; `blocks`) are captured once and
  are not part of that claim. The answer is not claimed current at delivery.
- **What is refused.** These are input faults: a link whose target leads
  outside the directory, as soon as that target is read (by the store's own
  resolution or by the guest's `readlink`, since a guest that followed it
  itself would otherwise be told "absent" where the host has a file; a link
  no check reads is not refused), a kind other than file, directory or link,
  a name or link target that is not UTF-8, any host error other than the
  expected ones, and any write. The expected errors (`ENOENT`, `ENOTDIR`, `EACCES`, `EPERM`, `EINVAL`
  from `readlink` of a non-link, `ELOOP`) are recorded and answered as the
  host gave them. An open that would create, truncate or append is refused
  before any host access; the host file is only ever opened read-only. The
  first input fault is final: it rejects the current, queued and later checks
  with the store's own reason, whatever the guest did after it. A store that
  cannot start fails `createOwnedChecker`.
- **Memory.** The store holds each file a check reads, once, until the checker
  is closed. A reused checker accumulates the union of its checks' reads. No
  general bound is claimed.
- **Links.** The store's substitution bound is the vendored runtime's own
  (`MAX_SUBSTITUTIONS`, 32), and a longer chain is answered `ELOOP` through
  the runtime's own `WasiError`, as a guest path call is.

### Addresses above 2 GiB

A WebAssembly `i32` reaches JavaScript signed, so a reactor address at or above
2 GiB arrives negative. Every import either reactor declares is wrapped by its
signature (`owned-boundary.mjs`): arguments are normalized as the unsigned (or,
for `fd_seek`'s delta, signed) values they encode; every memory range the call
addresses, including each iovec, must lie inside memory or the call answers
`EFAULT` without reaching the runtime; and a malformed value is refused, never
coerced. An import the table does not know refuses the module. The transport
and the worker normalize the reactors' exported pointers and lengths the same
way. The vendored runtime's synchronous `fd_pread`, `fd_pwrite`, `fd_read`
and `fd_write` decode each iovec's buffer address unsigned (C-vendor; see
`vendor/wasm-util/NOTICE`); the published 0.10.3 runtime reads such an
address relative to the end of memory, silently. Capacity beyond what the
measured projects needed (about 2.5 GB of producer memory) is not certified;
wasm32's 4 GiB is the inferred limit.

**WASI runtime.** The worker runs the package's own copy of the pinned
`@tybys/wasm-util` 0.10.3 WASI runtime (`vendor/wasm-util`; see its NOTICE,
LICENSE, UPSTREAM.json and `owned.patch`). Its path imports resolve paths
physically, as the host does: symlinks per component, `..` from the directory
actually reached. The upstream runtime's lexical `..` handling sent paths such
as `src/../lib/x.js` to the wrong file. Go's own guest-side lexical `..`
cleaning in the producer is not something a host layer can change.

**Tested:**
- Node 24.11.1 and Bun 1.4.0 on macOS arm64, through `make wasm-parity`
  (scripts/wasm-parity/README.md);
- a smoke subset in CI.

**Not yet established:** Windows, browsers, other runtime versions, bounded
memory, and `rule-options.json`. A check's producer memory grows with reuse.
