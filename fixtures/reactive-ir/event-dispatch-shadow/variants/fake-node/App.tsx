import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1125 (G6): `document.createElement` replaced, so the node Solid clones is a fake whose `addEventListener` keeps the listener.
const taken: Array<(event: Event) => void> = [];
const real = document.createElement.bind(document);
let armed = false;
document.createElement = ((tag: string) =>
  armed && tag === "template"
    ? { innerHTML: "", content: { firstChild: { cloneNode: () => ({ addEventListener(_type: string, listener: (event: Event) => void) { taken.push(listener); } }) } } }
    : real(tag)) as typeof document.createElement;

export function FakeNode() {
  createEffect(
    () => 1,
    () => {
      for (const run of taken) run(new Event("scroll"));
    },
  );
  armed = true;
  void (<div onScroll={() => { count(); }} />);
  armed = false;
  return <p />;
}
