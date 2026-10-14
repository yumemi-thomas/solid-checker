# ADR 0310: A source-edge refusal is a caller's recorded withholding

- Status: accepted design under the 2026-10-15 premise review (r1, with
  corrections C1–C4); a **dormant consumer** only. No production caller
  constructs a record, so no public finding moves. Assembly is not
  integration, certification or merge clearance.
- Owner: `solid-reactive-ir` (`package_boundary`, `source_refusal`, the
  accepted-contract index, the acceptance gate and the SC9005 projection).
- Phase P3's recorded-refusal consumer in docs/contract-removal-plan.md.
  Premise: `/tmp/sc-scratch.noindex/source-refusal-projection-premise.md`;
  review: `…-premise-review-r1.md`.

## Decision

`PackageBoundary` can hold **source-edge refusals**: for an importer and a
specifier as written, an ordered set of exact occurrence sites, each a form
(`import`, `export-from`, `import-equals`, `dynamic-import`, `require`) and the
byte range of the whole statement or expression. They are stored apart from
the optional package identities and create none, so `import()` answers `None`
for a key that only has sites. An empty set adds no bytes to the boundary
digest; a populated one adds one versioned section after the legacy sections.

The refusal is **the caller's recorded decision**: the analysis input recorded
that source-boundary analysis was withheld for that occurrence. The consumer
neither discovers nor authenticates it and does not establish that source
authority is absent. It says nothing about a missing module, a missing export,
a runtime target or execution. TypeScript reports export existence; this
reports a supplied analysis refusal. They are different propositions, and the
finding depends on no fixture stub signature.

### The exact join

One module, `source_refusal`, decides what an occurrence is, for validation and
emission alike. A site joins by exact membership of importer path, specifier
text, form and range — never by a nearby span, a name or a guessed target.
Occurrences are the runtime-load set of the facts: an import with a
value-referenced binding, an `export … from` with at least one runtime
specifier (or a star), an `import … = require`, a literal `import()` and a
literal, unshadowed `require()`. Type-only forms, side-effect imports,
imports with no referenced binding, an extractor-unrecorded specifier (a
nonliteral or options-bearing call) and an `export {} from` are excluded from
this API's supported occurrence set. That is a statement about what a record
may name, not about whether a module is loaded: a side-effect import does load
one.

The build errors (`BuildError::SourceEdge`) rather than guessing when the
importer is not a file of the facts, the importer is inside a dependency source
root (its findings are dropped after the solve), a site matches no occurrence,
a runtime namespace import is recorded or shares a key whose contract would be
denied, a key with a contract-bound site leaves one of its `import` or
`export … from` occurrences unrecorded, or such a key has an import statement
with an **unreferenced non-type binding** (named, default or namespace) that
is not itself a recorded occurrence (`UnreferencedCollateral`).
Completeness is checked across both forms together, so a mixed
import-plus-export key is not a loophole.

The last rule is the conservative answer to code review R1-B1. The accepted
contract projection answers every non-type binding of a key, referenced or
not, so `import { run } from "pkg"; import { idle } from "pkg"; run();` with
only the first statement recorded would have lost the explicit unknown the
analysis reports at `idle` (an accepted export whose owner requirements are
open), with the marker at the first statement standing in for a decision the
caller never made about the second. The key is rejected instead, with no new
flow or preservation machinery: a record must not authorize suppressing another
occurrence. A statement the record names is the occurrence itself, so an
unreferenced binding inside a recorded statement is not collateral. A
type-only or side-effect statement holds no binding the projection answers and
stays supported; a literal-only record denies nothing and needs no rejection.
Rejecting is broader than necessary where no accepted contract answers the key;
the check does not look, so a caller that wants such a record must also record
or remove the unreferenced import.

### What is denied

A key holding an `import` or `export-from` site gets its accepted contract
withheld **at every ordinary tier**: importer-keyed, artifact-admitted,
per-importer admitted, every nested scope and the browser view, recursively. It
is applied once at the shared build seam after `external_packages`. A key
holding only `import-equals`, `dynamic-import` or `require` sites denies
nothing, because the binder never bound those forms; an unrecorded static
import of the same key keeps its own, separate binding, and the marker stands
beside it. That is the deliberately narrower denial.

This is **not source denial**. Source declarations, alias roots, redirects and
interprocedural summaries are built independently of accepted contracts and are
unchanged, so an analyzed implementation supplied by the program can still
derive behavior beside the marker. A producer that needs complete withholding
must prune the relevant source authority before recording; this consumer does
not verify that. `semantic_identity`, `admitted_contracts`, the uncertifiable
markers and the admission explanations still report as before.

### Projection

A neutral SC9005 uncertifiable finding per recorded occurrence, with the
reused defect kind and the context `source-boundary-withheld:<form>`. Exactly:

- message: `source analysis was withheld for {module}: the available facts do
  not establish the source boundary needed to determine its reactive behavior`;
- evidence: `the analysis input recorded that source-boundary analysis was
  withheld for this exact occurrence`;
- hint: `This finding does not certify reactive behavior across this edge. It
  does not establish a runtime target, execution, or the existence of an
  export.`

It is never collapsed across occurrences and has no coverage family, so the
"imports from packages without a complete reactivity contract" heading never
prints it. A legacy marker at the same edge adds no second story. Disabling
SC9005 hides it exactly as it hides the legacy gate. The rule slug and the
defect variant name stay contract-era spellings until P2/P7.

## Limits, stated plainly

- **Membership is not provenance.** A record carries no generation or content
  hash, so an equal range in a changed facts generation joins exactly as before.
  The caller must reacquire its records for each coherent facts generation; this
  is characterized by a test, not rejected.
- **A failed call is not transactional.** An invalid record produces no new
  program or findings, but the incremental builder has already cleared its
  retained state; the next valid call is semantically equal to a fresh build
  and may rebuild. No pointer or reuse promise is made across it.
- **No emission or certification support.** `is_unresolved_obligation` counts
  the reused kind, but contract emission has inert-entrypoint returns before
  that path and attributes open claims rather than rejecting the project; the
  retained `dependency_bound` package summaries, the acceptance metadata and
  `exported_names_for_file` read the caller's original accepted index. These
  records are supported only for ordinary IR/diagnostic analysis. No production
  caller, emission transaction or certification transaction may construct or
  pass them.
- The host fork never reports `timings.reused`; baseline (no-host) diagnostics
  reuse is what the tests pin.

## Remaining integration (none of it cleared here)

A producer and its exposure decision, with an exact runtime join; the
filesystem witness, session refresh and the daemon answer cache (`answer` can
return a cached answer before the boundary is recomputed); alias and relative
exclusion; exact runtime admission and retained-session exemption; positive
withdrawal of dependent claims; the WASM host protocol carrying records; and
P7 wording and slug retirement. Whole external-source-boundary A stays on hold.
