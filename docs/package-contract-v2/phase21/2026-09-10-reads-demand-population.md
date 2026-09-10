# How many call sites still demand `reads` after scoping?

- **Status:** measurement, and it is a negative one. No production code changed.
- **Date:** 2026-09-10.
- **Question:** the `returns` demand-scoping slice raises an obligation only
  where a consumer can read the result. If `reads` were scoped the same way,
  how many call sites would still demand it?
- **Answer:** **not measurable from the corpus that exists**, and the reason
  is structural rather than a matter of effort. What *is* measurable is
  recorded below, along with what it would take.

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
