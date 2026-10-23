# reactive-source-uncaptured

`SC9011` · **warning** · uncertifiable

A reactive source flows into a position the analysis cannot see into, so whether
it is ever read in a tracking scope cannot be certified.

## What it does

Flags reactive sources — signal accessors, stores, derived functions — that
are passed to a *package-imported* function whose reactive behavior the
analysis does not know: no analyzed source or primitive semantics establish the behavior.

**The warning severity does not soften certification.** Certification status is
computed from a finding's *kind*, not its severity: like every `uncertifiable`
finding, an SC9011 warning fails `--certify` until the boundary is described (or
the rule is disabled in `rule-options.json`). "Advisory" describes the tone —
nothing has been proven wrong — not an exemption from the certification gate.

Only callees imported from a package are reported, at this boundary. Ambient globals (`setTimeout`, `console.log`, an array
method) are outside this package-import rule; absence of this finding does
not prove their reactive behavior.

## Why is this analysis-limiting?

A reactive value handed to an unknown function may be read immediately (severing
reactivity), called inside a tracking scope (correct), or stored for later. The
analysis can prove none of these, so every read that flows through the unknown
function becomes uncertifiable rather than certified or proven wrong.

## What the evidence states

The finding's evidence step points at the argument and says only that a
reference bound to a reactive accessor occupies that call expression's argument
position, and that, if the call executes, the available analysis does not
establish whether reads through the accessor stay tracked. The statement is
conditional. It does not claim that the call runs, that the API or import is
invalid, or that any read is untracked. Whether a call, import or member is
valid is TypeScript's to report; this evidence says nothing about it, so the
finding can appear beside a TypeScript error on the same call. That
co-occurrence is tracked as open item B5 in
[precision-backlog.md](../precision-backlog.md).
A callee span that names no symbol (`new Registry().register(count)`, `(c ? a : b).register(count)`) is decided by its resolved call, not dropped (ADR 0376).

## How to fix

Give the analysis a way to see the call:

- If the receiving function is yours, keep it in the project so its body is
  analyzed directly.
- If it comes from a package, provide analyzable runtime source under the
  correct declared host. Unavailable behavior remains uncertifiable.

Package contracts and receipts are no longer consumed (ADR 0312).

## Related

- [package-behavior-unavailable](package-behavior-unavailable.md) — required package behavior is unavailable from source
- [strict-read-untracked](strict-read-untracked.md) — reads proven untracked
