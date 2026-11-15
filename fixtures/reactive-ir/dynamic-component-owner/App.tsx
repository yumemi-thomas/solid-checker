import { createEffect, createMemo, onCleanup } from "solid-js";
import { Dynamic, render } from "@solidjs/web";
import * as Web from "@solidjs/web";
import { Own } from "./Own";

// ADR 0992: `Dynamic(props)` is `createComponent(dynamic(() => props.component), omit(props, "component"))`, and `dynamicCore`'s component calls the function it gets in
// `createMemo(() => … untrack(() => component(props)))` (`solid-js@2.0.0-rc.21` `dist/solid.dev.js:1544`; `@solidjs/web` `dist/web.dev.js:2451`): a function that is the exact
// value of `component` is rendered under an owner, as a tag renders it. Each component calls `onCleanup` in its body: the owner emission's SC4001 site (ADR 0970).

// Rendered by `<Dynamic component={Chosen} />` only: entered by that render. Silent.
function Chosen() {
  onCleanup(() => {});
  return <p>chosen</p>;
}
// The same through a namespace (`<Web.Dynamic>`).
function Spaced() {
  onCleanup(() => {});
  return <p>spaced</p>;
}
// A value picked by a memo's accessor: the flow the dialect vouches for (ADR 0138).
function Picked() {
  onCleanup(() => {});
  return <p>picked</p>;
}
const pick = createMemo(() => Picked);

// Also entered some other way: the stored reference is an entry no render shows. Open.
let saved: (() => unknown) | undefined;
function Kept() {
  onCleanup(() => {});
  return <p>kept</p>;
}
function Keeper() {
  saved = Kept;
  createEffect(
    () => 0,
    () => {
      saved!();
    },
  );
  return <Dynamic component={Kept} />;
}

// A project component named `Dynamic` is no `Dynamic` of the dialect: its `component` prop is only a prop. Open.
function Loose() {
  onCleanup(() => {});
  return <p>loose</p>;
}

function App() {
  return (
    <div>
      <Dynamic component={Chosen} />
      <Web.Dynamic component={Spaced} />
      <Dynamic component={pick()} />
      <Keeper />
      <Own component={Loose} />
      <Dynamic component="p" />
    </div>
  );
}

render(() => <App />, document.body);
