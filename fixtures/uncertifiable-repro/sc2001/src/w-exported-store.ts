// probus-hk stores/sheets.ts:52-58: a module-level store object exported with methods that write
// a signal declared without `ownedWrite`.
import { createSignal } from "solid-js";

const [modalDepth, setModalDepth] = createSignal(0);

export const sheets = {
  modalDepth,
  pushModal() {
    setModalDepth((depth) => depth + 1);
  },
};
