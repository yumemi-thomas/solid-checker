// Misuse: a property of the store `useSearchParams` returns is read in the
// body of a component rendered through JSX, outside any tracking scope.
import { createRouter, memoryHistory, useSearchParams } from "@solidjs/router";

function Reader() {
  const [search] = useSearchParams();
  const q = String(search.q);
  return <p>{q}</p>;
}

const Router = createRouter({
  routes: [{ path: "/stop/:id", component: () => <Reader /> }],
  history: memoryHistory("/stop/7?q=1"),
});

export default function App() {
  return <Router />;
}
