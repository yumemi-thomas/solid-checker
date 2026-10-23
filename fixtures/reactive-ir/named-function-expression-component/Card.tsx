import { createSignal } from "solid-js";

type CardProps = { label: string; onPick?: () => void };

// ADR 0377: a named function expression is reached through the `const` it
// initializes. Every tag of `Static` passes a string, so the read is quiet.
const Static = function Static(props: CardProps) {
  const label = props.label;
  return <div>{label}</div>;
};

// Every tag of `Reactive` passes a signal read: the read is a proven
// strict-read violation, no longer an obligation.
const Reactive = function Reactive(props: CardProps) {
  const label = props.label;
  return <div>{label}</div>;
};

// The inner name may differ from the `const`.
const Renamed = function Inner(props: CardProps) {
  const label = props.label;
  return <div>{label}</div>;
};

// A self-render through the inner name is a tag too: it passes a reactive
// label (depth 1 renders it), so the read is a proven violation although the
// outer tag passes a string.
const Tree = function Tree(props: CardProps & { depth: number }) {
  const label = props.label;
  const [depth] = createSignal(props.depth);
  return <div>{label}{props.depth > 0 ? <Tree label={String(depth())} depth={0} /> : null}</div>;
};

// Kept: the inner name handed out as a value reaches callers no tag shows.
const registry: unknown[] = [];
const Handed = function Handed(props: CardProps) {
  registry.push(Handed);
  const label = props.label;
  return <div>{label}</div>;
};

// Kept, the same hole for a declaration and an arrow: a component that hands
// itself out from its own body reaches callers no tag shows.
function SelfDecl(props: CardProps) {
  registry.push(SelfDecl);
  const label = props.label;
  return <div>{label}</div>;
}
const SelfArrow = (props: CardProps) => {
  registry.push(SelfArrow);
  const label = props.label;
  return <div>{label}</div>;
};

// Kept: a reference in a tag's own attribute is a value, not the tag (it
// spells the name, but only the opening and closing names render).
function SelfAttr(props: CardProps & { self?: (props: CardProps) => unknown }) {
  const label = props.label;
  const [title] = createSignal("t");
  const Self = props.self;
  return <div>{label}{Self ? <Self label={title()} /> : null}</div>;
}

// Kept: a `let` may be reassigned; only a `const` names the function.
let Mutable = function Mutable(props: CardProps) {
  const label = props.label;
  return <div>{label}</div>;
};

export function Screen() {
  const [title] = createSignal("t");
  return (
    <>
      <Static label="x" />
      <Reactive label={title()} />
      <Renamed label="y" />
      <Tree label="z" depth={1} />
      <Handed label="w" />
      <Mutable label="v" />
      <SelfDecl label="u" />
      <SelfArrow label="s" />
      <SelfAttr label="r" self={SelfAttr}></SelfAttr>
    </>
  );
}
