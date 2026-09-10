# The policy-2 fixture corpus, and the findings delta it makes measurable

- **Status:** landed. `the_catalog_bearing_fixtures_mint_a_policy_2_corpus` in
  `contract_closure_process.rs`.
- **Date:** 2026-09-10.
- **Result:** with contracts actually authorized, the same fourteen fixture
  projects produce **32 rule findings and 10 SC9005**, against a baseline of
  **16 and 28**. Real third-party reactive-misuse findings double; the
  uncertifiable count falls by two thirds.

## 1. What was in the way

Every accepted catalog in the tree carried an `obsolete-policy1` receipt. A
policy-1 catalog is rejected before any claim is read, so its SC9005 is the
obsolete-policy rejection path and no contract claim is ever consulted.
[§ 7 of the findings-delta measurement](../2026-09-10-findings-delta-measurement.md)
concluded re-certifying them was "not possible, and the reason is structural
rather than a matter of effort".

That was true of the *certification* route — commit `662dd7ba` deleted the
receipts and the fixtures cannot be re-certified against real published
bytes. It is not true of the *authorization* route, which is all a consumer
needs: the catalog's own `import` block is a valid resolver answer, and only
its policy-1 authorization is obsolete.

The test-scoped issuer and `policy2_main_closed_claims_root`, both landed
this morning, are what make replacing just the authorization possible.

## 2. What the corpus is

For each catalog-bearing fixture with a `tsconfig.json`: copy the tree, reuse
its `import` block verbatim, canonicalize the contract it already ships, mint
a policy-2 receipt over it with the test-scoped issuer, publish, and analyze
with the trust configuration supplied out of band.

**Fourteen of sixteen mint.** The two that do not publish more than one
contract, and `publish_policy2_catalog` writes a catalog holding exactly one —
reported rather than worked around, because hand-assembling a multi-entry
catalog would make the test assert its own idea of the on-disk shape.

The receipt's roots are shape-valid stand-ins, as they were for the single
fixture this generalizes: this replaces the authorization, and the *closed
claims root* is rebound from the document itself so a test issuer cannot
assert a closure the contract does not carry.

## 3. The delta

Same fourteen projects, obsolete baseline (the checked-in snapshots) against
the minted corpus:

| | obsolete | policy-2 |
| --- | --- | --- |
| findings total | 44 | 42 |
| of which SC9005 | 28 | **10** |
| of which rule findings | 16 | **32** |

The total barely moves and the composition inverts. What replaces the
obsolete-policy rejections is not silence — it is proven third-party reactive
misuse:

~~~
package-consumer                     prefer-for, strict-read-untracked
package-callback-consumer            missing-owner, strict-read-untracked
package-parameter-member-consumer    reactive-dispatch-unresolved, strict-read-untracked ×2
package-store-destructure            no-destructure
package-structured-return            strict-read-untracked ×6
v1-reactivity                        15 findings across 9 rules
~~~

Ten SC9005 remain, and they are now the honest kind — a claim the contract
leaves open — rather than a rejected authorization.

## 4. What this corrects

The morning's findings-delta measured **zero** improvement from contracts and
generalized it to the whole corpus. That measurement was correct about what
it measured and explicit that the fixture corpus was stale — "15 of the 17
added findings are the obsolete-policy rejection path … that says the fixture
corpus is stale; it says little about contract value."

It is now unstale, and contract value is measurable and positive: **contracts
double the rule findings on this corpus.** The morning's headline — that
contracts make consumer verdicts strictly worse — was an artifact of
authorization, not of the contract pipeline.

## 5. What it still does not settle

- **`reads` demand is still unmeasured.** The corpus is demand-evaluating now,
  which is the prerequisite
  [§ 3 of the demand-population measurement](2026-09-10-reads-demand-population.md)
  named, but the scoping predicate does not exist, so there is still no
  "after" to compare against.
- **These are fixtures.** Fourteen synthetic projects with hand-written
  contracts. The one real certified consumer remains the retained `seroval`
  case.
- **Two fixtures are unminted**, and both exercise multi-contract catalogs —
  precisely the shape a real multi-entrypoint package produces.
