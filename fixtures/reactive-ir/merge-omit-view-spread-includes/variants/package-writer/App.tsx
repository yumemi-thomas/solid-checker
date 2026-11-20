import "fz-includes";
import { omit } from "solid-js";
import type { JSX } from "@solidjs/web";

// Uncertifiable (SC8003): an installed package replaces
// `Array.prototype.includes` where it loads. It names no Solid package (a
// clear package), and still decides what `omit` hides.

type Attrs = {
  label?: string;
  textContent?: string;
  children?: JSX.Element;
};

function Leaky(props: Attrs) {
  const rest = omit(props, "children");
  return <div {...rest} innerHTML="<b>x</b>" />;
}

export function Page() {
  return <Leaky>child</Leaky>;
}
