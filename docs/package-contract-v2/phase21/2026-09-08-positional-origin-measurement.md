# Positional original input: measured, and the blocker it names

[ADR 0069](../../adr/0069-original-input-by-position.md) replaces the
opening-prefix origin proof with an ordering rule. It was built for
`@kobalte/utils@0.9.2`'s twentieth artifact case, the one
[ADR 0068 left refused](2026-09-08-graph-fallback-recovery.md). **It moves no
row, and the shortfall names the blocker exactly.** The slice is kept for the
reason ADR 0049 was: the premise is right, and the measured zero is what
locates the next fact.

## The producer states what it should

A focused producer probe over `scrollIntoViewport`'s shape states **five**
positional reads for slot 0, every one reachable and every one before the
`while` loop that reassigns `targetElement`; slot 1 (`opts`) is unwritten and
keeps its stronger `unwrittenParameters` row. The opening-prefix rule states
none of the five: the body's first statement is an `if`, and the guard is
computed from `window.getComputedStyle(...)`.

## The consumer refuses them, correctly

The scoped rerun of `@kobalte/utils@0.9.2|solid1|only` with the protocol-43
producer and its matching checker is **unchanged at 19 of 20 cases**, with the
same refusal on `./src/scroll-into-view.ts`:

> `recursive-value-shape (artifact-case:4428aaa5…:scrollIntoViewport):`
> `parameter-rooted read lacks positive original-input identity`

A diagnostic build — identical except that
`initial_parameter_input_identity` passes `allow_positional: true` — certifies
the whole package: **23 certified entrypoints with the root**, up from 19. So
the demand takes the **whole-root** arm (`input_path.is_empty()`), which ADR
0069 deliberately does not let a positional row answer. The diagnostic was
reverted; no measurement below used it.

That gate is not excess caution. The whole-root arm passes
`matches_use = |_| true`: it accepts *any* qualifying read of the slot, because
`solid_reactive_ir::contract_semantics::Operation` carries `inputs`, `guard`,
`cardinality` and `composed_from` but **no source location**, so there is
nothing to bind the operation's own read to. The inference it then makes —
"some read of slot *i* preceded every store, therefore *this* operation's
whole-root input from slot *i* is the caller's value" — is existence-quantified
and invalid in general. It is invalid for prefix rows too; ADR 0065 shipped it
that way, and ADR 0069's rule finds far more witnesses, so admitting them there
would enlarge an inference this repository cannot prove.

Two counterexample probes were built and neither settles it, which is worth
recording so the next attempt does not repeat them. An unconditional
replacement between an early property read and a later member call refuses —
but through the *member* arm, so it never asks the question. A whole-value read
after the replacement certifies, and its published summary is
`{"call": {}, "shape": "callable"}` — a vacuous certification that asserts
nothing about the read. Constructing a program whose refused demand actually
reaches the whole-root arm is the prerequisite for revisiting this.

**The named next step is therefore not another origin premise.** It is giving
an operation the site its input was read at, so the whole-root arm can bind the
exact use instead of quantifying over all of them. That is strictly sounder
than today's behaviour for prefix rows as well, and it is what turns ADR 0069's
five stated reads into a proof.

## Full corpus

`2026-09-08-positional-full-v2.json`, 418 probes, `--timeout 600`, exit 0.
Against the [ADR 0068 run](2026-09-08-graph-fallback-recovery.md) exactly one
row differs, and it is not this ADR: `@kobalte/core@0.13.13` recovers from that
run's host-contention timeout and certifies its usual 508 entrypoints with the
root.

Against the `initial-reads-full` baseline this is the current authoritative
state, and it is the combined result of ADRs 0066, 0067, 0068 and 0069:

| Metric | Baseline | Current |
| --- | ---: | ---: |
| Complete rows | 318 | **324** |
| Partial rows | 61 | 62 |
| Refused rows | 30 | **23** |
| Certified entrypoints | 1,123 | **1,148** |

Seven rows gain, **411 of 418 are identical, and nothing is lost**. The
[per-row comparison](2026-09-08-positional-full-measurement.json) holds status,
certified-entrypoint count, root status and refusal stage for every probe.

## Validation

Focused producer test: 43 cases, including three pins that deliberately flip to
one row (a read inside a branch, a read after a preceding call, and a read in a
call argument on the assignment's own right-hand side) and eight new cases
pinning branch-exclusive and pre-loop reads as admitted and post-branch,
post-loop, later-`switch`, `++` and destructuring-assignment reads as refused.
Verifier and client mutation suites extended with `positional` and a
both-markers row, which no producer rule can establish and both layers refuse.
Full `make verify` passes with actual exit 0, `TOTAL 145.18s`, and no
`FAILED during step` marker.

## An operator trap that cost a corpus run

The Type Facts source manifest covers `rust/crates/typefacts` as well as
`apps/solid-typefacts`, `shims/` and the schema — so editing a **Rust** client
test stales `bin/solid-typefacts`. A corpus run against that pair reports
`0/409 attempted`, every row refusing with "Type Facts source-manifest stamp
does not match the configured pin". Pre-flight it, before any measurement:

```sh
node scripts/typefacts-source-identity.mjs --build-id dev --digest
python3 -c "import json;print(json.load(open('bin/solid-typefacts.buildinfo'))['sourceDigest'])"
```

No snapshots, bundled contracts or receipt formats changed. No commit or push
was made.
