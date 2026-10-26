// app-game gpu-text-rendering GpuTextRendering.tsx:49 (`open`, via ViewerToolbar.tsx:180) and solid-svg-editor InspectorPanel.tsx:104: a child
// spreads its props onto a component that never names the callback (Button copies its props), so the callback is forwarded to a consumer
// that does not name it; `Bar` forwards once (`forwarded:prop-unused`), `Bars` twice.
import { createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";

type ButtonProps = JSX.ButtonHTMLAttributes<HTMLButtonElement>;

function Button(props: ButtonProps) {
  const domProps: ButtonProps = {};
  for (const key of Object.keys(props)) {
    Object.defineProperty(domProps, key, { enumerable: true, get: () => props[key as keyof ButtonProps] });
  }
  return <button {...domProps} />;
}

function Bar(props: ButtonProps) {
  return <Button {...props}>Go</Button>;
}

function Bars(props: ButtonProps) {
  return (
    <>
      <Button {...props}>Go</Button>
      <Button {...props}>Again</Button>
    </>
  );
}

export function Page() {
  const [n, setN] = createSignal(0);
  const [m, setM] = createSignal(0);
  return (
    <>
      <Bar onClick={() => setN(n() + 1)} />
      <Bars onClick={() => setM(m() + 1)} />
    </>
  );
}
