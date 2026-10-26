---
name: upstream-parity-policy
description: "Owner's rule for eslint-plugin-solid alignment: superset scope, upstream names unless inaccurate, semantic rules only (ADR 0381)"
metadata:
  node_type: memory
  type: feedback
  originSessionId: f7acefda-1e86-42dc-a3ef-0469297f2477
  modified: 2026-10-25T03:31:02.289Z
---

Stated by the owner on 2026-10-25 during the catalog review: "if upstream does more, we do it; if we do more we keep doing more; prefer the upstream rule name unless it is not accurate anymore." Confirmed scope: semantic rules only; policy rules retired by ADR 0110 stay retired. Invariants 1 and 2 still win over parity (a shape tsc reports or the runtime refutes is not adopted).

Owner's standing order (2026-10-25, during the implementation series): stay faithful and correct towards Solid 2 above all. Upstream eslint-plugin-solid supplies candidate shapes and names; the pinned rc.17 runtime and compiler bytes are the authority. A shape is adopted only with a probe or code-read proof (file:line) on the pinned bytes; where upstream and the runtime disagree, the runtime wins and the divergence is pinned in fixtures/ownership-cases.

**Why:** users migrating from eslint-plugin-solid's `v2` config expect the same keys and at least the same findings.

**How to apply:** when touching a rule with an upstream counterpart, read upstream's doc page first, list shapes upstream reports that we do not, and add them (with tsc-oracle/ownership-case pins where refused). Decisions recorded as ADRs 0381-0392 in docs/decisions.md; implementation was not started in that session.
