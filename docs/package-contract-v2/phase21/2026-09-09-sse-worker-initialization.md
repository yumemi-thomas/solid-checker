# SSE worker initialization: bounded investigation

The three requested worker cases are not yet certified. This investigation
establishes their exact retained artifacts, reproduces initialization and
message behavior in an explicitly simulated host, and rechecks preservation.
It does not add a production acceptance rule or represent a simulation as
ordinary consumer authority.

## Exact before and after

The baseline is the completed `2026-09-09-async-binding-full.json` report.
Fresh pinned release transactions requested `.`, `./worker`, and
`./worker-handler` under `import`, using authenticated registry-cache entries
only. Each exited 0 and published the same accepted set:

| Probe | Before | After | Transition |
| --- | --- | --- | --- |
| SSE 0.0.103, Solid 1 | `.`, `./worker` | `.`, `./worker` | partial → partial |
| SSE 1.0.0-next.2, Solid 2 floor | `.`, `./worker` | `.`, `./worker` | partial → partial |
| SSE 1.0.0-next.2, Solid 2 head | `.`, `./worker` | `.`, `./worker` | partial → partial |

Every previous main document is byte-identical. All three ordinary consumers
authenticated their accepted receipts and selected the exact cases. This
verifies the existing cases only: no worker-handler receipt was issued.
The missing entrypoint remains executable, not an asset or an inert module.

The worker runtime is `./dist/worker-handler.js`. Its SHA-256 is
`621f6e8177f43e9e9d8dbc182419f462fd3b0882dc55fdcc85196e68e4830c9e`
for 0.0.103 and
`52b88fd22ee4e3cffe531ad262c2626a462ff45860e8fb028a6c6c0a1901ff8d`
for both 1.0.0-next.2 probes. Identical worker bytes do not make the Solid 2
floor/head importer, dependency resolution or receipts interchangeable.
The private source-condition selections remain separately recorded as
unpublished targets; they are not silently substituted for dist artifacts.

[Exact case and preservation evidence](2026-09-09-sse-worker-investigation.json)
contains the prior/new mains, receipt digests and import bindings, expected
missing selections and their dispositions, plus the simulation observations.

## Reproducible observations, not proof authority

[The diagnostic script](prototypes/sse-worker-host.mjs) imports each unchanged
published worker module and its actual local SSE dependency through Node.
It supplies a simulated `self`, ports and EventSource; the latter never accesses
a network. This is not a browser or worker execution attestation, and Node's
dependency selection is not a worker dependency-selection premise.

All three simulations establish the same finite observations:

- Module load registers `message` and `connect` listeners but opens no SSE
  connection during registration.
- A dedicated message round trip returns data and explicit disconnect closes
  that simulated connection and removes its listeners.
- Shared responses return through the originating port; an absent port is
  ignored.
- A disconnect from another port with the same ID closes the first port's
  connection. The connection map is keyed by ID, not by channel.
- Two connects with the same ID replace the stored cleanup; disconnect closes
  the second connection and leaves the first open.

The three absent-`self` controls fail module loading with ReferenceError.
The scenarios do not prove every execution. In particular, do not extrapolate
per-port resource isolation or unconditional cleanup from a successful normal
round trip. These observed limits do not prevent a future accurate, restricted
registration claim; they prevent stronger lifecycle claims without additional
premises. No checker diagnostic or upstream package patch was added.

## Implementation boundary still open

`ModuleInitializationClaim` currently represents only `Inert`. Both worker
files perform `new Map()` and `self.addEventListener(...)`, and import
`./sse.js`. The imported module itself has initialization work and further
dependencies. A complete initialization proof must compose that exact closure;
the already accepted exported `makeSSE` contract does not prove module
initialization or the host's global bindings.

The ordinary `ResolvedImport`/receipt consumer has no worker-realm applicability
premise. A matching package hash, `import` condition, WebWorker declaration file,
or successful simulated host does not supply one. The existing controlled
execution lane intentionally cannot produce an ordinary acceptance token.
Putting a host hash in an unrelated receipt root would bypass this boundary.

The required production extension therefore spans three coordinated owners:

1. An artifact-case initialization effect model, distinguishing module load
   from later callbacks and recording unresolved effects explicitly.
2. Authenticated worker-host applicability: dedicated/shared realm, the exact
   global and prototype bindings used during initialization, entry loading and
   dependency selections. The ordinary consumer must reject a missing or
   mismatched host premise.
3. Replay of the exact initialization and callback claims being certified,
   with Map/EventTarget/MessagePort/EventSource behavior supplied by positive
   host contracts. Receipt and case-set publication must bind that evidence
   without replacing the existing accepted cases.

This is an architectural prerequisite, not another generator-routing fix.
The investigation deliberately leaves the current refusal in place rather
than introduce `effectful` as a label that purports to prove arbitrary effects.
No ownership permission request is pending; the unresolved work is the proof
and applicability implementation itself.

## Validation and measured shortfall

All three native scoped transactions exited 0; their workers remained omitted.
All six host simulations/absent-host controls exited 0 with their assertions
passing. The publication audit checked pointer/document/catalog/main/receipt
digests and preservation. JavaScript syntax checks and `git diff --check` pass.
No production code, receipts, snapshots, manifests or trust policies changed;
full `make verify` and another full corpus run were not repeated for this
diagnostic-only work. The preceding production change already passed full
verification and the 418-row corpus.

New certifications: **0**. Complete-row transitions: **0**. Measured shortfall:
**all three worker entrypoints**. The unchanged full counts remain 327 complete,
64 partial, 18 refused and 9 not advanced. Three complete rows remain a ceiling,
not an achieved or estimated gain.
