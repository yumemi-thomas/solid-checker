# Sub-agent review reports

These reports preserve the independent review work used to derive the approved
design. They are evidence and counterexample catalogs, not implementation
authority. The normative plan and model live one directory above.

- [Current architecture review](current-contract-review.md) — existing schema,
  normalization, generator, verifier, refusal, size, and migration baseline.
- [Adversarial schema review](adversarial-schema-review.md) — attacks against
  closure, environments, phases, values, ownership, evidence, and migration.
- [Solid RC.3 evidence review](solid-rc3-evidence-review.md) — published runtime,
  declaration, export-resolution, and behavior evidence.
- [Initial Phase 0 worktree audit](phase0-worktree-audit.md) — pre-execution
  inventory, worktree separation, and missing baseline gates.
- [Phase 0 RC.3 authority audit](phase0-rc3-authority-audit.md) — exact package
  identity, corrected registry `gitHead`, authority boundary, and closure gap.

The first three agents performed read-only reviews. The two Phase 0 reports were
written from their completed audits at the user's request. Their missing-
artifact findings describe the pre-execution state; current completion evidence
lives under `benchmarks/package-contract-v2/phase0/`.
