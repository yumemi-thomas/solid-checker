// app-game ui-components-examples solid-props-proxy/example-3.tsx:71-108 (`onInput` at :27-38, also example-4.tsx:38, iframe-preview.tsx:58): a child
// puts an `omit` view of its props through a project function and spreads the result onto a `Dynamic`, never naming the callback.
import { createSignal, omit } from "solid-js";
import { Dynamic } from "@solidjs/web";
import type { JSX } from "@solidjs/web";

type InputProps = JSX.InputHTMLAttributes<HTMLInputElement>;

function combine(base: InputProps, extra: InputProps): InputProps {
  return { ...base, ...extra };
}

function Target(props: InputProps & { component: "input" }) {
  const rest = omit(props, "component");
  const combined = combine(rest, { class: "proxy" });
  return <Dynamic component={props.component} {...combined} />;
}

export function Example() {
  const [counter, setCounter] = createSignal(0);
  return (
    <Target
      component="input"
      value={counter()}
      onInput={(event) => {
        const value = Number(event.currentTarget.value);
        if (!Number.isNaN(value)) setCounter(value);
      }}
    />
  );
}
