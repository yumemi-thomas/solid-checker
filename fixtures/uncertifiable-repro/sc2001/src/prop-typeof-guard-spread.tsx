// ai-memory-ui components/popover.tsx:117-135 (Trigger; `measure` from components/select.tsx:42-60): a child with an explicit `onClick` that spreads an
// `omit` view of its props after it, and calls the prop behind `typeof props.onClick === "function"`.
import { createSignal, omit } from "solid-js";
import type { JSX } from "@solidjs/web";

function Trigger(props: JSX.ButtonHTMLAttributes<HTMLButtonElement>) {
  const others = omit(props, "onClick", "type");
  return (
    <button
      type="button"
      onClick={(event) => {
        if (typeof props.onClick === "function") props.onClick(event);
      }}
      {...others}
    />
  );
}

export function Select() {
  const [width, setWidth] = createSignal(0);
  let anchor: HTMLDivElement | undefined;
  const measure = () => {
    if (anchor) setWidth(anchor.offsetWidth);
  };
  return (
    <div ref={anchor} onPointerDown={measure}>
      <Trigger onClick={measure}>{width()}</Trigger>
    </div>
  );
}
