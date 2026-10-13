import { onSettled } from "solid-js";
import { createPagination } from "@solid-primitives/pagination";

export default function App() {
  onSettled(() => {
    try {
      createPagination({ pages: 10 });
    } catch {
      // Keep the emitted PRIMITIVE_IN_FORBIDDEN_SCOPE diagnostic observable.
    }
  });
  return <p>ready</p>;
}
