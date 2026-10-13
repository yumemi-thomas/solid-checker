// Correct: the accessor `useMatch` returns is read where JSX tracks it.
import { createRouter, memoryHistory, useMatch } from "@solidjs/router";

function Reader() {
  const match = useMatch(() => "/stop/:id");
  return <p>{String(Boolean(match()))}</p>;
}

const Router = createRouter({
  routes: [{ path: "/stop/:id", component: () => <Reader /> }],
  history: memoryHistory("/stop/7?q=1"),
});

export default function App() {
  return <Router />;
}
