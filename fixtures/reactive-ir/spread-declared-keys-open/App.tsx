import { omit } from "solid-js";

type WithHandler = { class?: string; onClick?: () => void };
type Indexed = { class?: string; [attribute: string]: unknown };

// Each spread below keeps its obligation (SC4001, uncertifiable): the declared
// type does not prove the object holds no listener key.

// The declared type holds an `onClick` key.
export function Handler(props: WithHandler) {
  return <div {...props} />;
}

// `omit` hides `class` only; `onClick` stays in the declared type.
export function OmitKeepsHandler(props: WithHandler) {
  return <div {...omit(props, "class")} />;
}

// An index signature may hold any key.
export function IndexSignature(props: Indexed) {
  return <div {...props} />;
}

// `any` declares nothing.
export function Anything(props: any) {
  return <div {...props} />;
}

// A union with an open member is open.
export function OpenUnion(props: { class?: string } | Indexed) {
  return <div {...props} />;
}

// A type parameter's keys are the instantiation's.
export function Generic<T extends { class?: string }>(props: T) {
  return <div {...props} />;
}

// `omit` of a generic value names its keys through a type variable.
export function GenericOmit<T extends { class?: string }>(props: T) {
  return <div {...omit(props, "class")} />;
}

// `{}` admits any non-nullish value: with no property it declares nothing.
export function EmptyObject(props: {}) {
  return <div {...props} />;
}

// An empty interface is the same type.
interface Empty {}
export function EmptyInterface(props: Empty) {
  return <div {...props} />;
}

// `object` admits any non-primitive.
export function NonPrimitive(props: object) {
  return <div {...props} />;
}
