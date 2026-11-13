import { createContext, createEffect, createMemo, createTrackedEffect, Show, untrack } from "solid-js";
import { render } from "@solidjs/web";

// Batch 11, B2 (the review of #225, F2): a component element's children, and the value of its props, are a getter that runs where the component reads it. Read on an effect's
// apply the getter's tag is created on a stack with no owner (rc.21 PRIMITIVE_IN_EFFECT_CALLBACK, SC6002), and where a leaf owner's callback reads it the tag is a forbidden
// creation (PRIMITIVE_IN_FORBIDDEN_SCOPE, SC3001). The owner-call universe skipped the tag as a getter's, "read where the component reads it", and no row stood at the read for these
// codes, so the program certified. A getter's tag is a site now, ended only when the component never reads the prop; the checker cannot prove where a read is, so it files the
// code's uncertifiable row (an in-place renderer, which reads its children under its own owner, is silent).

function Inner() {
  return <p>inner</p>;
}

// The apply reads the children getter.
function Outer(props: { children?: unknown }) {
  createEffect(
    () => 0,
    () => {
      void props.children;
    },
  );
  return <p>outer</p>;
}

// A leaf owner's callback reads it.
function Leaf(props: { children?: unknown }) {
  createTrackedEffect(() => {
    void props.children;
  });
  return <p>leaf</p>;
}

// The apply reads a prop's value, a getter written the same way.
function Slot(props: { slot?: unknown }) {
  createEffect(
    () => 0,
    () => {
      void props.slot;
    },
  );
  return <p>slot</p>;
}

function ByChildren() {
  return (
    <p>
      <Outer>
        <Inner />
      </Outer>
      <Leaf>
        <Inner />
      </Leaf>
    </p>
  );
}

function BySlot() {
  return <Slot slot={<Inner />} />;
}

// A function the getter's own evaluation calls (a `.map` callback, an immediately invoked arrow, an `untrack` callback) runs where the component reads the prop too: the tag
// written in it is the getter's. A function handed over whole (`{() => <Inner />}`) is called by whoever has it, a site like any other.
function ByMap() {
  return <Outer>{[1].map(() => <Inner />)}</Outer>;
}
function ByIife() {
  return <Slot slot={(() => <Inner />)()} />;
}
function ByUntrack() {
  return <Slot slot={untrack(() => <Inner />)} />;
}
function ByFunctionChild() {
  return <Outer>{() => <Inner />}</Outer>;
}

// A creating call written as a getter runs there the same way (Chrome: PRIMITIVE_IN_EFFECT_CALLBACK at the `createMemo`): it was a site, and ended silently, on main, on #220's
// head and on the first build of B2, because the function that holds it is a component only a tag renders, where the owner graph gives an owner on every entry.
function ByCall() {
  return <Slot slot={createMemo(() => 1)} />;
}

// A component that reads its children in its body: the getter runs under the owner its own render sets. Nothing the checker reads proves where the read is, so this is a row too.
function InBody(props: { children?: unknown }) {
  return <p>{props.children}</p>;
}
function Rendered() {
  return (
    <InBody>
      <Inner />
    </InBody>
  );
}

// Controls. A component that never reads its children: the getter never runs.
function Ignores(_props: { children?: unknown }) {
  return <p>ignores</p>;
}
function Unread() {
  return (
    <Ignores>
      <Inner />
    </Ignores>
  );
}

// A dialect in-place renderer (`Show`) and a context provider render their children under their own owner.
function InShow() {
  return (
    <Show when={1}>
      <Inner />
    </Show>
  );
}
const Ctx = createContext(1);
function InProvider() {
  return (
    <Ctx value={1}>
      <Inner />
    </Ctx>
  );
}

// An element's own children are rendered with the element.
function InIntrinsic() {
  return (
    <p>
      <Inner />
    </p>
  );
}

function App() {
  return (
    <p>
      <ByChildren />
      <BySlot />
      <ByMap />
      <ByIife />
      <ByUntrack />
      <ByFunctionChild />
      <ByCall />
      <Rendered />
      <Unread />
      <InShow />
      <InProvider />
      <InIntrinsic />
    </p>
  );
}
render(() => <App />, document.body);
