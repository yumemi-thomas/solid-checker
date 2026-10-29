import { createContext, createMemo, createRenderEffect, createSignal, useContext } from "solid-js";
import type { Accessor } from "solid-js";
import { remote } from "./source";

// ADR 0510: the source an apply's first run meets pending may be a
// module-scope memo of a module that only creates nodes, or a memo the
// mounting parent creates and hands over by an exact prop or one context
// provider. The mounts are main.tsx's module-scope renders.

const DataCtx = createContext<{ shared: Accessor<number> }>();
const LateCtx = createContext<{ late: Accessor<number> }>();

// Violation (SC5001): the first render's render effect reads the module
// memo, pending since `source.ts` was evaluated.
export function ModuleRender() {
  createRenderEffect(
    () => 1,
    () => {
      remote();
    },
  );
  return <p>static</p>;
}

// Uncertifiable: a later render; the code before it (here an earlier mount)
// is not proven to leave the memo pending.
export function ModuleSecond() {
  createRenderEffect(
    () => 1,
    () => {
      remote();
    },
  );
  return <p>static</p>;
}

function PropChild(props: { data: Accessor<number> }) {
  createRenderEffect(
    () => 1,
    () => {
      props.data();
    },
  );
  return <p>static</p>;
}

// Violation (SC5001, in `PropChild`): the parent creates the memo and hands
// it over as the prop.
export function PropParent() {
  const data = createMemo(async () => 1);
  return (
    <div>
      <PropChild data={data} />
    </div>
  );
}

function OtherChild(props: { data: Accessor<number> }) {
  createRenderEffect(
    () => 1,
    () => {
      props.data();
    },
  );
  return <p>static</p>;
}

// Uncertifiable (in `OtherChild`): the parent runs code before its tree.
export function CallingParent() {
  const data = createMemo(async () => 1);
  JSON.stringify(1);
  return (
    <div>
      <OtherChild data={data} />
    </div>
  );
}

function ContextChild() {
  const ctx = useContext(DataCtx);
  createRenderEffect(
    () => 1,
    () => {
      ctx.shared();
    },
  );
  return <p>static</p>;
}

// Violation (SC5001, in `ContextChild`): the parent provides its memo.
export function ContextParent() {
  const data = createMemo(async () => 1);
  return (
    <div>
      <DataCtx value={{ shared: data }}>
        <ContextChild />
      </DataCtx>
    </div>
  );
}

function WrittenChild(props: { data: Accessor<number> }) {
  createRenderEffect(
    () => 1,
    () => {
      props.data = () => 2;
      props.data();
    },
  );
  return <p>static</p>;
}

// Uncertifiable (in `WrittenChild`): the child writes the prop before it
// calls it, so the call may not read the memo.
export function WrittenParent() {
  const data = createMemo(async () => 1);
  return (
    <div>
      <WrittenChild data={data} />
    </div>
  );
}

function LateChild() {
  createRenderEffect(
    () => 1,
    () => {
      ctx.late();
    },
  );
  const ctx = useContext(LateCtx);
  return <p>static</p>;
}

// Uncertifiable (in `LateChild`): the first run reads `ctx` on the call,
// before its declaration (a ReferenceError, not the pending read).
export function LateParent() {
  const data = createMemo(async () => 1);
  return (
    <div>
      <LateCtx value={{ late: data }}>
        <LateChild />
      </LateCtx>
    </div>
  );
}

// Uncertifiable, each: a call in a layer of the apply runs before the read
// and throws first (ADR 0510's correction of ADR 0504).
export function ThrowInIife() {
  const data = createMemo(async () => 1);
  createRenderEffect(
    () => 1,
    () => {
      (() => {
        JSON.parse("{");
        data();
      })();
    },
  );
  return <p>static</p>;
}

export function ThrowInWrapper() {
  const data = createMemo(async () => 1);
  const read = () => {
    JSON.parse("{");
    data();
  };
  createRenderEffect(
    () => 1,
    () => {
      read();
    },
  );
  return <p>static</p>;
}

function runFirst(callback: () => unknown) {
  JSON.parse("{");
  callback();
  return 1;
}

export function ThrowInHelper() {
  const data = createMemo(async () => 1);
  createRenderEffect(
    () => 1,
    () => {
      runFirst(() => {
        data();
      });
    },
  );
  return <p>static</p>;
}

// Uncertifiable (SC2001's write, ADR 0503): the IIFE throws before it.
export function ThrowBeforeWrite() {
  const [count, setCount] = createSignal(0);
  createRenderEffect(
    () => 1,
    () => {
      (() => {
        JSON.parse("{");
        setCount(1);
      })();
    },
  );
  return <p>{count()}</p>;
}
