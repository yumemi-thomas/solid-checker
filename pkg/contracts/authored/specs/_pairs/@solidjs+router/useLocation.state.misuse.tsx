// ADR 0272: state getter operation; misuse twin.
import { createRouter, memoryHistory, useLocation } from "@solidjs/router";

function Reader() {
  const location = useLocation();
  const value = location.state;
  return <p>{String(value)}</p>;
}
const Router = createRouter({
  routes: [{ path: "/stop/:id", component: () => <Reader /> }],
  history: memoryHistory("/stop/7?q=1#hash"),
});
export default function App() { return <Router />; }
