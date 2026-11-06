// The positions the census follows. Nothing here may answer a `value-*` blocker: the census's negative side.
import { createEffect, createMemo, createSignal, merge } from "solid-js";

function useCount() {
  const [count, setCount] = createSignal(1);
  return { count, setCount };
}

export const [shared] = createSignal(1);

export function Followed(props: { label?: string }) {
  const [c, set] = createSignal(1);
  const [other] = createSignal(1);
  const hook = useCount();
  const alias = c;
  const doubled = createMemo(() => c() * 2);
  const next: ReturnType<typeof c> = 2;

  // A statement, `void`, `typeof` and `===` take no value.
  c;
  void c;
  void (typeof c === "function");
  void (c === other);

  createEffect(c, (value) => void value);
  props = merge({ label: "x" }, props);

  return (
    <p title={props.label}>
      {c()}
      {alias()}
      {doubled()}
      {hook.count()}
      {next}
      <button onClick={() => set(2)}>x</button>
      <button onClick={() => hook.setCount(2)}>y</button>
    </p>
  );
}
