import { Show, createSignal } from "solid-js";

// A named function of `length` 0 is `Show`'s value, not a callback: `Show`
// returns it and the handler runs it, with no strict read in Chrome (rc.21).
export function NamedValue() {
  const [count] = createSignal(1);
  const value = () => {
    count();
  };
  const shown = (<Show when={1}>{value as any}</Show>) as any;
  return <button onClick={() => shown()()}>go</button>;
}

// A default parameter does not count toward `length`.
export function NamedDefaultParameter() {
  const [count] = createSignal(1);
  const value = (_unused = 1) => {
    count();
  };
  const shown = (<Show when={1}>{value as any}</Show>) as any;
  return <button onClick={() => shown()()}>go</button>;
}

// With a parameter `Show` calls it under `untrack`: a strict read.
export function NamedCallback() {
  const [count] = createSignal(1);
  const callback = (_item: any) => {
    count();
  };
  return <Show when={1}>{callback as any}</Show>;
}

// The inline twin of the first case.
export function InlineValue() {
  const [count] = createSignal(1);
  const shown = (
    <Show when={1}>
      {
        (() => {
          count();
        }) as any
      }
    </Show>
  ) as any;
  return <button onClick={() => shown()()}>go</button>;
}

// A value run in the component body reads a prop strictly (rc.21, Chrome): the
// child's position proves no run, so the read stays uncertifiable.
export function NamedValueInBody(props: { name: number }) {
  const value = () => {
    props.name;
  };
  const shown = (<Show when={1}>{value as any}</Show>) as any;
  shown()();
  return <span />;
}
