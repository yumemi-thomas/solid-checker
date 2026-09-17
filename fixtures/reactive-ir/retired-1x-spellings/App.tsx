import { batch, createComputed, createResource, createSignal, onMount } from "solid-js";

// Four names that exist in Solid 1.x and not in 2.0. TypeScript resolves all
// of them against the ambient declarations above, so nothing else in the
// toolchain objects; the bundled contract is the only thing that knows the
// real 2.0 export list.
const [count, setCount] = createSignal(0);
const [data] = createResource(async () => 1);

export function Migrated() {
  createComputed(() => count());
  onMount(() => setCount(1));
  batch(() => setCount(2));
  return <div>{data()}</div>;
}
