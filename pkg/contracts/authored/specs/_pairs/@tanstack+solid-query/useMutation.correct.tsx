// Correct: the store `useMutation` returns is read where JSX tracks it.
import { QueryClient, QueryClientProvider, useMutation } from "@tanstack/solid-query";

function Save() {
  const mutation = useMutation(() => ({ mutationFn: async (value: number) => value }));
  return <p>{mutation.status}</p>;
}

const client = new QueryClient();

export default function App() {
  return (
    <QueryClientProvider client={client}>
      <Save />
    </QueryClientProvider>
  );
}
