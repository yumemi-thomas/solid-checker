# WASM parity gate

`make wasm-parity` (a `make verify` step) checks the owned check that
`solid-checker-wasm/owned.mjs` ships. The package is packed, then installed
in isolation, and run against the native ordinary check with
`--no-package-contracts` over the same frozen bytes at the same absolute
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
- the Node worker's spawn and out-of-tree read, both denied.

The pre-S2 entry (`require`, `import`, `checkSync`) runs from the same
install under both engines. The packed `main`, `browser` and `types` fields,
the absence of an `exports` map, and every pre-S2 packed file are checked.

## Evidence

Each run writes `rust/target/wasm-parity/<timestamp>/`:
- `summary.json`, recording images, engines, the packed tarball and
  manifest, and the dependency closure;
- the raw native stdout and stderr of every native run;
- each engine's plan, driver output, URL log and old-entry output.

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
