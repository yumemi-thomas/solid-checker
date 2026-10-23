# WASM parity gate

`make wasm-parity` (a `make verify` step) checks the owned check that
`solid-checker-wasm/owned.mjs` ships. The package is packed, then installed
in isolation, and run against the native ordinary check with
the ordinary check over the same frozen bytes at the same absolute
paths. The gate replaces the S1 manual prototype
(`packages/wasm/prototypes/owned-check.mjs`), whose cases and assertions it
carries.

## What runs where

1. **Make builds the inputs.** It builds the package
   (`build-wasm-package`: the Node-API module, then the two owned reactors)
   and the native checker. It also runs the `typefacts` bridge test with
   `TYPEFACTS_BRIDGE_FRAMES`, which records a Rust session's update frames
   over the native producer. The gate builds nothing. A missing input,
   toolchain or artifact fails the gate. Nothing is cached.
2. **The gate packs the package and installs it in isolation.**
   - It runs `bun pm pack` and extracts the tarball into
     `<scratch>/consumer/node_modules/solid-checker-wasm`.
   - It copies in the declared production closure (dependencies and
     non-optional peers, recursively) from the frozen local install. Each
     version is checked against both lockfiles and each tree is digested.
     Registry integrity is recorded, not verified. Pinning the direct
     dependencies does not make the closure immutable.
   - No ancestor of the install, the case trees or the recorded tree may
     hold `node_modules`, `package.json`, `tsconfig.json`, `jsconfig.json` or
     `.solid-checker`.
3. **The driver runs from the install, under both engines.**
   - **Node** runs under `--permission`. It may read only the install, the
     case trees and the recorded frames, and write only its results
     directory. It may start a worker but no process. A `module.registerHooks`
     preload logs every resolved URL, in the worker too.
   - **Bun** runs without a permission model. The Bun row shows engine
     behaviour, not isolation.
   - Ordinary cases use only `createOwnedChecker` and `mountDirectory`.
     Negative controls attach the package's own client (`owned-client.mjs`)
     to `fault-worker.mjs`. That worker imports the package's own `serve` and
     transport from the install. It injects faults only at the producer's
     exports or around the production imports.
4. **Comparison.** Each answering case's owned snapshot must equal native's
   as JSON in field order, and its notes must equal native standard error.

## Expectations

`expectations.json` holds the legacy cases' exact values. They were
transplanted from the S1b consumer witness
`p5-consumer-checkpoint/owned-check/witness-2026-10-15T11-53-39-184Z`
(`summary.json` sha256 2b98734e…) and are not learned from any run of this
gate.

The only rendering is literal substitution: the case root becomes `<root>`,
and its working-directory-relative spelling in `tsc` output becomes
`<root-relative>`. No errno or message normalization is applied. A
difference is a failure that needs a disposition, never an update.

The case list is declared, and the executed list must equal it under each
engine.

One row is regenerated rather than checked: `w7BadHandshake` pins the handshake
text, which names the protocol number and the schema digest. A protocol bump
changes both, and the forged digest (the same schema with its last hex digit
altered) follows the schema. That row is updated with the bump, in the same
commit, as `635ed74d4` did for protocol 78 to 79. A row that moves for any other
reason is still a failure.

**Answering cases.** W1–W4, P1 ×3, the P1 importer spellings ×3,
install-absent and W6 fresh. Each pins:
- the request and argv;
- native status, notes, finding rows, metrics and premises;
- the owned snapshot status and notes;
- the producer ledger (opened, closed, exchanges, held results);
- the producer call count;
- `tsc` where the witness ran it.

**Refusal rows.** These keep both raw outputs. Each pins native exit and
every stderr line, the owned error, and the exact call list:

| Case | Native | Owned | Calls |
|---|---|---|---|
| w7-missing-project | `os error 2` | `os error 44` | none (errno hold) |
| w7-open-failure | the producer's two lines plus the lost pipe | the producer's open error | one failed open (startup hold) |
| refuse-dangling-catalog-root | the provenance refusal | the producer's tsconfig parse refusal | one failed open (stage hold) |
| refuse-dangling-plain-catalog | the same message as owned | — | 8 exchanges |
| refuse-dangling-installation | the same message as owned | — | 7 exchanges |
| w4-omission | the same message as owned | — | 4 exchanges |

**W5.** Eight forged answers on one producer and checker pair: truncated,
trailing, garbage, wrong request, wrong generation, wrong project, wrong
state token, and an exchange answered by a decoy program.
- Each must match the witness's status, error, ledger and decoy close.
- Each is followed by a clean check equal to native.
- The producer's whole program sequence must match the witness, with
  nothing left open.

**W7 handshake.** A forged handshake byte.

**W6 replay.** The recorded update frames replayed into the packaged
producer. Every answer must be byte-equal except analyze `timings`, with the
witness's operation sequence.

**Package controls.** These are new in S2 and pinned by the gate itself:
- reuse after an answer and after a refusal;
- close with one check in flight and one queued;
- poison by a trap, the worker's exit, a leaked program and an uncaught
  error, settling the queued check and every later one;
- a result whose answer is invalid JSON, the wrong shape or an unknown
  status (forged at the fault worker's port), settling the active, queued and
  later checks and stopping the worker;
- a short copy destination refused and cleaned up, then reuse;
- a corrupted artifact refused by digest, and a corrupted module refused at
  start;
- two concurrent checkers;
- mount refusals: a link leaving the root, a FIFO, a non-normal path, a
  duplicate, an undeclared parent, an unsupported kind;
- mount capture: a path or link-target getter whose later answer leaves the
  root, and a getter that mutates other entries, the array and an earlier
  buffer during capture. Each property is read once, and only what was read
  and validated is mounted: the answers equal the plain mount's, `/outside`
  is absent, and the captured records are checked exactly. A caller's buffer
  written after the checker starts changes nothing;
- an unknown `createOwnedChecker` option;
- project spellings through a link and `..`, equal to native with the same
  spelling, plus a relative project, refused;
- an unsupported installed `solid-js` (SC9013), answered equal to native with
  no producer call;
- the Node worker's spawn and its write outside its results tree, both
  denied. Reads are granted everywhere, since directory mode reads the host as
  native does (S3d).

**Directory mode (S3b).** Every answering and refusal case, the three project
spellings and SC9013 run a second time through
`createOwnedChecker({ directory })`. Each must give exactly the mount's
answer text and producer traffic, refusals included, so the native and
witness comparisons above carry over. Two link controls are pinned:
- a link leaving the root that no check reaches does not refuse;
- a check that reaches it is rejected with the store's reason.

**Project spelling (S3c, S3d).** One directory tree, `g3`, has the links and
shapes beside its root, and every spelling also runs natively as itself:
- equal to native, with the canonical spelling's producer traffic: an
  outside link into the root, a two-link outside chain, `alias/../project`,
  and the scratch through macOS's own `/var` or `/tmp` link (required on
  macOS; the Node driver may read that one link); and, since directory mode
  reads the host's namespace (S3d), a link to a project outside the root;
- exact refusals with no producer call, beside native's own error: a missing
  path through the link, a dangling link, a file as a directory, a link loop;
- recorded only, as held contrasts (A4): a case-variant link target, and a
  case variant inside the root;
- a mount given the outside spelling keeps `ENOENT`;
- two permission trees, each denial checked real before any run and lifted
  only after both engines' checks and validation: an outside directory
  without search permission (refused `EACCES`; native fails too; the gate's
  observing worker reads back the store's own record at the denied path),
  and a search-only parent inside the root (equal to native; the observing
  worker shows its listing was never taken).

**Inputs above the directory (S3d).** Seven trees put what native discovery
finds beside the project instead of in it, each run natively and in
directory mode, each whole case tree manifested before the runs and checked
after both engines:
- `a2-hoisted`: the project's install hoisted above it (real rc.13 types and
  a real violation): equal;
- `a2-unsupported-major`: a Solid 1.x runtime above it: SC9013 in both;
- `a2-local-override`: a 1.x runtime above it and the project's own rc.13
  install: the nearest wins in both;
- `a2-absent`: no install anywhere: equal;
- `a2-store-link`: the hoisted `node_modules` a link into a store beside it:
  equal;
- `a2-dependency-source` and `a2-dependency-missing`: a selected dependency
  source hoisted above the project, and the same with its imported module
  removed: equal, or the same failure.

`mount:a2-hoisted` and `mount:a2-unsupported-major` record the closed mount
beside them: it sees nothing above its root, by contract.

The pre-S2 entry (`require`, `import`, `checkSync`) runs from the same
install under both engines. The packed `main`, `browser` and `types` fields,
the absence of an `exports` map, and every pre-S2 packed file are checked.

**Path semantics (S3a).** The owned worker's WASI runtime is the package's
vendored `@tybys/wasm-util` copy, and the Node run must resolve it from the
install.

- **The controls:** `path-semantics.mjs` builds one disk tree of shapes:
  - `..` at depths 1–4;
  - `..` after a symlinked directory;
  - relative, absolute and chained links;
  - a dangling link and a loop;
  - `again/again/f`;
  - `f/.`, `f/..`, `missing/..`;
  - trailing slashes, in the request and in stored link targets;
  - a pnpm-like store inside the tree.

  One control guest (`guests/path-control`, raw WASI calls and Rust std in
  Rust) runs its rows over the mounted copy. The rows cover every repaired operation crossed with the lookup
  flags, dirfds (preopen, opened directory, file, closed), mutation flags
  (`CREAT`/`EXCL`/`TRUNC`/`APPEND`, mkdir/rmdir/unlink), readlink buffers
  (0, 1, len−1, len, len+16 with sentinels) and Rust std rows.
- **The reference:** `path-host.mjs` computes host POSIX on the same tree,
  always under Node.
- **What must hold:** every owned row equals the host, except the four
  overrides pinned with reasons in `path-expectations.json` (WASI-level
  ENOTCAPABLE and EBADF, unchanged ENOSYS, the
  retained final-slash drop). The upstream runtime must fail exactly the
  pinned repaired rows and match the rest.
- **The fixture:** `dotdot-fixture` and `dotdot-missing` run the
  `dependency-source-dotdot` fixture, and the same tree with its `../`
  module removed, against native. `tsc` must be silent on both, and on the
  published rc.13 types.

## Evidence

Each run writes `rust/target/wasm-parity/<timestamp>/`:
- `summary.json`, recording images, engines, the packed tarball and
  manifest, and the dependency closure;
- the raw native stdout and stderr of every native run;
- each engine's plan, driver output, URL log and old-entry output;
- `paths/host.json` and `paths/host-request.json`, the host POSIX answers
  every path-semantics row was compared with.

The scratch tree is removed only when the run passes.

## Not claimed

- Native ancestor discovery above a mount root.
- Links leaving the root, including pnpm stores.
- Resolved symlink-chain equivalence beyond these cases.
- Modes and permissions.
- Bounded memory: an empty ledger proves no leak of programs, not of bytes.
- Browser, Windows and other Node or Bun versions.
- `rule-options.json`.
- The full fixture, corpus, oracle and phase-19 breadth, which is S3's.
