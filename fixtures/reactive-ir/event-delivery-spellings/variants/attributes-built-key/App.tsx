import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);
declare function key(): string;

// ADR 1133 round 2 (N1): the collection under a key the program builds from pieces (a proof gives the text `attributes`, which the literal-key test did not read).
export function Page() {
  let box!: HTMLDivElement;
  let link!: HTMLAnchorElement;
  createEffect(
    () => 1,
    () => {
      (link as any)["attri" + "butes"].href.value = "java" + "script:void 0"; const kept = (link as any)["attri" + "butes"]; kept.src.value = "x";
    },
  );
  return (
    <div ref={box} onClick={() => { count(); }}>
      <a ref={link} href="/x">x</a>
    </div>
  );
}
