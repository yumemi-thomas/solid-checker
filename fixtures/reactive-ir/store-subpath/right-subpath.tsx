import { createStore } from "solid-js/store";

const [state] = createStore({ count: 0 });

export function RightSubpath() {
  return <div>{state.count}</div>;
}
