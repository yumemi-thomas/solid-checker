// Misuse: the accessor `useIsRouting` returns is read in the body of a
// component rendered through JSX, outside any tracking scope.
import { createRouter, memoryHistory, useIsRouting } from "@solidjs/router";

function Reader() {
  const isRouting = useIsRouting();
  const routing = String(isRouting());
  return <p>{routing}</p>;
}

const Router = createRouter({
  routes: [{ path: "/stop/:id", component: () => <Reader /> }],
  history: memoryHistory("/stop/7?q=1"),
});

export default function App() {
  return <Router />;
}
