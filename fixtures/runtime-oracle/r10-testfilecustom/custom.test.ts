import { createRouter, memoryHistory } from "@solidjs/router";
const k = ["params", "Wrapper"].join("");
export const R = createRouter({ routes: [], history: { ...memoryHistory(), utils: { [k]: () => ({ id: {} }) } } });
