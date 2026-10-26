// probus-hk stores/sheets.ts:60: a module-level store object exported with a member that reads
// signals (`phoneBarHidden: () => modalDepth() > 0 || …`).
import { createSignal } from "solid-js";

const [moreOpen] = createSignal(false);
const [modalDepth] = createSignal(0);

export const sheets = {
  moreOpen,
  /** Whether a modal sheet has replaced the phone tab bar. */
  phoneBarHidden: () => modalDepth() > 0 || moreOpen(),
};
