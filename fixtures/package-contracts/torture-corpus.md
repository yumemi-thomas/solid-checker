# Package-contract torture corpus

These five packages are deliberately small release blockers for contract
generation. Their `expected.json` files are normalized output pins; the corpus
runner never updates them automatically.

| Fixture | Shape under test | Safety assertion |
| --- | --- | --- |
| `torture-runtime-namespace` | runtime-mutated object namespace and getter | the mutable namespace remains an unstructured value |
| `torture-conditional-semantics` | browser and node exports with different callback timing | both conditional callback behaviors are retained |
| `torture-getter-exports` | getter-backed object/function exports | getter syntax does not fabricate a reactive claim; the real memo return stays explicit |
| `torture-deep-barrel` | two-level `export *` and named re-export barrel | the exact deep declaration is followed once |
| `torture-dts-disagreement` | declarations disagree with the runtime module | runtime exports win; declaration-only names never become claims |

Run `make contract-corpus` to regenerate each package in a temporary
directory, compare its output with the checked-in pin, and collect V8 execution
ranges for the generator. A mismatch is a required review of the expected
contract, not an invitation to broaden a claim.

The corpus runner registers more than these five (see `scripts/contract-corpus.mjs`
for the list and why each one is there), and a fixture may pin a second file:

- `expected.json` — the normalized contract document.
- `expected-generation.json` — *optional*, and the review plan's closure record
  rather than the contract: per entrypoint, its `targets`, its modules as
  package-relative paths, and its `notes`/`runtimeNotes`. Module hashes are not
  pinned (the file bytes already are, and pinning both would make every source
  edit a two-file edit) but the runner refuses a module recorded without one.
  Carry this file when the fixture's claim is about the **attested closure
  record** — which modules the analyzing program reported it opened, and which of
  the generator's own walk problems survived reconciliation against that. Such a
  regression is invisible in the contract document, which is why nothing caught
  the class before, and it needs the real producer to test: a stub cannot resolve
  a module, so `scripts/contract-generation.test.mjs` cannot answer "did the
  compiler resolve this specifier".

Nine fixtures carry it today. Six exist for the record alone
(`attested-record-matches-walk`, `asset-import`, `attested-specifier-restated`,
`seed-attestation-discrepancy`, `non-literal-dynamic-import`,
`conditional-imports-side-effect`), and three carry it because their own claim is
about which bytes were read: `torture-deep-barrel` (every barrel hop is hashed,
once), `declaration-sibling-reach` (the record names a `.d.ts` *and* the runtime
module beside it, which is where the split is visible as bytes), and
`class-expression-kind` (the analysis reads an installed dependency's artifact
and the record excludes it, because a dependency's bytes are described by that
package's own contract).

It is deliberately not carried by every fixture. A blanket pin would make each
one a two-file review for a claim it does not make, and would turn any change to
the scope rules into a 37-fixture diff whose intent nobody could read. Two
closure properties that no committed package can express — a symlink escaping the
package root, and one file reached by two case spellings — are pinned against the
real producer in `scripts/contract-closure-record.test.mjs` instead.
