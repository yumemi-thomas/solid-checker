// Negative: `./bridge.js` resolves to `bridge.d.ts` (no allowJs); the runtime
// `bridge.js` beside it imports `bridge-b.tsx`, which imports this module.
import "./bridge.js";
import { createSignal } from "solid-js";
const [, setValue] = createSignal(0);
export const helper = () => {
  setValue(1);
};
