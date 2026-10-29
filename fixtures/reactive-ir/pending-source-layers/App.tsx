import { createMemo, createRenderEffect } from "solid-js";
import type { Accessor } from "solid-js";

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
