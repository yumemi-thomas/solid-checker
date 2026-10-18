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

export function App() {
  const [count] = createSignal(0);
  const view = {
    label() {
      register(function () {
        return count();
      });
      return "x";
    },
  };
  return <div>{view.label()}</div>;
}

export function App3() {
  const [count] = createSignal(0);
  const view = {
    label() {
      function inner() {
        return count();
      }
      register(inner);
      return "x";
    },
  };
  return <div>{view.label()}</div>;
}
