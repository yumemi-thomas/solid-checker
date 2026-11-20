import { omit } from "solid-js";
import type { JSX } from "@solidjs/web";

// ADR 1129 review F1: every binding a component is entered through is a
// caller (ADR 1127's `entry_bindings`). A named function expression is
// entered through its own name as well as through the `const` holding it, so
// a tag or a call through that name is a caller whose keys the spread may
// carry. The prop types declare the content keys and a listener, as
// `JSX.HTMLAttributes` does, so no declared key set closes these spreads.

type Attrs = {
  label?: string;
  title?: string;
  nested?: boolean;
  innerHTML?: string;
  children?: JSX.Element;
  onClick?: () => void;
};

function boom() {
  console.log("boom");
}

// Proved (clean): a named function expression whose own name nothing
// references: its callers are the holder's tags.
const Plain = function PlainInner(props: Attrs) {
  const rest = omit(props, "label");
  return <span {...rest}>{props.label}</span>;
};

// Proved (clean): the own name renders the component again with keys that
// carry no content key and no listener.
const Quiet = function QuietInner(props: Attrs): JSX.Element {
  const rest = omit(props, "label", "nested");
  return (
    <div>
      <span {...rest}>{props.label}</span>
      {props.nested ? null : <QuietInner nested title="q" />}
    </div>
  );
};

// Uncertifiable (SC8003): the own name is a caller that writes `innerHTML`,
// which reaches the spread beside the span's children.
const Row = function Inner(props: Attrs): JSX.Element {
  const rest = omit(props, "label", "nested");
  return (
    <div>
      <span {...rest}>{props.label}</span>
      {props.nested ? null : <Inner nested innerHTML="<b>x</b>" />}
    </div>
  );
};

// Uncertifiable (SC8003): the own name spelled as the holder's (`function
// Same` under `const Same`) is still a binding of its own.
const Same = function Same(props: Attrs): JSX.Element {
  const rest = omit(props, "label", "nested");
  return (
    <div>
      <span {...rest}>{props.label}</span>
      {props.nested ? null : <Same nested innerHTML="<b>y</b>" />}
    </div>
  );
};

// Uncertifiable (SC8003): the own name called as a function is a caller the
// proof cannot read as a JSX site.
const Called = function CalledInner(props: Attrs): JSX.Element {
  const rest = omit(props, "label", "nested");
  return (
    <div>
      <span {...rest}>{props.label}</span>
      {props.nested ? null : CalledInner({ nested: true, innerHTML: "<b>z</b>" })}
    </div>
  );
};

// Uncertifiable (SC4001): the own name's caller writes a listener that is no
// function literal, so the spread's handlers are not all known.
const Act = function ActInner(props: Attrs): JSX.Element {
  const rest = omit(props, "label", "nested");
  return (
    <div>
      <button {...rest}>{props.label}</button>
      {props.nested ? null : <ActInner nested onClick={boom} />}
    </div>
  );
};

export function Page() {
  return (
    <div>
      <Plain label="a" title="b" />
      <Quiet label="c" />
      <Row label="d" />
      <Same label="e" />
      <Called label="f" />
      <Act label="g" onClick={() => console.log("g")} />
    </div>
  );
}
