import { createRouter as build, browserHistory, hashHistory, memoryHistory } from "@solidjs/router";
export const browser = build({ routes: [], history: browserHistory() });
export const hash = build({ routes: [], history: hashHistory() });
export const memory = build({ routes: [], history: memoryHistory("/") });
export const defaults = build({ routes: [], preloadLinks: true });
