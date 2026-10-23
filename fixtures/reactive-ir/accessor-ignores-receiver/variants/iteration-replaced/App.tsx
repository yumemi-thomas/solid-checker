import { createSignal } from "solid-js";

// A visible write to the array iterator (ADR 0349): the destructuring pattern
// may no longer bind the tuple's slots.
const values = Array.prototype.values;
Array.prototype[Symbol.iterator] = values;

declare function send(value: number): Promise<number>;

function createMutation(call: () => Promise<number>) {
  const [state, setState] = createSignal({ kind: "idle" });
  const run = async () => {
    setState({ kind: "running" });
    return await call();
  };
  return { state, run };
}

// The parent's `Save`: quiet there, an obligation here, where the premise is
// withdrawn by the write above.
export function Save() {
  const [count] = createSignal(0);
  const save = createMutation(() => send(count()));
  return <button onClick={() => save.run()}>{save.state().kind}</button>;
}
