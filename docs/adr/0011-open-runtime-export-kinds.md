# 0011 — Preserve unresolved runtime export kinds as explicit unknown knowledge

Status: accepted; implemented
Date: 2026-09-04

## Decision before code

Carry an unresolved runtime export as the existing stable-v1
`shape: "unknown"`, with every behavioral domain open, rather than refusing
all independent exports of the artifact case. This is not proof that the
export is non-callable and not certification of its behavior. Its exact
runtime/declaration binding and full executable closure still require ordinary
snapshot replay. Consumers can distinguish it in the main document: unknown
shape, no closed domains, no behavioral operations. Calling it still requires
the missing behavioral facts; re-exporting it must preserve the uncertainty.

The public format already supports this knowledge state. The current compact
inference representation admits only `function` and `value`, and the latter
implies non-callability. An explicit internal unknown kind must survive
normalization, accepted-contract projection, and re-export. Do not encode an
unknown as a plain value or close its callback/creates domains to empty.

For a present but unresolved runtime-kind answer, discard the inferred
behavioral summary and emit wholly unknown knowledge. An absent answer stays
a refusal: it may indicate a missing exact identity rather than classified
uncertainty. Closed callable/non-callable answers keep their current handling,
including refusal of contradictory non-callable summaries with function effects.
No known creates closure or its mandatory census/veto may be dropped by this
rule; only the export whose runtime kind is unresolved is weakened before it
becomes a closure candidate. Other exports undergo unchanged proof demands.

## Alternatives

Retain the whole-case refusal: sound, but one unclassified export blocks
independent exports. Substitute the published declaration type: rejected,
because a declaration's non-callable type does not prove its runtime value.
Recognize `Object.assign(Object.create(null), ...)` by spelling: rejected,
because shadowing, replacement, and evaluation effects can change which
functions are called. A sound intrinsic proof remains a possible later slice.
Project only requested dependency exports: plausible, but it introduces export
selection and propagation machinery when existing unknown knowledge may suffice.

The narrower claim is defensible because acceptance authenticates exactly the
knowledge stated, and this export states no behavioral knowledge. Every
independent closed claim still needs its own proof. Acceptance must never be
reported as complete proof of the unknown export; `exportsProven` must not
count it. Full binding census and runtime dependencies remain present, so an
unknown export's module initialization cannot disappear from the probe workspace.

## Required evidence and stopping conditions

First reproduce Kobalte 0.9.2's published graph refusal on `solid-js@1.9.14`
`./web`'s `Aliases`; the offline baseline is retained under the probe-ts scratch
directory's `092-frontier/baseline.audit.json`. It reproduced the exact
`(Unknown, Unknown)` runtime-kind refusal with zero cache misses.

Pin a real native fixture containing an unknown-kind export and an independent
callable JS sibling. Verify unknown remains unknown through certification and
consumer/re-export projection, cannot acquire empty negative domains, and does
not block the sibling's census and veto. Include a callable hidden behind any,
a non-callable unknown, and existing wrong-kind/missing-fact cases. A forged
plain-value or closed behavioral claim must still refuse. Measure the actual
dependency graph again and report subsequent blockers rather than assuming
Aliases was the final one. Run the original three rows only after a concrete
behavior change is ready, with explicit recipe-corpus comparisons.

If an existing consumer turns unknown into a negative claim, fix and pin that
seam before enabling this production behavior; otherwise retain refusal. No
new public schema field, Type Facts protocol change, transformer, or sandbox
policy change is intended. Scheme 6 and all harness guarantees remain.

## Measured result

The native fixture certifies the independent `noop` creates closure through a
real completed gate while preserving both hidden callable and hidden object
exports as unknown. Forged plain-value and closed-creates variants refuse.
Consumer projection and normalization tests retain all open domains.

The 88-fixture corpus moves only four export cases across three fixtures:
`class-expression-kind`'s `./unresolvable`, `published-export-entity`'s
`./mixed` and `./unknown`, and `non-emitting-module-target-control`'s
`./default-export`. Each adds unknown shape and no behavioral operation or
closure candidate. Existing claims are unchanged. The non-updating gate and a
complete inspection of generated differences preceded snapshot updates.

Kobalte 0.9.2 passes the reproduced Aliases refusal, then encounters the
same-package target rebinding issue addressed in ADR 0012. This is progress in
graph preparation, not a completed Kobalte certification.
