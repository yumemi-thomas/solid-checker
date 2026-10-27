import { omit } from "solid-js";

type Styled = { class?: string; style?: string };
type Props = { class?: string; label: string };
type WithHandler = { class?: string; onClick?: () => void };

// No spread below reports: the compiler resolves each declared type to a
// complete key set and the dialect lowers none of those keys as a listener.

// A project helper's declared return type.
function attrs(...names: string[]): Styled {
  return { class: names.join(" ") };
}

export function Helper() {
  return <div {...attrs("a", "b")} />;
}

// A declared object type with no `on` key.
export function Plain(props: Props) {
  return <div {...props} />;
}

// `omit` of a known `Props`, written in place.
export function Omitted(props: Props) {
  return <div {...omit(props, "class")} />;
}

// `omit` bound to an identifier first.
export function Rest(props: Props) {
  const rest = omit(props, "class");
  return <div {...rest} />;
}

// The omitted key is the listener key: what stays holds none.
export function OmitHandler(props: WithHandler) {
  return <div {...omit(props, "onClick")} />;
}

// A union of two handler-free object types.
export function Union(props: Styled | { title?: string }) {
  return <div {...props} />;
}

// An intersection of handler-free object types.
export function Intersection(props: Styled & { title?: string }) {
  return <div {...props} />;
}

// Exact object literals keep their own resolution (ADR 0358).
export function Literal() {
  return <div {...{ class: "x" }} />;
}
