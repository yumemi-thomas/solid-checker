// Hand-authored probe recipe for the `creates: []` claim domain of
// `viaHelperChain`. See `plain.mjs` for what a recipe can and cannot do; this
// one exists so the three-hop local recursion the census proves also has its
// mandatory veto executed.

import { viaHelperChain } from "implementation-census-creates-package";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  viaHelperChain(() => {});
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
