# ADR 0172: fresh fixed structural returns require every member and completion

Date: 2026-09-30. Status: implemented; source verified; tier regeneration in progress.

The stable contract language already describes tuples and objects. Previously
the runtime return census could not prove their complete construction, leaving
fresh fixed containers open or proposing only their known reactive members.
An incomplete member list cannot certify the entire returned value.

Protocol 77 adds an optional bounded structural tree to each return site and
conditional arm, plus an exact body frame and explicit body-end reachability. Each tree states literal
construction, exact ordered member spans and keys, closed enumeration, and
independent primitive, whole-parameter or existing owned-accessor evidence.
Unknown evidence is retained as unknown. Limits withhold the entire tree.

Certification compares the complete actual tree with the entire proposed shape.
Every live completion and every proposed alternative needs a witness. An absent
end-reach fact, live fallthrough or bare return cannot exclude `undefined`.
Bounds use the exact body the producer walked, tied to the resolved declaration's
source; both the query and a named declaration may span only an identifier.
Positive member paths and callability must agree with that same tree. Declaration
signatures and runtime probes cannot substitute for its behavioral evidence.

The synthesized runtime veto checks every returned member through own data
descriptors, never invoking a getter or an accessor. It checks array brand,
exact length and keys, plain-object prototype and keys, primitive leaves,
whole-parameter identity and accessor callability. Every alternative is bounded
to depth eight and 128 nodes. A throwing-only run or a reflection failure is
incomplete. A Proxy can imitate reflection, and finite samples do not prove
freshness or accessor behavior; those remain census obligations.
The observation serves both the call's return enumeration and each returned
container's member-enumeration gate under its own exact claim identity. Hand
recipes keep precedence. Serving only the call-domain gate leaves a tuple
uncertified even if its positive members are known.

The initial grammar admits fresh fixed arrays without holes or spreads and
objects with ordinary identifier/string property assignments. It refuses
computed/numeric keys, shorthand, spreads, methods, accessors, duplicate decoded
keys and `__proto__`. Saved containers, dynamic arrays and arbitrary callable
members remain open. Async/generator and unclassified completion paths refuse.

The generator selects the approved complete literal grammar in normalized
syntax before proposing members, preserving independently inferred
reactive identities. Plain syntax candidates remain proposals until the census
proves every leaf. A separate open async summary does not suppress a literal
proposal: the producer and certifier independently enforce plain completion.
Project return projection keeps differing structural alternatives open instead
of projecting one reactive arm over a plain alternative.

Consumers instantiate whole-parameter members only through exact local value
identities and source-producing calls. Direct binding slots exclude nested,
default, computed and rest patterns. Member identities are withheld after
whole-binding writes, member writes through exact aliases, deletion, unknown
method calls or escape into unenumerated containers/expressions. These checks
are conservative over the whole file, not an ordering or lifetime proof.
Typed source discovery requires an exact callable identifier or static member
property symbol distinct from its receiver, with that expression's own
callability fact. A tuple's nested accessor alias cannot classify the tuple
itself as an accessor.

Controls include complete versus prefix/partial trees, unknown leaves, bounds,
overlap, conditional arms, objects with nested tuples, whole parameters,
fallthrough, async completion, rebinding, alias writes, deletion and escape.
The consumer fixture's misuse and correct-use examples also compile against
published rc.9 Solid/signals/web declarations with renderer-owned JSX typing.
Its explicit `() => number` carrier type permits replacing a branded source
with a plain function without weakening the published source accessor type.

The published trial uses
`@solid-primitives/vibrate@1.0.0-next.2:frequencyToPattern`. Its reactive input
snapshot and memo twin compile against published rc.9 declarations. A complete
result requires both the call-level return enumeration and the tuple's own
member enumeration to survive generation, independent proof and consumer
admission. The proposal sidecar alone is insufficient. A failed construction
proof withdraws its entire return operation through the existing withholding
mechanism; it cannot retain guessed positive members or block a separately
provable sibling export.

The isolated published trials now certify and admit, per none/browser/node,
`frequencyToPattern`, including `closed: ["items"]` on both plain tuple
members and all four call domains. Each exact canonical consumer environment
certifies the memo twin with zero findings and reports one SC1001 violation
on line 6 of the untracked-read twin. All six compile against the installed
published typings without diagnostics. This is a usable export result; the
fresh per-host checkpoint and delivered tier remain in progress. The pure
conversion helper has no separate intrinsic misuse class, so this caller-side
read does not manufacture a criterion-3 ledger entry.

This decision does not claim completion of the primitives checkpoint. Namespace
admission and unresolved computed dispatch are measured explicitly; neither
receives guessed reactive behavior. Real-package certification and regenerated
accepted artifacts determine the coverage gain after verification.

Full source `make verify` passes in the isolated verification worktree, exit
zero, TOTAL 851.47 s, with no `FAILED during step`. This includes 755 backend
library tests, the new complete-return/sibling-withdrawal integration test,
Go race tests, fmt/clippy, 142 fixture projects with 737 findings, 120 contract
corpus fixtures, ownership, performance, 375 script tests, the 102-case
TypeScript oracle, obligation audit and contract conformance. Main `make
test-rust` also passes; the subsequent runtime-gate wiring is covered by the
focused integration test and the final full verification.
