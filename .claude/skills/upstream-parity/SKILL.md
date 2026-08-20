---
name: upstream-parity
description: Investigate retained eslint-plugin-solid behavior after parity-corpus retirement. Use when an upstream_compat heuristic looks wrong or a product-owned case differs from upstream.
---

# Upstream compatibility work

eslint-plugin-solid 0.14.5 at commit `6d3bc311` is the audited source for the
retained Solid 1.x compatibility heuristics. The former 465-case parity corpus
has been retired: `fixtures/ownership-cases/migration-ledger.json` reconciles
all of it, and retained behavior now lives in product-owned cases with explicit
TypeScript ownership.

## Running the replacement gate

~~~sh
make ownership-gate
~~~

The gate uses a fresh debug checker, real published typings, exact UTF-8 spans,
and the cases in `fixtures/ownership-cases/cases.json`. A behavior change must
update its focused case and, when relevant, `docs/precision-backlog.md` in the
same semantic commit. There is no deviation allowlist to update.

## The upstream-faithfulness rule

Code under `rust/crates/solid-reactive-ir/src/upstream_compat/` began as a
byte-faithful port. Some heuristics look odd but are intentional upstream
behavior. Before changing one:

1. Read the upstream source at the pinned revision:

   ~~~sh
   gh api "repos/solidjs-community/eslint-plugin-solid/contents/<path>?ref=6d3bc311" --jq .content | base64 -d
   ~~~

2. Establish which semantic owner decides the product behavior. TypeScript
   owns type errors, the pinned Solid compiler owns lowering, runtime probes
   own execution behavior, and package contracts own external callbacks.
3. If no stronger evidence distinguishes the behavior, retain the audited
   upstream heuristic.
4. If the checker deliberately differs for evidence-backed precision, encode
   the positive and negative behavior directly in ownership cases and explain
   the evidence in the nearest rule page or precision entry.
5. If the port genuinely mismatches upstream, fix it with a focused fixture.

Never preserve upstream behavior by duplicating a TypeScript diagnostic,
weakening semantic resolution, adding blanket trust, or guessing from names.
Moving the upstream pin is a reviewed dependency change under
`docs/monorepo.md`, not a debugging step.
