# ADR 0301: Star barrels retain contracts; apply messages describe tracking

- Status: accepted and implemented (2026-10-14).
- Owners: `solid-reactive-ir::contracts`, `execution_role`, and `findings`.

## Decision

A demanded import through `export *` retains the accepted package export's
exact identity and claims. Star chains and static members of `export * as ns`
follow the same rule. An open claim raises SC9005, as a named re-export does.
Explicit local exports precede stars. Distinct bindings, duplicate explicit
exports, namespace declaration surfaces, cycles and unknown competing providers
remain unbound. Two paths to the same exact provider edge are one binding;
equal receipt semantics alone cannot identify two installed copies. Default
exports never flow through a bare star. Computed and nested namespace dispatch
remain unbound.

SC1001 still reports an untracked apply read when its paired compute reads the
same source. Its message says the effect can re-run and Solid warns in
development; it does not say the effect never updates. The observation joins
exact immutable getter values and callback positions after merging current
reads. An allocation template alone does not identify a factory instance.
It changes neither finding kind nor evidence, and grants no certification.
Unknown callback pairs and summary-only source reads grant no wording change.
Named computes additionally require an unconditional synchronous call of a
proven accessor; nested, JSX, asynchronous and conditional calls grant none.
Mutable or unresolved getter values and instance-dependent helper captures
grant no wording change. Module singleton origins may retain an exact identity.

## Evidence

`contract-star-consumer` compares a star, a chain, a namespace member and a
local shadow with the named-form contract fixture. Process controls cover
ambiguity, identical bindings, unknown providers and direct namespace exports.
`render-effect-apply-root` pins compute/apply wording, including namespace and
named callbacks, immutable aliases, different factory instances, and a
different-source control. Its new cases compile cleanly
against published Solid 2.0.0-rc.13 typings. Coverage retains message text for
this fixture; other snapshots omit messages.
