// Misuse: a signal is written inside the options function, which `useQuery`
// runs as the tracked compute of a memo it creates: an owned scope.
import { QueryClient, QueryClientProvider, useQuery } from "@tanstack/solid-query";
import { createSignal } from "solid-js";

function Reader() {
  const [seen, setSeen] = createSignal(0);
  const query = useQuery(() => {
    setSeen(1);
    return { queryKey: ["probe"], queryFn: async () => 1 };
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
