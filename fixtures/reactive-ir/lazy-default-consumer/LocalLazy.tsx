import { createLoop } from "dep-loop";

const frame = () => {};

// A project function spelled `lazy`: not the dialect's primitive.
function lazy<T>(fn: () => Promise<{ default: T }>): T {
  void fn;
  throw new Error("not a loader");
}

const LocalLoop = lazy(async () => {
  function LocalCube() {
    const [running, start] = createLoop(frame);
    start();
    return <div>{String(running())}</div>;
  }
  return { default: LocalCube };
});

export function LocalLazyPage() {
  return (
    <main>
      <LocalLoop />
    </main>
  );
}
