import { Show as SolidShow, createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";

// A project component that renders its children later: a wrapper the climb
// does not see through (the shadowed holder of ADR 0570's fixture).
export function Panel(props: { children: JSX.Element }) {
  const [open, setOpen] = createSignal(false);
  return (
    <div onClick={() => setOpen(true)}>
      <SolidShow when={open()}>{props.children}</SolidShow>
    </div>
  );
}

// A project lookalike of `Show` (shadowed name): it shows its children on a
// click whatever `when` says, so `<Show when={true}>` here proves nothing.
export function Show(props: { when: boolean; children: JSX.Element }) {
  const [open, setOpen] = createSignal(false);
  return <div onClick={() => setOpen(props.when)}>{open() ? props.children : null}</div>;
}
