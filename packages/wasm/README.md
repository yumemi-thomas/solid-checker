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

No package contract is consumed, and `acceptedContracts` is no longer part of
the request. This request carries no package
files and no import attestation, so source analysis cannot run here: every
runtime import or re-export of a bare specifier (Solid core and `node:`
built-ins excepted) keeps an open requirement, reported as
`package-behavior-unavailable` (`SC9005`) where it matters. For package
behaviour from source, use the [owned check](#owned-check). `installedPackages`,
`exportConditions` and `bundledContracts` are still accepted, so an older
host's request decodes, and have no effect.

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
answers over the same files. No package document is read, so a package import
stays an open obligation unless its source is analyzed. The request takes the native
options: `runtime`, `dependencySources`, `windowRealm`, `dialect`, `presets`, `enableRules`. `projectId` must be
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
  below), `readlink`, a directory listing taken between two `lstat`s, and a
  file's bytes, read once with `fstat` before and after. Every later answer
  comes from these records.
  Paths resolve as a kernel resolves one syscall, with at most 32 link
  substitutions. Listings are ordered by name bytes on every engine.
- **The identity.** For every file and link, wherever it lies, and for every
  directory inside the directory the checker was created for, the identity is
  `dev, ino, mode, nlink, size, mtimeNs, ctimeNs`. For a directory outside it
  -- `/`, the ancestors, a store a link leads to -- it is `dev, ino, mode`
  only: such a directory is shared with the rest of the host, and its
  timestamps, size and link count move with unrelated activity beside the
  paths a check walks through it (`$TMPDIR`, a home directory). The entries a
  check reaches in it are their own records, and a listing of it is its own
  record, so a replaced directory, a changed reached entry and a changed
  listing are all detected. A change to only its timestamps, size or link
  count is not detected, and those fields are answered as first recorded,
  not as current. No consumer in the two reactors is known to read them; that
  is a reading of today's sources, not a guarantee for future ones.
- **The preopen.** Each WASI runtime opens `/` once when it is created, as the
  anchor every path resolves from; it never reads that descriptor (listings
  are taken by path). Native analysis needs only search permission on `/`, so
  that one open is answered from `/`'s recorded `lstat` without a host open.
  Any other open or listing of `/`, by either reactor, is a real host
  operation, denied as the host denies it.
- **Namespace.** It is the host's. Native discovery reads above the project
  without bound: the nearest `node_modules/solid-js` selects the dialect,
  TypeScript looks up `node_modules` and `package.json` in every ancestor, host
  inference looks for a workspace root, and `extends` or `paths` may name files
  above the project. So any path a reactor reaches -- inside the directory,
  above it, or through a link leading anywhere -- is answered from its own
  host records, `/` and the directory's ancestors included, and nothing is
  answered absent unless the host says so. A hoisted install above the
  directory, a pnpm store outside it and a project path spelled through a host
  link (macOS `/tmp` or `/var`) are therefore read as native reads them.
  `directory` names the canonical directory the checker is created for: its
  identity is checked at start and after every check, and it anchors the
  precondition. A mount, by contrast, is closed: nothing above its root
  exists in it.
- **Project spelling.** Native `--project` is canonicalized by the host's
  `realpath(3)`; the checker canonicalizes it through WASI over the same host
  namespace, the same physical walk. One difference is held: on a
  case-insensitive volume `realpath(3)` rewrites a name to its directory
  entry's spelling, and the WASI walk keeps the request's (or a link target's)
  spelling, so the output may spell such a path differently.
- **Precondition.** The caller keeps the host namespace from `/` through the
  directory, and everything a check reaches -- inside the directory or not --
  unchanged while the checker lives. The store issues ordinary path-string
  calls, so it is not a sandbox: it cannot stop the operating system
  following a link someone swaps in.
- **What is detected.** After every check, every record the checker ever
  captured is performed again and compared exactly: identity (as above), link
  target, listing, and each recorded error. Any difference rejects that check and
  every later one, and the answer is discarded. When every record validates,
  there was an instant between the check's last read and the start of
  validation at which the host held every recorded byte, identity, target,
  listing and error. This rests on the filesystem changing a file's identity
  whenever it changes. A rewrite restored to the same size, `mtime` and
  `ctime` (for example within one timestamp tick on a filesystem with coarse
  timestamps) is not detected. Stat fields outside the identity (`atime`,
  which reading may itself change; `blksize`; `blocks`) are captured once and
  are not part of that claim. The answer is not claimed current at delivery.
- **What is refused.** These are input faults: a kind other than file, directory or link,
  a name or link target that is not UTF-8, any host error other than the
  expected ones, and any write, wherever the path lies. The expected errors
  (`ENOENT`, `ENOTDIR`, `EACCES`, `EPERM`, `EINVAL` from `readlink` of a
  non-link, `ELOOP`) are recorded and answered as the
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
