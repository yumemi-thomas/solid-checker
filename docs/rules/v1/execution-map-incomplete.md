# v1/execution-map-incomplete

`SC9004` · **error** · uncertifiable

The checker does not have a complete, conformant execution map for the exact
source and compiler configuration being analyzed.

## What it does

Protects rules that need compiler-owned JSX execution semantics. The execution
map must enumerate every relevant source site and give each site exactly one
value or callback decision. Missing coverage, missing decisions, conflicting
decisions, stale source identity, and unsupported compiler modes are not
treated as “runs once.”

## Why is this analysis-limiting?

The checker must know whether a JSX value reruns reactively, runs eagerly, is
invoked in the caller's context, runs later as a callback, or is removed by the
compiler. Guessing would create either false warnings or missed errors.

## Coverage boundary

The controlled 1.x compiler producer currently classifies every supported JSX
site or rejects malformed facts before rule analysis starts. An ordinary fresh
1.x project using the bundled producer should therefore not reach `SC9004`.

The rule remains fail-closed for custom, stale, or future fact producers. Its
catalog wording is exercised by the synthetic catalog-prose program, and the
execution-map validation tests inject incomplete and conflicting maps directly.

## How to fix

Re-run analysis with the matching controlled compiler. If fresh analysis still
fails, report the source pattern and compiler options: the compiler producer or
contract adapter is incomplete, not the application expression.

## Related

- [v1/strict-read-untracked](./strict-read-untracked.md) — consumes execution facts
  when deciding whether a reactive read tracks
