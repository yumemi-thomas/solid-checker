import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1133 round 1 (adjacent spelling of F5): the attribute node of `href` out of the `attributes` collection (`getNamedItem`), written in place as `getAttributeNode`'s is.
export function Page() {
  let box!: HTMLDivElement;
  let link!: HTMLAnchorElement;
  let frame!: HTMLIFrameElement;
  createEffect(
    () => 1,
    () => {
      link.attributes.getNamedItem("href")!.value = "java" + "script:void 0";
    },
  );
  return (
    <div ref={box} onClick={() => { count(); }}>
      <a ref={link} href="/x">x</a>
      <iframe ref={frame} />
    </div>
  );
}
