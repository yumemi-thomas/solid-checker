# Synthesized whole-parameter return vetoes

Status: implemented, 2026-09-12.

ADR 0075 proves exhaustive whole-parameter return identity from the authenticated
implementation census. The veto synthesizer previously handled only empty
return enumerations, so these supported positive closures still required a hand
recipe. A missing recipe is an early withholding reason, not evidence that the
census would succeed.

The synthesizer now selects its observation from the exact artifact case,
export, and normalized return operation. It supports an empty enumeration or
exactly one `Return` whose output is a whole parameter with an empty path.
Unsupported shapes and unreviewed domains remain withheld. Selection can read
partial knowledge because recipe gating has already opened the candidate;
neither selecting nor running a recipe restores closure without independent
census verification.

For a whole-parameter claim at index `i`, capture the original `args[i]`, call
the export, and emit `return-outside-identity` exactly when a normal completion
is not SameValue-equivalent to the original (`Object.is` semantics). The
comparator uses JavaScript operators so a package replacing `Object.is` during
import cannot corrupt the observation. A copied object, another distinct argument,
or a replacement value contradicts the claim. Mutating the original object's
contents does not: `returns` is not a claim about writes. Promises and iterators
are compared directly, never unwrapped. The census independently excludes
async/generator completions, wrong identity, and incomplete return paths.

Every overload must expose the claimed ordinary parameter position and have
sampleable non-rest inputs. Missing positions, a rest parameter at or before
the selected position, malformed positional indexes, and empty sample sets
remain recipe gaps. The finite sampler gives object and callable arguments
distinct identities, gives broad strings/numbers/bigints distinct slot values,
and varies the returned slot separately so identical boolean cycles do not hide
wrong-argument returns. It respects complete literal partitions and finite-only
numeric domains, and includes signed zero and, where admitted, NaN.

There are at most twelve tuples per overload. Structural object requirements,
variadic tails, and paths outside that finite sample are not covered. Argument
construction does not claim to satisfy every structural or generic declaration
constraint. A throwing call supplies no return observation and is not evidence
of invalid package behavior; a run with no normal completion throws and
is withheld as an incomplete veto. At least one normal completion can satisfy
the veto without proving anything about calls that threw or were not sampled.
The existing empty-return and creates observations retain their behavior.

Hand recipes still take precedence. Generated modules and their exact
observations pass through the existing corpus, copied-byte digests, mandatory
probe gates, and receipt binding. No producer protocol, contract schema, proof
premise, or acceptance authority changes.

The sibling `synthesized_vetoes_tests.rs` executes generated modules against
counterexamples and controls. Direct certification and a one-node graph both
run with an empty hand corpus and require the identity claim and a nonempty
veto root in the result. The factory-parent tracer pins a remaining scheduling
limit: graph-wide Type Facts acquisition asks for the child's closed identity
before the synthesis pass can restore it. A hand recipe serves that graph;
automatic child synthesis needs a separate staged-acquisition change. No
ungated proposal is substituted to bypass the dependency proof. The existing
unrun-veto, wrong-parameter, mutated-binding, and importer controls remain.

The first ecosystem run exposed a second boundary: new vetoes on Solid 1
subpaths encounter same-package dependency edges that the private workspace
cannot currently materialize. An `UnauthenticatedDependency` from a synthesized
corpus now follows the existing cannot-run withholding path. The graph drops
the generated corpus and retries with the hand corpus, leaving those claims
open. A hand recipe that requires the same missing bytes still refuses. This
does not admit self-resolution or substitute the installed package's bytes;
proper self-reference support needs exact runtime-target and condition checks
throughout private-workspace construction and resolution verification.
The self-dependency graph tracer checks both outcomes against an authenticated
archive: the generated candidate is withheld with its original claim id and
missing-snapshot reason, while an addressed hand recipe still refuses.

The [inventory](../package-contract-v2/phase21/2026-09-12-returns-synthesis-inventory.md)
separates repeated closure entries from distinct claims and exports. Its counts
are shape eligibility bounds, not measured certification gains. The independent
[reads experiment](../package-contract-v2/phase21/2026-09-12-reads-observation-experiment.md)
does not change production policy or add recipes.

## Validation and observed result

Fresh offline certification of `@solid-primitives/i18n@3.0.0-next.4` publishes
closed `returns` for `identityResolveTemplate`, `missingKeyAsPath`, and
`template`, using the existing corpus with no hand recipe for these claims.
Ordinary analysis authenticates the receipt and selects the exact case. The
[retained measurement](../package-contract-v2/phase21/2026-09-12-returns-synthesis-i18n-measurement.json)
binds the package, runtime, declaration, main, receipt, and audit identities.
This establishes three actual closures; it is not a contemporaneous control
comparison or an ecosystem-wide delta.

Before the final missing-snapshot withholding adjustment, `make verify` passed
in 166.70 seconds, including all 15 synthesis tests, the
direct and graph identity tracers, the armed workspace suite, Go race suite,
coverage, ownership, contract corpus, TypeScript oracle, and conformance. No
finding snapshots, bundled contracts, or public schema artifacts changed for
this slice. Pre-existing class-construction/protocol changes were included in
the shared worktree validation and are not part of this ADR.

The corrected `make ecosystem-regression` completed all 418 probes and passed
its configured thresholds at 2026-09-12T06:14:11.062Z. Against the retained
screenshot report (`report-0094.json`), the counted certified domain entries
are creates 5,321 → 5,326, reads 40 → 40, and returns 285 → 1,180 (+895).
Withheld returns are 1,164 → 223. These are repeated corpus entries, not unique
exports, and the historical comparison includes the shared class work; it is
not an isolated before/after control. The run also has one remaining
infrastructure failure: `solid-js@1.9.14|solid1|only` exceeded the 600-second
certification budget. No other previously certified root or entrypoint coverage
was lost. A passing configured threshold does not make that timeout a success.

The focused self-dependency regression passed after the final adjustment,
checking both synthesized withholding and strict hand-recipe refusal. The
full verification suite was not rerun after that small adjustment, and the
timeout was not investigated further, after the user requested tighter usage.
Raw comparison reports remain under `rust/target/ecosystem-regression/`;
neither the checked-in benchmark baseline nor public contract artifacts were
updated by this slice.
