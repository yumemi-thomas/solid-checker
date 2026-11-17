import * as R from "@solidjs/router";
import { Home } from "./Home";

// A namespace import of the router: ADR 0335's premise refuses it, so no render is proven.
export const Router = R.createRouter({ routes: [{ path: "/", component: Home }] });

export const app = () => <Router />;
