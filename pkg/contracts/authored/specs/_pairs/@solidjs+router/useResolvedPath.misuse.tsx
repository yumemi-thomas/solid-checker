// Misuse: the accessor `useResolvedPath` returns is read in the body of a
// component rendered through JSX, outside any tracking scope.
import { createRouter, memoryHistory, useResolvedPath } from "@solidjs/router";

function Reader() {
  const resolved = useResolvedPath(() => "detail");
  const path = String(resolved());
  return <p>{path}</p>;
}

const Router = createRouter({
  routes: [{ path: "/stop/:id", component: () => <Reader /> }],
  history: memoryHistory("/stop/7?q=1"),
});

export default function App() {
  return <Router />;
}
