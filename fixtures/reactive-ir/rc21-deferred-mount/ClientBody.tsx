import { createMemo, createSignal } from "solid-js";

const pending = createMemo(async () => 1);

// Its own body: the first mount has no window, a later one does.
function Child() {
  const [a] = createSignal(0);
  const v = a();
  return <i>{v}</i>;
}

export default function ClientBody() {
  const [a] = createSignal(0);
  const v = a();
  const w = pending();
  return (
    <p>
      {v}
      {w}
      <Child />
    </p>
  );
}
