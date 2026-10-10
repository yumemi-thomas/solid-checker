// Misuse: the accessor `useMatch` returns is read in the body of a component
// rendered through JSX, outside any tracking scope.
import { createRouter, memoryHistory, useMatch } from "@solidjs/router";

function Reader() {
  const match = useMatch(() => "/stop/:id");
  const matched = String(Boolean(match()));
  return <p>{matched}</p>;
}

const Router = createRouter({
  routes: [{ path: "/stop/:id", component: () => <Reader /> }],
  history: memoryHistory("/stop/7?q=1"),
});

export default function App() {
  return <Router />;
}
