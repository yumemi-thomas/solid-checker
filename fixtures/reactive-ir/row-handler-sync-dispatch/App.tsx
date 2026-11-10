import { createSignal, For } from "solid-js";

const [a] = createSignal(0);

// Uncertifiable: a handler in a `For` row, and the row dispatches its event
// synchronously, so the handler runs inside the row's strict-read window.
// Chrome (rc.21) warns STRICT_READ_UNTRACKED at both reads.
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
        el.dispatchEvent(new Event("scroll"));
        named.dispatchEvent(new Event("scroll"));
        return view;
      }}</For>
    </section>
  );
}
