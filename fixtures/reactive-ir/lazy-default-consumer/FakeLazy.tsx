import { lazy } from "fake-lazy";
import { createLoop } from "dep-loop";

const frame = () => {};

// Another package's `lazy`: same spelling, not the dialect's primitive.
const PackageLoop = lazy(async () => {
  function PackageCube() {
    const [running, start] = createLoop(frame);
    start();
    return <div>{String(running())}</div>;
  }
  return { default: PackageCube };
});

export function FakeLazyPage() {
  return (
    <main>
      <PackageLoop />
    </main>
  );
}
