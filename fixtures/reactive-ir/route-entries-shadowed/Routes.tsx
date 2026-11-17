import { Home } from "./Home";

// A local `createRouter`: no router renders `Home`; it may call it from anywhere.
function createRouter(config: { routes: { path: string; component: () => unknown }[] }) {
  setTimeout(() => config.routes.forEach((route) => route.component()), 0);
  return () => null;
}

export const Router = createRouter({ routes: [{ path: "/", component: Home }] });

export const app = () => <Router />;
