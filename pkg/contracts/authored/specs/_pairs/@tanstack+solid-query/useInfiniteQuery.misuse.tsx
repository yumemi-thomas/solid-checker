// Misuse: a signal is written inside the options function, which `useInfiniteQuery`
// runs as the tracked compute of a memo it creates: an owned scope.
import { QueryClient, QueryClientProvider, useInfiniteQuery } from "@tanstack/solid-query";
import { createSignal } from "solid-js";

function Reader() {
  const [seen, setSeen] = createSignal(0);
  const query = useInfiniteQuery(() => {
    setSeen(1);
    return { queryKey: ["probe"], queryFn: async () => 1, initialPageParam: 0, getNextPageParam: () => undefined };
  });
  return <p>{String(query.status)} {seen()}</p>;
}

const client = new QueryClient();

export default function App() {
  return (
    <QueryClientProvider client={client}>
      <Reader />
    </QueryClientProvider>
  );
}
