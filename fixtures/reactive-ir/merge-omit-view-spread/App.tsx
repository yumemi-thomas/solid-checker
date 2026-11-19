import { merge, omit } from "solid-js";
import * as Solid from "solid-js";

// ADR 1129: a `merge`/`omit` view of a component's own props, spread onto an
// intrinsic element, hands the element the keys its callers write (and the
// view's literals), minus the `omit` keys. Every caller below is a JSX site
// with literal attribute keys, so the spread's keys and its listener values
// are known. The prop types declare the content keys and a listener, as
// `JSX.HTMLAttributes` does, so no declared key set closes these spreads.

type Attrs = {
  label?: string;
  tone?: string;
  title?: string;
  class?: string;
  innerHTML?: string;
  children?: unknown;
  onClick?: () => void;
};
type ButtonProps = { variant?: "primary" | "plain"; type?: string; onClick?: () => void; children?: unknown };

// Proved: defaults by `merge`, the rest by `omit` (the cheatsheet's Props
// helpers): no caller writes a content key or a listener.
function Badge(props: Attrs) {
  const withDefaults = merge({ label: "new", tone: "info" }, props);
  const rest = omit(withDefaults, "label", "tone");
  return (
    <span {...rest} data-tone={withDefaults.tone}>
      {withDefaults.label}
    </span>
  );
}

// Proved: the listener a caller writes reaches the spread as that function,
// and `children` is omitted.
function Button(props: ButtonProps) {
  const merged = merge({ variant: "plain" as const, type: "button" }, props);
  const rest = omit(merged, "variant", "children");
  return (
    <button {...rest} class={merged.variant}>
      {merged.children}
    </button>
  );
}

// Proved: an `omit` written in the spread.
function InlineOmit(props: Attrs) {
  return <span {...omit(props, "label")}>{props.label}</span>;
}

// Proved: a `merge` view spread whole.
function MergedOnly(props: Attrs) {
  const merged = merge({ class: "m" }, props);
  return <span {...merged}>m</span>;
}

// Proved: `omit` through a namespace import of the exact primitive.
function NamespaceOmit(props: Attrs) {
  const rest = Solid.omit(props, "label");
  return <span {...rest}>{props.label}</span>;
}

// Misuse twin: a caller writes `innerHTML`, which reaches the spread beside
// the children. Stays uncertifiable.
function Html(props: Attrs) {
  const rest = omit(props, "label");
  return <span {...rest}>{props.label}</span>;
}

// Misuse twin: a caller writes `children`, the spread carries it beside the
// element's own `innerHTML`. Stays uncertifiable.
function ChildrenThrough(props: Attrs) {
  const rest = omit(props, "label");
  return <div {...rest} innerHTML="<b>x</b>" />;
}

// Refused: a project `merge` shadows the import.
function Shadowed(props: Attrs) {
  const merge = (a: Attrs, b: Attrs): Attrs => ({ ...a, ...b });
  const rest = merge({ tone: "x" }, props);
  return <span {...rest}>s</span>;
}

// Refused: `omit` through a member of a project object.
const helpers = { omit };
function Member(props: Attrs) {
  const rest = helpers.omit(props, "label");
  return <span {...rest}>{props.label}</span>;
}

// Refused: a project wrapper of `omit`.
const withoutLabel = (p: Attrs) => omit(p, "label");
function Wrapper(props: Attrs) {
  return <span {...withoutLabel(props)}>{props.label}</span>;
}

// Refused: an `omit` key that is not a string literal.
const hiddenKey = "label" as const;
function DynamicKey(props: Attrs) {
  const rest = omit(props, hiddenKey);
  return <span {...rest}>{props.label}</span>;
}

// Refused: an `omit` predicate.
function Predicate(props: Attrs) {
  const rest = omit(props, (key) => key === "label");
  return <span {...rest}>{props.label}</span>;
}

// Refused: a caller spreads into the component, so its keys are not read.
function SpreadCaller(props: Attrs) {
  const rest = omit(props, "label");
  return <span {...rest}>{props.label}</span>;
}

// Refused: the component is also used as a value.
function AsValue(props: Attrs) {
  const rest = omit(props, "label");
  return <span {...rest}>{props.label}</span>;
}
export const asValue = AsValue;

// Refused: the component writes its props object.
function WritesProps(props: Attrs) {
  props.title = "t";
  const rest = omit(props, "label");
  return <span {...rest}>{props.label}</span>;
}

// Refused: a view read through a call (the callee gets it as `this`).
function CallsMember(props: Attrs) {
  const rest = omit(props, "label");
  rest.onClick?.();
  return <span {...rest}>{props.label}</span>;
}

// Refused: a literal source whose listener is a getter.
function GetterListener(props: Attrs) {
  const merged = merge(
    {
      get onClick() {
        return () => {};
      },
    },
    props,
  );
  return <span {...merged}>g</span>;
}

// Refused: exported, so callers outside this file are not seen.
export function Exported(props: Attrs) {
  const rest = omit(props, "label");
  return <span {...rest}>{props.label}</span>;
}

const extra: Attrs = { title: "e" };

export function Page() {
  return (
    <div>
      <Badge />
      <Badge label="hot" tone="warn" title="hint" />
      <Button variant="primary" onClick={() => console.log("save")}>
        save
      </Button>
      <Button>cancel</Button>
      <InlineOmit label="a" title="b" />
      <MergedOnly title="c" />
      <NamespaceOmit label="d" />
      <Html label="e" innerHTML="<i>e</i>" />
      <ChildrenThrough label="f">f</ChildrenThrough>
      <Shadowed title="g" />
      <Member label="h" />
      <Wrapper label="i" />
      <DynamicKey label="j" />
      <Predicate label="k" />
      <SpreadCaller {...extra} />
      <AsValue label="l" />
      <WritesProps label="m" />
      <CallsMember label="n" />
      <GetterListener title="o" />
      <Exported label="p" />
    </div>
  );
}
