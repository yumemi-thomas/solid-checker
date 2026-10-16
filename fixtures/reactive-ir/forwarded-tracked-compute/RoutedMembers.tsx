import { createSignal } from "solid-js";
import { escapedRoutedMember, freshRoutedMember, mixedRoutedMember, receiverRoutedMember, routedMember, sampledRoutedMember, siblingRoutedMember, subjectReceiverRoutedMember } from "./member-helpers";

export function RoutedTracked() {
  const [count] = createSignal(0);
  const value = routedMember({ read: () => count() });
  return <div>{value()}</div>;
}
export function RoutedMixed() {
  const [count] = createSignal(0);
  const value = mixedRoutedMember({ read: () => count() });
  return <div>{value()}</div>;
}
export function RoutedFresh() {
  const [count] = createSignal(0);
  freshRoutedMember({ read: () => count() });
  return <div />;
}
export function RoutedSampled() {
  const [count] = createSignal(0);
  sampledRoutedMember({ read: () => count() });
  return <div />;
}
export function RoutedEscaped() {
  const [count] = createSignal(0);
  escapedRoutedMember({ read: () => count() });
  return <div />;
}

export function RoutedSiblingArrow() {
  const [count] = createSignal(0);
  const value = siblingRoutedMember({ read: () => count() });
  return <div>{value()}</div>;
}
export function RoutedSiblingReceiver() {
  const [count] = createSignal(0);
  const value = receiverRoutedMember({ read: () => count() });
  return <div>{value()}</div>;
}

export function RoutedSubjectReceiver() {
  const [count] = createSignal(0);
  const value = subjectReceiverRoutedMember({ read: () => count() });
  return <div>{value()}</div>;
}
