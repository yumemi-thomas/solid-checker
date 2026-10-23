import { createSignal, onCleanup } from "solid-js";

// ADR 0361 (amended): a callback whose walk ends anywhere that is not an
// intrinsic `on…` attribute has no terminal type. `Keeper` stores the prop in a
// module array the walk cannot follow, so nothing proves where it runs: its
// `onCleanup` and its write stay obligations while any dispatch exists, as
// before the join, although `focus()` fires no `scroll`.
function focusIt(target: HTMLElement) {
  target.focus();
}

const kept: Array<() => void> = [];

function Keeper(props: { onScrolled: () => void }) {
  kept.push(props.onScrolled);
  return <div>kept</div>;
}

export function runKept() {
  for (const fn of kept) fn();
}

export function App() {
  const [count, setCount] = createSignal(0);
  return (
    <div>
      <Keeper onScrolled={() => { onCleanup(() => undefined); }} />
      <Keeper onScrolled={() => { setCount(count() + 1); }} />
      <button onClick={(event) => focusIt(event.currentTarget as HTMLElement)}>focus</button>
    </div>
  );
}
