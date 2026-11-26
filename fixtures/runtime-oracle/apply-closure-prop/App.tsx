import { createEffect, createSignal } from "solid-js";
import type { Accessor, Setter } from "solid-js";

// ADR 1229: each export is the matching reactive-ir fixture `apply-closure-prop(-read, -write-legal, -read-unrun)` and
// `apply-closure-prop-carrier` (`CarrierEmpty`, `CarrierSeeded`).
// rc.21 warns STRICT_READ_UNTRACKED in `Read` and `CarrierSeeded`; `WriteLegal`, `ReadUnrun` and `CarrierEmpty` raise nothing.

function CRead(props: { r1: Accessor<number> }) {
  createEffect(
    () => 1,
    () => {
      const f1 = () => {
        props.r1();
      };
      f1();
    },
  );
  return <div />;
}

export function Read() {
  const [r1] = createSignal(3);
  return (
    <div>
      <CRead r1={r1} />
    </div>
  );
}

function CWriteLegal(props: { setR1: Setter<number> }) {
  createEffect(
    () => 1,
    () => {
      const f1 = () => {
        props.setR1(4);
      };
      f1();
    },
  );
  return <div />;
}

export function WriteLegal() {
  const [, setR1] = createSignal(3);
  return (
    <div>
      <CWriteLegal setR1={setR1} />
    </div>
  );
}

function CReadUnrun(props: { r1: Accessor<number> }) {
  createEffect(
    () => 1,
    () => {
      const f1 = () => {
        props.r1();
      };
      return void f1;
    },
  );
  return <div />;
}

export function ReadUnrun() {
  const [r1] = createSignal(3);
  return (
    <div>
      <CReadUnrun r1={r1} />
    </div>
  );
}

function CCarrierEmpty(props: { r1: Accessor<number> }) {
  createEffect(
    () => 1,
    () => {
      ([] as number[]).forEach(() => {
        props.r1();
      });
    },
  );
  return <div />;
}

function CCarrierSeeded(props: { r1: Accessor<number> }) {
  createEffect(
    () => 1,
    () => {
      [1].forEach(() => {
        props.r1();
      });
    },
  );
  return <div />;
}

export function CarrierEmpty() {
  const [r1] = createSignal(3);
  return (
    <div>
      <CCarrierEmpty r1={r1} />
    </div>
  );
}

export function CarrierSeeded() {
  const [r1] = createSignal(3);
  return (
    <div>
      <CCarrierSeeded r1={r1} />
    </div>
  );
}
