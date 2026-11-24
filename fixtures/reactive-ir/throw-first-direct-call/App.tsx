import { createReaction, createSignal, onSettled } from "solid-js";

const flag = Math.random() > 2;

// ADR 1204: an evaluation before the read visibly may throw, so the read is not proven to run: uncertifiable
// (`earlier-evaluation-may-throw`). Chrome is silent at each read (runtime-oracle/throw-first-direct-call).
function HandedThrows() {
  const [s] = createSignal(1);
  const f = () => {
    s();
  };
  const box = {
    get cb(): () => void {
      throw new Error("no callback");
    },
  };
  onSettled(box.cb);
  f();
  return <div />;
}

function DirectThrows() {
  const [s] = createSignal(1);
  const box = {
    get cb(): () => void {
      throw new Error("no callback");
    },
  };
  void box.cb;
  s();
  return <div />;
}

function LocalThrows() {
  const [s] = createSignal(1);
  const boom = () => {
    throw new Error("boom");
  };
  boom();
  s();
  return <div />;
}

// W9's shape (reaction-settled-run `SettledMember` with a direct call): the reaction's invalidation runs an IIFE whose
// closure call follows the throwing getter's Get.
function ReactionLayer() {
  const [r1] = createSignal(3);
  const [t1, setT1] = createSignal(1);
  const box: { readonly cb: () => void } = {
    get cb(): () => void {
      throw new Error("no");
    },
  };
  const track1 = createReaction(() => {
    (() => {
      const f = () => {
        r1();
      };
      onSettled(box.cb);
      f();
    })();
  });
  track1(() => {
    t1();
  });
  return <button onClick={() => setT1(2)}>go</button>;
}

// Twins that stay violations: the throw is caught before the read, the member is a data property, and a `throw` on a branch,
// which reads as reached as any branch does (ADR 0458): the checker does not decide `flag`.
function CaughtGetter() {
  const [s] = createSignal(1);
  const box = {
    get cb(): () => void {
      throw new Error("no callback");
    },
  };
  try {
    void box.cb;
  } catch {
    // handled
  }
  s();
  return <div />;
}

function ConditionalThrow() {
  const [s] = createSignal(1);
  if (!flag) throw new Error("not yet");
  s();
  return <div />;
}

function DataMember() {
  const [s] = createSignal(1);
  const f = () => {
    s();
  };
  const box = { cb: () => {} };
  onSettled(box.cb);
  f();
  return <div />;
}

export function App() {
  return (
    <div>
      <HandedThrows />
      <DirectThrows />
      <ConditionalThrow />
      <LocalThrows />
      <ReactionLayer />
      <CaughtGetter />
      <DataMember />
    </div>
  );
}
