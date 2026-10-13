import { createSignal } from "solid-js";
import {
  useEscapedInArrow,
  useEscapedInObject,
  useEscapedReturnedMember,
  useEscapedRoot,
  useEscapedSubProxy,
  useEscapedThis,
  useMemoRead,
  useNestedThrow,
  useThrowingEffect,
} from "./hook";

// Violations: the hook returns its store, read untracked in the body.
export function ThrowingEffect() {
  const thing = useThrowingEffect();
  const [config] = createSignal(thing.search);
  return <main>{config()}</main>;
}
export function MemoRead() {
  const thing = useMemoRead();
  const [config] = createSignal(thing.search);
  return <main>{config()}</main>;
}
export function NestedThrow() {
  const thing = useNestedThrow();
  const [config] = createSignal(thing.search);
  return <main>{config()}</main>;
}

// No proof: the store root or a sub-proxy escaped.
export function EscapedRoot() {
  const thing = useEscapedRoot();
  const [config] = createSignal(thing.search);
  return <main>{config()}</main>;
}
export function EscapedInArrow() {
  const thing = useEscapedInArrow();
  const [config] = createSignal(thing.search);
  return <main>{config()}</main>;
}
export function EscapedInObject() {
  const thing = useEscapedInObject();
  const [config] = createSignal(thing.search);
  return <main>{config()}</main>;
}
export function EscapedSubProxy() {
  const thing = useEscapedSubProxy();
  const [config] = createSignal(thing.search);
  return <main>{config()}</main>;
}
export function EscapedReturnedMember() {
  const thing = useEscapedReturnedMember();
  const [config] = createSignal(thing.search);
  return <main>{config()}</main>;
}
export function EscapedThis() {
  const thing = useEscapedThis();
  const [config] = createSignal(thing.search);
  return <main>{config()}</main>;
}
