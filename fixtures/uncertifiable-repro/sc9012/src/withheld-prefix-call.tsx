// probus-hk components/Alert.tsx:35 (stores/dismissed.ts:19-24): a component calls a method of a module's object
// literal whose members are all function literals; the module runs a call (`persistedCollection(...)`) before it.
import { createSignal } from "solid-js";

function persistedRows(): string[] {
  return [];
}

const [rows] = createSignal<string[]>(persistedRows(), { ownedWrite: true });

const alerts = {
  has(id: string): boolean {
    return rows().includes(id);
  },
};

export function AlertSheet(props: { stopId: string; onClose: () => void }) {
  const armed = () => alerts.has(props.stopId);
  const toggle = () => {
    if (armed()) props.onClose();
  };
  return <button onClick={toggle}>{armed() ? "armed" : "off"}</button>;
}
