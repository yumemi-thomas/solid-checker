# ADR 0173: alternative owner calls cover normal completions

Date: 2026-10-01. Status: bounded implementation on the separate
`codex/owner-cover-experiment` branch and published-package experiment;
main-verifier and accepted-tier promotion deferred. No contract or Type Facts
schema change. The prototype checkout is `.claude/worktrees/codex-owner-cover-experiment`.

The existing `min: 1, max: many, scope: call` ownership registration claim needs
a lower bound. ADR 0159's individual `unconditional` call is sufficient, but
mutually exclusive calls can satisfy the same requirement together. Neither
branch call becomes individually unconditional.

The fallback reads authenticated runtime snapshot bytes and the existing exact
resolved declaration, body frame and unreachable-return facts. It binds one
synchronous function body by exact spans. Candidate calls use the existing
exact resolved `solid-js` owner-role identity and exclude captured, unresolved
and unreachable calls. A cleanup requirement cannot be witnessed by an effect
role, or vice versa; combined child-owner and cleanup requirements remain open.

The syntax owner, `solid-facts`, supplies a bounded normal-completion cover.
Its two path states record whether a candidate call has occurred. Blocks
compose sequentially, `if` joins both arms, and normal returns contribute their
current states. Throws contribute no normal completion. A throwing-only body
does not prove a positive occurrence. Exact unreachable returns are treated as
extra fallthrough paths, an overapproximation that can only refuse more covers.

Only direct, non-optional identifier call statements contribute a hit.
Transparent TypeScript wrappers are preserved. Namespace/member calls, loops,
switch, labels, jumps, `with`, and catch/finally stay outside this proof grammar.
Nested callable bodies contribute no hit. Depth, source size and statement
budgets fail closed. This is a new sufficient proof of an existing operation,
not a weakening of its reachability floor or a new value-shape family.

Backend operation-reachability and operation-cardinality demands both require
the cover when the individual-call proof fails. The witness records the bound
body, snapshot, exact candidate identities and unreachable-return facts used.
The existing authentication and mandatory veto pipeline remains authoritative.
The generator does not automatically strengthen its possible registrations.

The published browser listener's mutually exclusive `createEffect` and
`createRenderEffect` statements now prove the authored guaranteed registration.
Its module-scope finding changes from SC4001 uncertifiable to violation; its
root twin has no owner finding. Both remain SC9005 uncertifiable for independent
contract gaps. Node and host-free guaranteed proposals refuse. Ten stronger
claims across five other published packages all refuse, while six existing
guaranteed claims survive. The [experiment report](../package-contract-v2/phase22/2026-10-01-owner-cover-experiment.md)
separates proof feasibility from checkpoint coverage.
