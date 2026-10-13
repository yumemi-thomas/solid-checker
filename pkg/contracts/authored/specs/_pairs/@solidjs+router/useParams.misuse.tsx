// Misuse: a property of the store `useParams` returns is read in the body of a
// component rendered through JSX, outside any tracking scope.
import { createRouter, memoryHistory, useParams } from "@solidjs/router";

function Reader() {
  const params = useParams();
  const id = params.id;
  return <p>{id}</p>;
}

const Router = createRouter({
  routes: [{ path: "/stop/:id", component: () => <Reader /> }],
  history: memoryHistory("/stop/7?q=1"),
});

export default function App() {
  return <Router />;
}
