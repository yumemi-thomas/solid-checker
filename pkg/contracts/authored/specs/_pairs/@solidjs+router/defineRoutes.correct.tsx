import { defineRoutes } from "@solidjs/router";
import { createStore } from "solid-js";

export default function App() {
  const [definitions] = createStore([{ path: "/stop" }]);
  const routes = defineRoutes(definitions);
  return <p>{routes[0].path}</p>;
}
