# 0020 — Compose an independently verified creates census

Status: accepted and implemented; measured outcomes in docs/2026-09-04-published-js-probe-unlock.md
Date: 2026-09-04

With graph probes armed, Kobalte's createGenerateId refuses on uncensused
template coercion. Removing that candidate lets the other ten census demands
pass, but composition refuses demand
`sha256:fd6d529f4523e455bde617052bee83bf955c065be08a627a6327d65dd170be39`:
the dependency receipt lacks the parent's clamp claim
`claim:v1:sha256:9953ad788080372a55ebfe0c75f540b36763e0880e65dc23c3baf782e47bfd9a`.
Planning currently forms a Cartesian product of dependencies and parent
closures. The parent claim ID includes the parent's artifact identity, so it
cannot identify an equivalent dependency claim.

Do not pretend the dependency contains that claim or drop dependency demands.
For creates only, allow composition to use the parent's already verified
implementation-census witness as independent evidence. That census enumerates
every possibly executing invoking form and resolves it through authenticated
local runtime recursion, exact compiled negative callee axioms, or reviewed
default-library rules. It does not read a dependency receipt's behavioral
claims. Unsupported forms, opaque calls and missing transcripts still refuse.

The composition verifier must still authenticate the exact dependency receipt,
its planned/gated semantics, graph edge, trust and verifier identity. If the
receipt lacks the named claim, require an opaque VerifiedTypeFactsEvidence
token containing the exact parent plan's creates DomainExhaustiveness demand.
Bind that census evidence root and the independent-proof disposition into the
composition witness. A digest supplied by a caller, another plan's evidence,
another export/domain, or an absent census must not qualify. The public
receipt-only authentication path retains its existing refusal.

This proves the same negative creates claim independently of dependency
semantic assumptions; it does not infer any missing dependency behavior.
The parent still runs its own mandatory veto against the full authenticated
runtime closure. Other domains keep their existing composition requirements
until exact dependency claim mapping is implemented. The policy-2 demand set,
claim meaning and required families stay unchanged; the new method is bound
by its composition evidence and compiled verifier identity. Sandbox scheme 6
does not change.

Removing the Cartesian demands wholesale would lose protection for genuinely
dependency-derived claims. Matching by export name or stripping artifact
identity would invent equivalence. Accepting receipt identity alone repeats
the previously repaired vacuous-composition bug. Reusing an independently
verified universal census is the bounded alternative chosen here.
