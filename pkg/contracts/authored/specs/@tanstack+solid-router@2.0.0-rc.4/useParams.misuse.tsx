// Misuse: the accessor `useParams` returns is read in the body of a
// component rendered through JSX, outside any tracking scope.
import { RouterProvider, createMemoryHistory, createRootRoute, createRoute, createRouter, useParams } from "@tanstack/solid-router";

function Reader() {
  const params = useParams({ strict: false });
  const id = String(params().id);
  return <p>{id}</p>;
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
