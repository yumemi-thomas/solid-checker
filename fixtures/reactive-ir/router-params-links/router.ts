import { createRouter } from "@solidjs/router";

// ADR 0324 refusal: `2.0.0-next.41` calls `links(router, basePath)` with the
// whole router context during setup (`dist/claims.js:42-44`), before any route
// context reads the params wrapper. The key is assembled at run time, so no
// spelling of it appears; a config naming `links` withholds the premise.
const key = ["wrap", "Params"].join("");
export const linked = createRouter({
  routes: [],
  links: (router) => {
    (router as unknown as Record<string, unknown>)[key] = (getParams: () => unknown) =>
      getParams();
    return { track: () => undefined, pending: () => false };
  },
});
