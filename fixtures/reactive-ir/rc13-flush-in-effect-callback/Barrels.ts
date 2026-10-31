// ADR 0517: flush() through a project module that re-exports Solid. Type
// Facts carries no signature declaration for these calls; the package is the
// one the callee's exact symbol is declared in (rc.17 in Chrome:
// FLUSH_IN_EFFECT_CALLBACK, probes/barrel/ns-flush-apply).
import { createEffect, createRoot, createSignal } from "solid-js";
import * as Star from "./star";
import { flush as namedFlush } from "./named";
import { flush as starFlush } from "./star";

createRoot(() => {
  const [value] = createSignal(1);

  // Positive: by namespace, by a renamed import of `export *`, and of a named
  // re-export.
  createEffect(value, () => {
    Star.flush();
  });
  createEffect(value, () => {
    starFlush();
  });
  createEffect(value, () => {
    namedFlush();
  });

  // Negative: the same flush in the compute function, which runs outside the
  // effect callback's marker.
  createEffect(
    () => {
      Star.flush();
      return value();
    },
    () => {},
  );
});
