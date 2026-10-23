import { createSignal, onCleanup } from "solid-js";

// ADR 0361 (amended) with ADR 0364: the terminal attribute is followed by a
// spread the analysis cannot read (`attrs()` is a parameter's call), which may
// replace `onScroll`, so the host is not proven to run the forwarded callback
// as a `scroll` handler. The walk names no terminal: its `onCleanup` and its
// write stay obligations while any dispatch exists, although `focus()` fires no
// `scroll`.
function focusIt(target: HTMLElement) {
  target.focus();
}

function Scroller(props: { onScrolled: () => void; attrs: () => object }) {
  return <div onScroll={props.onScrolled} {...props.attrs()}>scroll</div>;
}

export function App(props: { attrs: () => object }) {
  const [count, setCount] = createSignal(0);
  return (
    <div>
      <Scroller attrs={props.attrs} onScrolled={() => { onCleanup(() => undefined); }} />
      <Scroller attrs={props.attrs} onScrolled={() => { setCount(count() + 1); }} />
      <button onClick={(event) => focusIt(event.currentTarget as HTMLElement)}>focus</button>
    </div>
  );
}
