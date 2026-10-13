// Correct: the accessor `useParams` returns is read where JSX tracks it.
import { RouterProvider, createMemoryHistory, createRootRoute, createRoute, createRouter, useParams } from "@tanstack/solid-router";

function Reader() {
  const params = useParams({ strict: false });
  return <p>{String(params().id)}</p>;
}

function Stop() {
  return <Reader />;
}

const rootRoute = createRootRoute();
const stopRoute = createRoute({ getParentRoute: () => rootRoute, path: "/stop/$id", component: Stop });
const router = createRouter({
  routeTree: rootRoute.addChildren([stopRoute]),
  history: createMemoryHistory({ initialEntries: ["/stop/7"] }),
});

export default function App() {
  return <RouterProvider router={router} />;
}
