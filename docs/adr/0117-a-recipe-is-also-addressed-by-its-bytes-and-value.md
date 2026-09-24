# ADR 0117: A recipe is also addressed by its claim's bytes and value

- Status: accepted and implemented (2026-09-24); written with the implementation
- Date: 2026-09-24
- Owners: the recipe corpus loader (`probe_harness.rs`, `RecipeCorpus::load`),
  the semantic model's canonical encodings (`contract_semantics/canonical.rs`),
  closure identity (`artifact_resolution.rs`), plan construction
  (`contract_certification.rs`), and the corpus tooling
  (`scripts/probe-recipe-{addressing,scaffold}.mjs`,
  `packages/cli/scripts/certify-contract.mjs`)
- Relation: adds a second address to ADR 0006's claim-addressed recipe corpus.
  Nothing ADR 0006, 0008 or 0036 decided about what a recipe can do changes:
  a recipe is still an input, never a root of trust, and a hand recipe still
  wins over a synthesized one.

## Context

A recipe is found by exact equality on `claimId`, the semantic claim id. That
id hashes the artifact case, and the case's id and `dependency_closure` both
hash every accepted dependency edge's `accepted_contract_digest`
(`closure_digest`). So any change to what a dependency certifies re-addresses
every dependent claim, and every dependent recipe then addresses nothing: its
candidate is withheld as `no recipe in corpus`, its domain opens, and the row
still certifies. It happened three times between 2026-09-23 and 2026-09-24 to
the ten `@solid-primitives/rootless` and `trigger` recipes, after each change
to what `@solid-primitives/utils` certifies, and each time cost a second
census run and a verbatim carry-over (ways-to-improve § 3.2).

The recipe's bytes did not change, the package's bytes did not change, and
the claim's value did not change. Only a digest of a dependency's certified
contract did.

## Decision

**A corpus entry may carry `recipeAddress`, and the loader binds an entry
whose `claimId` names no plan claim to the one plan claim whose recipe
address equals it.**

### The address

`recipe-address:v1:sha256:<hex>` over, in order:

1. the **case byte identity**: the package's name, version, integrity and
   manifest artifact; the case's entrypoint, resolution trace, runtime,
   declarations and transform artifacts; and the **closure byte identity**.
   Never the case id and never `dependency_closure`.
2. the export identity (entrypoint, public name, runtime and declaration
   export targets);
3. the claim path;
4. the claim's **normalized value**: the domain's knowledge set as the
   semantic digest encodes it, followed by the full encoding of every
   operation its items reference (with composed provenance). An operation or
   resource id is written as `local:` plus its suffix after the last
   `:operation:`/`:resource:`, and an `artifact-case` guard atom naming the
   addressed case as `local:artifact-case`, so no case prefix, and with it no
   dependency digest, enters the address.

The **closure byte identity** hashes the closure's entries (role, path, byte
digest, transform digest) and hazards exactly as `closure_digest` does, and
each accepted dependency edge as its specifier, its package name and **that
dependency's own case byte identity**, recursively. It never reads an edge's
`artifact_case` or `accepted_contract_digest`.

### Where it exists

A plan's case byte identity is computed at construction, where planning
already hands each plan its descendant plans: an edge is answered by the
in-transaction plan with the same selected case and package, using that
plan's own identity. A closure with no edge needs nothing. An edge no
in-transaction plan answers, two plans answering it differently, or a
descendant without an identity leaves the plan with none, and then no claim
of it has an address and only `claimId` can match. So on the plain lane only
dependency-free cases carry addresses; on the graph lanes every node does.
Only call-domain closure candidates are addressed, since those are the only
subjects the probe gate schedules.

### Binding

In `RecipeCorpus::load(directory, plan)`, after the manifest's own claim ids
are checked unique:

- an entry whose `claimId` is a plan claim is kept as it is, and **an exact
  id always wins**;
- otherwise an entry whose address equals exactly one plan claim's address,
  and whose claim no entry holds yet, is rebound to that claim; entries are
  visited in claim-id order and the first binds;
- anything else addresses nothing, as a stale entry always has.

Every later lookup (gating, the private workspace copy, launch, import kinds,
dependency authentication, the browser harness) reads the loaded corpus, so it
sees one binding. The corpus root gains one `recipe-address:<claim>:<address>`
line per entry **bound by its address**, and none otherwise: every corpus
that binds nothing that way hashes byte-identically to before, and so does
every receipt binding it.

### Reporting

The native certifier prints `solid-checker:recipe-addresses=` once per plan
and per graph node, listing every addressed candidate's claim id and address;
`certify-contract.mjs` keeps them whole in the audit as `recipeAddresses`. A
withheld closure carries `recipeAddress` when its plan has one. The
addressing script counts an entry bound only by its address as addressed
(`addressedByAddress`), and `--annotate` writes addresses into the manifest
from a run's audits. The scaffold writes the address it is given and treats a
gap whose address the manifest holds as addressed.

## Why this is sound

A recipe only decides whether a closure the implementation census proved gets
vetoed. Binding one to a claim it was not written for can make it pass
vacuously, and a vacuous recipe is a clean non-observation that *satisfies*
the gate. So the address must never bind across a change that could matter to
the observation. It binds only when the package bytes, the case's resolution,
every file of the closure, every dependency's bytes (recursively), the export,
the path and the claimed value are all identical. What it ignores is what a
dependency's *contract* says, which the recipe does not observe: the probe runs
the dependency's bytes, and those are in the identity.

Without the value digest a recipe could stay bound to a broadened claim, for
example a closure that gained an operation, and go silent against it. With it,
a changed value re-addresses the claim exactly as before.

## Consequences

- A change to what a dependency certifies no longer orphans a dependent's
  recipes on the graph lanes, provided each entry carries an address.
- Entries without an address behave exactly as before. The migration writes
  addresses only for entries a census run states one for; an entry whose case
  no run produces any more stays without one, and stays stale.
- Changing any byte of the package or of a dependency, or the claim's value,
  still orphans the recipe, on purpose.
- The address format is versioned (`v1`) separately from the semantic claim
  id; a change to what it hashes is a new version, not an edit.
