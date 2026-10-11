// Correct: argument 0 only reads while the hook's memo tracks it.
import { createSignal } from "solid-js";
import { createRouter, memoryHistory, useResolvedPath } from "@solidjs/router";

function Reader() {
  const [path, setPath] = createSignal("/stop/7");
  const result = useResolvedPath(() => { return path(); });
  return <p>{String(result())}</p>;
}

const Router = createRouter({ routes: [{ path: "/stop/:id", component: () => <Reader /> }], history: memoryHistory("/stop/7") });
export default function App() { return <Router />; }
