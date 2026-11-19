import { Loading, affects, createDeferred, createMemo, createSignal, refresh } from "solid-js";

const fetchRows = async (period: number) => [period, period + 1];

export function Rows() {
  const [period, setPeriod] = createSignal(1);
  const rows = createDeferred(() => fetchRows(period()));
  const total = createMemo(() => rows().length);
  return (
    <Loading fallback={<p>loading</p>}>
      <p>{total()}</p>
      <button
        type="button"
        onClick={() => {
          setPeriod((n) => n + 1);
          affects(rows);
          refresh(rows);
        }}
      >
        next
      </button>
    </Loading>
  );
}
