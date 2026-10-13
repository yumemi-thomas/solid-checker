// Correct: the accessor `useIsRouting` returns is read where JSX tracks it.
import { createRouter, memoryHistory, useIsRouting } from "@solidjs/router";

function Reader() {
  const isRouting = useIsRouting();
  return <p>{String(isRouting())}</p>;
}

const Router = createRouter({
  routes: [{ path: "/stop/:id", component: () => <Reader /> }],
  history: memoryHistory("/stop/7?q=1"),
});

export default function App() {
  return <Router />;
}
