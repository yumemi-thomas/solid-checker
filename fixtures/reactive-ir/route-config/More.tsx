import { createSignal } from "solid-js";
import { createRouter, defineRoute, memoryHistory } from "@solidjs/router";
import { routes } from "./table";

const [count] = createSignal(0);

// Positive: a definition through the identity helper `defineRoute`, an
// inline component value, and a stock history in the config.
function Defined() {
  const value = count();
  return <p>{value}</p>;
}

export const MoreRouter = createRouter({
  routes: [
    defineRoute({ path: "/defined", component: Defined }),
    {
      path: "/inline",
      component: () => {
        const value = count();
        return <p>{value}</p>;
      },
    },
  ],
  history: memoryHistory(),
});

export const TableRouter = createRouter({ routes });
