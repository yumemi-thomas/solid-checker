// Clean. Mirrors donegeon-client web/apps/client/src/app.tsx:36-50: a Switch over query state flags,
// the first matching Match renders; the last branch renders a Dynamic component prop.
import { Match, Switch } from "solid-js";
import { Dynamic } from "@solidjs/web";
import type { Component } from "solid-js";

function Guard(props: { component: Component; state: { pending: boolean; error: boolean; data?: string } }) {
  return (
    <Switch>
      <Match when={props.state.pending}>
        <main>Loading...</main>
      </Match>
      <Match when={props.state.error}>
        <main>Failed</main>
      </Match>
      <Match when={props.state.data}>
        <Dynamic component={props.component} />
      </Match>
    </Switch>
  );
}

export function App() {
  return <Guard component={() => <p>ready</p>} state={{ pending: false, error: false, data: "x" }} />;
}
