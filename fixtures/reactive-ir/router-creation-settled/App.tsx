import { createEffect, createMemo } from "solid-js";
import { createRouter } from "mini-router";

// ADR 1066: `Router` is a `const` bound to a call of `mini-router`'s `createRouter` (read as dependency source); its children function runs
// where the package's component runs it. Each creation below stays uncertifiable (README.md).
const Router = createRouter({ base: "/" });

function Layout(props: { children?: unknown }) {
  createMemo(() => 1);
  createEffect(
    () => 1,
    () => {},
  );
  return <div>{props.children}</div>;
}

export function App() {
  return (
    <Router>
      {(route) => {
        createMemo(() => route.path);
        return <Layout>{route.path}</Layout>;
      }}
    </Router>
  );
}
