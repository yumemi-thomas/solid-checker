# Owned-check prototype

`bun run --cwd packages/wasm prototype:owned-check` is manual evidence for S1
of the WASM host protocol. It is not a gate, a CI step, or a maintained or
packaged WASM provider.

## What it runs

Two reactors share one mounted namespace:

- the canonical Type Facts producer, `./apps/solid-typefacts` built for
  `GOOS=wasip1 GOARCH=wasm` with `-buildmode=c-shared` and the native
  producer stamp's build id (`GOPROXY=off`, `GOTOOLCHAIN=local`);
- the checker, `solid-checker-wasm` built for `wasm32-wasip1` with
  `--no-default-features --features reactor,dialect-v2`, calling
  `run_owned_check`.

The harness never runs Cargo. Build the checker reactor beforehand with the
same certification pins Make's `CERTIFICATION_ENV` computes for the native
build, then pass it with `--checker-wasm` (the default is
`rust/target/wasm32-wasip1/release/solid_checker_wasm.wasm`).

The native reference is `rust/target/debug/solid-checker-rust` (override with
`--native-checker`). It is run with:

- `--format json --no-package-contracts`;
- the project's `.solid-checker/runtime.json` arguments;
- the daemon off;
- `bin/solid-typefacts` as the producer.

The run fails if that producer's stamp is stale against the producer source
identity. `run_owned_check` always selects the same consumption, which
[the CLI README](../../cli/README.md#checking-without-package-contracts)
describes: no tier's claims are read, and an import a tier covers stays open.

## The host ABI

The checker imports six functions from `solid_typefacts`; the harness answers
them from the producer's exports, copying bytes directly between the two
linear memories during each call:

| Import | Producer export |
| --- | --- |
| `open(envelope, length, handle_out)` | `typefacts_open` on the CBOR open envelope; the handle goes to `handle_out` |
| `handshake(handle)` | `typefacts_handshake` |
| `exchange(handle, frame, length)` | `typefacts_lifecycle(handle)` on one request frame |
| `result_length(handle)`, `copy_result(handle, destination, capacity)` | the one held result, copied and released |
| `close(handle)` | `typefacts_close(handle)` |

A nonzero status means the held result is a UTF-8 message; a failed open holds
its message under handle 0. Lifecycle bytes are never parsed by the host,
except where a fault case forges one (below).

## Mounted namespace

Each case copies its project into a fresh, realpath-resolved scratch tree
outside the repository (default
`/tmp/sc-scratch.noindex/p5-owned-producer-roundtrip/owned-check`, override
with `--scratch`) and freezes it. One `@napi-rs/wasm-runtime` memfs holds
exactly those bytes at the same absolute paths, preopened as `/`, and both
instances read it. The disk tree is re-hashed after the native run, and each
case records its mount manifest.

## Cases

- **W1** — real typings: the audited Solid 2 install the tsc oracle
  provisions, its strict compiler options, and source text from oracle
  keystone cases (TypeScript silent, checker reports). `tsc --noEmit` must be
  silent. Exactly `SC1004` (violation) and `SC1003` (uncertifiable).
- **W2** — a clean project on the same install (certified), and
  `package-contract-absent` (SC9005 from the requirement-only boundary).
- **W3** — `dependency-source-program` and
  `dependency-source-integrity-r2-visible-js`: installed JavaScript analyzed as
  dependency source.
- **P1** — the review's wrong-certification counterexample and its controls:
  - `package-unknown-callback-consumer`: a local reactive callback passed to an
    installed package that names no Solid dependency, behind a catalog entry.
    Exactly one `SC9005` stays open at its import, with both obligations and
    the Disabled note, and no violation.
  - `package-contract-paths-shadow`: the `paths`-mapped specifier stays
    project source; only the installed import is open.
  - `package-contract-install-shapes`: the ambiently typed install and the
    subpath under an unnamed manifest stay open; the `@types`-redirected
    specifier raises nothing.
  - The same callback with the catalog importer spelled absolute, through
    `node_modules/..`, and through a symlinked directory: each keeps its
    canonical key and both obligations on both sides.
  - A dangling plain catalog, and a covered unresolved import whose
    installation is a dangling link, refuse with the same message on both
    sides and no snapshot. With that package genuinely absent, the check
    answers and the import carries no requirement.
- **W4** — one input missing identically on disk and in the mount:
  - a module the selected dependency source imports, with its declarations
    intact so `tsc` output does not change: both sides refuse with the same
    message;
  - a project source another source imports (TS2307): the omitted file's
    finding goes and nothing is reported at the missing import.
- **W5** — forged producer answers on a shared pair of instances: truncated,
  trailing, garbage, wrong request id, wrong generation, wrong project, wrong
  state token, and an exchange answered by a different program the same
  producer owns.
  - Each must refuse with no snapshot and leave no held result.
  - The same instances must then answer a clean check identically.
  - **Program ledger:** every program the producer instance issues is
    recorded, whether through a check's imports or the harness's own decoy
    open. The decoy is closed in a `finally`. After every fault and its clean
    follow-up, no program may remain open.
- **W6** — changed frozen trees through fresh checks: W1 with `Gate` returning
  once loses exactly that finding. With `--update-frames <dir>`, it also
  replays a Rust `Session` update recorded over the native producer by the
  `typefacts` bridge test (`TYPEFACTS_BRIDGE_FRAMES=<dir>`) into the producer
  reactor; every answer must be byte-identical except analyze answers' own
  `timings`.
- **W7** — a missing project (refused before any open), a configuration the
  producer cannot open, and a forged handshake. Each refuses with no snapshot
  and leaves no program open.

Every non-refusal case compares the snapshots as JSON values in field order,
and the notes against native standard error. Every case also requires that no
held result and no producer program is left behind.

## Known differences

- **Dangling `.solid-checker` link:** natively, the checker refuses because
  its provenance cannot be established. Under WASI, the producer's own
  tsconfig parse refuses first. Both refuse with no snapshot; the messages
  differ.
- **Errno numbers:** a missing project is `os error 2` natively and
  `os error 44` under WASI.
- **Producer open failures:** natively the producer process dies opening the
  project and the checker reports the lost pipe; the reactor returns the open
  error itself. Both refuse.
- **Project path:** WASI has no realpath. `run_owned_check` refuses a project
  path with `.` or `..` components, and otherwise trusts the host to name the
  project by its canonical path.

## What it does not show

- **Maintained parity:** it is not a maintained gate or packaged WASM
  (`wasm32-wasip1-threads`, napi worker, browser filesystem). It does not show
  parity over every fixture and corpus app.
- **Filesystem fidelity:** it does not cover symlinks, casing, permissions or
  large-memory behavior.
- **Retained sessions:** `run_owned_check` is a fresh check. Retained update
  is shown only through the replayed frames.
- **Authority:**
  - The host is trusted to forward the producer's bytes unchanged and to keep
    the mount immutable.
  - Nothing here authenticates either, and a host that forges every byte
    consistently is not detected.
  - A bridged session refuses certification.
- **Platform lint:** `clippy` for `wasm32-wasip1` with
  `reactor,dialect-v2` is not green. It stops on 5 errors (4 dead-code, 1
  `drop` of a non-`Drop` value), all in the unchanged
  `contract_certification/probe_harness` sources. The owned module itself
  reports none.

Outputs, built images, raw frames and the frozen trees stay under the scratch
directory.
