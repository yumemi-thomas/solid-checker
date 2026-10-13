// Correct: the accessor `useResolvedPath` returns is read where JSX tracks it.
import { createRouter, memoryHistory, useResolvedPath } from "@solidjs/router";

function Reader() {
  const resolved = useResolvedPath(() => "detail");
  return <p>{String(resolved())}</p>;
}

const Router = createRouter({
  routes: [{ path: "/stop/:id", component: () => <Reader /> }],
  history: memoryHistory("/stop/7?q=1"),
});

export default function App() {
  return <Router />;
}
