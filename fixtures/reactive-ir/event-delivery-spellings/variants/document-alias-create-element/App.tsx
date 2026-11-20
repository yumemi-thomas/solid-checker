import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1133 round 1 (adjacent spelling of F2): `createElement` out of a `const` alias of the document makes a script element.
export function Page() {
  let box!: HTMLDivElement;
  let link!: HTMLAnchorElement;
  let frame!: HTMLIFrameElement;
  createEffect(
    () => 1,
    () => {
      const doc = document; const { createElement: make } = doc; const script = make.call(doc, "script") as HTMLScriptElement; script.text = "1"; document.head.append(script);
    },
  );
  return (
    <div ref={box} onClick={() => { count(); }}>
      <a ref={link} href="/x">x</a>
      <iframe ref={frame} />
    </div>
  );
}
