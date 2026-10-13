# 0015 — Returned callable identity does not prove descendant execution

Status: accepted and implemented; measured outcomes in docs/2026-09-04-published-js-probe-unlock.md
Date: 2026-09-04

## Decision

Do not infer deferred callback execution merely because the callback's
containing function is a lexical descendant of a returned callable. Require
the exact containing callable to be carried by the return value. Transparent
TypeScript wrappers and existing identity-preserving return helpers remain
eligible. An unproven descendant invocation leaves callback knowledge unknown.

The reduced `returned-callback-descendant` fixture reproduces the bug:
`Dormant` returns a function declaring `mapper() { callback(); }` without
calling it, yet receives the same queued callback proposal as `Direct`, which
returns `() => callback()`. `Invoked` explicitly calls its nested mapper and
distinguishes missing interprocedural execution evidence from lexical
containment. The current return inference cannot prove that chain; it must
not use Dormant's invalid premise to publish Invoked's claim either.

Two generator branches currently overreach: a returned function expression's
span contains every descendant, and the fallback expressly searches escaping
ancestors. Replace containment with exact returned-function identity and
remove the ancestor fallback. Keep the native verifier's execution requirement.

## Alternatives and contract

Accepting all descendants is disproven by Dormant. Teaching the verifier to
accept lexical containment would certify the same nonexistent invocation.
An exact bounded invocation graph could recover Invoked and Solid's named
mapper chains, but requires invocation and scheduling evidence for every edge;
it is separate work. Preserving an explicit unknown callback domain prevents
an unsupported positive claim while allowing unrelated proof demands to run.
It does not prove absence of callbacks or certify their execution.

No creates census, mandatory veto, authenticated artifact, Node/harness pin,
or sandbox scheme changes. The real baseline is ADR 0014's Kobalte graph
indexArray operation-reachability refusal, demand
`sha256:0cc7f64b7816e102584b7c786aacd96b5364fc8a89f4090befe97c88f8e357b1`.
Require direct/descendant corpus controls and a native forged-Dormant refusal;
review all snapshot moves before update and repeat the offline graph.
