// ADR 0272: assign getter operation; correct twin.
import { createRouter, memoryHistory, useLocation } from "@solidjs/router";

function Reader() {
  const location = useLocation();
  return <p>{String(Object.assign({}, location).pathname)}</p>;
}
const Router = createRouter({
  routes: [{ path: "/stop/:id", component: () => <Reader /> }],
  history: memoryHistory("/stop/7?q=1#hash"),
});
export default function App() { return <Router />; }
