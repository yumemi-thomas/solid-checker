import { Show, createMemo } from "solid-js";
import { render } from "@solidjs/web";

function Unread(_props: { value: number }) {
  return <p>{1}</p>;
}

function Rendered(props: { value: number }) {
  return <p>{props.value}</p>;
}

export function Prop() {
  const memo = createMemo(async () => 1);
  return <Unread value={memo()} />;
}

export function PropRendered() {
  const memo = createMemo(async () => 1);
  return <Rendered value={memo()} />;
}

// A member tag is a component whatever its head's case.
function Held(props: { Comp: (props: { value: number }) => unknown }) {
  const memo = createMemo(async () => 1);
  return <props.Comp value={memo()} />;
}

const ui = { Unread };

export function Namespaced() {
  const memo = createMemo(async () => 1);
  return <ui.Unread value={memo()} />;
}

const _Under = Unread;

export function Underscore() {
  const memo = createMemo(async () => 1);
  return <_Under value={memo()} />;
}

export function Intrinsic() {
  const memo = createMemo(async () => 1);
  return <p>{memo()}</p>;
}

export function ControlFlow() {
  const memo = createMemo(async () => 1);
  return (
    <Show when={memo()}>
      <p>{1}</p>
    </Show>
  );
}

render(
  () => (
    <div>
      <Prop />
      <PropRendered />
      <Held Comp={Unread} />
      <Namespaced />
      <Underscore />
      <Intrinsic />
      <ControlFlow />
    </div>
  ),
  document.body,
);
