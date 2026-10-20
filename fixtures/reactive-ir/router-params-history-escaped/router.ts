import { createRouter, memoryHistory } from "@solidjs/router";
import type { RouterHistory } from "@solidjs/router";

function install(history: RouterHistory) {
  history.utils = { [["params", "Wrapper"].join("")]: (getParams: () => unknown) => getParams() };
}
const history = memoryHistory();
install(history);
export const router = createRouter({ routes: [], history });
