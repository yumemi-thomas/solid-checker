import { createTrackedEffect, flush } from "solid-js";
import { createRouter, memoryHistory } from "@solidjs/router";

// ADR 1026: `flush()` in a tracked effect is a plain throw (ADR 0893). A top-level route component throws where its
// `<Router>` renders, when the URL matches: reported unless a boundary above takes it. An uncaught throw halts the reactive
// system (REACTIVITY_HALTED), so each route that throws has a case of its own.

// Violation: the route at the initial URL.
function First() {
  createTrackedEffect(() => {
    flush();
  });
  return <p>first</p>;
}

export const FirstRouter = createRouter({ routes: [{ path: "/first", component: First }], history: memoryHistory("/first") });
