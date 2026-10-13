import { createSignal, untrack } from "solid-js";
import { createPagination } from "@solid-primitives/pagination";

export default function App() {
  const [pages] = createSignal(10);
  untrack(() => createPagination(() => {
    const current = pages();
    return { pages: current };
  }));
  return <p>{String(pages())}</p>;
}
