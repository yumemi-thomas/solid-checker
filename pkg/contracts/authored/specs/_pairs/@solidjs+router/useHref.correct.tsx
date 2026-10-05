// Correct: the accessor `useHref` returns is read where JSX tracks it.
import { createRouter, memoryHistory, useHref } from "@solidjs/router";

function Reader() {
  const href = useHref(() => "/elsewhere");
  return <p>{String(href())}</p>;
}

const Router = createRouter({
  routes: [{ path: "/stop/:id", component: () => <Reader /> }],
  history: memoryHistory("/stop/7?q=1"),
});

export default function App() {
  return <Router />;
}
