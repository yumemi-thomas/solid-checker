---
status: deferred
---

# A dialect axiom about its own audited package

<!--
`deferred` is a new value in this directory's front-matter vocabulary; ADRs
0001-0004 are all `accepted`, and `docs/rfcs/0002-machine-verified-contracts.md`
is `superseded by ADR-0003`. It means: the problem and the sound *form* of the
premise are recorded, no code ships, and the required shape of a future version
is stated below so a later round starts from the objections rather than
rediscovering them.
-->

## The structural wall this would answer

Certification proves an export's contract from evidence in the artifact under
certification. For an owner requirement the evidence is a reachable, uncaptured
call to an exact `solid-js` dialect primitive whose name maps through
`solid_dialect::unambiguous_owner_requirement_role` to the demanded role
(`require_owner_operation_call`,
`rust/crates/solid-facts-backend/src/contract_certification/type_facts.rs:4015`;
the module gate at `:4036`).

That evidence cannot exist inside the package that *defines* the primitives.
`@solidjs/signals`' `onSettled` is the function `solid-js` re-exports under that
name — `solid-js@2.0.0-rc.3/types/index.d.ts` is one
`export { … } from "@solidjs/signals"` — and its own body calls `getOwner`,
`createTrackedEffect`, `untrack` and `enqueue` as locals of its own bundle. No
callee in it has `target_module == "solid-js"` and none ever will. The refusal

    owner requirement has no exact dialect primitive call;
    observed ["getOwner", "createTrackedEffect", "untrack", "enqueue", "e"]

is therefore structural, not a missing producer fact, and no amount of producer
work removes it. The row is
`@solidjs/signals@2.0.0-rc.3|solid2|only`, demand
`sha256:78a165588bd85e84dd94c2598e9271279c741475da2089d5d6e93f3164cbbbca`.

## The sound form of the premise

If a dialect may state a premise about anything, it is about itself. The shape
that would be defensible:

> When the artifact under certification **is** a dialect's own
> primitive-defining package, at exactly the identity whose bytes that
> dialect's tables were audited against, and the export under certification is
> a canonically spelled primitive of that dialect whose audited tables
> establish the demanded owner-requirement role, the demand is discharged with
> a witness naming the axiom rather than a source location —
> `dialect-axiom:<package>@<version>:<export>:<role>`.

The premise is an identity claim, so every gate on it must be an identity gate,
and the package identity must come from the authenticated artifact snapshot —
never from a filesystem path, an import specifier, or a producer-reported
module name.

The dialect tables are the right owner for this: they are not documentation.
`rust/crates/solid-dialect/src/solid_2.rs` records, per table, the exact
published bytes each row was read from, so an axiom over those rows restates an
audit rather than extending it — *provided* the identity in the axiom is the
identity in the citations. That proviso is where the shipped attempt failed.

## Why it did not ship

A first implementation was written and reverted whole. Five confirmed
objections; the first two are soundness defects, and either alone is
disqualifying.

### 1. The identity gate was name + version, where every neighbour is name + version + integrity

The attempt compared `plan.snapshot.package_name()` and
`plan.snapshot.package_version()` against a static table and nothing else. Every
neighbouring identity site in this subsystem binds integrity as well:

- `rust/crates/solid-facts-backend/src/contract_certification.rs:975-993` — the
  `ProofFamily::PackageIdentity` witness roots and sites carry
  `package_name`, `package_version`, **`package_integrity`**, `snapshot.root()`
  and `provenance_root()`.
- `rust/crates/solid-facts-backend/src/contract_certification/dependencies.rs:1226-1246`
  — the lock replay refuses on disagreement in name, version, **or integrity**.

The audited SRI already exists and is checked in:
`pkg/contracts/bundled/solid-v2/solidjs-signals.json:38-46` carries
`package.integrity` `sha512-/yPhTf3xS1FRR4MX8kTYCd4MjsFxzwkO+KyOTfbu35lTEiaJ4Fxy+JL91XonDzt31GV1mYaZ9CGD2TQIzvXuNA==`
alongside the manifest digest, name and version. So a name+version gate is
strictly weaker than the evidence on hand, and the failure mode is concrete:
any registry or mirror serving a *self-consistent* `@solidjs/signals@2.0.0-rc.3`
coordinate — a snapshot that authenticates against whatever integrity that
registry states — receives the axiom on bytes nobody in this repository ever
read. That is precisely the "unread bytes trusted by coordinate" class the
precision contract exists to prevent.

### 2. The version gate was inverted against the dialect's own citations

The attempt keyed the axiom to `2.0.0-rc.3` and refused rc.0 and rc.5, on the
stated ground that rc.3 is "the tuple the audit read". The two dialect rows the
axiom actually rested on cite **rc.0**:

- `rust/crates/solid-dialect/src/solid_2.rs:247` — *"Source: rc.0 `onSettled`
  (`dev.js:4855-4893`)"*, for `leaf_owner_requires_owned_call_site`
  (`solid_2.rs:255`).
- `rust/crates/solid-dialect/src/solid_2.rs:315` — *"Source: the
  `solid-js@2.0.0-rc.0` and `@solidjs/web@2.0.0-rc.0` implementations, read
  rather than inferred"*, for `callback_owners` (`solid_2.rs:334`), the row that
  gives `onSettled`'s slot 0 `CallbackOwner::Leaf` (`solid_2.rs:365`).

So for the *ownership* question rc.0 is the audited runtime and rc.3 has no
ownership citation at all. (rc.3 is cited elsewhere in the same file, at
`solid_2.rs:452-464`, but for `reactive_result_slot` — the signal/memo return
shapes — not for owners.) An independent check confirms the citations are
against different bytes: `onSettled` sits at line 6064 of
`@solidjs/signals@2.0.0-rc.3`'s `dist/dev.js`, not at `4855-4893`.

The attempt therefore granted the axiom to the one version with no ownership
audit and refused the version the audit was performed on. It was not narrow; it
was pointed the wrong way.

### 3. The axiom was floor-blind

`require_owner_operation_call` consults `floor.admits(call.reach)`
(`type_facts.rs:4036`) so that a `Reachable`-floor demand — one whose
cardinality states a lower bound above zero — is answered only by a call the
implementation provably reaches, while `MayExecute` admits `Reachable` or
`Unknown` (`ReachabilityFloor::admits`, `type_facts.rs:3278-3285`;
`operation_reachability_floor`, `:3310`). The attempt placed the axiom after the
scan loop, at the `if !found` site (`type_facts.rs:4069`), where the floor is no
longer in scope. It would therefore discharge a `Reachable`-floor demand against
a transcript whose only owner call is `Unreachable` — a strictly stronger claim
than the ordinary premise makes from the same bytes.

### 4. The owner claim may derive from the same rows that would discharge it

Unsettled, and it must be settled before any version ships. In the generated
proposal the `onSettled` create operation's requirement is produced by
`apply_owner_requirement`
(`rust/crates/solid-facts-backend/src/inferred_contract.rs:433-445`): it sets
`owner.requirements.owner = Required` and `source = AmbientAtCall`
unconditionally, and maps a `ContractOwnerRequirement` whose operation is
`Effect` or `Boundary` to `child_owners = Required`. That `Effect` comes from
`project_owner_requirements`
(`rust/crates/solid-reactive-ir/src/contracts.rs:320-342`), which sends every
non-cleanup create there. The demanded `child_owners = Required` and the
`Effect` role the axiom would have matched are then two spellings of one
derivation, and the "proof" is the premise restated.

Sharper still, the **audited** contract disagrees with the generated one about
whether the operation exists. In
`pkg/contracts/bundled/solid-v2/solidjs-signals.json`, `onSettled`'s summary
(`:356`, referenced from `:25`) has `"creates": []` **closed** (`:381`) and
models the leaf owner as a `resources` entry, `onSettled-leaf-owner` (`:454`) —
no `owner-requirement-*` create operation at all. The generated proposal for
the same bytes invents one. Whatever a future round does here, it cannot
discharge a demand the audited authority for those exact bytes says is not
there.

### 5. Zero rows moved

With the axiom in place the signals row does not certify: its first refusal
advances one demand, to `argument-binding`
`sha256:40cc236b1354cce01e555595129f2873232a80c1f62beafa3f925fef1b82ae34`, still
on `onSettled` — *callback parameter has no exact direct-call or
resolved-argument flow*, the same structural wall in a different family, since
`onSettled` hands its callback to `untrack`, another local of its own bundle. No
other corpus row changed verdict. So the change bought no certification and cost
two soundness defects; there is no "ship it narrowly and fix the gate later"
version of that trade.

## Required shape of a future version

Every item is a precondition, not a suggestion.

1. **Integrity-bound tuple.** The audited table row carries name, version *and*
   the SRI from `pkg/contracts/bundled/solid-v2/solidjs-signals.json:39`, and
   the gate compares all three against `plan.snapshot.package_integrity()`, the
   same field `contract_certification.rs:981` and `dependencies.rs:1235`
   already bind. A snapshot that matches the coordinate but not the integrity
   refuses, and says which field disagreed.
2. **Version keyed to the actually-audited runtime bytes.** Either key the row
   to rc.0, the runtime `solid_2.rs:247` and `:315` cite for the two ownership
   rows, or re-read rc.3's `dist/dev.js` and update both citations to it —
   updating the citations is a dialect audit with its own review, not a
   comment edit. The table and the citations must name the same bytes; a future
   reader must not have to check which.
3. **`floor == MayExecute` only.** The axiom is reached with the floor in scope
   and refuses under `ReachabilityFloor::Reachable`, so it can never answer a
   demand asserting a lower bound above zero. A test pins the `Reachable`-floor
   refusal.
4. **A contract-corpus fixture named `@solidjs/signals@2.0.0-rc.3` exercising
   `from_plan`.** The identity derivation is the half no unit test in this
   repository can reach — nothing here builds a `CertificationPlan` — so it
   needs a corpus fixture. That fixture is also the live demonstration of
   objection 1: the corpus fabricates integrity, so a fixture claiming the
   audited coordinate must be *refused* by an integrity-bound gate and would be
   *accepted* by the name+version gate that was written. Both directions get
   pinned.
5. **The generator's evidence for the owner claim named.** State where the
   proposed `owner.requirements` for a primitive-defining package's export comes
   from and show it is not the dialect rows the axiom would use, or reconcile
   the generated proposal with the audited bundled contract's closed empty
   `creates`. Until that is written down, the discharge cannot be distinguished
   from circular.

Until all five hold, `@solidjs/signals@2.0.0-rc.3|solid2|only` stays refused on
`sha256:78a16558…` and the wall above is the recorded explanation.
