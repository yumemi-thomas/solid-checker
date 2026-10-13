// This is a local homonym with no router declaration identity. The explicit
// premise remains Assumed; missing identity cannot supply a visible veto.
const RouterContext = {};
const createRouter = (config: unknown) => config;
export const context = RouterContext;
export const Router = createRouter({ history: {} });
