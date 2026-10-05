// Misuse: the accessor `useSearch` returns is read in the body of a
// component rendered through JSX, outside any tracking scope.
import { RouterProvider, createMemoryHistory, createRootRoute, createRouter, useSearch } from "@tanstack/solid-router";

function Reader() {
  const search = useSearch({ strict: false });
  const q = String(search().q);
  return <p>{q}</p>;
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
