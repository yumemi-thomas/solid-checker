// probus-hk components/AlertSheet.tsx:40 (stores/alerts.ts:38, `items: store.rows`): a module runs a call before an
// object literal that holds a reference to a signal accessor as a member, which a component calls.
import { createSignal } from "solid-js";

function persistedRows(): string[] {
  return [];
}

const [rows] = createSignal<string[]>(persistedRows(), { ownedWrite: true });

const alerts = {
  items: rows,
  has(id: string): boolean {
    return rows().includes(id);
  },
};

export function Alert(props: { stopId: string; onClose: () => void }) {
  const armed = () => alerts.has(props.stopId);
  const toggle = () => {
    if (armed() || alerts.items().length > 0) props.onClose();
  };
  return <button onClick={toggle}>{armed() ? "armed" : "off"}</button>;
}
