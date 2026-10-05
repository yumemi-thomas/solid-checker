// Misuse: the accessor `useHref` returns is read in the body of a component
// rendered through JSX, outside any tracking scope.
import { createRouter, memoryHistory, useHref } from "@solidjs/router";

function Reader() {
  const href = useHref(() => "/elsewhere");
  const link = String(href());
  return <p>{link}</p>;
}

const Router = createRouter({
  routes: [{ path: "/stop/:id", component: () => <Reader /> }],
  history: memoryHistory("/stop/7?q=1"),
});

export default function App() {
  return <Router />;
}
