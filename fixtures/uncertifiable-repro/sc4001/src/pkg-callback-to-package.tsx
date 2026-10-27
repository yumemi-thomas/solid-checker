// app-game paint ui-example navigation-popup.tsx:38: a prop is read in a callback a package keeps (here: a callback stack's push).
import { createCallbackStack } from "@solid-primitives/utils";

export function Popup(props: { onActive: (value: boolean) => void }) {
  const stack = createCallbackStack<boolean>();
  stack.push((value) => props.onActive(value));
  return <svg />;
}
