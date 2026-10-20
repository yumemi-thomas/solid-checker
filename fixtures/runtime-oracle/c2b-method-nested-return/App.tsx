import { createSignal } from "solid-js";

export function App() {
  const [count] = createSignal(0);
  const actions = {
    make() {
      return function () {
        return count();
      };
    },
  };
  const g = actions.make;
  return <button onClick={g}>show</button>;
}

export function App2() {
  const [count] = createSignal(0);
  const actions = {
    make() {
      function inner() {
        return count();
      }
      inner();
      return 1;
    },
  };
  return <button onClick={actions.make}>show</button>;
}
