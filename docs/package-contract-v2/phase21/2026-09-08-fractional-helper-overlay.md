# Fractional Indexing helper-read experiment

The temporary Go overlay exercises private candidate discovery without
changing production source, wire fields, verifier rules, or binaries. It
parses the retained Fractional Indexing 3.4.0 runtime as JavaScript and finds
two pre-store argument transfers in `generateKeyBetween`. Each joins an
exact local callee and argument slot to the helper's positively unwritten
parameter and direct `.slice` call. These are candidate facts, not receipts.

The experiment scans stores in parameter initializers as well as the body,
rejects nested callables and module-level `arguments`/`eval`, requires unique
parameter declarations, and rejects reassigned local callee bindings. It
does not use a read after the `a`/`b` swap as a witness for the original slot.
Sixteen prototype cases pass, including the published JavaScript and controls
for stores, sibling initializers, loops, overwritten callees, captures,
unreachable calls, spreads, redeclarations, wrong members, wrong argument
slots, and overwritten helper parameters.

An earlier candidate failed the redeclaration and module-level `eval`
controls. Those results are excluded. The tightened overlay plus the existing
initial-read regressions pass in 2.058 seconds. The fixed member path is an
explicit diagnostic query; it is not a package-name or export-name proof rule.
A production implementation still needs a consumer-bound argument-origin
premise and exact composition checks. Shared-interface ownership remains
pending, and no new interface was introduced here.

The controls also exposed a current producer defect independently:

```ts
export function check(input: string) {
  var input = "local";
  const first = input.slice(0);
  input = "later";
  return first;
}
```

The current producer emits a positional original-input read for `input.slice`
despite the preceding redeclaration initializer. The negative overlay test
fails on that exact fact (0.039 seconds). TypeScript accepts the snippet with
`--strict --noEmit --skipLibCheck --target es2023`, exit 0; this is a proof
origin error, not a TypeScript diagnostic. An overlay guard requiring a unique
parameter declaration rejects the erroneous fact and preserves the existing
initial-read regressions. This narrowing must be applied after the active
full-corpus measurement is archived; it does not require a new wire field.

The refined guard checks only the parameter whose read is being proved.
A separate positive control preserves that read when another parameter is
redeclared. The refined overlay and the existing regressions pass in 1.558
seconds; its source and result are retained separately in the JSON record.

The retained-preparation full corpus has now completed and its matching
binaries are archived. The guard and its two focused regressions were then
applied to production under ADR 0085. The helper-transfer prototype remains
an overlay; no new argument-origin interface or helper-read receipt is issued.

Zero new accepted cases or complete rows are claimed. The
[machine-readable record](2026-09-08-fractional-helper-overlay.json) retains
the overlay sources, artifact hashes and exact command outcomes. The running
retained-preparation corpus remains a measurement of its frozen build, with
this newly discovered proof limitation explicitly recorded.
