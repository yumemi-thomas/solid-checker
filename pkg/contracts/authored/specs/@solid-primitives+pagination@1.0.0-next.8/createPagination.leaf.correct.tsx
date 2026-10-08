import { onSettled, untrack } from "solid-js";
import { createPagination } from "@solid-primitives/pagination";

export default function App() {
  untrack(() => createPagination({ pages: 10 }));
  onSettled(() => {});
  return <p>ready</p>;
}
