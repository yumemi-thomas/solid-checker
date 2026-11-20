import { createRenderEffect, createSignal, merge, omit } from "solid-js";
import type { JSX } from "@solidjs/web";

// ADR 1129 in a closed program (`.solid-checker/runtime.json`), where an
// exact handler's owner is decided: the function a caller writes under a
// listener key reaches the intrinsic element's spread through a `merge`/`omit`
// view, so it is that element's handler and runs with no owner.

const [count, setCount] = createSignal(0);

type Props = { variant?: "primary" | "plain"; onPointerDown?: () => void; onClick?: () => void; children?: JSX.Element };

function Trigger(props: Props) {
  const merged = merge({ variant: "plain" as const }, props);
  const rest = omit(merged, "variant", "children");
  return <button {...rest}>{merged.children}</button>;
}

// The default under the listener key, from the view's literal.
function Defaulted(props: Props) {
  const merged = merge({ onPointerDown: () => setCount(count() + 1) }, props);
  return <button {...merged} />;
}

// The same handler shape in a component a caller spreads into: its keys
// are not read, so the handler stays an obligation.
function Spreaded(props: Props) {
  const rest = omit(props, "variant");
  return <button {...rest} />;
}

// ADR 1134 beside ADR 1129: the delegated dispatcher passes over a disabled
// control, so a handler the view spreads onto one may never run; the spread's
// handlers stay unresolved (an obligation), never a violation.
function DefaultedOff(props: Props) {
  const merged = merge({ onClick: () => { createRenderEffect(() => count(), () => {}); } }, props);
  return <button disabled {...merged} />;
}

// The view may own `disabled` (a caller writes it).
function KeyedOff(props: Props & { disabled?: boolean }) {
  const merged = merge({ onClick: () => { createRenderEffect(() => count(), () => {}); } }, props);
  return <button {...merged} />;
}

// Positive twin: an enabled button runs the view literal's handler, which creates an effect with no owner.
function DefaultedOn(props: Props) {
  const merged = merge({ onClick: () => { createRenderEffect(() => count(), () => {}); } }, props);
  return <button {...merged} />;
}

const more: Props = {};

export function Page() {
  return (
    <div>
      {/* Negative: the handler only writes a signal, which an event may do. */}
      <Trigger onPointerDown={() => setCount(count() + 1)}>clean</Trigger>
      {/* Misuse twin: the handler creates a render effect when the event fires. */}
      <Trigger
        onPointerDown={() => {
          createRenderEffect(() => "pointermove", () => {});
        }}
      >
        creates
      </Trigger>
      <Defaulted />
      <DefaultedOff />
      <KeyedOff disabled />
      <DefaultedOn />
      <Spreaded onPointerDown={() => setCount(count() + 1)} />
      <Spreaded {...more} />
    </div>
  );
}
