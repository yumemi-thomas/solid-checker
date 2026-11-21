import { Errored, Loading, createMemo, createSignal } from "solid-js";

const load = async (page: number) => {
  if (page > 3) throw new Error("no more pages");
  return [`item ${page}a`, `item ${page}b`];
};

export function Pages() {
  const [page, setPage] = createSignal(1);
  const items = createMemo(() => load(page()));
  return (
    <div>
      <button type="button" onClick={() => setPage((n) => n + 1)}>
        next page
      </button>
      <Errored fallback={(error) => <p>{String(error())}</p>}>
        <Loading fallback={<p>loading</p>}>
          <p>{items().join(", ")}</p>
        </Loading>
      </Errored>
    </div>
  );
}
