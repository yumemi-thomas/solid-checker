import { createRouter, hashHistory, memoryHistory } from "@solidjs/router";

export const absent = createRouter({ routes: [] });
export const direct = createRouter({ routes: [], history: memoryHistory("/a") });
const history = hashHistory();
export const held = createRouter({ routes: [], history });
export const read = () => history.get();
export const move = () => history.set({ value: "/b" });
