import { omit } from "solid-js";
import type { JSX } from "@solidjs/web";
import "./writer";

// Uncertifiable (SC8003): `writer.ts` replaces `includes` on the prototype
// `proto.ts` reached with `Object.getPrototypeOf([])` (ADR 1129 round 2, F2).

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
