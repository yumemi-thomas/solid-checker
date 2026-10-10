// Correct: a property of the store `useParams` returns is read where JSX
// tracks it.
import { createRouter, memoryHistory, useParams } from "@solidjs/router";

function Reader() {
  const params = useParams();
  return <p>{params.id}</p>;
}

const Router = createRouter({
  routes: [{ path: "/stop/:id", component: () => <Reader /> }],
  history: memoryHistory("/stop/7?q=1"),
});

export default function App() {
  return <Router />;
}
