import { createRouter } from "@solidjs/router";

const history = {
  get: () => "/",
  set: () => {},
  utils: { [["params", "Wrapper"].join("")]: (getParams: () => unknown) => getParams() },
};
export const router = createRouter({ routes: [], history });
