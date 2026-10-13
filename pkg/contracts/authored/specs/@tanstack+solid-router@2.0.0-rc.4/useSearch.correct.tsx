// Correct: the accessor `useSearch` returns is read where JSX tracks it.
import { RouterProvider, createMemoryHistory, createRootRoute, createRouter, useSearch } from "@tanstack/solid-router";

function Reader() {
  const search = useSearch({ strict: false });
  return <p>{String(search().q)}</p>;
}

function Root() {
  return <Reader />;
}

const router = createRouter({
  routeTree: createRootRoute({ component: Root }),
  history: createMemoryHistory({ initialEntries: ["/?q=1"] }),
});

export default function App() {
  return <RouterProvider router={router} />;
}
