// app-game gpu-text-rendering GpuCanvasProvider.tsx:35 (createGpuCanvas): a component hands a context value to
// a helper that calls a DOM method on a member path of it (`gpu.signal.addEventListener`).
import { createContext, useContext } from "solid-js";

interface GpuRoot {
  readonly signal: AbortSignal;
}

const GpuScope = createContext<GpuRoot>({ signal: new AbortController().signal });

function createGpuCanvas(root: GpuRoot, label: string): string {
  const onAbort = () => console.log(label);
  root.signal.addEventListener("abort", onAbort, { once: true });
  return label;
}

export function GpuCanvas() {
  const root = useContext(GpuScope);
  const label = createGpuCanvas(root, "canvas");
  return <canvas aria-label={label} />;
}
