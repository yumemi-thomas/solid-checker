import { createComponent, lazy } from "solid-js";
import Twin from "./Twin";
import * as BothModule from "./Both";

// ADR 0992: `lazy` renders the export it names under an owner (`createMemo(() => createComponent(Comp, props), { sync: true })`), so the cleanup of `Page` and `Named` has an owner
// and Chrome reports nothing; the checker is silent there. `Twin` is also called from a timer, and `Both` is also read through a namespace and called from a timer: Chrome reports
// NO_OWNER_CLEANUP at their `onCleanup`, and the checker must answer (a row), never stay silent.
const PageLazy = lazy(() => import("./Page"));
const NamedLazy = lazy(() => import("./Named"), { export: "Named" });
const TwinLazy = lazy(() => import("./Twin"));
const BothLazy = lazy(() => import("./Both"));

export function ByLazy() {
  return (
    <div>
      <PageLazy />
      <NamedLazy />
    </div>
  );
}

export function ByTimer() {
  setTimeout(() => {
    Twin();
  }, 0);
  return <TwinLazy />;
}

export function ByNamespace() {
  const run = BothModule.default;
  setTimeout(() => {
    run();
  }, 0);
  return <BothLazy />;
}

// The shapes the corpus writes (every-deck-of-cards, probus-hk, app-game): the `lazy` wrapper is handed to a router, which renders it (`createComponent(props.component, {})`), and the
// wrapper may even be called with no owner current (a timer): its memo is the owner of what it renders, so `Page`'s cleanup has an owner either way. Chrome reports nothing.
function Outlet(props: { component: () => unknown }) {
  return createComponent(props.component as (props: object) => any, {});
}

export function ByRouter() {
  return <Outlet component={PageLazy} />;
}

export function ByWrapperTimer() {
  setTimeout(() => {
    (PageLazy as unknown as (props: object) => unknown)({});
  }, 0);
  return <p>wrapper called from a timer</p>;
}
