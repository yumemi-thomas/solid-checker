import { createPagination } from "@solid-primitives/pagination";
export default function App() {
  const [, page] = createPagination({ pages: 10 });
  return <p>{page()}</p>;
}
