import { Errored, Show, createContext, createDeferred, createEffect, createMemo, createRenderEffect, createTrackedEffect, isPending, resolve, untrack, useContext } from "solid-js";

// Default-less and never provided: a read under an owner throws ContextNotFoundError.
const Ctx = createContext<number>();

// Violations: a read that reports the stored throw.
export function ReadInEffect() {
  const m = createMemo(() => {
    void resolve(() => 1);
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div></div>;
}

export function ReadInJsx() {
  const m = createMemo(() => {
    void resolve(() => 1);
    return 1;
  });
  return <div>{m()}</div>;
}

export function ReadInHandler() {
  const m = createMemo(() => {
    void resolve(() => 1);
    return 1;
  });
  return <button onClick={() => m()}>x</button>;
}

export function ChainRead() {
  const m = createMemo(() => {
    void resolve(() => 1);
    return 1;
  });
  const n = createMemo(() => m());
  createEffect(() => n(), () => {});
  return <div></div>;
}

export function ReadThroughHelper() {
  const m = createMemo(() => {
    void resolve(() => 1);
    return 1;
  });
  const read = () => m();
  createEffect(() => read(), () => {});
  return <div></div>;
}

export function ReadUnderUntrack() {
  const m = createMemo(() => {
    void resolve(() => 1);
    return 1;
  });
  createEffect(() => untrack(() => m()), () => {});
  return <div></div>;
}

// One entry that reports keeps the violation, whatever the other entries are.
export function EffectEntry() {
  const h = () => {
    void resolve(() => 1);
    return 1;
  };
  const m = createMemo(() => h());
  void m;
  createEffect(() => h(), () => {});
  return <div></div>;
}

export function ContextRead() {
  const m = createMemo(() => {
    useContext(Ctx);
    return 1;
  });
  createEffect(() => m(), () => {});
  return <div></div>;
}

export function LoggingBoundary() {
  const m = createMemo(() => {
    void resolve(() => 1);
    return 1;
  });
  return <Errored fallback={() => <p>x</p>}><p>{m()}</p></Errored>;
}

// Obligations (uncertifiable, computation-error-unobserved): the throw is stored and nothing proven reports it.
export function Unread() {
  const m = createMemo(() => {
    void resolve(() => 1);
    return 1;
  });
  void m;
  return <div></div>;
}

export function ContextUnread() {
  const m = createMemo(() => {
    useContext(Ctx);
    return 1;
  });
  void m;
  return <div></div>;
}

export function ChainUnread() {
  const m = createMemo(() => {
    void resolve(() => 1);
    return 1;
  });
  const n = createMemo(() => m());
  void n;
  return <div></div>;
}

export function DeferredUnread() {
  const d = createDeferred(() => {
    void resolve(() => 1);
    return 1;
  });
  void d;
  return <div></div>;
}

export function ReadInTry() {
  const m = createMemo(() => {
    void resolve(() => 1);
    return 1;
  });
  createEffect(() => {
    try {
      return m();
    } catch {
      return 0;
    }
  }, () => {});
  return <div></div>;
}

export function TryAboveHelper() {
  const m = createMemo(() => {
    void resolve(() => 1);
    return 1;
  });
  const read = () => m();
  createEffect(() => {
    try {
      return read();
    } catch {
      return 0;
    }
  }, () => {});
  return <div></div>;
}

export function ReadInProbe() {
  const m = createMemo(() => {
    void resolve(() => 1);
    return 1;
  });
  createEffect(() => isPending(() => m()), () => {});
  return <div></div>;
}

export function ErrorArm() {
  const m = createMemo(() => {
    void resolve(() => 1);
    return 1;
  });
  createEffect(() => m(), { effect: () => {}, error: () => {} });
  return <div></div>;
}

export function HandlingBoundary() {
  const m = createMemo(() => {
    void resolve(() => 1);
    return 1;
  });
  return <Errored fallback={(e) => <p>{String(e())}</p>}><p>{m()}</p></Errored>;
}

function Kid() {
  const m = createMemo(() => {
    void resolve(() => 1);
    return 1;
  });
  return <p>{m()}</p>;
}

export function HandlingBoundaryAboveMount() {
  return <Errored fallback={(e) => <p>{String(e())}</p>}><Kid /></Errored>;
}

export function ReadInGetter() {
  const m = createMemo(() => {
    void resolve(() => 1);
    return 1;
  });
  return <Show when={false}><p>{m()}</p></Show>;
}

export function ReadPastAwait() {
  const m = createMemo(() => {
    void resolve(() => 1);
    return 1;
  });
  void (async () => {
    await 0;
    m();
  })();
  return <div></div>;
}

export function AliasedComputeUnread() {
  const inner = () => {
    void resolve(() => 1);
    return 1;
  };
  const compute = inner;
  const m = createMemo(compute);
  void m;
  return <div></div>;
}

// A reference is no run, and what runs a handed function may take its throw.
export function ReferenceOnly() {
  const inner = () => useContext(Ctx);
  const c = inner;
  void c;
  return <div></div>;
}

function Catching(p: { run: () => unknown }) {
  try {
    p.run();
  } catch {
    // The child takes the throw.
  }
  return <i />;
}

export function PropRunInChildTry() {
  return <Catching run={() => useContext(Ctx)} />;
}

// A promise handler's throw rejects a promise something may hold; a timer's reaches the host.
export function ThenCatch() {
  void Promise.resolve(1).then(() => useContext(Ctx)).catch(() => {});
  return <div></div>;
}

function RenderKid() {
  createRenderEffect(() => useContext(Ctx), () => {});
  return <i />;
}

function TrackedKid() {
  createTrackedEffect(() => {
    useContext(Ctx);
  });
  return <i />;
}

export function RenderEffectUnderBoundary() {
  return <Errored fallback={(e) => <p>{String(e())}</p>}><RenderKid /></Errored>;
}

export function TrackedEffectUnderBoundary() {
  return <Errored fallback={(e) => <p>{String(e())}</p>}><TrackedKid /></Errored>;
}

// A throw the program catches fires no diagnostic, around the site or a caller.
export function TryAroundSite() {
  createEffect(() => {
    try {
      void resolve(() => 1);
    } catch {
      return 0;
    }
    return 1;
  }, () => {});
  return <div></div>;
}

export function TryAboveThrower() {
  const h = () => {
    void resolve(() => 1);
    return 1;
  };
  createEffect(() => {
    try {
      return h();
    } catch {
      return 0;
    }
  }, () => {});
  return <div></div>;
}

// The one tracked entry is unread; the body runs with no observer.
export function BodyEntry() {
  const h = () => {
    void resolve(() => 1);
    return 1;
  };
  const m = createMemo(() => h());
  void m;
  h();
  return <div></div>;
}

// A local `createMemo` is not the primitive: the call runs where it is written, with no observer.
export function ShadowedMemo() {
  const createMemo = (f: () => number) => f();
  createMemo(() => {
    void resolve(() => 1);
    return 1;
  });
  return <div></div>;
}
