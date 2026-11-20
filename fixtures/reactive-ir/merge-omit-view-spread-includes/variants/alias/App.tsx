import { omit } from "solid-js";
import type { JSX } from "@solidjs/web";

// Uncertifiable (SC8003): `includes` replaced through an alias of the
// prototype (a write to a member named `includes`).
const proto: unknown[] = Array.prototype;
const includes = proto.includes;
proto.includes = function (this: unknown[], value: unknown, from?: number): boolean {
  return value === "children" ? false : includes.call(this, value, from);
};

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
