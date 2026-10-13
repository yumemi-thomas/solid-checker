# Fixed structural return proofs: initial implementation boundary

Date: 2026-09-30. Status: implemented, verified and delivered; integrated checkpoint measured.
This is an implementation plan, not certified coverage or a runtime audit.

## First supported structures

Use the existing stable contract `Tuple` and `Object` constructors. Initially
admit only fresh literal returns, including transparent TypeScript wrappers:
fixed tuples without holes or spreads, and plain objects with exact literal
keys. Refuse computed keys, spreads, getters, setters, methods, duplicate keys
and `__proto__`. A binding to a previously constructed container stays open
until a separate identity, mutation and escape census proves it.

Each member needs its own exact proof. Start with grammar-proved primitive
values, the caller's own parameters and accessor identities already supported
by the current owned-accessor witnesses. Preserve every existing archive,
options, spread and lexical-scope restriction. An unknown member cannot
become `Plain`, and a callable signature is not a behavioral proof.

## Producer and certification responsibilities

The producer must state whether its structural census is exhaustive, the
root's construction and member spans, exact member keys/indexes, and each
member's identity evidence. An enumerated prefix is not a complete tuple.
Evidence must cover every reachable return arm and classified completion;
missing facts and mixed unsupported alternatives refuse.

The certifier binds each location to the authenticated package bytes and
matches the entire structure to the claim. It then discharges each member's
positive demand with existing exact identity or accepted dependency evidence.
Closed member enumeration and member behavior are separate obligations. A
new producer fact requires the Go producer, Rust client, schema, protocol and
digest to move together. No declaration-only or probe-only discharge is
introduced.

## Consumer controls before delivery

Prove useful behavior through destructuring and exact named/indexed members.
Test mutation before invocation: replacing a returned accessor with a plain
function must clear its reactive identity; replacing a plain member with an
unknown value must not preserve certification. Test aliases, unknown computed
keys and namespace admission explicitly. If existing consumer facts cannot
invalidate identities soundly, delivery remains blocked until that is fixed.

Every diagnostic example and correct-use twin must compile against the
package's published declarations. Unknown evidence stays uncertifiable;
proven misuse reports a violation only when execution facts justify it.

## Success criterion

Choose a real primitives export whose complete set of blockers can be closed
within this boundary. Deliver its accepted proof and a misuse/correct-use
pair for none, browser and node where the runtime supports it. Rerun the
checkpoint and record actual changes in closed exports and passing misuse
cases. Partial domain closure alone is not a completed export. The wider
97-package objective and actual application environments remain in scope.

## Application environment priority

The existing fresh app-import measurement concentrates all 292 primitives
sites in two applications: 269 in `SonyStone/app-game` at commit
`3064fed238a043da90041f8eb86b357e253330bf`, using a project runtime tuple of
rc.4, and 23 in readingroom, using rc.8. The four highest-demand primitives
packages remain resize-observer (59), event-listener (53), utils (42) and raf
(39). These are baseline demand counts, not new certification results.

The app-game install is retained locally under
`rust/target/app-import-metric/apps/app-game`. Resolution from its actual
Solid package reaches `solid-js`, `@solidjs/signals` and `@solidjs/web`, all
rc.4. Node selects their CommonJS server builds; the complete audit also needs
the browser/development paths. A comparison with rc.3's audited file inventory
finds different runtime bytes in all three packages, so copying the rc.3 rows
under a new version is not justified. Additional rc.4 files and every required
call-graph closure must also be inventoried and reviewed.

After the checkpoint proofs are usable, rc.4 is the first actual-consumer
audit and delivery target. Certification must resolve each primitive's
dependencies from that package's own location; the project's runtime tuple
alone does not establish its installed environment. Nothing is admitted merely
because it has a nearby prerelease number.
