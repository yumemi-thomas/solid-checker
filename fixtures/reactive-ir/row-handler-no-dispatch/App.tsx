import { createSignal, For } from "solid-js";

const [a] = createSignal(0);

// Negative: a handler in a `For` row, and nothing dispatches: only a user's
// event runs it, outside every window. Chrome (rc.21) reports nothing.
export function Row() {
  return (
    <section>
      <For each={[1]}>{() => {
        let el!: HTMLDivElement;
        let named!: HTMLDivElement;
        const view = (
          <section>
            <div ref={el} onScroll={() => { void a(); }} />
            <div ref={named} onScroll={() => { const g = () => { void a(); }; g(); }} />
          </section>
        );
        return view;
      }}</For>
    </section>
  );
}
