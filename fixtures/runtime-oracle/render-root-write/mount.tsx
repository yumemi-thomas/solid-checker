import { hydrate, render } from "@solidjs/web";
import Home, { Control, markHydrated } from "./App";

render(() => Home() as unknown as HTMLElement, document.createElement("div"));

// hydrate reads the server's `_$HY` table; a page with no server markup supplies an empty
// one, which takes hydrate's plain path to render and so to its code callback.
Object.assign(globalThis, { _$HY: { done: false, events: [], completed: new Set(), r: {} } });
hydrate(() => {
  markHydrated();
  return document.createElement("span");
}, document.createElement("div"));

render(() => Control() as unknown as HTMLElement, document.createElement("div"));
