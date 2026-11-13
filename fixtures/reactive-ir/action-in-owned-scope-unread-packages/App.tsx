import { action, createMemo, createRoot } from "solid-js";
import { Stepper, StepView, reset, runStep, step } from "reachpkg";
import * as Reach from "reachpkg";
import { measure } from "purepkg";
import { load } from "dynpkg";
import { mode } from "envpkg";
import { use } from "lostpkg";
import { wrapped } from "wrappkg";

// Uncertifiable: a package whose closure loads solid-js, called where an owner is active.
export function ReachBody() {
  runStep(() => {});
  return null;
}
export function ReachMemo() {
  const value = createMemo(() => {
    runStep(() => {});
    return 1;
  });
  return <p>{value()}</p>;
}
export function ReachRoot() {
  setTimeout(() => {
    createRoot(() => {
      runStep(() => {});
    });
  });
  return null;
}
export function ReachNamespace() {
  Reach.runStep(() => {});
  return null;
}
export function ReachNew() {
  void new Stepper();
  return null;
}
export function ReachTag() {
  return <p>{step`x`}</p>;
}
export function ReachRender() {
  return <StepView />;
}
export function ReachValue() {
  const run = runStep;
  run(() => {});
  return null;
}
export function ReachRenderMember() {
  return <Reach.StepView />;
}
export function ReachRenderAlias() {
  const View = Reach.StepView;
  return <View />;
}
export function ReachNamespaceValue() {
  [1].map(Reach.reset);
  return null;
}
export function ReachShorthand() {
  const table = { runStep };
  void table;
  return null;
}
export function ReachDestructured() {
  const { runStep: go } = Reach;
  go(() => {});
  return null;
}
export function ReachThroughPackage() {
  wrapped(() => {});
  return null;
}

// Uncertifiable: closures the source does not decide.
export function DynamicLoad() {
  load(() => {});
  return null;
}
export function UnknownGlobal() {
  return <p>{mode(() => {})}</p>;
}
export function MissingSource() {
  use(() => {});
  return null;
}

// Silent: the same package where its role allows an action, or a package whose closure is clear.
export function ReachTimer() {
  setTimeout(() => {
    runStep(() => {});
  });
  return null;
}
export function ReachHandler() {
  return <button onClick={() => runStep(() => {})}>go</button>;
}
export function ReachValueTimer() {
  setTimeout(runStep, 0, () => {});
  return null;
}
export function ReachValueHandler() {
  return <button onClick={reset}>reset</button>;
}
export function PureBody() {
  return <p>{measure(() => {})}</p>;
}
export function Ambient() {
  legacyGlobal(() => {});
  return null;
}
export function Shadowed() {
  const runStep = (cb: () => void) => cb();
  runStep(() => {});
  return null;
}

// An action the program creates and calls in place: a violation in a body, silent in a handler.
export function InlineBody() {
  void action(function* () {})();
  return null;
}
export function InlineHandler() {
  return <button onClick={() => void action(function* () {})()}>go</button>;
}

export function App() {
  return (
    <>
      <ReachBody />
      <ReachMemo />
      <ReachRoot />
      <ReachNamespace />
      <ReachNew />
      <ReachTag />
      <ReachRender />
      <ReachRenderMember />
      <ReachRenderAlias />
      <ReachNamespaceValue />
      <ReachShorthand />
      <ReachDestructured />
      <ReachValue />
      <ReachThroughPackage />
      <DynamicLoad />
      <UnknownGlobal />
      <MissingSource />
      <ReachTimer />
      <ReachHandler />
      <ReachValueTimer />
      <ReachValueHandler />
      <PureBody />
      <Ambient />
      <Shadowed />
      <InlineBody />
      <InlineHandler />
    </>
  );
}
