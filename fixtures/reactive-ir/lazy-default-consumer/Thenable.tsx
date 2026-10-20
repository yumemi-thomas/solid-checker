import { createSignal, lazy, type Component } from "solid-js";
import { createLoop } from "dep-loop";

const frame = () => {};

// Review round 1 of ADR 0292: a module object with an own `then` is a
// thenable. Resolving the async loader's return assimilates it, so the
// promise fulfills with whatever `then` delivers and `default` may never be
// the component `lazy` renders. Each keeps the object-retention refusal.

// The reviewer's minimal shape: `then` replaces the module.
const [count] = createSignal(0);
function readCount() {
  return count();
}
const ReplacedLoop = lazy(async () => {
  function Loaded() {
    readCount();
    return <div />;
  }
  return {
    default: Loaded,
    then: (resolve: (module: { default: Component }) => void) =>
      resolve({ default: () => <div>OK</div> }),
  };
});

// A quoted `then` key: the same own property.
const QuotedLoop = lazy(async () => {
  function QuotedCube() {
    const [running, start] = createLoop(frame);
    start();
    return <div>{String(running())}</div>;
  }
  return {
    default: QuotedCube,
    "then": (resolve: (module: { default: Component }) => void) =>
      resolve({ default: () => <div /> }),
  };
});

// A `then` method that never settles: `QuietCube` never renders.
const MethodLoop = lazy(async () => {
  function QuietCube() {
    const [running, start] = createLoop(frame);
    start();
    return <div>{String(running())}</div>;
  }
  return {
    default: QuietCube,
    then(_resolve: (module: { default: Component }) => void) {},
  };
});

export function ThenablePage() {
  return (
    <main>
      <ReplacedLoop />
      <QuotedLoop />
      <MethodLoop />
    </main>
  );
}
