# An accepted contract a fixture can supply

- **Status:** landed. `solid_facts_backend::fixture_authorization`,
  `solid-contract-authorize`, and the authorization path in
  `scripts/coverage.mjs`.
- **Date:** 2026-09-15.
- **Result:** the snapshot corpus can analyze a fixture against a contract the
  checker **accepted**. The first one,
  `fixtures/reactive-ir/package-merged-props-consumer`, measures ADR 0109's
  consumer arm, which had been recorded as correct-by-construction.

## 1. What was actually in the way

Not the absence of a minting mechanism. `the_catalog_bearing_fixtures_mint_a_policy_2_corpus`
([2026-09-10](2026-09-10-policy2-fixture-corpus.md)) has minted policy-2
receipts over fixture catalogs for five days. Three things it could not do:

- **It is a Rust process test**, so what it proves lives in its own assertions.
  The repository's "no finding moved" instrument is `fixtures/findings-snapshots/`,
  and every snapshot of a contract-consumer fixture records the *rejection* path.
- **It requires a pre-existing `obsolete-policy1` catalog.** A new fixture has
  none, and writing one would be a fixture claiming a history it does not have.
- **It reuses whatever document the fixture already ships.** Fine for measuring
  the fourteen; useless for pinning a claim that needs a document stating it.

So three separate claims were written down as unmeasurable — the
`returns_reactive_tuple` row, ADR 0109's consumer arm, and the ownership
filter's `SC4001`-versus-`source: created` pair — each with the same sentence
about a receipt a fixture cannot forge.

## 2. What a fixture supplies, and what it does not

A fixture that wants an accepted contract ships
`.solid-checker/authorize-contract.json`:

~~~json
{ "document": "node_modules/<pkg>/solid-reactivity.json", "import": { … } }
~~~

The `import` block is the resolver answer the sixteen older fixtures have always
hand-written, and it is reused byte for byte. **Only the authorization is
minted.** Nothing here invents a resolution, a closure digest or an integrity.

It does *not* ship a catalog. Un-authorized — a plain
`solid-checker-rust --project` over the directory — the project simply has no
accepted contract, which is the truthful baseline for a package nobody
certified. This is why the request is a separate file rather than a catalog with
a `status` field: a new fixture claiming `obsolete-policy1` would be asserting
it was once accepted under a policy that no longer exists.

## 3. Why a fixed signing key is not a forgery

The issuer is `solid-checker-fixture` with a constant seed, checked into the
source. That is sound for exactly one reason, and the reason is structural
rather than a matter of care: **the trust configuration is never written into
the project.** A catalog does not reference trust bytes — `contract_interface.rs`:
"Trust bytes are deliberately not referenced by the project catalog" — so a
signature buys nothing until a verifier is *separately* told to trust the key,
and nothing a fixture can commit does the telling.

Measured rather than asserted. The same authorized tree, same receipt, one flag
apart:

~~~
--receipt-trust-configuration …   violation, SC1001 strict-read-untracked
(omitted)                         refused: policy-2 acceptance receipt requires
                                  authenticated issuer provenance
~~~

`an_authorized_catalog_is_refused_without_the_trust_configuration` pins both
directions. `solid-contract-authorize` additionally refuses a `--trust-output`
path *inside* the project, because a trust file there would be inert and would
read as though a project could authorize itself.

Every other receipt binding is a shape-valid stand-in, as it was for the
fourteen. The exception is `closed_claims_root`, rebound from the canonical
document, so a fixture issuer cannot assert a closure the contract it signs does
not carry.

## 4. Why the gate works on a copy

The receipt binds the **absolute importer path** the consumer will itself
compute. Three consequences, all of them forced:

- An authorized tree is bound to where it sits, so a receipt cannot be committed.
- Authorizing in place would move the fixture's directory digest underneath the
  gate cache mid-run and race the other 94 projects.
- A copy sits at a different depth, and dialect selection takes the nearest
  `node_modules/solid-js/package.json` *above* the project. So an authorized
  fixture must ship its own stub; coverage refuses one that does not, by name,
  rather than letting it silently change catalog.

Coverage copies to `rust/target/fixture-authorization/<id>/`, writes the trust
beside it, analyzes there, and respells the finding paths as the fixture the
snapshot is about. The authorizing tool joins the cache key only for the
projects that use it.

## 5. One implementation, two callers

`fixture_authorization` is a library module, not test-local code, and the
process test now calls it. Two implementations of "what authorized means" —
one in `contract_closure_process.rs` for the fourteen and one for snapshot
fixtures — would drift, and the corpus composition pin `(14, 2, 32, 10)` would
keep passing while the two diverged. It still passes, which is what makes the
refactor a refactor.

## 6. What it does not settle

- **The fourteen still snapshot their rejection.** Their catalogs are
  unchanged; this adds a road, it does not move them onto it. Whether they
  should now ship authorization requests instead is a separate, larger change:
  several exist precisely to pin refusal.
- **Two of the three blocked claims are still blocked, for unrelated reasons.**
  The `returns_reactive_tuple` row needs a contract stating an `argument` or
  `callback-result` return, which is now authorable but not written; the
  `mergeDefaultProps` yield needs the kobalte corpus, which this repository does
  not have and no fixture capability can supply.
- **These are still fixtures.** A hand-written document authorized by a
  test-scoped issuer is not evidence that *certification* can produce such a
  contract. It cannot yet, and the closure levers are the separate work.
