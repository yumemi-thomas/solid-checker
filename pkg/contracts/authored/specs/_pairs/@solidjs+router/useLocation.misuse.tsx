// Misuse: a property of the store `useLocation` returns is read in the body of
// a component rendered through JSX, outside any tracking scope.
import { createRouter, memoryHistory, useLocation } from "@solidjs/router";

function Reader() {
  const location = useLocation();
  const path = location.pathname;
  return <p>{path}</p>;
}

const Router = createRouter({
  routes: [{ path: "/stop/:id", component: () => <Reader /> }],
  history: memoryHistory("/stop/7?q=1"),
});

export default function App() {
  return <Router />;
}
