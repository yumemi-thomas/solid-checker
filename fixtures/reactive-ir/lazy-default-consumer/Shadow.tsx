import { lazy } from "solid-js";
import { createLoop } from "dep-loop";

const frame = () => {};

// The module-level `ShadowCube` is held by an exported registry. The loader's
// own `ShadowCube` shadows it: the witness is for the inner declaration, so
// the outer one keeps its refusal.
function ShadowCube() {
  const [running, start] = createLoop(frame);
  start();
  return <div>{String(running())}</div>;
}
export const shadowRegistry = { ShadowCube };

const ShadowLoop = lazy(async () => {
  const ShadowCube = () => <div />;
  return { default: ShadowCube };
});

export function ShadowPage() {
  return (
    <main>
      <ShadowLoop />
    </main>
  );
}
