import { createRouter, memoryHistory, useRouteMatches } from "@solidjs/router";

function Reader() {
  const matches = useRouteMatches();
  return <p>{matches().length}</p>;
}

const Router = createRouter({
  routes: [{ path: "/stop/:id", component: () => <Reader /> }],
  history: memoryHistory("/stop/7"),
});

export default function App() {
  return <Router />;
}
