import { createSignal } from "solid-js";
const [, setS] = createSignal(0);
function pageSlice<T>(items: readonly T[]): unknown {
  return items.slice(0, 2);
}
function K(props: { items: readonly number[] }) {
  return <p>{String(pageSlice(props.items))}</p>;
}
export function App() {
  return <K items={{ slice: setS } as unknown as readonly number[]} />;
}
