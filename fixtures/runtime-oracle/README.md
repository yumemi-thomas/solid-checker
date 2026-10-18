# runtime-oracle

Cases for `scripts/runtime-oracle.mjs`, which runs each one in Chrome on the
Solid 2 dev build and compares the runtime's own diagnostics with the
checker's verdict at the same site. It measures unproven clean claims
directly: a dev diagnostic where the checker reports nothing is a finding the
checker is missing, which no corpus A/B by finding key can see.

```sh
SOLID_CHECKER_NATIVE_BIN="$PWD/rust/target/release/solid-checker-rust" \
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
node scripts/runtime-oracle.mjs --out <fresh.json> [--only id,...]
```

By default the cases run inside `../solid-checker-rc13-corpus/apps/helge-dev`
(solid-js, @solidjs/web and @solidjs/signals 2.0.0-rc.13, vite,
@solidjs/vite-plugin, playwright) and the newest playwright Chromium. A run of
all cases takes about 30 seconds at `--concurrency 3`.

## A case

A directory with an `App.tsx` and any other sources. These directories carry
no `tsconfig.json`, so `coverage.mjs` does not pick them up. Every exported
component is rendered in its own root, and the entry then dispatches
click, keyboard, focus, pointer and form events on every element in two passes,
so most cases need no scripted steps. Type stubs (`*.d.ts`) and
`node_modules` are not copied: a case runs against the real packages, and a
case `tsc` rejects is reported as `type-error` and proves nothing.

`oracle.json` is optional:

- `note`: what the case pins;
- `steps`: extra `feedback run` scenario steps;
- `open`: known unproven clean sites, each `{ code, site: "File.tsx:line", reason }`.
  They are debt, not passes. A listed site that stops firing fails the run as
  `stale-open`, so a fix updates the list.

## Verdicts

Per runtime diagnostic in a case file, against the checker rules that claim
its code (`codeRules` in the script):

- `covered-violation` or `covered-uncertifiable`: a finding of that kind sits on
  the diagnostic's line or on an application frame of its stack (the
  collector's `stack` field), because a finding may anchor at the call that led
  to the read rather than at the read itself;
- `unproven-clean`: nothing there. The run exits 1;
- `known-open`: an `unproven-clean` listed in `open`;
- `no-rule`: a code no rule claims (`REACTIVITY_HALTED`).

## Sources

The `c*` and `r*` cases are the reviewer counterexamples from the ADR 0319 to
0325 rounds (copied from `/tmp/adr319-review.noindex`, preserved in full under
`rust/target/research/session-2026-10-19/`). Two were edited only to
type-check against the real typings: `c3-misc` (`run(undefined)`) and `r4b`
(`(String(), g)()`). The `oracle-*` cases are minimal probes for the families
the first run found. `control-body-read` is a known-detected control.

The `r10` (router) and `r11` (imported constant) reviewer sets are not here
yet: they need a router install and a scenario for navigation.
