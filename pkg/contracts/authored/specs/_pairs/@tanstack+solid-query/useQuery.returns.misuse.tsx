// Misuse: a property of the object `useQuery` returns is read in the component
// body, outside any tracking scope.
import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/solid-query";

function Reader() {
  const query = useQuery(() => ({ queryKey: ["probe"], queryFn: async () => 1 }));
  const status = query.status;
  return <p>{status}</p>;
}

const client = new QueryClient();

export default function App() {
  return (
    <QueryClientProvider client={client}>
      <Reader />
    </QueryClientProvider>
  );
}
