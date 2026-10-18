import { createSignal } from "solid-js";

let pending: (() => number) | undefined;

function Child() {
  const v = pending ? pending() : 0;
  return <p>{v}</p>;
}

export function App() {
  const [count] = createSignal(0);
  const [open, setOpen] = createSignal(false);
  const actions = {
    show() {
      pending = function () {
        return count();
      };
      setOpen(true);
    },
  };
  return (
    <div>
      <button onClick={actions.show}>show</button>
      {open() && <Child />}
    </div>
  );
}
