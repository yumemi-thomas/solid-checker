// ADR 0914: a static `dynamic` source that returns a Promise throws a plain
// error on every run (rc.21), with no diagnostic at the call. Here the throw is
// reached through a function a class holds: an arrow a constructor stores on
// `this`, or a method. The walk finds no entry of the constructor or the method
// (`new` is no plain call, and no reference names them), so it stops there.
// Where that function is written inside a memo nothing reads, the memo keeps
// the error and nothing reports it: uncertifiable, as for a site written in
// the memo itself. Written in a component body, or in a memo an effect reads,
// the throw is reported: the violation stays.
import { createEffect, createMemo, flush } from "solid-js";
import { dynamic } from "@solidjs/web";

function wrapUse() { dynamic(() => Promise.resolve("p" as const), { static: true }); }

function wrapMethod() { dynamic(() => Promise.resolve("q" as const), { static: true }); }

function wrapRead() { dynamic(() => Promise.resolve("r" as const), { static: true }); }

function wrapBody() { dynamic(() => Promise.resolve("s" as const), { static: true }); }

// Uncertifiable: the fuzzer's program (lg-recall flush-read-stored, wrapper,
// handoff-assign-this). The first `g()` throws inside the memo, which keeps
// the error; `push` never runs, so the body's call runs nothing.
export function FuzzShape() {
  const hkSaved = [] as (() => void)[];
  const hkTwice = (g: () => void) => { g(); hkSaved.push(g); };
  const m1 = createMemo(() => {
    flush(() => {
      hkTwice(() => {
        class HK {
          m?: () => void;
          constructor() { this.m = () => { wrapUse(); }; }
        }
        new HK().m?.();
      });
    });
    return 1;
  });
  hkSaved[0]?.();
  return <div></div>;
}

// Uncertifiable: the same throw through a method, in a memo nothing reads.
export function MethodInMemo() {
  const m2 = createMemo(() => {
    class K {
      run() { wrapMethod(); }
    }
    new K().run();
    return 1;
  });
  return <div></div>;
}

// Uncertifiable: the site written in the stored arrow itself (the lexical
// fallback ADR 0893 already had).
export function DirectInMemo() {
  const m3 = createMemo(() => {
    class HD {
      m?: () => void;
      constructor() { this.m = () => { dynamic(() => Promise.resolve("t" as const), { static: true }); }; }
    }
    new HD().m?.();
    return 1;
  });
  return <div></div>;
}

// Violation: an effect reads the memo, so its stored error is reported.
export function ReadMemo() {
  const m4 = createMemo(() => {
    class HR {
      m?: () => void;
      constructor() { this.m = () => { wrapRead(); }; }
    }
    new HR().m?.();
    return 1;
  });
  createEffect(() => m4(), () => {});
  return <div></div>;
}

// Violation: the class is written in the component body, whose throw fails
// the render.
export function InBody() {
  class HB {
    m?: () => void;
    constructor() { this.m = () => { wrapBody(); }; }
  }
  new HB().m?.();
  return <div></div>;
}
