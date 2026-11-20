import { omit } from "solid-js";
import type { JSX } from "@solidjs/web";

// Uncertifiable (SC8003): `includes` replaced by a definer whose receiver is
// read under a built key (`"proto" + "type"`) and whose key is built too, so
// no name is spelled at the write (ADR 1129 round 2, F2).
const A: any = Array;
const key = ["incl", "udes"].join("");
const includes: (value: unknown, from?: number) => boolean = A["proto" + "type"][key];
Object.defineProperty(A["proto" + "type"], key, {
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
