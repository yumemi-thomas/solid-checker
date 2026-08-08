import { createStore } from "solid-js";

const [state] = createStore({ count: 0 });

export function WrongSubpath() {
  return <div>{state.count}</div>;
}
