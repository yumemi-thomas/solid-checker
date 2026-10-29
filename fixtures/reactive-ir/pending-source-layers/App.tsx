import { Loading, createContext, createMemo, createRenderEffect, useContext } from "solid-js";
import type { Accessor } from "solid-js";
import { useData, useDataThenParse } from "./hooks";

// ADR 0511: a read of a memo the mounting parent hands over as an exact prop
// is placed through the in-place layers of the apply (an IIFE, a named
// closure called once, a helper that calls its callback first) that the
// first-run proof walks. The mounts are main.tsx's module-scope renders.

function IifeChild(props: { data: Accessor<number> }) {
  createRenderEffect(
    () => 1,
    () => {
      (() => {
        props.data();
      })();
    },
  );
  return <p>static</p>;
}

// Violation (SC5001, in `IifeChild`): the IIFE runs in the first run.
export function PropIife() {
  const data = createMemo(async () => 1);
  return (
    <div>
      <IifeChild data={data} />
    </div>
  );
}

function ClosureChild(props: { data: Accessor<number> }) {
  createRenderEffect(
    () => 1,
    () => {
      const read = () => {
        props.data();
      };
      read();
    },
  );
  return <p>static</p>;
}

// Violation (SC5001, in `ClosureChild`): the closure's one call runs it.
export function PropClosure() {
  const data = createMemo(async () => 1);
  return (
    <div>
      <ClosureChild data={data} />
    </div>
  );
}

function runNow(callback: () => unknown) {
  callback();
  return 1;
}

function HelperChild(props: { data: Accessor<number> }) {
  createRenderEffect(
    () => 1,
    () => {
      runNow(() => {
        props.data();
      });
    },
  );
  return <p>static</p>;
}

// Violation (SC5001, in `HelperChild`): the helper calls the callback first.
export function PropHelper() {
  const data = createMemo(async () => 1);
  return (
    <div>
      <HelperChild data={data} />
    </div>
  );
}

const kept: Array<() => unknown> = [];
function keep(callback: () => unknown) {
  kept.push(callback);
  return 1;
}

function KeptChild(props: { data: Accessor<number> }) {
  createRenderEffect(
    () => 1,
    () => {
      keep(() => {
        props.data();
      });
    },
  );
  return <p>static</p>;
}

// Uncertifiable (in `KeptChild`): the helper keeps the callback and never
// calls it; nothing reads the prop.
export function PropKept() {
  const data = createMemo(async () => 1);
  return (
    <div>
      <KeptChild data={data} />
    </div>
  );
}

function ThrowChild(props: { data: Accessor<number> }) {
  createRenderEffect(
    () => 1,
    () => {
      (() => {
        JSON.parse("{");
        props.data();
      })();
    },
  );
  return <p>static</p>;
}

// Uncertifiable (in `ThrowChild`): the IIFE throws before the read.
export function PropThrowFirst() {
  const data = createMemo(async () => 1);
  return (
    <div>
      <ThrowChild data={data} />
    </div>
  );
}

// Violation (SC5001): the memo an inert hook of another module creates in
// this body is pending at the render effect's first run.
export function HookDestructured() {
  const { data } = useData();
  createRenderEffect(
    () => 1,
    () => {
      data();
    },
  );
  return <p>static</p>;
}

// Uncertifiable: the hook is called after the effect, whose first run meets
// `data` in its temporal dead zone.
export function HookAfterEffect() {
  createRenderEffect(
    () => 1,
    () => {
      data();
    },
  );
  const { data } = useData();
  return <p>static</p>;
}

// Uncertifiable: the hook runs code after the creation (it throws).
export function HookRunsCode() {
  const { data } = useDataThenParse();
  createRenderEffect(
    () => 1,
    () => {
      data();
    },
  );
  return <p>static</p>;
}

function AliasChild(props: { data: Accessor<number> }) {
  const read = props.data;
  createRenderEffect(
    () => 1,
    () => {
      read();
    },
  );
  return <p>static</p>;
}

// Violation (SC5001, in `AliasChild`): a `const` alias of the prop, declared
// before the effect, is the handed memo.
export function PropAlias() {
  const data = createMemo(async () => 1);
  return (
    <div>
      <AliasChild data={data} />
    </div>
  );
}

function LateAliasChild(props: { data: Accessor<number> }) {
  createRenderEffect(
    () => 1,
    () => {
      read();
    },
  );
  const read = props.data;
  return <p>static</p>;
}

// Uncertifiable (in `LateAliasChild`): the alias is declared after the
// effect; the first run meets it in its temporal dead zone.
export function PropAliasLate() {
  const data = createMemo(async () => 1);
  return (
    <div>
      <LateAliasChild data={data} />
    </div>
  );
}

const DataCtx = createContext<{ shared: Accessor<number> }>();

function DestructuredChild() {
  const ctx = useContext(DataCtx);
  const { read } = { read: ctx.shared };
  createRenderEffect(
    () => 1,
    () => {
      read();
    },
  );
  return <p>static</p>;
}

// Violation (SC5001, in `DestructuredChild`): the provided memo taken out of
// a local object literal of data properties.
export function ContextDestructured() {
  const data = createMemo(async () => 1);
  return (
    <div>
      <DataCtx value={{ shared: data }}>
        <DestructuredChild />
      </DataCtx>
    </div>
  );
}

function LoadingChild(props: { data: Accessor<number> }) {
  createRenderEffect(
    () => 1,
    () => {
      props.data();
    },
  );
  return <p>static</p>;
}

// Violation (SC5001, in `LoadingChild`): `<Loading>` builds its children in a
// computed when it is created, so the child renders during this tree and its
// render effect's first run, on the call, meets the memo pending.
export function LoadingHolder() {
  const data = createMemo(async () => 1);
  return (
    <div>
      <Loading fallback={0}>
        <LoadingChild data={data} />
      </Loading>
    </div>
  );
}

function OnChild(props: { data: Accessor<number> }) {
  createRenderEffect(
    () => 1,
    () => {
      props.data();
    },
  );
  return <p>static</p>;
}

// Uncertifiable (in `OnChild`): an `on` dependency list, which runs code and
// may re-arm the boundary, is not admitted.
export function LoadingOn() {
  const data = createMemo(async () => 1);
  return (
    <div>
      <Loading fallback={0} on={1}>
        <OnChild data={data} />
      </Loading>
    </div>
  );
}
