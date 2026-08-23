# RFC 0001: A package-contract registry

- **Status:** Draft
- **Authors:** solid-checker maintainers
- **Date:** 2026-08-22
- **Affects:** [package-contracts.md](../package-contracts.md),
  [precision-backlog.md](../precision-backlog.md), `packages/cli`
- **Does not affect:** the analyzer, the loader's four discovery tiers, the
  contract schema's precedence rules

## Summary

A reviewed package contract is expensive to produce and, today, impossible to
share. This RFC proposes a registry of reviewed contracts and one new explicit
command, `solid-checker contract fetch <package>`, that resolves against the
*installed* artifact, verifies a signed registry entry, and materializes the
contract into the existing **local** tier at
`.solid-checker/contracts/<package>/solid-reactivity.json` for the consumer to
commit and code-review.

Nothing about analysis changes. There is no new discovery tier, no new
precedence rule, and no network access at analysis time. The registry is a
distribution channel for a file the loader already knows how to read, keyed by
`(package name, exact version, npm integrity)` and content-addressed by the
contract's SHA-256 identity — the identity
`rust/crates/solid-facts-backend/src/diagnostics.rs` already computes when it
loads a contract. The consumer owns the resulting file the way a shadcn/ui user
owns a vendored component: it lands in the repository, it shows up in a diff,
and no runtime dependency on the registry survives the fetch.

## Motivation

### What exists

A contract reaches a project through exactly four channels: this checker's
**bundled** artifacts, a **published** `solid-reactivity.json` inside the
installed package, a **local** file under `.solid-checker/contracts/`, and an
**explicit** `--contract` path. There is no fifth, no shared corpus, and no way
for one team's review work to reach another project.

### Why that hurts more than it looks

Evidence is enforced, not decorative. A contract emitted by `contract generate`
carries `evidence.kind: "inferred"`; consumers report it as `unverified`, and
its summaries **are not inserted into Reactive IR at all**. It cannot prove a
violation, cannot discharge a proof obligation, and cannot certify a consumer.
Generation deliberately never promotes an inferred claim, and the sibling
`<contract>.review.md` checklist exists precisely because the promotion is a
human act.

So the only path available to an application developer importing a dozen
Solid-aware packages this checker does not bundle is: run `contract generate`
twelve times, then perform twelve reviews. Until each review lands, each package
reports `unverified`, contributes nothing, and every import of it yields an
uncertifiable `SC9005 package-contract-incomplete` finding.

Three properties make that cost compound rather than amortize:

1. **Review is per artifact, not per package.** A contract names the exact
   version it was reviewed against and is refused when the installed version
   differs; a contract that records `package.integrity` is refused when the
   installed tarball's integrity differs even under an unchanged version string.
   Both refusals are correct — a version string is not a pin — and both mean an
   upgrade re-opens the review.
2. **The cost is O(packages × upgrades) and is paid by the worst-positioned
   party.** The application developer has neither the package author's knowledge
   of the implementation nor any authority to publish the result. Their review
   dies inside one repository.
3. **The reachable supply is drafts, not evidence.** The ecosystem benchmark
   records 403 complete contracts, 6 partial, and 7 failures across a
   416-probe manifest — strong *generation* reachability, and every one of those
   403 is `inferred`. The gap between "the generator can describe this package"
   and "someone has certified what it says" is the entire problem.

The registry does not reduce the amount of review that must happen. It makes
each review reusable, attributable, and auditable, and it moves the work to
parties who can actually do it.

## Threat model

**A contract is a finding suppressor.** This is the fact that determines every
other decision in this document.

Omitting an effect field is a *reviewed negative claim*: a summary with no
`callbacks` field certifies "this export never invokes a caller-supplied
callback". A certifying contract's summaries are inserted into Reactive IR and
participate in reactive-read, callback-timing, owner-requirement, and async
conclusions. A contract that lies by omission therefore silences the checker
*exactly where it should fire* — and the resulting failure is invisible. A
malicious package misbehaves observably at runtime; a malicious contract
produces a clean report.

Installing a registry contract is a trust decision of the same magnitude as
installing the package it describes. Distribution is a supply-chain surface.

### Adversaries and failure modes

| Threat | Mechanism | Design response |
| --- | --- | --- |
| Malicious submission | An attacker publishes a contract omitting `callbacks` for an export that does invoke callbacks, silencing findings in every consumer. | Governance: no open uploads; PR review by a named maintainer; the review plan and the reviewer's resolved decisions are published with the contract. |
| Compromised transport | A proxy or mirror substitutes a different contract for the same package. | Content addressing: the registry entry pins the contract's SHA-256; the client verifies the bytes it received against the entry, and the entry against a signature. |
| Compromised registry | The registry host serves an entry the maintainers never merged. | Signatures over the entry, verified against an explicit local trust set — not against anything the registry itself asserts. |
| Compromised reviewer key | A stolen key signs an entry that would never have been merged. | Revocation of both entries and keys, published by the registry and consulted by `contract check --refresh`; the git history of the registry repository is the audit trail. |
| Confused artifact | A republished tarball keeps the version while replacing the bytes the review described. | The lookup key includes npm integrity, so a republished artifact **misses** in the registry rather than matching wrongly. |
| Silent swap after the fact | A later fetch, or a compromised registry, quietly replaces a contract already in a project. | `contracts-lock.json` records the contract hash, the entry hash, and the signing identities per package; drift is a refusal and a visible diff. |
| Rollback | An old, since-revoked contract is served in place of a current one. | Revocation list keyed by contract hash; `--refresh` reports and quarantines. |
| Analysis-time compromise | Network access during analysis makes results non-deterministic and puts the analyzer on the attack surface. | Fetch-time only. The analyzer never opens a socket. |

### Non-goals

The registry makes claims *about* packages; it does not vouch for packages. A
correct, reviewed, signed contract for a malicious package is still correct. The
registry is not a security advisory database and its revocation list means "this
reviewed claim was wrong", not "this package is dangerous".

## Detailed design

### 1. `solid-checker contract fetch <package>`

**Status: does not exist.** `packages/cli/bin/solid-checker.mjs` already
dispatches three subcommands: `contract generate` and `contract review` into
Node (`scripts/generate-package-contract.mjs`, `scripts/review-contract.mjs`),
and `contract check` to the native checker's `--check-contracts`. `fetch` is a
fourth branch and is Node-only: network access lives on the Node side of the
process seam and never reaches the Rust analyzer.

Resolution proceeds against the installed package, never against a specifier or
a manifest range:

1. Locate `node_modules/<package>/package.json` by the same ancestor walk
   contract discovery uses, and read the installed `name` and `version`.
2. Recover the installed tarball's npm integrity from the project's lockfile,
   using exactly the recovery rules the loader's integrity-drift check already
   documents: `package-lock.json` or `node_modules/.package-lock.json` at
   `lockfileVersion` 2 or 3, whose `packages` map is keyed by install path.
   Every other case — pnpm, Yarn, no lockfile, `lockfileVersion` 1, a workspace
   link or `file:`/git dependency with no `integrity`, two lockfiles that
   disagree, an unparseable lockfile — yields **no fact**.
3. Look the entry up by `(name, version, integrity)`.
4. Verify the entry's signature against the configured trust set (§3), then
   verify the bytes of each fetched file — the contract, the review plan, and
   the review state — against the SHA-256 the entry pins for it.
5. Write `.solid-checker/contracts/<package>/solid-reactivity.json`, the review
   plan `solid-reactivity.review.json` and the resolved review state
   `solid-reactivity.review-state.json` beside it, and update
   `.solid-checker/contracts-lock.json`.
6. Print the path, the reviewer identities, and the instruction to commit and
   review the diff.

**When integrity is unavailable**, the fetch degrades to a `(name, version)`
lookup, states that it has done so on stderr, and records
`"integritySource": "none"` in the lock. This is a real weakening — on a pnpm or
Yarn project the fetched contract binds to nothing but a version string, the
same residue the integrity-drift section already records — and it must be
legible rather than silent. A registry entry may set `requireIntegrity: true`,
in which case the degraded lookup is refused outright rather than served.

Refusals are refusals. A missing entry, an unsigned entry, a signature from an
identity outside the trust set, a contract whose bytes do not match the entry
hash, or a revoked entry all cause `contract fetch` to write nothing and exit
non-zero. It never writes a contract with a downgraded evidence kind, and in
particular never writes one as `inferred`: that would produce a file that
reports `unverified` forever while looking like a successful fetch.

Supporting shapes: `--all` fetches for every package `contract check` reports as
`missing` or `unverified`; `--dry-run` prints what would be written; `--registry
@namespace` selects among the namespaces configured in
`.solid-checker/registries.json` (§3).

### 2. Key and lock

The lookup key is the artifact, not the name:

```
(package name, exact version, npm sha512 integrity) -> entry
entry.contract.hash = "sha256:<lowercase hex>"   # the loader's own identity form
```

Content addressing is what makes the fetch verifiable without trusting the
transport. The client computes the SHA-256 of the bytes it received and compares
it to the entry; the entry is what the signature covers. A registry that serves
different bytes than the entry pins is caught by arithmetic, not by policy.

`.solid-checker/contracts-lock.json` is new and is committed:

```json
{
  "schemaVersion": 1,
  "contracts": {
    "@solid-primitives/scheduled": {
      "registry": "@solid-checker",
      "package": {
        "version": "1.5.3",
        "integrity": "sha512-oNwLE6E6lxJAWrc8QXuwM0k2oU1BnANnkChwMw82aK1j3+mWGJkG1IFe5gCwbV+afYmjI76t9JJV3md/8tLw+g==",
        "integritySource": "package-lock.json"
      },
      "contract": {
        "path": ".solid-checker/contracts/@solid-primitives/scheduled/solid-reactivity.json",
        "hash": "sha256:1f0d3c5b8a2e4d7690ab1c2d3e4f50617283940a5b6c7d8e9f0a1b2c3d4e5f60"
      },
      "review": {
        "plan": { "hash": "sha256:9c8b7a6f5e4d3c2b1a0918273645362718094a5b6c7d8e9f0a1b2c3d4e5f6071" },
        "state": { "hash": "sha256:5d4c3b2a19087f6e5d4c3b2a1908f7e6d5c4b3a29180f7e6d5c4b3a291807f6e" }
      },
      "entry": {
        "hash": "sha256:0a1b2c3d4e5f60718293a4b5c6d7e8f9001122334455667788990aabbccddeeff",
        "signedBy": ["review-key:2026-a"]
      },
      "fetched": "2026-08-22T09:14:07Z"
    }
  }
}
```

A later `contract fetch` for a package already in the lock must reproduce the
recorded hashes. When it does not — a rotated entry, a re-review, a compromised
registry — the fetch refuses by default and requires `--accept-change`, which
rewrites the lock and leaves the substitution as a reviewable diff of both the
lock and the contract. A swap that nobody sees is the failure this file exists
to prevent.

**The lock is not a loader input.** Analysis reads contracts and nothing else;
adding a lock consultation to the analyzer would make an offline, deterministic
pipeline depend on a file the analyzer has no way to validate. `contract fetch`
and `contract check` read the lock; the analyzer does not know it exists.

### 3. Signatures and the trust set

A registry entry is signed detached over a canonical serialization of the entry
with the `signatures` array removed. The signature therefore covers the package
key, the contract hash, the review-plan and review-state hashes, the reviewer
identities, and the declared evidence kind as one unit; nothing in that set can
be altered independently.

Verification is against an **explicit, small, local trust set** — a configured
list of identities and keys, not "whatever the registry says is trusted" and not
trust-on-first-use. One project-level file, `.solid-checker/registries.json`,
holds both halves of the configuration — the registry namespaces and the trust
set — and it is committed beside `contracts-lock.json`:

```json
{
  "schemaVersion": 1,
  "registries": {
    "@solid-checker": "https://raw.githubusercontent.com/…/registry.json"
  },
  "trustedReviewers": [
    { "id": "review-key:2026-a", "alg": "ed25519", "publicKey": "…" }
  ]
}
```

A `registries` value is the URL of that registry's **self-describing index**
(`registry.json`, §7) and nothing else. Everything downstream of it — where
entries, signatures, revocations, and the registry's own key set live — comes
from the templates the index declares, so a registry can move its layout without
every consumer editing a URL pattern.

The registry publishes its own current key set, but that file is a
*convenience for bootstrapping*, not an authority: importing it is an explicit
act (`contract trust import`, printing every key and requiring confirmation),
and once imported the local set is what verification consults. A registry that
adds a key does not thereby gain the ability to certify anything in an existing
project.

**Sigstore is the intended second step, not the MVP.** npm package provenance
already uses Sigstore's keyless flow: a single-use keypair, a Fulcio-issued
certificate binding it to a CI job's OIDC identity, and the attestation recorded
in the Rekor transparency log, verifiable with `npm audit signatures`. That
model fits the review attestation well — a reviewer identity is a person or a CI
job rather than a long-lived secret, and a transparency log gives exactly the
"was this entry ever actually merged" property that a bare signature does not.
The costs are a much heavier client and an online verification dependency at
fetch time; the latter is acceptable because fetch is already online. The MVP
ships detached Ed25519 signatures with an explicit key set because that can be
implemented, reviewed, and rotated with no new infrastructure, and because the
client's contract with the registry — *content-addressed, signature-verified
files* — does not change when the signature format does.

### 4. Evidence mapping

A registry-maintainer-reviewed contract lands with `evidence.kind: "attested"`.
That is consistent with the reserved meaning already recorded in
[package-contracts.md](../package-contracts.md): `verified` means mechanical
artifact/surface/behavior checks passed, `reviewed` records an explicit human
review, and `attested` is reserved for a *verifier-produced release identity* —
which is precisely what a registry entry is. The registry's CI performs the
mechanical checks, a named maintainer performs the human review, and the entry
binds both to one release artifact.

Two enforcement consequences follow, and both are mechanically checkable in the
registry's CI:

- A contract published as a certifying evidence kind must contain **no
  `inferred` row evidence**. Certification already rejects an inferred row inside
  an otherwise-certifying document precisely so a promoted contract cannot hide
  an uncertified claim; the registry must refuse to publish what the consumer
  would then reject.
- It must contain **no `unknown` sentinels** in `callbacks`, `reactiveReads`,
  `returns`, `ownerRequirements`, or `asyncBehavior`. Unknown is not evidence and
  cannot be promoted; review resolves a marker by replacing it with the audited
  value or by deleting the field to certify absence.

**A problem worth stating plainly.** Recording *which* verifier produced the
attestation inside the contract is not a free schema-v1 addition. The `evidence`
object in `schema/solid-reactivity.schema.json` is
`"additionalProperties": false` over `kind` and `generator`, and the loader fails
closed on unknown JSON fields — via the *malformed* path, which fails the
analysis outright rather than refusing the contract and continuing. A contract
carrying `evidence.verifier` would therefore hard-fail every older
`solid-checker`, which is the opposite of a backward-compatible field addition.

Two options, and the MVP takes the second:

- **(a) Add `evidence.verifier` to schema v1.** Additive at the schema level,
  but it partitions clients by version. It would need a `checkerRange` on the
  registry entry, and `contract fetch` would have to refuse to write a document
  its own loader would reject.
- **(b) Keep verifier identity out of the contract.** The reviewer identities
  live in the registry entry and in `contracts-lock.json`, where they are signed
  and committed. The contract stays byte-identical to what a careful reviewer
  would hand-author, which also means a fetched contract is indistinguishable
  from a locally reviewed one to the analyzer — a property worth keeping.

Option (b) loses the ability to answer "who attested this?" from the contract
alone, which is a real cost and is listed as an unresolved question.

### 5. Revocation, on day one

A registry that cannot say "that review was wrong" is a registry that ships its
mistakes permanently. Revocation is in the MVP.

The registry publishes `revocations.json`, keyed by **contract hash** rather
than by package, so a revocation is unambiguous about which reviewed claim it
withdraws:

```json
{
  "spec": "solid-checker-registry/1",
  "revocations": [
    {
      "contract": "sha256:1f0d3c5b8a2e4d7690ab1c2d3e4f50617283940a5b6c7d8e9f0a1b2c3d4e5f60",
      "package": { "name": "@solid-primitives/scheduled", "version": "1.5.3" },
      "revoked": "2026-09-04T00:00:00Z",
      "reason": "callbacks omitted for `leading`; the scheduler factory argument is invoked inline"
    }
  ]
}
```

`solid-checker contract check` gains an **opt-in** `--refresh` that fetches the
revocation list and compares it against `contracts-lock.json`. A revoked
contract is reported with the same vocabulary and consequence as `stale`: the
package needs action, the command exits 1, and the contract must not certify.

The mechanism deserves precision, because the consequence has to survive
offline. The analyzer has no "revoked" input today, and giving it one would be a
new loader input for a fact it cannot verify. So `--refresh` **quarantines** the
revoked file — moving it to `.solid-checker/contracts/<package>/revoked/` and
recording the revocation in the lock — which returns the package to the
uncontracted path with no engine change at all: an uncertifiable `SC9005
package-contract-incomplete` at the package import, the run continuing, and
`--certify` exiting 1. That is exactly the fail-closed consequence a stale
contract has, and the quarantine is a visible diff.

The cost of achieving it this way is that the *offline* message says "no
contract" rather than "revoked, because …". Teaching the loader a revoked status
so the finding can carry the reason is the more informative design and is listed
as an unresolved question; it is not a prerequisite for shipping revocation.

Revocation never deletes. The entry, its review plan, its resolved review state,
and its signature stay in the registry's history; the revocation is an addition.

### 6. Governance: the DefinitelyTyped model

Publication is by pull request into a community repository, reviewed and merged
by a small maintainer set. There are no open uploads and no publish tokens that
bypass review. Anyone may propose a contract; a maintainer with review authority
merges it, and the signature on the entry is the merging maintainer's.

The distinguishing commitment is that **the review is published alongside the
contract, item by item**. `contract generate` already writes a sibling
`<contract>.review.md` for a human to read and a machine-readable
`<contract>.review.json` plan behind it, whose sections are the entrypoints the
generator refused, the legacy manifest field a root contract was resolved from,
the contract's artifact binding, exports with no summary, unknown export claims,
callbacks with no execution row, callbacks with no owner row, generated owner
requirements needing caller review, inherited rows, and environment-branching
entrypoints. Nothing mutates the `.md`: `contract review` records the reviewer's
decisions in `<contract>.review-state.json`, one resolution per plan item.

So the two review files published beside the contract are the **plan** and the
**resolved state** — the questions the generator raised and what the reviewer
concluded about each one. A consumer can therefore audit **what was reviewed**, not merely
that someone asserts a review happened, and the negative-claim items are in that
audit by construction.

Two sections carry most of that weight. "Callbacks with no execution row" is
where a reviewer is told which exports are about to certify "never invokes a
caller-supplied callback" — omitting `callbacks` is a negative claim, and this is
the item that makes it a deliberate one. "Entrypoints the generator refused" is
where a *partial* contract announces itself: an entrypoint the generator could
not analyze produces a contract that describes less of the package than its name
suggests, and publishing that unnoticed would ship silence as coverage. A
registry entry whose published review state leaves either kind unresolved is not
mergeable.

Package authors get a fast path — they can be added as reviewers for their own
package — but not a bypass. A package author reviewing their own package is
signing a suppression of findings in their own code, which is a conflict of
interest worth recording rather than hiding; the entry carries it, and the
consumer can see it.

### 7. MVP: a git repository, not a service

The first registry is a git repository. Directory per package, per version, per
artifact; contract, review plan, resolved review state, entry, and signature per
artifact.

```text
solid-checker-contracts/
  registry.json                     # the index, self-describing (see below)
  revocations.json
  reviewers/
    trust.json                      # current reviewer identities and public keys
    retired.json                    # rotated and revoked keys, with dates
  packages/
    @solid-primitives/
      scheduled/
        1.5.3/
          a0dc0b…f0fa.entry.json    # named by the tarball sha512, hex-encoded
          a0dc0b…f0fa.entry.sig
          a0dc0b…f0fa/
            solid-reactivity.json
            solid-reactivity.review.json
            solid-reactivity.review-state.json
```

The artifact directory is named by the **hex** encoding of the same sha512
digest the lockfile records in base64 (`sha512-…==`), because the base64 alphabet
contains `/` and `+` and is not a path. The name above really is that encoding of
the integrity in the entry below: `sha512-oNwLE6…8tLw+g==` decodes to 64 bytes
whose hex spelling begins `a0dc0b` and ends `f0fa`. The entry carries the
canonical npm spelling; the client converts. Two
republished tarballs under one version are two sibling directories, which is the
correct shape: they are two artifacts.

An entry:

```json
{
  "spec": "solid-checker-registry/1",
  "package": {
    "name": "@solid-primitives/scheduled",
    "version": "1.5.3",
    "integrity": "sha512-oNwLE6E6lxJAWrc8QXuwM0k2oU1BnANnkChwMw82aK1j3+mWGJkG1IFe5gCwbV+afYmjI76t9JJV3md/8tLw+g==",
    "requireIntegrity": true
  },
  "contract": {
    "path": "solid-reactivity.json",
    "hash": "sha256:1f0d3c5b8a2e4d7690ab1c2d3e4f50617283940a5b6c7d8e9f0a1b2c3d4e5f60",
    "evidence": "attested",
    "schemaVersion": 1,
    "compilerFactsProtocol": 1
  },
  "review": {
    "plan": {
      "path": "solid-reactivity.review.json",
      "hash": "sha256:9c8b7a6f5e4d3c2b1a0918273645362718094a5b6c7d8e9f0a1b2c3d4e5f6071"
    },
    "state": {
      "path": "solid-reactivity.review-state.json",
      "hash": "sha256:5d4c3b2a19087f6e5d4c3b2a1908f7e6d5c4b3a29180f7e6d5c4b3a291807f6e"
    }
  },
  "reviewers": [
    { "id": "review-key:2026-a", "role": "maintainer", "packageAuthor": false }
  ],
  "checkerRange": ">=0.1.0",
  "published": "2026-08-22T09:02:11Z"
}
```

The entry hashes all three published files because all three are what a consumer
audits. The plan is itself bound to the contract it was derived from, so a plan
and a state cannot be presented as the review of some other document.

And the index, deliberately shaped as an open, self-describable spec so third
parties can host their own:

```json
{
  "$schema": "https://solid-checker.dev/schema/registry.json",
  "spec": "solid-checker-registry/1",
  "name": "@solid-checker",
  "homepage": "https://github.com/solidjs-community/solid-checker-contracts",
  "entries": "packages/{name}/{version}/{integrity}.entry.json",
  "signatures": "packages/{name}/{version}/{integrity}.entry.sig",
  "revocations": "revocations.json",
  "trust": "reviewers/trust.json"
}
```

`{name}`, `{version}`, and `{integrity}` are the resolution template, directly
analogous to shadcn's `{name}` URL pattern — with one deliberate divergence. In
shadcn's `components.json` the consumer configures the URL *template*; here the
consumer configures only the address of the index, and the templates come from
the index itself, because our index must also say where revocations and the
registry's key set live. A `registries` entry in
`.solid-checker/registries.json` (§3) is therefore a namespace mapped to a
`registry.json` URL — as a bare string, or as an object when a private registry
needs credentials, with `${ENV_VAR}` expansion exactly as shadcn does it:

```json
{
  "registries": {
    "@solid-checker": "https://raw.githubusercontent.com/…/main/registry.json",
    "@acme": {
      "url": "https://contracts.acme.internal/registry.json",
      "headers": { "Authorization": "Bearer ${ACME_CONTRACT_TOKEN}" }
    }
  }
}
```

The headers configured for a namespace are used for every request resolved
through that index, so a private registry needs its credential written down
once.

An internal registry of contracts for private packages is then a normal
configuration rather than a fork, and **the trust set decides what certifies,
not the transport**: an entry from `@acme` certifies exactly when it is signed
by an identity the project trusts.

**The registry's CI runs machinery that already exists.** For every proposed
entry:

- `solid-checker --validate-contract` on the contract document;
- artifact hash verification for any `artifacts` the contract records;
- a regeneration diff: install the exact claimed artifact, run
  `contract generate --package-root`, and compare the *export surface* against
  the submission, so a contract cannot claim exports the package does not have.
  (The regeneration proves the surface; it cannot prove the semantics — that is
  what the published review state is for, and the asymmetry is the same one
  `make contracts-check` already lives with for dialect contracts.);
- refusal of any `inferred` row evidence or `unknown` sentinel in a document
  claiming a certifying evidence kind;
- refusal of a review state that does not resolve every item of the plan it was
  produced from, and of a plan that is not the one the submitted contract yields
  — in particular an unresolved "callbacks with no execution row" item, which
  would publish an unexamined negative claim, or an unresolved "entrypoints the
  generator refused" item, which would publish a partial contract as if it
  described the whole package.

Git hosting supplies transport, history, and audit trail for free. A signed HTTP
service can replace the transport later **without changing the client**, because
the client only ever consumed content-addressed, signature-verified files
resolved through a template.

### 8. Composition with bundled contracts

The registry and a bundled ecosystem corpus are not competing proposals. The
registry is where reviews live and are governed; bundling is a *snapshot of the
top of it*, shipped with each `solid-checker` release for zero-configuration
coverage of the most-imported packages. They compose.

One hazard has to be handled explicitly. A fetched contract lands in the local
tier, and project-owned contracts override contracts from `node_modules` and the
bundled artifacts. A fetch for a package this checker bundles would therefore
shadow the checker's own audited artifact with a registry one. `contract fetch`
refuses such packages by default, names the bundled contract in the refusal, and
requires `--override-bundled` to proceed — recording the override in the lock so
it is visible in review.

### 9. Dependencies and sequencing

**Artifact-keyed review transfer is a hard dependency for maintainability.**
Because a contract binds to one artifact, every upstream release turns every
registry entry for that package into a `stale` contract for anyone who upgrades.
Without a mechanism to re-review only the *diff* between two releases, the
corpus rots on every publish and the maintainer burden grows with ecosystem
velocity rather than with ecosystem size. That mechanism now exists in the review
command: `contract review <new> --transfer-from <old>` carries the previous
review's resolutions onto a regenerated contract for every entrypoint whose
runtime-module closure is byte-identical to the one the reviewer resolved
against, leaving every other item open (see
[package-contracts.md](../package-contracts.md#transferring-a-review-to-a-regenerated-contract)).
The registry is usable without it and is not *maintainable* without it. What the
transfer gate can and cannot witness about those bytes is the residue recorded in
unresolved question 9.

**Seeding is review labor, not generation.** The benchmark's 403-of-416 complete
contracts are drafts. Seeding means reviewing the most-imported Solid-aware
packages first, and the prioritization needs a number rather than an intuition
(see unresolved questions).

## Prior art

**shadcn/ui's registry — the primary UX model.** `npx shadcn add <item>` fetches
an item from a registry and *vendors the source into the consumer's repository*,
where the consumer owns it, reviews it in a pull request, and retains no runtime
dependency on the registry. The registry is an open spec rather than a service:
[`registry.json`](https://ui.shadcn.com/docs/registry/registry-json) carries
`$schema`, `name`, `homepage`, and an `items` array (plus `include` for composing
several files), and each item conforms to
[`registry-item.json`](https://ui.shadcn.com/docs/registry/registry-item-json)
with `name`, `type`, `files`, `dependencies`, and `registryDependencies`.
[Namespaces](https://ui.shadcn.com/docs/registry/namespace) are decentralized —
`@namespace/item`, configured in `components.json`'s `registries` field as a URL
template containing `{name}`, with optional `headers` and `${ENV_VAR}` expansion
for private registries. That is the model this RFC copies almost directly:
fetch-time vendoring, consumer ownership, an open index spec, and third-party
registries without a central authority. The one place we must diverge is trust —
shadcn items are code the consumer reads, whereas a contract's effect is to make
findings *not appear*, which is far harder to review by eye. Hence signatures, a
trust set, and a lock, none of which shadcn needs.

**TanStack Intent.** Announced 2026-03-04 by Sarah Gerrard and Kyle Mathews,
`@tanstack/intent` is a CLI for library maintainers to generate, validate, and
ship *Agent Skills* — markdown guidance for AI coding agents, in the open
[agentskills.io](https://agentskills.io) format — alongside their npm packages.
The distribution model is deliberately **not** a registry: skills ship inside the
package tarball, so the guidance is versioned by the package version and travels
through npm, and `npx @tanstack/intent install` discovers intent-enabled packages
through the dependency graph and wires their skills into agent configuration.
Maintainers validate with `intent validate` and detect drift with `intent stale`,
in CI. There is a browsable index at `tanstack.com/intent/registry`, but
resolution for consumers goes through installed packages rather than through it.

Intent is therefore the closest existing analogue of our **published** tier, and
it is instructive in two directions. In its favor: shipping in the tarball gets
version binding for free and needs no trust model beyond the one npm already
provides — the maintainer who publishes the code publishes the guidance. Against
it, for our purposes: it requires every maintainer to opt in, it makes the
package author the sole author of claims about their own package, and its
artifacts are advisory prose rather than machine-consumed suppressors of
diagnostics, so a wrong skill degrades an agent's suggestions while a wrong
contract silently removes findings. Intent's `stale` command is the piece worth
borrowing outright: an explicit, CI-runnable notion of "this artifact no longer
matches the source it describes" is exactly what artifact-keyed review transfer
needs.

**DefinitelyTyped.** The governance model, adopted nearly wholesale: PR-reviewed
publication into a community repository, a maintainer set with merge authority,
anyone may propose, package authors have a fast path but not a bypass.
DefinitelyTyped also demonstrates the failure mode we are sequencing against —
type definitions that drift from the packages they describe, at a rate set by
upstream release velocity — which is why §9 treats review transfer as a
dependency rather than a nicety.

**RustSec advisory-db and `cargo-audit`.** A git repository of TOML-front-matter
advisories, fetched by a client that audits `Cargo.lock`. The parallels are
exact and encouraging: a git repository as the MVP transport, the *lockfile* as
the resolution input rather than the manifest, and an offline audit against a
locally cached database. The important inversion is polarity. An advisory
database is *additive* — a missed advisory means a missed warning, and the
default state is quiet. A contract registry is *subtractive* — a wrong contract
means a suppressed finding, and the default state without one is already
fail-closed. That is why our entries are signed and content-addressed while
advisory-db's are not, and why revocation is in our MVP rather than a later
concern.

**npm provenance and Sigstore.** npm's `--provenance` flow requires an OIDC token
from a supported CI, requests a short-lived signing certificate from Fulcio
binding the key to that CI identity, and records the attestation in the Rekor
transparency log; `npm audit signatures` verifies registry signatures and
provenance attestations against an installed tree or a lockfile. It fits our
signature story well and is the intended second step (§3): the identity being
attested is a review job rather than a person holding a long-lived key, and the
transparency log answers "was this entry ever really merged". It is not the MVP
because it imports a substantial client dependency, and because our client's
obligations — verify a hash, verify a signature over a canonical entry — are
unchanged by which signature scheme satisfies them.

## Alternatives considered

**Fetch at analysis time.** Rejected. Analysis would stop being offline,
deterministic, and reproducible; the analyzer would acquire a network attack
surface and a failure mode with no correct answer (what should a checker report
when the registry is unreachable — the last cached claim, or nothing?); editor
runs would take a network round trip; and CI results would depend on a remote
service's current state rather than on the repository's contents. The vendoring
model has none of these properties, and the thing that makes vendoring
acceptable — the consumer reviews and commits the artifact — is also the thing
that makes the trust story tractable.

**Open uploads with post-hoc moderation.** Rejected. The npm-style model, where
publication is a token and review is a reaction, is wrong for an artifact whose
effect is to *suppress* diagnostics. A malicious package announces itself by
misbehaving; a malicious contract announces itself by nothing happening. There
is no observable signal that would drive post-hoc moderation, so review has to
be a precondition. This does bound throughput to maintainer attention, which is
the tradeoff DefinitelyTyped also makes.

**Loosen the version pin so one contract covers a range.** Rejected, and not a
close call. It contradicts the precision contract directly: a contract describes
an exact artifact, a version string is not a pin (which is why
`package.integrity` exists at all), and a semver range would let a reviewed claim
about 1.5.3 certify 1.5.4's rewritten scheduler. The registry makes re-review
cheaper to distribute; it does not make a stale claim true. The correct response
to "every release invalidates the corpus" is artifact-keyed review transfer
(§9), not a looser key.

**Publish only through the package (grow the `published` tier).** Rejected as
*sufficient*, retained as complementary. It is the best outcome per package —
TanStack Intent's model, with version binding for free — but it requires every
maintainer to opt in, leaves unmaintained packages permanently uncovered, and
makes the package author the sole reviewer of claims that suppress findings in
their own code. The registry covers packages whose maintainers have not opted
in and provides independent review where they have.

**A bundled ecosystem corpus only.** Not rejected — this is §8. Bundling alone
would put all review labor on this project, pin every contract to one release
per checker release, and give the community no way to contribute a review. It is
a good distribution channel and a bad governance model, so it becomes the
registry's release-time snapshot.

## Unresolved questions

1. **Trust-set bootstrap.** A project's first `contract trust import` is a
   trust-on-first-use moment however carefully it is worded. Options include
   shipping the community registry's root keys inside the `solid-checker`
   package (moving the problem to npm's trust, which the user already accepted
   by installing the checker) or requiring out-of-band key confirmation. The
   former is probably right and needs a decision.
2. **Key rotation and reviewer revocation.** Rotating a maintainer key must not
   invalidate the entries it already signed, and revoking a *compromised* key
   must invalidate exactly the entries signed after the compromise — a date the
   registry cannot always establish. What `--refresh` should do with an entry
   signed by a since-revoked key (report, quarantine, or require re-fetch of a
   re-signed entry) is undecided.
3. **Verifier identity in the contract.** §4 defers `evidence.verifier` because
   the loader's unknown-field failure is the outright-malformed path, so the
   field would hard-fail older clients. Whether schema v1 should gain it anyway
   behind a `checkerRange`, or whether identity should stay in the lock
   permanently, is open.
4. **Revocation as a loader status.** Quarantine achieves the right fail-closed
   consequence with no engine change, at the cost of an offline message that
   says "no contract" instead of naming the revocation reason. A first-class
   revoked status would be more informative and is a new loader input.
5. **Private registries and mixed trust.** A project consuming both the community
   registry and an internal one needs a policy for which identities may certify
   which package namespaces. A flat trust set says any trusted key may sign any
   package, which is probably too permissive for an internal deployment.
6. **Scoped-package namespacing.** Registry namespaces (`@solid-checker`,
   `@acme`) and npm scopes (`@solid-primitives/…`) both use `@`, and the
   resolution template puts an npm scope inside a path segment. The collision is
   cosmetic but the spelling needs to be pinned before the index spec is
   published, since it is not changeable afterwards.
7. **Quantitative seeding priority.** "Most-imported Solid packages first" needs
   a measured ordering — npm download counts, imports observed across the
   ecosystem benchmark corpus, or both — before review effort is spent.
8. **Integrity-free ecosystems.** On pnpm and Yarn projects the fetch key
   degrades to `(name, version)` and the loader's integrity-drift refusal never
   engages. Whether `contract fetch` should refuse outright there, or proceed
   loudly as proposed, depends on how much of the user base that excludes.
9. **What review transfer still cannot witness.** The mechanism §9 depends on
   exists, so the open question is its soundness residue rather than its
   arrival. Transfer is gated on byte identity of a *generator-side* record: the
   closure is walked by scanning each runtime module's source for relative
   `import`/`export`/`require` specifiers, not by asking a compiler what the
   entrypoint actually loads. Extending the TypeFacts protocol to return the
   module list it resolved is the exact fix, and it is the only thing that would
   make the record attested rather than approximated. The failure direction is
   conservative for every specifier the walk can see: a target whose closure
   could not be walked, a module whose bytes were unreadable, a relative or
   `#` specifier that resolves to nothing, and a dynamic `import()` of a
   non-literal all leave a closure note, and a noted record never transfers.
   What remains approximated is the walk itself — a module reference the
   scanner does not recognize as one leaves no note, because nothing saw it.
   Nobody has measured how often real packages hit the noted paths, and that
   frequency is what the corpus's steady-state maintenance cost actually
   depends on.

## Staged adoption plan

**Stage 0 — index and entry specification.** Publish
`schema/registry.json` and `schema/registry-entry.json`, and the canonical
serialization used for signing. No client work. This is the piece third parties
need in order to host anything, and it is the piece that is expensive to change
later.

**Stage 1 — `contract fetch` against a local directory.** Implement resolution,
integrity recovery, entry verification, contract hashing, `contracts-lock.json`,
and the write into the local tier, with the registry configured as a `file:`
path. Everything except transport is exercised, and the whole stage is testable
with fixtures and no network.

**Stage 2 — the registry repository and its CI.** Stand up the git repository,
the maintainer set, and the CI gates of §7 — all of which reuse existing
machinery. Seed it with a small number of contracts whose review is already
done, including `@solid-primitives/scheduled@1.5.3`, which this checker already
bundles for Solid 1.x and which therefore exercises §8's bundled-override
refusal.

**Stage 3 — signatures, trust set, and revocation.** Add Ed25519 entry
signatures, `contract trust import`, `revocations.json`, and `contract check
--refresh` with quarantine. Until this stage lands, `contract fetch` refuses to
run against any non-`file:` registry: an unsigned fetch over HTTP is exactly the
threat model's compromised-transport case.

**Stage 4 — scale.** Third-party registry configuration, `--all`, the
release-time bundled snapshot of §8, and the Sigstore migration. Each is
independent of the others and none changes the client's contract with the
registry.

## Sources

- [shadcn/ui — registry.json](https://ui.shadcn.com/docs/registry/registry-json)
- [shadcn/ui — registry-item.json](https://ui.shadcn.com/docs/registry/registry-item-json)
- [shadcn/ui — Namespaces](https://ui.shadcn.com/docs/registry/namespace)
- [TanStack — Introducing TanStack Intent: Ship Agent Skills with your npm Packages](https://tanstack.com/blog/from-docs-to-agents)
- [TanStack/intent on GitHub](https://github.com/tanstack/intent)
- [RustSec advisory-db](https://github.com/rustsec/advisory-db)
- [cargo-audit](https://github.com/rustsec/rustsec/tree/main/cargo-audit)
- [GitHub Blog — Introducing npm package provenance](https://github.blog/security/supply-chain-security/introducing-npm-package-provenance/)
- [Sigstore Blog — cosign verification of npm provenance](https://blog.sigstore.dev/cosign-verify-bundles/)
