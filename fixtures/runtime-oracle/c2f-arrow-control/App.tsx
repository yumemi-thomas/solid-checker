import { createSignal } from "solid-js";

const registry: Array<() => number> = [];
function register(f: () => number) {
  registry.push(f);
}

export function Child() {
  let total = 0;
  for (const f of registry) total += f();
  return <p>{total}</p>;
}

function useRow(read: () => number) {
  return {
    onKeyDown() {
      register(() => read());
    },
  };
}

export function C() {
  const [count] = createSignal(0);
  const row = useRow(() => count());
  return <div {...row}>c</div>;
}
