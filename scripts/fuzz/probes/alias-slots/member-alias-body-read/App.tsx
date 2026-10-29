// ADR 0511: a factory's returned signal taken out by a member Get (`const rows = store.rows`) and read in the component body
// is the factory's signal: rc.17 warns STRICT_READ_UNTRACKED at the read, as for `store.rows()`.
import { createSignal } from "solid-js";

function collection(initial: number) {
  const [rows] = createSignal(initial);
  return { rows, plain: () => 7 };
}

export function C1() {
  const store = collection(1);
  const rows = store.rows;
  const value = rows();
  return <p>{value}</p>;
}
