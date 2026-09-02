# The controls the non-emitting premise must never clear

`non-emitting-module-target` proves that an entrypoint whose bytes emit no
JavaScript asserts nothing. That claim is only meaningful against the shapes
that look identical to a census and must keep refusing. Every entrypoint here is
a real published shape.

## Refused, and why the obvious rule would have cleared them

All three of these have an **empty runtime ESM export surface** — the predicate
that was tried and reverted on 2026-09-02 — and all three emit on their first
statement:

- `./effects` → `effects.js`: `import { start } from "./start.js"; start();`
  with no export at all. This is `@solid-devtools/ext-adapter@0.17.0`'s
  `dist/index.js`, the exact control the reverted rule cleared. The value import
  alone refuses it.
- `./vitest` → `vitest.js`: `import { expect } from "vitest";
  expect.extend({});`, with `vitest` declared an **optional** peer dependency and
  not installed. This is `@solidjs/diagnostics@2.0.0-rc.3`'s `./vitest`, a
  matcher-registration module that is side-effect-only by design. Nothing about
  the dependency being optional, or absent, makes it inapplicable.
- `./cjs` → `bundle.cjs`: a CommonJS bundle. `"use strict";` is a statement and
  it emits, so the premise stops there and never reaches `module.exports`. This
  is `@solid-devtools/babel-plugin@0.3.1`'s shape.

All three refuse with the *same* reason as each other — `has no runtime ESM
exports` — which is the point: an empty export surface separates none of them
from a type-only module, and only emission does.

Two more refusals guard the erasability table itself:

- `./default-export` → `export default 1;`. The emitted module binds its
  default export to an evaluated expression. This is the tail of `@solidjs/h`'s
  `types/hyperscript.d.ts` in miniature (`declare const _default; export default
  _default;`), and it is why that member is not answered by this rule.
- `./enum` → `runtime-enum.ts`: a non-`declare` `enum` emits an object at
  runtime, so the case stays ordinary — and it **certifies**, which proves the
  refusals above are the shapes and not the fixture.

## The declaration-file premise's own traps

The second premise admits the member's `.d.ts` suffix as evidence, conjoined
with a declaration-grammar parse and an ambient gate. These three are what stop
the suffix from doing the work on its own:

- `./runtime-barrel` → `runtime-barrel.js`, whose bytes are **identical** to
  `non-emitting-module-target`'s `types/barrel.d.ts`. Under a runtime suffix
  they are a working re-export a consumer really evaluates, so the case is
  ordinary — and it **certifies**, which is a stronger control than a refusal:
  the pair proves the premise is the suffix *conjoined with* the ambient parse,
  never the bytes alone and never the suffix alone.
- `./implemented` → `implemented.d.ts`, carrying `declare const value = 1;`
  (TS1039). The suffix claims a declaration file and the bytes refute it.
- `./evaluated-default` → `evaluated-default.d.ts`, whose default export is
  `createRenderer()` rather than an ambient binding of the same bytes.

The last two are recorded as refusals, which is what the trap requires, but
note *why* they refuse today: a `.d.ts` runtime target that reaches the emission
batch hits the pre-existing "names source outside its configured project"
refusal (the false M1 message recorded in `docs/precision-backlog.md`), not a
message about the premise. The pin still works — widening the premise to clear
either of them moves the row from `refusals` to `inapplicable` and the snapshot
fails — but the reason string is not the premise's own.

## Inapplicable, and why these two are not a suffix guess

- `./ambient-js` → `ambient.js`, whose bytes are
  `export declare function createRenderer(): void;`. A `.js` name over ambient
  bytes must get the same answer as the identical bytes in a `.d.ts`, and it
  does. This is the fixture's load-bearing assertion: it is only satisfiable by
  a rule that reads statements, so it fails the moment the premise becomes a
  `.d.ts` classification.
- `./declare-enum` → `declare-enum.ts`, a single `declare enum`. Erased whole,
  and it declares a name, so the module is a deliberate type module rather than
  a vacuous one.

`.` certifies, so every disposition above is recorded inside a proposal that
exists rather than being the reason a package refuses entirely.

Every shape in this fixture also appears in the shared statement corpus,
`fixtures/module-emission/cases.json`, which holds the generator's TypeScript
table and the verifier's Oxc table to one answer per premise. This fixture pins
what the *pipeline* does with them; that corpus pins what the two tables say.
