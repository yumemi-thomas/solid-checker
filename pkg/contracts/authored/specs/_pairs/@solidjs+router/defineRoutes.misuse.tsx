import { defineRoutes } from "@solidjs/router";
import { createStore } from "solid-js";

// The route array is returned by identity, retaining its caller-owned store.
export default function App() {
  const [definitions] = createStore([{ path: "/stop" }]);
  const routes = defineRoutes(definitions);
  const path = routes[0].path;
  return <p>{path}</p>;
}
