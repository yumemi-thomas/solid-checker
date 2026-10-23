import { omit } from "solid-js";

type CardProps = { label: string; onPick?: () => void; class?: string };

// ADR 0373: nothing in this closed program renders or calls `Unused`, so no
// tag passes it a prop: the read is no strict-read obligation, and neither
// the forwarded `onPick` nor the spread view of its props hands a handler.
function Unused(props: CardProps) {
  const label = props.label;
  const rest = omit(props, "label");
  return (
    <div onClick={props.onPick} {...rest}>
      {label}
    </div>
  );
}

// The same, exported: a closed program has no importer outside its files,
// and none inside imports it.
export function UnusedExported(props: CardProps) {
  const label = props.label;
  return (
    <div onClick={props.onPick} {...props}>
      {label}
    </div>
  );
}

// An unnamed arrow bound to a `const` is censused through the `const`.
export const UnusedArrow = (props: CardProps) => {
  const label = props.label;
  return <div onClick={props.onPick}>{label}</div>;
};

// Kept: a named function expression's own name is visible only inside it,
// so its references say nothing about the outer `const`.
export const UnusedNamed = function UnusedNamed(props: CardProps) {
  const label = props.label;
  return <div onClick={props.onPick}>{label}</div>;
};

// Control: a referenced twin keeps what the census gives a rendered
// component (its tag passes a static label and an arrow).
function Rendered(props: CardProps) {
  const label = props.label;
  return <div onClick={props.onPick}>{label}</div>;
}

export function Screen() {
  return <Rendered label="x" onPick={() => {}} />;
}
