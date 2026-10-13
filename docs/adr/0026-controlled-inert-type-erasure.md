# 0026 — Controlled execution of an inert TypeScript module

Status: implemented for the controlled inert subset
Date: 2026-09-04

## Decision before implementation

Implement `node-strip-inert-esm-v1` as a separate, checker-owned execution
transaction. This is the first admitted subset of ADR 0025's proposed profile,
not admission of the general import-free population. The only allowed module
is one directly exported, synchronous, non-generator function with no
parameters, type parameters, directives or executable statements other than
an optional bare return. Its only optional annotation is a void return type.
The complete Oxc statement tree decides this restriction in solid-facts;
absence of calls in a partial table is insufficient. The native implementation
census and every mandatory contradiction gate remain required.

The consumer is a fresh invocation owned by the native launcher, after receipt
authentication. It imports the exact selected source URL through the pinned
strip-only loader and calls the selected export with no arguments. It does not
return an AcceptedContract, publish an accepted catalog, or enter the ordinary
analyzer's contract index. The result and receipt describe this controlled
execution only. Arbitrary applications, compilers, bundlers and browsers cannot
consume it. This deliberately small consumer makes compatibility enforceable
instead of asking users to assert it in configuration.

## Evidence and scope

The POC's 40 parsable / 32 import-free / 26 completed samples establish
feasibility, not certifications. Its reflection counterexample rules out
cross-compiler reuse. Kobalte 0.9.2's source noop is inside this smaller grammar;
the other 39 original source candidates are not promised admission. Published
JS remains on its existing path. Importing and browser-dependent candidates
remain separate work, using ADR 0025's measured three extensionless edges.

The native parser computes the only permitted erasure: replace the optional
void annotation with whitespace, preserving line terminators. The pinned Node
strip-only transformer must produce exactly these bytes before any package or
recipe import. Both source and output are bound and copied into the watched
private workspace. This is a reviewed preservation premise for the census:
neither module initialization nor the function body contains any executable
expression, reference, import or transformation-sensitive construct. The
census still supplies all ordinary positive evidence and the mandatory veto
still independently blocks a contradiction. This premise does not extend to
reflection, enum/decorator lowering, erased imports, parameters or callbacks.

## Binding and compatibility

A new receipt version 3 uses a distinct signature domain and explicit profile
and proof identity. It binds the canonical claim document, all native proof
roots, exact archive/source case, source and output hashes, export name,
ESM interpretation, source URL rule, empty runtime graph, measured conditions,
dependency materialization, Node/transformer/harness identity, runtime and
sandbox policy. It never contains a reusable signed version-2 receipt.
The stable main schema retains its meaning; the new execution result is a
separate format and grants no unscoped contract knowledge. Version-2 consumers
reject version 3 before authentication. No profile opt-in changes that.

The launcher authenticates the receipt against its own just-verified proof and
execution binding, not caller-provided expected digests. It independently
prepares and launches the consumer with the same compiled pins and checks the
actual source/output/profile echo and exact resolution. There is no public
constructor for the capability and no serialized capability or persistent
profile cache. Portable replay and acceptance by arbitrary build consumers are
unsupported. Receipt bytes alone never authorize execution or grant knowledge.

Worker protocol advances to v3 and sandbox scheme to 8: the trusted worker can
install a narrowly scoped format override at the one exact source URL. Package
imports, query/fragment variants and CommonJS are refused for this profile.
The startup/run frame count, primordial capture and freeze before import,
0700 workspace, env_clear allowlist, compiled pins, condition replay,
verify_reported_resolution, process-group killpg and all-exit write census
remain. The source URL points to the original .ts member; derived bytes have
their own watched file and digest. Ordinary published-JS launches install no
hook. A changed transformer, source, output, profile or environment refuses
inside the gate/consumer transaction, with no fallback or repinning.

## What the scoped veto proves

The veto can falsify the claim for the derived execution it actually observes;
a completed finite sample cannot establish closure. Native census evidence and
the restricted preservation premise establish the positive claim. Erasure can
change reflected source, locations and initialization in a broader language,
so this receipt makes no claim about any other transformation. Here those
observations cannot occur in the admitted module, and the sole supported
consumer executes the verified derived bytes under the same runtime model.
That is the defensible scoped claim; widening either grammar or consumer needs
another reviewed premise and profile identity.

## Verification and measurements

The completed measurement admits one original TS candidate, Kobalte 0.9.2
noop: its exact formerly incomplete gate completes and the controlled consumer
runs. The other 39 refuse the complete syntax whitelist. Ordinary before/after
rows are unchanged, and exportsProven stays zero. See
`docs/2026-09-05-controlled-inert-type-erasure.md` for exact outcomes and checks.

Pin annotated and unannotated inert modules, ordinary JS, source/output/profile
and transformer mismatch, unsupported syntax and imports, reflection refusal,
mandatory derived contradiction and refusal by ordinary consumers. Measure
the original three rows with the same recipe corpus before and after, and
report the explicit controlled transaction separately. Do not call POC samples
accepted closures or treat the no-recipe ecosystem baseline as comparable.
