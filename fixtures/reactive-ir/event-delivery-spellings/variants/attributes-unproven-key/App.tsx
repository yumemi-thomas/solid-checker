import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);
declare function key(): string;

// ADR 1133 round 2 (N1): `link[key()]` is the element's `attributes` collection when `key()` returns it, and `.href` of the collection is the attribute node of `href`; K4 read a member named `href` as plain data, so the unproven read was inert and the write of the node's `value` certified the read.
export function Page() {
  let box!: HTMLDivElement;
  let link!: HTMLAnchorElement;
  createEffect(
    () => 1,
    () => {
      (link as any)[key()].href.value = "java" + "script:void 0";
    },
  );
  return (
    <div ref={box} onClick={() => { count(); }}>
      <a ref={link} href="/x">x</a>
    </div>
  );
}
