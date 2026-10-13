import { lazy, type Component } from "solid-js";
import * as Solid from "solid-js";
import { createLoop } from "dep-loop";

const frame = () => {};

// The loader awaits before it builds the module: the await delays producing
// the component, not its later synchronous body. A `const` arrow, a
// transparent `as` wrapper on the default, and a closing tag.
const AwaitedLoop = lazy(async () => {
  await Promise.resolve();
  const AwaitedCube = () => {
    const [running, start] = createLoop(frame);
    start();
    return <div>{String(running())}</div>;
  };
  return { default: AwaitedCube as Component };
});

// The same primitive through a namespace import.
const NamespacedLoop = Solid.lazy(async () => {
  function NamespacedCube() {
    const [running, start] = createLoop(frame);
    start();
    return <div>{String(running())}</div>;
  }
  return { default: NamespacedCube };
});

export function AwaitedPage() {
  return (
    <main>
      <AwaitedLoop></AwaitedLoop>
      <NamespacedLoop />
    </main>
  );
}
