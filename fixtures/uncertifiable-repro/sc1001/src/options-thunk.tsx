// ai-memory-ui screens/activity.tsx:64-67: a thunk returning an options object
// whose callback reads a signal, handed to the app's own `useQuery` wrapper.
import { createSignal } from "solid-js";

type QueryOptions<T> = { queryKey: unknown[]; queryFn: () => Promise<T> };

function useQuery<T>(options: () => QueryOptions<T>) {
  return { key: () => options().queryKey, fetch: () => options().queryFn() };
}

export function Activity() {
  const [days, setDays] = createSignal(7);
  const q = useQuery(() => ({
    queryKey: ["activity", days()],
    queryFn: async () => days() * 2,
  }));
  return <button onClick={() => setDays(30)}>{q.key().length}</button>;
}
