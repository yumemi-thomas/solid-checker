// app-game gpu-text-rendering GpuTextRendering.tsx:49 (`open`, via ViewerToolbar.tsx:180) and solid-svg-editor InspectorPanel.tsx:104: a child hands
// a callback prop on to a component that never names it (Button copies its props); `Bar` forwards once (`forwarded:prop-unused`), `Bars` twice.
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

function Bar(props: { onPress: () => void }) {
  return <Button onClick={props.onPress}>Go</Button>;
}

function Bars(props: { onPress: () => void }) {
  return (
    <>
      <Button onClick={props.onPress}>Go</Button>
      <Button onClick={props.onPress}>Again</Button>
    </>
  );
}

export function Page() {
  const [n, setN] = createSignal(0);
  const [m, setM] = createSignal(0);
  return (
    <>
      <Bar onPress={() => setN(n() + 1)} />
      <Bars onPress={() => setM(m() + 1)} />
    </>
  );
}
