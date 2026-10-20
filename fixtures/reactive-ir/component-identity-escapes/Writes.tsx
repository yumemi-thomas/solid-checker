import { createSignal, untrack } from "solid-js";

const [value, setValue] = createSignal(0);

// Uncertifiable: `Step` is handed to `map` in an event handler, where a write
// is legal; only as a component would its body be an owned scope
// (runtime-probed: no REACTIVE_WRITE_IN_OWNED_SCOPE). An inline `untrack`
// runs in that same undecided body.
function Step() {
  setValue(1);
  untrack(() => setValue(2));
  return <b>{value()}</b>;
}

export function Steps() {
  return <button onClick={() => { [1].map(Step); }}>step</button>;
}
