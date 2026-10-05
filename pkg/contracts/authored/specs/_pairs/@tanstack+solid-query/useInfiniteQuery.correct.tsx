// Correct: a signal is read inside the options function, which `useInfiniteQuery`
// runs as a tracked compute, so the query follows it.
import { QueryClient, QueryClientProvider, useInfiniteQuery } from "@tanstack/solid-query";
import { createSignal } from "solid-js";

function Reader() {
  const [key] = createSignal("a");
  const query = useInfiniteQuery(() => ({ queryKey: ["probe", key()], queryFn: async () => 1, initialPageParam: 0, getNextPageParam: () => undefined }));
  return <p>{String(query.status)}</p>;
}

const client = new QueryClient();

export default function App() {
  return (
    <QueryClientProvider client={client}>
      <Reader />
    </QueryClientProvider>
  );
}
