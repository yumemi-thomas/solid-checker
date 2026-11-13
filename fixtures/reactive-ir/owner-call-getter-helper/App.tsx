import { createEffect, createMemo, Show } from "solid-js";
import { render } from "@solidjs/web";
import { sharedMemo, sharedView } from "./helpers";

// Batch 11, B2 (the review of #225, F2, found while fixing it): a CALL written in a component element's getter runs the callee where the component reads the prop, so it
// places no callee. `renderView()` and `makeMemo()` are written outside the element and called inside its prop; `Outer` reads the prop on an effect's apply, where no owner is set: rc.21
// throws PRIMITIVE_IN_EFFECT_CALLBACK at the tag and at the memo. The analyses (the role engine, the owner graph's edge, the entry walk) took the call for a run in the render
// that wrote it, a component only a tag renders, and ended both sites silently on main, on #220's head and on the first build of B2. A call in a getter is now an entry no edge shows,
// so the callee's sites keep their rows (decline `entry-by-reference`) unless something else proves them.

function Inner() {
  return <p>inner</p>;
}

function renderView() {
  return <Inner />;
}

function makeMemo() {
  return createMemo(() => 1);
}

// A helper that calls the helper: the layer the getter's call enters is `renderTwice`, whose own entry is the open one.
function renderTwice() {
  return renderView();
}

const makeLocal = () => createMemo(() => 2);

function Outer(props: { slot?: unknown; children?: unknown }) {
  createEffect(
    () => 0,
    () => {
      void props.slot;
      void props.children;
    },
  );
  return <p>outer</p>;
}

function ByHelperSlot() {
  return <Outer slot={renderView()} />;
}
function ByHelperChild() {
  return <Outer>{renderView()}</Outer>;
}
function ByMakeMemo() {
  return <Outer slot={makeMemo()} />;
}
function ByNested() {
  return <Outer slot={renderTwice()} />;
}
function ByArrowConst() {
  return <Outer slot={makeLocal()} />;
}
function ByImported() {
  return (
    <p>
      <Outer slot={sharedView()} />
      <Outer slot={sharedMemo()} />
    </p>
  );
}

// Controls: helpers nothing but a render calls. The owner the component's render sets is current, and nothing else enters them: silent.
function controlView() {
  return <Inner />;
}
function controlMemo() {
  return createMemo(() => 3);
}
function Rendered() {
  const view = controlView();
  const memo = controlMemo();
  return (
    <p>
      {view}
      {memo()}
    </p>
  );
}

// A call in the getter of a component that renders it in place (the dialect's `Show`) is created under the owner that render sets: no getter, the callee keeps its silence.
function showMemo() {
  return createMemo(() => 4);
}
function InShow() {
  return <Show when={showMemo()}>x</Show>;
}

function App() {
  return (
    <p>
      <ByHelperSlot />
      <ByHelperChild />
      <ByMakeMemo />
      <ByNested />
      <ByArrowConst />
      <ByImported />
      <Rendered />
      <InShow />
    </p>
  );
}
render(() => <App />, document.body);
