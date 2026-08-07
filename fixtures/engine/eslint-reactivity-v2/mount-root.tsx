import { render } from "@solidjs/web";
import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// `render` creates the root owner and runs its code callback under it, so an
// effect created inside the callback has an owner and reports nothing. The
// same effect at module top level has no owner and still reports — the
// difference is exactly the mount root the dialect now models.
render(() => {
  createEffect(
    () => count(),
    () => undefined,
  );
  return null;
}, {});

createEffect(
  () => count(),
  () => undefined,
);
