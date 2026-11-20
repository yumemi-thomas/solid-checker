import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1133 round 1 (adjacent spelling of F5): `parseFromString` parses markup into a document whose nodes the program moves into the live one,
// where the image's `onerror` runs: the markup is written as `innerHTML`'s is.
export function Page() {
  let box!: HTMLDivElement;
  createEffect(
    () => 1,
    () => {
      const parsed = new DOMParser().parseFromString("<img src=x on" + "error=\"document.querySelector('div').$" + "$click()\">", "text/html");
      box.append(parsed.body.firstChild!);
    },
  );
  return <div ref={box} onClick={() => { count(); }} />;
}
