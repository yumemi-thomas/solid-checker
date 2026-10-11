// Correct: construct the hook memo in the component owner.
import { onSettled } from "solid-js";
import { createRouter, memoryHistory, useResolvedPath } from "@solidjs/router";

function Reader() {
  const result = useResolvedPath(() => "/stop/7");
  onSettled(() => undefined);
  return <p>{String(result())}</p>;
}

const Router = createRouter({ routes: [{ path: "/stop/:id", component: () => <Reader /> }], history: memoryHistory("/stop/7") });
export default function App() { return <Router />; }
