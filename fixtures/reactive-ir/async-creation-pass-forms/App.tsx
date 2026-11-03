import { Show, createContext, createMemo, createRoot, createSignal, onSettled, useContext } from "solid-js";
import type { Accessor } from "solid-js";
import { h1 } from "./helpers";

// ADR 0570: the fuzzer's lg-pairs later-mount shapes (a body, a root and
// onSettled; alias, destructured, hook-member and direct bindings; IIFE,
// module-helper and closure layers), each read of a context-handed memo,
// mounted once by its creator's first render and once from a button.

const FirstCtx = createContext<{ a1: Accessor<number> }>();
const LaterCtx = createContext<{ a1: Accessor<number> }>();

function FirstBodyAliasIife() {
  const ctx = useContext(FirstCtx);
  const useIt = ctx.a1;
  (() => {
    useIt();
  })();
  return <div />;
}
function FirstBodyDestructuredHelper() {
  const ctx = useContext(FirstCtx);
  const { useIt } = { useIt: ctx.a1 };
  h1(() => {
    useIt();
  });
  return <div />;
}
function FirstBodyHookMemberModuleHelper() {
  const ctx = useContext(FirstCtx);
  const useThing = () => ({ useIt: ctx.a1 });
  const thing = useThing();
  h1(() => {
    thing.useIt();
  });
  return <div />;
}
function FirstBodyDirectClosure() {
  const ctx = useContext(FirstCtx);
  const f1 = () => {
    ctx.a1();
  };
  f1();
  return <div />;
}
function FirstRootAliasIife() {
  const ctx = useContext(FirstCtx);
  const useIt = ctx.a1;
  createRoot(() => {
    (() => {
      useIt();
    })();
  });
  return <div />;
}
function FirstRootDestructuredHelper() {
  const ctx = useContext(FirstCtx);
  const { useIt } = { useIt: ctx.a1 };
  createRoot(() => {
    h1(() => {
      useIt();
    });
  });
  return <div />;
}
function FirstRootHookMemberModuleHelper() {
  const ctx = useContext(FirstCtx);
  const useThing = () => ({ useIt: ctx.a1 });
  const thing = useThing();
  createRoot(() => {
    h1(() => {
      thing.useIt();
    });
  });
  return <div />;
}
function FirstRootDirectClosure() {
  const ctx = useContext(FirstCtx);
  createRoot(() => {
    const f1 = () => {
      ctx.a1();
    };
    f1();
  });
  return <div />;
}
function FirstSettledAliasIife() {
  const ctx = useContext(FirstCtx);
  const useIt = ctx.a1;
  onSettled(() => {
    (() => {
      useIt();
    })();
  });
  return <div />;
}
function FirstSettledDestructuredHelper() {
  const ctx = useContext(FirstCtx);
  const { useIt } = { useIt: ctx.a1 };
  onSettled(() => {
    h1(() => {
      useIt();
    });
  });
  return <div />;
}
function FirstSettledHookMemberModuleHelper() {
  const ctx = useContext(FirstCtx);
  const useThing = () => ({ useIt: ctx.a1 });
  const thing = useThing();
  onSettled(() => {
    h1(() => {
      thing.useIt();
    });
  });
  return <div />;
}
function FirstSettledDirectClosure() {
  const ctx = useContext(FirstCtx);
  onSettled(() => {
    const f1 = () => {
      ctx.a1();
    };
    f1();
  });
  return <div />;
}
function LaterBodyAliasIife() {
  const ctx = useContext(LaterCtx);
  const useIt = ctx.a1;
  (() => {
    useIt();
  })();
  return <div />;
}
function LaterBodyDestructuredHelper() {
  const ctx = useContext(LaterCtx);
  const { useIt } = { useIt: ctx.a1 };
  h1(() => {
    useIt();
  });
  return <div />;
}
function LaterBodyHookMemberModuleHelper() {
  const ctx = useContext(LaterCtx);
  const useThing = () => ({ useIt: ctx.a1 });
  const thing = useThing();
  h1(() => {
    thing.useIt();
  });
  return <div />;
}
function LaterBodyDirectClosure() {
  const ctx = useContext(LaterCtx);
  const f1 = () => {
    ctx.a1();
  };
  f1();
  return <div />;
}
function LaterRootAliasIife() {
  const ctx = useContext(LaterCtx);
  const useIt = ctx.a1;
  createRoot(() => {
    (() => {
      useIt();
    })();
  });
  return <div />;
}
function LaterRootDestructuredHelper() {
  const ctx = useContext(LaterCtx);
  const { useIt } = { useIt: ctx.a1 };
  createRoot(() => {
    h1(() => {
      useIt();
    });
  });
  return <div />;
}
function LaterRootHookMemberModuleHelper() {
  const ctx = useContext(LaterCtx);
  const useThing = () => ({ useIt: ctx.a1 });
  const thing = useThing();
  createRoot(() => {
    h1(() => {
      thing.useIt();
    });
  });
  return <div />;
}
function LaterRootDirectClosure() {
  const ctx = useContext(LaterCtx);
  createRoot(() => {
    const f1 = () => {
      ctx.a1();
    };
    f1();
  });
  return <div />;
}
function LaterSettledAliasIife() {
  const ctx = useContext(LaterCtx);
  const useIt = ctx.a1;
  onSettled(() => {
    (() => {
      useIt();
    })();
  });
  return <div />;
}
function LaterSettledDestructuredHelper() {
  const ctx = useContext(LaterCtx);
  const { useIt } = { useIt: ctx.a1 };
  onSettled(() => {
    h1(() => {
      useIt();
    });
  });
  return <div />;
}
function LaterSettledHookMemberModuleHelper() {
  const ctx = useContext(LaterCtx);
  const useThing = () => ({ useIt: ctx.a1 });
  const thing = useThing();
  onSettled(() => {
    h1(() => {
      thing.useIt();
    });
  });
  return <div />;
}
function LaterSettledDirectClosure() {
  const ctx = useContext(LaterCtx);
  onSettled(() => {
    const f1 = () => {
      ctx.a1();
    };
    f1();
  });
  return <div />;
}

// Violation (SC5001) at each reader's read, except where the binding's
// own transport is an obligation (SC9012): the provider is the creator's
// static child, and main.tsx renders the creator at module scope.
export function FirstParent() {
  const a1 = createMemo(async () => 1);
  return (
    <div>
      <FirstCtx value={{ a1 }}>
        <div>
          <FirstBodyAliasIife />
          <FirstBodyDestructuredHelper />
          <FirstBodyHookMemberModuleHelper />
          <FirstBodyDirectClosure />
          <FirstRootAliasIife />
          <FirstRootDestructuredHelper />
          <FirstRootHookMemberModuleHelper />
          <FirstRootDirectClosure />
          <FirstSettledAliasIife />
          <FirstSettledDestructuredHelper />
          <FirstSettledHookMemberModuleHelper />
          <FirstSettledDirectClosure />
        </div>
      </FirstCtx>
    </div>
  );
}

// Uncertifiable, never a violation: the readers mount after the click,
// when the memo has settled (each a violation on 719b0e49a).
export function LaterParent() {
  const a1 = createMemo(async () => 1);
  const [on, setOn] = createSignal(false);
  return (
    <div>
      <button onClick={() => setOn(true)}>mount</button>
      <Show when={on()}>
        <LaterCtx value={{ a1 }}>
          <div>
            <LaterBodyAliasIife />
            <LaterBodyDestructuredHelper />
            <LaterBodyHookMemberModuleHelper />
            <LaterBodyDirectClosure />
            <LaterRootAliasIife />
            <LaterRootDestructuredHelper />
            <LaterRootHookMemberModuleHelper />
            <LaterRootDirectClosure />
            <LaterSettledAliasIife />
            <LaterSettledDestructuredHelper />
            <LaterSettledHookMemberModuleHelper />
            <LaterSettledDirectClosure />
          </div>
        </LaterCtx>
      </Show>
    </div>
  );
}
