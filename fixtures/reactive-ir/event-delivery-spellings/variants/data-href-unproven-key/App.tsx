import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);
declare function key(): string;
const links = [{ href: "/x", onClick: () => {}, label: "x" }];
// ADR 1133 round 2 (N1), the accepted cost: a data member named like an attribute (`href`, `src`, `onClick`) under a key nothing proves is no plain data key any more, because the key may be `attributes`; the read of a list's `href` withholds the event proofs of the program. A name no attribute has (`label`) stays plain.
export function Page() {
  let box!: HTMLDivElement;
  let link!: HTMLAnchorElement;
  createEffect(
    () => 1,
    () => {
      void links[key()].href; void links[key()].onClick;
    },
  );
  return (
    <div ref={box} onClick={() => { count(); }}>
      <a ref={link} href="/x">x</a>
    </div>
  );
}
