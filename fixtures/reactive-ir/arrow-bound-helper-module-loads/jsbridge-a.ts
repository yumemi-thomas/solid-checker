// Negative: `./jsbridge.js` has no declaration file and allowJs is off, so no analyzed
// file carries it; its runtime imports jsbridge-b.tsx, which imports this module.
import "./jsbridge.js";
import { createSignal } from "solid-js";

const [, setValue] = createSignal(0);

export const helper = () => {
  setValue(1);
};
