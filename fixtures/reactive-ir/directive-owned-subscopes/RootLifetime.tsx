import { createMemo, createRoot } from "solid-js";
import * as solid from "solid-js";

export function RootLifetime() {
  return <>
    {/* Neither allocation alone proves a leak. */}
    <div ref={() => { createRoot(dispose => { dispose(); }); }} />
    <div ref={() => { solid.createRoot(() => {}); }} />
    {/* A callback may retain work, but proving its lifetime is a separate obligation. */}
    <div ref={() => { createRoot(() => { createMemo(() => 1); }); }} />
    {/* A factory-returned ref must make the same lifetime distinction. */}
    <div ref={factory()} />
    {/* Control: a directly unowned computation is still a violation. */}
    <div ref={() => { createMemo(() => 1); }} />
  </>;
}

function factory() {
  return () => { createRoot(dispose => { dispose(); }); };
}

// A local homonym creates no Solid root.
export function ShadowedRoot() {
  const createRoot = (callback: () => void) => callback();
  return <div ref={() => { createRoot(() => {}); }} />;
}
