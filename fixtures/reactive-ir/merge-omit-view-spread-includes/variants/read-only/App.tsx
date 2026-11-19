import { omit } from "solid-js";
import type { JSX } from "@solidjs/web";

// Negative (clean): the program calls `includes` but writes no member of
// that name, so `omit` hides `children` and the div's `innerHTML` stands alone.
export const known = ["a", "b"].includes("a");

type Attrs = {
  label?: string;
  innerHTML?: string;
  children?: JSX.Element;
};

function Leaky(props: Attrs) {
  const rest = omit(props, "children");
  return <div {...rest} innerHTML="<b>x</b>" />;
}

export function Page() {
  return <Leaky>child</Leaky>;
}
