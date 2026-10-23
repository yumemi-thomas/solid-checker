import { createSignal } from "solid-js";

// A visible write to `Function.prototype.bind`: the runtime builds the
// accessor with `bind`, so what it returns is no longer read.
const bind = Function.prototype.bind;
Function.prototype.bind = function (this: Function, ...args: [unknown, ...unknown[]]) {
  return bind.apply(this, args);
} as typeof bind;

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
