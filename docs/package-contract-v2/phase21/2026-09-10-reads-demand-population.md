# How many call sites still demand `reads` after scoping?

- **Status:** measurement, and it is a negative one. No production code changed.
- **Date:** 2026-09-10.
- **Question:** the `returns` demand-scoping slice raises an obligation only
  where a consumer can read the result. If `reads` were scoped the same way,
  how many call sites would still demand it?
- **Answer at the time:** not measurable, for the structural reason in § 2.
- **Answered (2026-09-10, § 5):** with the policy-2 corpus minted, **zero of
  thirteen** projects consult the projection.

## 1. Who demands `reads`

The [rule × claim-domain matrix](../2026-09-09-rule-demand-proposal.md) § 1.2
marks `reads` required for six rules. One of them is
`package-contract-incomplete` itself, which is circular: SC9005 demands the
domain in order to report that the domain is open. The five real consumers:

| rule | corpus findings |
| --- | --- |
| `strict-read-untracked` (+ v1) | 110 |
| `components-return-once` (+ v1) | 13 |
| `reactive-dispatch-unresolved` (+ v1) | 11 |
| `prefer-for` (+ v1) | 0 |
| `prefer-show` (+ v1) | 0 |
| **total** | **136 of 548** |

37 of 94 fixture projects carry at least one finding from those rules.

**That is rule activity, not demand.** A rule demands the domain when its
proof consults a bound import's `reads` projection, whether or not it then
fires. So 136/548 and 37/94 bound how *involved* these rules are; neither is
the number asked for.

## 2. Why the corpus cannot give the number asked for

Demand for a contract domain arises only at a bound import of a **certified**
package. In this repository:

- 20 fixture trees carry an accepted catalog; 16 have a `tsconfig.json` and
  are analyzable.
- **All 20 catalogs are `obsolete-policy1`.**

A policy-1 catalog is rejected before any claim is read, so its SC9005 is the
obsolete-policy rejection path and no open-claim demand is ever evaluated —
the same fact [§ 2 of the findings-delta measurement](../2026-09-10-findings-delta-measurement.md)
records from the other direction. The corpus therefore contains **zero call
sites at which reads demand is evaluated at all**, before or after scoping.

The one real certified consumer is the retained `seroval` case, and it did
demand `reads` — an n of 1, on a hand-written consumer.

## 3. What it would take

Two things, in order:

1. **A policy-2 fixture corpus.** Newly possible: the test-scoped issuer and
   `policy2_main_closed_claims_root` landed this morning, and
   `contract_closure_process.rs` already mints a receipt for one fixture.
   Minting catalogs for the other catalog-bearing fixtures makes their
   imports demand-evaluating for the first time.
2. **The scoping predicate itself**, on the `returns_shed_symbols` template:
   a bound symbol sheds `reads` when no reference of it reaches a position
   any of the five rules inspects. Until that exists there is nothing to
   measure *after*.

Only then does "how many call sites still demand `reads`" have an answer, and
the number will be a property of those five rules' source discovery rather
than of the contract pipeline.

## 4. What this does to the strategic question

It does not answer whether human-less certification can finish; it says the
evidence for the cheap route is not in hand. The argument for scoping over
solving `reads` still rests on the demand matrix's five rules and on one
worked example (`returns`), not on a measured consumer population.

Anyone quoting "only 6 of 29 rules need `reads`" — including this document's
author, earlier today — should note it is a count of rules, not of call
sites, and that the two differ by however much source discovery propagates.

## 5. Answered: zero of thirteen

The blocker in § 2 was the corpus, and
[minting it](2026-09-10-policy2-fixture-corpus.md) removed it. The measurement
is `how_many_corpus_projects_consult_a_reads_projection`, and it works by
difference: each project is analyzed twice against its own minted catalog,
once as its contract stands and once with `reads` reopened in the document.

`package-contract-incomplete` is excluded from the comparison. It demands the
domain by definition — it exists to report that a claim is open — so
reopening one always moves it and it says nothing about whether a *rule's*
proof depended on the claim.

**Not one project's rule findings change.** Every finding the corpus produces
survives `reads` being open. All sixteen fixture contracts close the domain,
so the reopen is meaningful in each; one project is excluded because
reopening moved nothing at all (`package-unknown-export` imports an export
the contract does not describe, so it is uncertifiable for a reason no claim
can change), and for the remaining thirteen the reopen is verified to have
taken by watching SC9005 move.

### What that means

On this corpus, **SC9005 is the only consumer of the `reads` projection.**
Nothing else asks. `strict-read-untracked` fires in several of these projects
and fires identically with the domain open, so whatever reactive identity it
uses, it is not coming from a closed `reads` claim.

That is the case for scoping rather than serving, and it is now measured
rather than argued from a rule count. The domain that cannot close in bulk —
because no synthesized veto can observe a read of a source the export owns —
is demanded by exactly one rule, and that rule demands it of every import
regardless of what the consumer does.

## 6. The matrix tension, resolved — and it was my measurement, not the matrix

The paragraph that stood here said the result sat in tension with the
[demand matrix](../2026-09-09-rule-demand-proposal.md) § 1.2, which marks
`reads` required for five rules, three of which fire in this corpus without
moving. The matrix is right. The measurement above answers a narrower
question than the words I attached to it.

**A `reads` claim carries two separable things**: the operations it
enumerates, and the proof that the enumeration is complete. *Reopening* the
domain drops the completeness and leaves the items in place. So a rule that
consumes a read operation still receives it from a reopened claim, and
"nothing moved" says only that nothing depends on the *closure*.

Measured the other way — strip the read operations and leave the domain
closed — **exactly the two fixtures that state a positive read operation
change**: `package-consumer` and `package-parameter-member-consumer`. The
other eleven enumerate no reads, so there is nothing to remove.

| what is removed | projects whose rule findings change |
| --- | --- |
| the completeness proof (domain reopened) | **0 of 13** |
| the read operations (items stripped) | **2 of 2 that have any** |

So: **rules consume `reads` items; only SC9005 consumes `reads`
completeness.**

### Why this makes the case stronger, not weaker

The information rules need arrives whether or not the domain is closed. What
cannot be obtained in bulk — completeness, which needs a hand recipe per
export because no synthesized veto can observe an owned read — is demanded by
exactly one rule, and that rule demands it of every import regardless of what
the consumer does.

A contract stating `reads` items without closing the domain would therefore
serve every rule in this corpus completely, and cost only SC9005. That is a
sharper target than "scope the demand": it is one conjunct, in one predicate.

### What it still does not settle

Thirteen synthetic fixtures with hand-written contracts, and only two of them
state a read operation at all — a thin basis for the second row of that
table. A real consumer may differ; the retained `seroval` case reported
`reactiveReads`, though that too was SC9005 and so the same single consumer.
