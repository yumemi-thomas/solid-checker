import { For, createSignal, onCleanup } from "solid-js";
import { h2 } from "./helpers";

// A project helper that calls its callback in its own body on every
// completion: the callback runs on the stack of the `h1` call.
function h1(cb: () => unknown) {
  cb();
  return 1;
}

// Violations (ADR 0522): a read written in a layer that runs in place inside a
// named function belongs to that function's summary, at the layer's call in
// its body; the function's call in the component body (as a `ref` factory
// the JSX evaluates eagerly, or as a statement) reads untracked there. rc.17
// warns STRICT_READ_UNTRACKED while the element is created.

// A helper's callback in a two-phase directive factory's body.
export function RefFactoryBodyHelper() {
  const [r1] = createSignal(3);
  const dir1 = () => {
    h1(() => {
      r1();
    });
    return () => {};
  };
  return <div><p ref={dir1()} data-view="v1">x</p></div>;
}

// An IIFE in the factory's body.
export function RefFactoryBodyIife() {
  const [r1] = createSignal(3);
  const dir1 = () => {
    (() => {
      r1();
    })();
    return () => {};
  };
  return <div><p ref={dir1()} data-view="v1">x</p></div>;
}

// A wrapper declared in the component and called once, inside a module
// helper's callback in the factory's body: the climb passes through the
// wrapper, the callback and the factory.
export function RefFactoryBodyWrapperModuleHelper() {
  const [r1] = createSignal(3);
  const wrapUse = () => { r1(); };
  const dir1 = () => {
    h2(() => {
      wrapUse();
    });
    return () => {};
  };
  return <div><p ref={dir1()} data-view="v1">x</p></div>;
}

// A closure called once in the factory, with a helper's callback inside it.
export function RefFactoryBodyWrapperClosureHelper() {
  const [r1] = createSignal(3);
  const wrapUse = () => { r1(); };
  const dir1 = () => {
    const f1 = () => {
      h1(() => {
        wrapUse();
      });
    };
    f1();
    return () => {};
  };
  return <div><p ref={dir1()} data-view="v1">x</p></div>;
}

// Not ref-specific: the same factory called as a statement in the body.
export function StatementCallHelper() {
  const [r1] = createSignal(3);
  const dir1 = () => {
    h1(() => {
      r1();
    });
  };
  dir1();
  return <div><p data-view="v1">x</p></div>;
}

// A hook whose body runs the read in a helper's callback, called in the body.
export function HookHelperLayer() {
  const [r1] = createSignal(3);
  const useThing = () => {
    h1(() => {
      r1();
    });
    return 1;
  };
  useThing();
  return <div><p data-view="v1">x</p></div>;
}

// Legal (rc.17 silent): the compiler runs an attribute expression in a
// tracked render effect; a timer's callback runs on a fresh stack; a stored
// callback runs where nothing here follows; an owner operation in the
// factory's body has the component's owner.

export function AttributeFactoryHelper() {
  const [r1] = createSignal(3);
  const dir1 = () => {
    h1(() => {
      r1();
    });
    return "t";
  };
  return <div><p title={dir1()} data-view="v1">x</p></div>;
}

export function TimerLayerInFactory() {
  const [r1] = createSignal(3);
  const dir1 = () => {
    setTimeout(() => {
      r1();
    }, 0);
    return () => {};
  };
  return <div><p ref={dir1()} data-view="v1">x</p></div>;
}

const box: { cb?: () => void } = {};

export function StoredCallbackInFactory() {
  const [r1] = createSignal(3);
  const dir1 = () => {
    box.cb = () => {
      r1();
    };
    return () => {};
  };
  return <div><p ref={dir1()} data-view="v1">x</p></div>;
}

export function CleanupInFactoryBodyHelper() {
  const dir1 = () => {
    h1(() => {
      onCleanup(() => {});
    });
    return () => {};
  };
  return <div><p ref={dir1()} data-view="v1">x</p></div>;
}

// Uncertifiable (ADR 0516's answer, never silence): a class member's body is
// run by a construction or a member call nothing follows, so a helper or a
// hook's accessor called there is an obligation, as a read written there is.

export function ClassCtorWrapper() {
  const [r1] = createSignal(3);
  const wrapUse = () => { r1(); };
  class K9 {
    constructor() {
      wrapUse();
    }
  }
  new K9();
  return <div><p data-view="v1">x</p></div>;
}

export function ClassMethodHookResult() {
  const [r1] = createSignal(3);
  const useThing = () => ({ useIt: r1 });
  const { useIt } = useThing();
  class K9 {
    m() {
      useIt();
    }
  }
  new K9().m();
  return <div><p data-view="v1">x</p></div>;
}

// Uncertifiable (ADR 0516's stepped generator, through `ref_applied_read_role`):
// a generator stepped at its own call in a function an intrinsic `ref` applies
// runs some of its body, or none, while the element is created; rc.17 warns
// (the step reaches the read), the checker places it nowhere, never deferred.

export function RefNamedGenerator() {
  const [r1] = createSignal(3);
  const rf1 = () => {
    (function* () {
      r1();
      yield 1;
    })().next();
  };
  return <div><p ref={rf1} data-view="v1">x</p></div>;
}

export function RefFactoryGenerator() {
  const [r1] = createSignal(3);
  const dir1 = () => {
    return () => {
      (function* () {
        r1();
        yield 1;
      })().next();
    };
  };
  return <div><p ref={dir1()} data-view="v1">x</p></div>;
}

export function RefRowGenerator() {
  const [r1] = createSignal(3);
  return (
    <div>
      <For each={[1]}>
        {() => (
          <li>
            <p ref={() => {
              (function* () {
                r1();
                yield 1;
              })().next();
            }} data-view="v1">x</p>
          </li>
        )}
      </For>
    </div>
  );
}
