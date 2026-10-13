// Correct: the object `useQuery` returns is read where JSX tracks it.
import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/solid-query";

function Reader() {
  const query = useQuery(() => ({ queryKey: ["probe"], queryFn: async () => 1 }));
  return <p>{query.status}</p>;
}

const client = new QueryClient();

export default function App() {
  return (
    <QueryClientProvider client={client}>
      <Reader />
    </QueryClientProvider>
  );
}
