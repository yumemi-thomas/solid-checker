// Misuse: argument 0 writes while the hook's memo tracks it.
import { createSignal } from "solid-js";
import { createRouter, memoryHistory, useHref } from "@solidjs/router";

function Reader() {
  const [path, setPath] = createSignal("/stop/7");
  const result = useHref(() => { setPath("/stop/8"); return path(); });
  return <p>{String(result())}</p>;
}

const Router = createRouter({ routes: [{ path: "/stop/:id", component: () => <Reader /> }], history: memoryHistory("/stop/7") });
export default function App() { return <Router />; }
