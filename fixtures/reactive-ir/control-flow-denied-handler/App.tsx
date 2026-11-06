import { createSignal, For, Show } from "solid-js";

const [c] = createSignal(1);

export function Mounted() {
  return (
    <Show when={1}>
      {() => {
        const frame = <iframe onLoad={() => { c(); }} />;
        document.body.append(frame as Node);
        return <p />;
      }}
    </Show>
  );
}

export function Row() {
  return (
    <For each={[1]}>
      {() => {
        const frame = <iframe onLoad={() => { c(); }} />;
        document.body.append(frame as Node);
        return <p />;
      }}
    </For>
  );
}

export function Clicked() {
  return (
    <For each={[1]}>
      {() => <iframe onClick={() => { c(); }} />}
    </For>
  );
}
