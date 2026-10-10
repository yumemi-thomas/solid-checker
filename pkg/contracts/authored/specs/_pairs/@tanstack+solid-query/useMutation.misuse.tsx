// Misuse: a property of the store `useMutation` returns is read in the
// component body, outside any tracking scope.
import { QueryClient, QueryClientProvider, useMutation } from "@tanstack/solid-query";

function Save() {
  const mutation = useMutation(() => ({ mutationFn: async (value: number) => value }));
  const status = mutation.status;
  return <p>{status}</p>;
}

const client = new QueryClient();

export default function App() {
  return (
    <QueryClientProvider client={client}>
      <Save />
    </QueryClientProvider>
  );
}
