// Misuse: initial argument 0 writes while the hook's memo tracks it. Later recomputation is not exercised.
import { createSignal } from "solid-js";
import { createRouter, memoryHistory, useResolvedPath } from "@solidjs/router";

function Reader() {
  const [path, setPath] = createSignal("/stop/7");
  const result = useResolvedPath(() => { setPath("/stop/8"); return path(); });
  return <p>{String(result())}</p>;
}

const Router = createRouter({ routes: [{ path: "/stop/:id", component: () => <Reader /> }], history: memoryHistory("/stop/7") });
export default function App() { return <Router />; }
