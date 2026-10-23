import { createSignal, For } from "solid-js";
import { Dynamic } from "@solidjs/web";

const [count] = createSignal(0);

// Uncertifiable: only `Dynamic` renders `Panel`. It is a component (ADR 0138),
// but rc.17's `dynamicCore` calls it as `untrack(() => component(props))`
// inside a memo, with no strict-read label, so no window opens and the read
// raises no STRICT_READ_UNTRACKED (runtime-oracle
// `component-identity-dynamic-only`).
function Panel() {
  const value = count();
  return <b>{value}</b>;
}

// Uncertifiable: the same read reached through a helper the body calls.
function readCount() {
  return count();
}
function Helped() {
  const value = readCount();
  return <b>{value}</b>;
}

// Positive: a tag renders `Both` too, and `createComponent` opens the window.
function Both() {
  const value = count();
  return <i>{value}</i>;
}

export function DynamicOnly() {
  return (
    <div>
      <Dynamic component={Panel} />
      <Dynamic component={Helped} />
      <Dynamic component={Both} />
      <Both />
    </div>
  );
}

// Positive: a `<For>` row callback opens its own strict-read window
// (`mapArray` labels it "<For>"), however the body around it is entered.
function Listed() {
  return (
    <For each={[1]}>
      {(item) => {
        const value = count();
        return <b>{value}:{item}</b>;
      }}
    </For>
  );
}

export function ListedPage() {
  return <Dynamic component={Listed} />;
}
