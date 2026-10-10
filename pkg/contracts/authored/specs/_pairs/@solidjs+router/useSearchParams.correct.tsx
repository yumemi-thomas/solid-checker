// Correct: a property of the store `useSearchParams` returns is read where JSX
// tracks it.
import { createRouter, memoryHistory, useSearchParams } from "@solidjs/router";

function Reader() {
  const [search] = useSearchParams();
  return <p>{String(search.q)}</p>;
}

const Router = createRouter({
  routes: [{ path: "/stop/:id", component: () => <Reader /> }],
  history: memoryHistory("/stop/7?q=1"),
});

export default function App() {
  return <Router />;
}
