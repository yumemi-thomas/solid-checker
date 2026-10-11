// Correct: pathname is a reactive getter read in tracked JSX.
// ADR 0272 describes next.26; older context-substitutable versions stay open.
import { createRouter, memoryHistory, useLocation } from "@solidjs/router";

function Reader() {
  const location = useLocation();
  return <p>{location.pathname}</p>;
}

const Router = createRouter({
  routes: [{ path: "/stop/:id", component: () => <Reader /> }],
  history: memoryHistory("/stop/7?q=1"),
});

export default function App() {
  return <Router />;
}
