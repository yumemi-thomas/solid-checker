// Negative: `libws` is a workspace package: `node_modules/libws` is a symlink to
// `libws-src`, outside `node_modules` once resolved, so it is project code seen
// only through `index.d.ts`. Its runtime `index.js` imports `ws-b.tsx`, which
// imports this module back, so `ws-b.tsx` can run before `helper` is declared.
import "libws";
import { createSignal } from "solid-js";

const [, setValue] = createSignal(0);

export const helper = () => {
  setValue(1);
};
