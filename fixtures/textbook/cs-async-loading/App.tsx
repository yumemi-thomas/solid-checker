import { Loading, createMemo, createSignal, isPending } from "solid-js";

const fetchUser = async (id: number) => ({ id, name: `user ${id}` });

export function UserCard() {
  const [id, setId] = createSignal(1);
  const user = createMemo(() => fetchUser(id()));
  return (
    <Loading fallback={<p>loading</p>}>
      <p>{isPending(() => user()) ? "updating" : user().name}</p>
      <button type="button" onClick={() => setId((n) => n + 1)}>
        next
      </button>
    </Loading>
  );
}
