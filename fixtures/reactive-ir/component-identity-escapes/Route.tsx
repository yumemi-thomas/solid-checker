import { For, createSignal } from "solid-js";

const [count] = createSignal(0);
const [items] = createSignal(["a", "b"]);

// A route component handed to a router the analysis does not read, inside an
// options object (`createFileRoute(...)({ component })`, TanStack's shape).
// Only the router decides whether it is rendered with `createComponent` or
// called plainly, so its own body read is uncertifiable. The `For` children
// callback runs in the context `For` creates however `Screen` is entered: a
// violation either way.
function Screen() {
  const top = count();
  return (
    <ul>
      <li>{top}</li>
      <For each={items()}>
        {(item) => {
          const prefix = count();
          return <li>{prefix}:{item}</li>;
        }}
      </For>
    </ul>
  );
}

declare function registerRoute(options: { component: () => unknown }): void;
registerRoute({ component: Screen });
