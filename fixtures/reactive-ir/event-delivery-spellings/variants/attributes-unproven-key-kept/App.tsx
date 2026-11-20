import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);
declare function key(): string;

// ADR 1133 round 2 (N1): the same node kept in a name and written by a helper (`textContent` and `nodeValue` write it as `value` does).
export function Page() {
  let box!: HTMLDivElement;
  let link!: HTMLAnchorElement;
  createEffect(
    () => 1,
    () => {
      const node = (link as any)[key()].href; const set = (target: any) => { target.textContent = "java" + "script:void 0"; }; set(node);
    },
  );
  return (
    <div ref={box} onClick={() => { count(); }}>
      <a ref={link} href="/x">x</a>
    </div>
  );
}
