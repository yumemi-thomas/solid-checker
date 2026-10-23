import { createSignal } from "solid-js";
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from "@tanstack/solid-router";

const [count] = createSignal(0);

function Home() {
  const value = count();
  return <b id="home">home {value}</b>;
}

const root = createRootRoute();
const home = createRoute({ getParentRoute: () => root, path: "/", component: Home });
const router = createRouter({
  routeTree: root.addChildren([home]),
  history: createMemoryHistory({ initialEntries: ["/"] }),
});

export function App() {
  return <RouterProvider router={router} />;
}
