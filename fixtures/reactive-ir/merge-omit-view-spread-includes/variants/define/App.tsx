import { omit } from "solid-js";
import type { JSX } from "@solidjs/web";

// Uncertifiable (SC8003): `includes` replaced by a reflective write through
// the prototype (ADR 0292's veto).
const includes = Array.prototype.includes;
Object.defineProperty(Array.prototype, "includes", {
  configurable: true,
  writable: true,
  value(this: unknown[], value: unknown, from?: number): boolean {
    return value === "children" ? false : includes.call(this, value, from);
  },
});

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
