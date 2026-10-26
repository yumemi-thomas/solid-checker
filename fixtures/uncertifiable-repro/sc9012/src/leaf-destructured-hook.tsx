// app-game three-examples Sprites.tsx:25 (`const { camera, resize } = useCamera()`, Camera.provider.ts:75):
// a leaf callback calls accessors destructured from the result of a project hook that is itself a binding
// (`const [CameraProvider, useCamera] = createContextProvider(..)`), whose value reaches it through a context.
import { createContext, createSignal, createTrackedEffect, useContext, type Accessor } from "solid-js";

type Camera = { size: Accessor<number>; resize: () => void };

function createContextHook<T>(fallback: T) {
  const Context = createContext<T>(fallback);
  return [Context, () => useContext(Context)] as const;
}

export const [CameraContext, useCamera] = createContextHook<Camera>({ size: () => 0, resize: () => {} });

export function Sprites() {
  const { size, resize } = useCamera();
  const [tick, setTick] = createSignal(0);
  createTrackedEffect(() => {
    tick();
    resize();
    size();
  });
  return <button onClick={() => setTick(1)} />;
}
