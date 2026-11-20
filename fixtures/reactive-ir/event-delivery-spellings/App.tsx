import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);
const rows = [{ done: false }];
declare const props: { open: boolean; location: string };
declare function key(): string;
declare function work(): void;

// The delivery is intact: a written dispatch (the census reads it), and the handler's read is proved.
export function Light() {
  const box = (<div onScroll={() => { count(); }} />) as HTMLDivElement;
  createEffect(
    () => 1,
    () => {
      box.dispatchEvent(new Event("scroll"));
    },
  );
  return <div />;
}

// The neighbours of the hazard spellings in this group's variants, none of which is a hazard: the program still delivers the handler's
// event, so `Light`'s read stays proved. Each line is the closest legal spelling of one variant.
export function Neighbours() {
  let box!: HTMLDivElement;
  let link!: HTMLAnchorElement;
  let frame!: HTMLIFrameElement;
  createEffect(
    () => 1,
    () => {
      // F1: `Reflect.apply`, `construct` and `has` read or call what they are given and no member of their own.
      Reflect.apply(work, null, []);
      Reflect.has(box, "id");
      // F2: the written calls with inert markup and a tag that is no script.
      box.insertAdjacentHTML("beforeend", "<i>x</i>");
      document.write("<p>hi</p>");
      document.createElement("div");
      // F3: a dispatch written as a dispatch is the census's, and a flag test reads nothing out.
      if (box.click) work();
      // F4: `JSON` and `constructor` used as the object of a member read, a replacer-less stringify, `typeof`.
      JSON.stringify(rows);
      JSON.parse("1");
      const name = (box as any)["constructor"]["name"];
      const kind = typeof JSON;
      // F5: an attribute node of a plain name, a plain `baseVal`, a scheme that is none, a reference past the scheme or outside an
      // attribute's value, and `open` or `location` taken from props.
      link.getAttributeNode("title");
      (link as any).href.baseVal = "/x";
      link.protocol = "https:";
      box.innerHTML = "<p title=\"a\">x &amp; y &times;</p>";
      frame.srcdoc = "<p>hi</p>";
      const { open, location } = props;
      // K4: a plain data key under an unknown key.
      (rows as any)[key()].done = true;
      void [name, kind, open, location];
    },
  );
  return (
    <div ref={box}>
      <a ref={link} href="/x?a=1&b=2">x</a>
      <iframe ref={frame} />
    </div>
  );
}
