import { createPagination } from "@solid-primitives/pagination";
import { untrack } from "solid-js";
export default function App() {
  const [props] = untrack(() => createPagination({ pages: 10 }));
  return <p>{String(props().length)}</p>;
}
