# ADR 0173: alternative owner calls cover normal completions

Date: 2026-10-01. Status: implemented. Promoted to the main verifier and
generator on 2026-10-04 (see "Promotion" below); the prototype was the separate
`codex/owner-cover-experiment` branch. No contract or Type Facts schema change.

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

## Promotion (2026-10-04)

The certifier half (`owner_call_cover.rs`, `solid_facts::ast::completion_call_cover`)
is carried over unchanged. One generator change is added. The generator now
strengthens a possible registration in exactly the shape the census can prove:

- `generated_owner_requirements_by_symbol` collects, per function and role
  (effect or cleanup; never a settled cleanup or a boundary), the direct
  dialect-primitive sites that are not individually unconditional;
- it proposes `min: 1` when `completion_call_cover` covers every normal
  completion of the function's own body with them.

The emission source is already host-folded (ADR 0166), so a dead server guard
is `;` there. The census reaches the same verdict from the producer's
unreachable-return facts.

A proposal the census cannot prove is withdrawn by name. It is not demoted to
its `min: 0` item, which is the regression the experiment warned about. On the
browser checkpoint no such withdrawal occurred.

Pinned by:

- `fixtures/package-contracts/owner-call-cover`:
  - `listen` (two effect constructors, one per arm) proposes `min: 1`;
  - the missing `else`, mixed roles and an early guard stay `min: 0`;
- the prototype's syntax and source-binding tests.

Measured with the browser tier regenerated
([report](../package-contract-v2/phase22/2026-10-04-open-owner-requirements.md)):

- the misuse ledger's static violations go from 49 to 53 of 123:
  `createEventListener` (module scope and effect apply), `createScrollPosition`
  and `createSwitchTransition`;
- each is runtime-detected, and no correct twin is flagged.

The cover is also what certifies `createSwitchTransition`. Its single
`createRenderEffect` call fails the strict floor, because the producer does not
state it unconditional, but it covers the function's every normal completion.
