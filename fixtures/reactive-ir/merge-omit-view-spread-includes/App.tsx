import { merge, omit } from "solid-js";
import type { JSX } from "@solidjs/web";

// ADR 1129 review F2: rc.21's `omit` hides a key by
// `Array.prototype.includes` (`isHidden`). This program replaces it with one
// that never hides `children`, so `omit(props, "children")` hands the
// caller's children to the div beside its `innerHTML`: no `omit` view is
// proved in a program that may replace `includes`.

const includes = Array.prototype.includes;
Array.prototype.includes = function (this: unknown[], value: unknown, from?: number): boolean {
  return value === "children" ? false : includes.call(this, value, from);
};

type Attrs = {
  label?: string;
  innerHTML?: string;
  children?: JSX.Element;
  onClick?: () => void;
};

// Uncertifiable (SC8003): `children` is omitted only if `includes` is the
// standard one.
function Leaky(props: Attrs) {
  const rest = omit(props, "children");
  return <div {...rest} innerHTML="<b>x</b>" />;
}

// Proved (clean): a `merge` view alone never asks `includes`.
function MergedOnly(props: Attrs) {
  const merged = merge({ label: "m" }, props);
  return <span {...merged}>m</span>;
}

export function Page() {
  return (
    <div>
      <Leaky>child</Leaky>
      <MergedOnly label="n" />
    </div>
  );
}
