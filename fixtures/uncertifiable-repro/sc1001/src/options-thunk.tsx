// ai-memory-ui screens/activity.tsx:64-67 (lib/query.ts:23-37): a thunk returning an options object whose callback reads a signal is
// handed to the app's own `useQuery` wrapper, which destructures the callback out of the object it gets and hands the object on.
import { createSignal } from "solid-js";

type QueryOptions<T> = { queryKey: unknown[]; queryFn: () => Promise<T> };

function useQuery<T>(options: () => QueryOptions<T>) {
  const resolved = options();
  const { queryFn } = resolved;
  return { ...resolved, queryFn: () => queryFn() };
}

export function Activity() {
  const [days, setDays] = createSignal(7);
  const q = useQuery(() => ({
    queryKey: ["activity", days()],
    queryFn: async () => days() * 2,
  }));
  return <button onClick={() => setDays(30)}>{q.queryKey.length}</button>;
}
