// Hand-authored probe recipe for the `creates: []` claim domain of
// `@kobalte/utils@0.9.2`'s `noop`, on the `./src/noop.ts` artifact case.
//
// This is the corpus's isolating recipe. `noop` is
// `export function noop() { return; }` — no call, no loop, no invoking form of
// any kind — so the implementation census has nothing to refuse and the only
// thing standing between the claim and a certified closure is the probe gate
// itself. Every other candidate on the three rows that carry one refuses inside
// the census first, so without this recipe the workspace-side blocker below
// would be a prediction rather than a measurement.
//
// What it measures: `@kobalte/utils`'s only non-`.` entrypoint is
// `"./src/*": "./src/*"`, and every artifact case a candidate survives on is
// therefore a **TypeScript source file**. The private probe workspace places
// the authenticated snapshot at `<private>/node_modules/@kobalte/utils/`, and
// the pinned interpreter refuses to strip types from a file under
// `node_modules` (`ERR_UNSUPPORTED_NODE_MODULES_TYPE_STRIPPING`). So this
// module cannot evaluate, the run fails, and the gate refuses — which is the
// honest outcome and not something a recipe can be written around.
//
// It hands the package no `session` and no `harness`, and performs no `create`.

import { noop } from "@kobalte/utils/src/noop.ts";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });

  const before = Object.keys(globalThis).length;

  const answered = noop();
  if (answered !== undefined) {
    throw new Error(`noop returned ${JSON.stringify(String(answered))}`);
  }

  if (Object.keys(globalThis).length !== before) {
    harness.emit({
      marker: "create-operation",
      kind: "call",
      phase: "enter"
    });
  }

  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
