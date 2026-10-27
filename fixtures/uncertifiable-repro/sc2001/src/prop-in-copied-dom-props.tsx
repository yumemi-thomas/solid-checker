// donegeon-client Button.tsx:60 (used at AppShell.tsx:60, HomeLabelsManager.tsx:61, Dialog.tsx:71): Button never names the callbacks it is
// given; it hands its props to a project function that copies every key into getters on a DOM props object, spread onto a `<button>`.
import { createSignal, onSettled } from "solid-js";
import type { JSX } from "@solidjs/web";
type ButtonProps = JSX.ButtonHTMLAttributes<HTMLButtonElement>;
function reactiveDomProps(props: ButtonProps): ButtonProps {
  const domProps: ButtonProps = {};
  for (const key of Object.keys(props)) {
    Object.defineProperty(domProps, key, { enumerable: true, get: () => props[key as keyof ButtonProps] });
  }
  return domProps;
}
function Button(props: ButtonProps) {
  return <button {...reactiveDomProps(props)} />;
}

function Dialog(props: { onClose: () => void }) {
  onSettled(() => {
    const onKeyDown = (event: KeyboardEvent) => event.key === "Escape" && props.onClose();
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  });
  return <div role="dialog" />;
}

export function Screen() {
  const [open, setOpen] = createSignal(false);
  const close = () => setOpen(false);
  const [step, setStep] = createSignal(0);
  const clear = () => setStep(0);
  const reset = () => clear();
  return (
    <>
      <Button onClick={() => setOpen(!open())}>Toggle</Button>
      <Dialog onClose={close} />
      <Button onClick={close}>Close</Button>
      <Button onClick={reset}>Reset {step()}</Button>
    </>
  );
}
