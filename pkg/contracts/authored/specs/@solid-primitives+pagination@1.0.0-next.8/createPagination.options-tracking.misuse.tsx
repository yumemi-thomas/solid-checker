import { createSignal, untrack } from "solid-js";
import { createPagination } from "@solid-primitives/pagination";

export default function App() {
  const [pages, setPages] = createSignal(10);
  try {
    untrack(() => createPagination(() => {
      const current = pages();
      setPages(current + 1);
      return { pages: current };
    }));
  } catch {
    // Keep the case-file REACTIVE_WRITE_IN_OWNED_SCOPE diagnostic observable.
  }
  return <p>{String(pages())}</p>;
}
