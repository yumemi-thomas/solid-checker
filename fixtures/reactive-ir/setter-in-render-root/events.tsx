import { createSignal } from "solid-js";
import { hydrate } from "@solidjs/web";

const [label, setLabel] = createSignal("idle");

// Negative: the write sits in a click handler `hydrate`'s code creates. The
// handler runs on the event, not under the root, so it stays legal.
hydrate(
  () => <button onClick={() => setLabel("clicked")}>{label()}</button>,
  document.createElement("div"),
);
