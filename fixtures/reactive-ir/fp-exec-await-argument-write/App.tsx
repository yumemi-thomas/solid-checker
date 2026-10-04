import { createSignal } from "solid-js";
declare function fetchRows(): Promise<string[]>;
export function Page() {
  const [rows, setRows] = createSignal<string[]>([]);
  async function load() {
    setRows(await fetchRows()); // the write runs after the await, outside any owner
  }
  load();
  return <div>{rows().length}</div>;
}
// Positive control: the write runs before the first await, during the call.
export function Before() {
  const [rows, setRows] = createSignal<string[]>([]);
  async function load() {
    setRows([]);
    await fetchRows();
  }
  load();
  return <div>{rows().length}</div>;
}
