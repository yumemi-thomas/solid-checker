import { createSignal } from "solid-js";

const registry: Array<() => number> = [];
function register(f: () => number) {
  registry.push(f);
}

function Child() {
  let total = 0;
  for (const f of registry) total += f();
  return <p>{total}</p>;
}

export function App() {
  const [count] = createSignal(0);
  const [open, setOpen] = createSignal(false);
  const actions = {
    show() {
      register(function () {
        return count();
      });
      setOpen(true);
    },
  };
  return (
    <div>
      <button onClick={actions.show}>show</button>
      {open() ? <Child /> : null}
    </div>
  );
}
