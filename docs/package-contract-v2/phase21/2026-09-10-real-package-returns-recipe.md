# Closing `returns` on a real published package with a hand recipe

- **Status:** measurement, plus one checked-in corpus recipe.
- **Date:** 2026-09-10.
- **Question:** a probe recipe can only *veto*, never establish closure. So
  what does authoring one actually buy on a real package, and does a consumer
  see it?
- **Answer:** it converts a candidate the census already proved but that was
  withheld for want of a recipe into a closed domain, and the consumer's
  SC9005 loses that domain from its message. On `seroval@1.5.6`
  `createReference` the finding goes from `reactiveReads,returns` to
  `reactiveReads`. `reads` is then the only thing left, on an export where
  every other domain is closed.

## 1. The candidate

The retained real certification at
`/private/tmp/solid-checker-rc7-reproduction` carried two withheld closures.
One was a census refusal; the other was a recipe gap:

~~~json
{
  "artifactCase": "artifact-case:f6c68044fe9f010a5420013066e261cdf012288d8f98348f33212cafc5915dde",
  "domain": "returns",
  "export": "createReference",
  "reason": "no recipe in corpus",
  "semanticClaimId": "claim:v1:sha256:9e82b17c7e39cb8ee4394875fc28ea98328ea085d19812c1b9f9ca5edd7bcf14"
}
~~~

`recipe_gated_plan` ([contract_certification.rs:1330](../../../rust/crates/solid-facts-backend/src/contract_certification.rs))
withholds a proposable closure candidate exactly when the corpus has no recipe
for its claim id. Since ADR 0036 most such candidates are served by a veto
*synthesized* from the export's Type Facts call signature; this one is not,
and `createReference<T>(id: string, value: T): T` is why — synthesis has no
sample to derive for a bare type parameter.

The census's claim is `output: {index: 1, kind: "parameter", path: []}` —
the export returns its second argument by identity. The published body agrees:

~~~js
function createReference(id, value) {
  REFERENCE.set(value, id);
  INV_REFERENCE.set(id, value);
  return value;
}
~~~

Both registries are plain `Map`s rather than `WeakMap`s, so the sample can
cross the primitive/object boundary instead of stopping at objects.

## 2. The recipe

`scripts/ecosystem-benchmark/probe-recipes/seroval-create-reference-identity-development.mjs`,
addressed to that claim id. Eleven samples — object, frozen object, function,
string, `0`, `-0`, `NaN`, `null`, `undefined`, symbol, and one value
re-registered under a second id — each emitting `return-outside-identity` only
when `Object.is(output, input)` is false. It hands neither `session` nor
`harness` to package code.

## 3. What it measured

`contract certify` against the exact published artifact, same package root,
same integrity, same entrypoint and conditions, twice. The only difference
between the runs is whether this one module is in the corpus.

| corpus | withheld for `createReference` | contract |
| --- | --- | --- |
| 13 recipes (without this one) | `returns` — `no recipe in corpus` | `closed: ["creates"]` |
| 14 recipes | *nothing* | `closed: ["creates", "returns"]` |

Retained: [treatment audit](2026-09-10-seroval-returns-recipe-audit.json),
[control audit](2026-09-10-seroval-returns-control-audit.json). In both runs
`resolvePlugins`/`creates` stays withheld on a census refusal, which no recipe
addresses.

## 4. The falsifier

A recipe that never emits cannot be distinguished from a gate that never ran,
so the closure above is worth nothing without a control that proves the gate
reads the emissions. A scratch copy of the corpus with this module replaced by
one that emits `return-outside-identity` unconditionally:

~~~
solid-checker: witness-acquisition refused: solid-checker-rust: policy-2 proof
finalization failed: probe contradiction at
sha256:1ba72f8c59a0f22041fb4f13eca3e5e2f06cd916a3111950e5061e8abaa7219a for
claim:v1:sha256:9e82b17c7e39cb8ee4394875fc28ea98328ea085d19812c1b9f9ca5edd7bcf14
~~~

Exit 2, naming the exact claim. The gate reads the recipe.

## 5. The consumer verdict

A consumer that binds the result and reads it:

~~~js
const value = createMemo(() => true);
const registered = createReference("solid-checker:consumer", value);
export const read = registered();
~~~

Analyzed against each catalog, with the receipt trust configuration each run
issued:

| catalog | finding |
| --- | --- |
| none | SC9011 — "the reactive source `value` is passed to `createReference`, whose reactive behaviour is not described anywhere" |
| control (returns withheld) | SC9005 — "leaves **reactiveReads,returns** unknown for imported export `createReference`" |
| treatment (returns closed) | SC9005 — "leaves **reactiveReads** unknown for imported export `createReference`" |

The result is not discarded, so the `returns` demand-scoping slice
([contracts.rs](../../../rust/crates/solid-reactive-ir/src/contracts.rs),
`returns_shed_symbols`) does demand the domain here — it drops out of the
message because it is closed, not because it stopped being asked for.

**This is the first measured case where authoring a recipe changed what a
consumer is told about a real published package.** It is also, precisely, the
demonstration that `reads` is the whole remaining gap: on this export
`creates` is proved, `returns` is now proved, no callback is taken, and the
verdict is still uncertifiable on `reads` alone — which the implementation
census [cannot decide at all](2026-09-10-reads-veto-observation-design.md).

## 6. What this does not license

- **The recipe proves nothing on its own.** Finite samples can only contradict.
  The authenticated implementation census is what proves the closure (ADR 0008);
  the recipe's whole job is to fail to falsify it.
- **The consumer verdict did not become clean.** It went from two open domains
  to one. No violation was enabled and no finding was removed.
- **The claim id is a content digest.** Anything that moves the emitted contract
  for this package moves the id and the recipe then addresses nothing. Re-read
  `withheldClosures` after any generator change.
- **The measurement vehicle is not a normal consumer.** `contract certify`
  derives its own synthetic importer path
  (`certificationImporterPathFor`), and `AcceptedContractIndex` keys on the
  exact `(importer, specifier)` pair, so the CLI has no way to mint a catalog
  bound to an arbitrary consumer file. Rewriting the binding is correctly
  refused: `policy-2 acceptance receipt authentication failed: receipt does not
  bind the current importer`. The measurement therefore put the consumer body
  *at* the certification importer path. That is an addressing device; the
  contract, the bytes, the census and the receipt are all the real ones. A
  harness that certifies against a project's own importers is still missing,
  and is what the retained rc7 tree had.

## 7. Reproducing

~~~sh
export P=/private/tmp/solid-checker-rc7-reproduction/projects/primitives
SOLID_CHECKER_NATIVE_BIN="$PWD/rust/target/debug/solid-checker-rust" \
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
SOLID_CHECKER_PROBE_NODE="$(node -e 'process.stdout.write(require("fs").realpathSync(process.execPath))')" \
node packages/cli/bin/solid-checker.mjs contract certify \
  --package-root "$P/node_modules/seroval" \
  --integrity "sha512-rVQVWjjSvlINzaQPZH5JFqsqEsIWdTxY3iJZCnTL/5gQbXIRooVZKI60tVCkOVfzcRPejboxO2t0P89dg5mQaA==" \
  --entrypoint . --conditions development \
  --issuer-configuration …/issuer.json \
  --trust-configuration-output …/trust.json \
  --catalog …/catalog/accepted-contracts.json \
  --probe-recipe-corpus "$PWD/scripts/ecosystem-benchmark/probe-recipes" \
  --audit-output …/certification-audit.json
~~~

Needs the registry (the archive is reacquired and authenticated on every run)
and a checker built with the certification pins — `make build-checker-debug`.
`certify` refuses to overwrite an existing certification importer file, so
delete it between runs.
