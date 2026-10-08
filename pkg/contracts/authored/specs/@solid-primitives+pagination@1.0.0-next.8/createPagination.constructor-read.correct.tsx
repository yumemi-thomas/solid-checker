import { untrack } from "solid-js";
import { createPagination } from "@solid-primitives/pagination";
export default function App() {
  const [, page] = untrack(() => createPagination({ pages: 10 }));
  return <p>{page()}</p>;
}
