import { createPagination } from "@solid-primitives/pagination";
import { untrack } from "solid-js";
export default function App() {
  const [, page] = untrack(() => createPagination({ pages: 10 }));
  const current = page();
  return <p>{String(current)}</p>;
}
