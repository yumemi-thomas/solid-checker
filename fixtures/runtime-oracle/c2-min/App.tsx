import { createSignal } from "solid-js";

const registry: Array<() => number> = [];
function register(f: () => number) {
  registry.push(f);
}

// Runs every registered function while its body renders.
function Child() {
  let total = 0;
  for (const f of registry) total += f();
  return <span>{total}</span>;
}

function useRow(read: () => number, opened: () => void) {
  return {
    onKeyDown() {
      register(function () {
        return read();
      });
      opened();
    },
  };
}

export function App() {
  const [count] = createSignal(0);
  const [open, setOpen] = createSignal(false);
  const row = useRow(() => count(), () => setOpen(true));
  return <div {...row}>{open() ? <Child /> : "closed"}</div>;
}
