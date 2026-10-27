import {
  createEffect,
  createMemo,
  createOwner,
  createSignal,
  getOwner,
  runWithOwner,
  untrack,
} from "solid-js";

// `runWithOwner(null, fn)` runs `fn` with no owner, and the write guard keys on
// the owner: every write the callback runs on its own stack is legal, directly
// or through a function it runs in place (ADR 0455).

export function NullDirect() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    setV(1);
  });
  return <div>{v()}</div>;
}
export function NullIife() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    (() => {
      return setV(1);
    })();
  });
  return <div>{v()}</div>;
}
export function NullForEach() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    [1].forEach(() => {
      setV(1);
    });
  });
  return <div>{v()}</div>;
}
export function NullNestedLayers() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    (() => {
      [1].forEach(() => {
        (() => setV(1))();
      });
    })();
  });
  return <div>{v()}</div>;
}
// `untrack` keeps the caller's owner, and here the caller has none.
export function NullUntrackIife() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    untrack(() => {
      (() => {
        setV(1);
      })();
    });
  });
  return <div>{v()}</div>;
}
export function NullNamedClosure() {
  const [v, setV] = createSignal(0);
  const bump = () => setV(1);
  runWithOwner(null, () => {
    bump();
  });
  return <div>{v()}</div>;
}
// The same inside a memo or effect compute: the callback has no owner whatever
// the compute around the call has.
export function NullIifeInMemo() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    runWithOwner(null, () => {
      (() => {
        setV(1);
      })();
    });
    return 1;
  });
  return <div>{v()}{m()}</div>;
}
export function NullIifeInEffect() {
  const [v, setV] = createSignal(0);
  createEffect(
    () => {
      runWithOwner(null, () => {
        (() => {
          return setV(1);
        })();
      });
      return 1;
    },
    () => {},
  );
  return <div>{v()}</div>;
}

// Controls: the owner reaches the write, or a layer builds its own.

// A memo created inside the detached callback owns its compute.
export function NullMemoInside() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    createMemo(() => {
      (() => {
        setV(1);
      })();
      return 1;
    });
  });
  return <div>{v()}</div>;
}
// An owner the call creates, and a component's own, are real owners.
export function CreatedOwnerIife() {
  const [v, setV] = createSignal(0);
  runWithOwner(createOwner(), () => {
    (() => {
      setV(1);
    })();
  });
  return <div>{v()}</div>;
}
export function ComponentOwnerIife() {
  const [v, setV] = createSignal(0);
  const owner = getOwner();
  runWithOwner(owner, () => {
    (() => {
      setV(1);
    })();
  });
  return <div>{v()}</div>;
}
// An owner that may be null decides nothing: uncertifiable (a bound
// `createOwner()` result is not proven non-null here).
declare const attached: boolean;
export function MaybeNullOwnerIife() {
  const [v, setV] = createSignal(0);
  const owner = attached ? getOwner() : null;
  runWithOwner(owner, () => {
    (() => {
      setV(1);
    })();
  });
  return <div>{v()}</div>;
}
export function BoundOwnerIife() {
  const [v, setV] = createSignal(0);
  const owner = createOwner();
  runWithOwner(owner, () => {
    (() => {
      setV(1);
    })();
  });
  return <div>{v()}</div>;
}
// The component body itself, without a `runWithOwner`.
export function BodyIife() {
  const [v, setV] = createSignal(0);
  (() => {
    setV(1);
  })();
  return <div>{v()}</div>;
}

// The owner reaches a write directly in the callback too: a proven owner is a
// violation (rc.17 throws), also when the call sits in a handler, since the
// callback runs under that owner wherever the call is; an owner not proven
// non-null is uncertifiable.
export function ComponentOwnerDirect() {
  const [v, setV] = createSignal(0);
  const owner = getOwner();
  runWithOwner(owner, () => {
    setV(1);
  });
  return <div>{v()}</div>;
}
export function CreatedOwnerDirect() {
  const [v, setV] = createSignal(0);
  runWithOwner(createOwner(), () => {
    setV(1);
  });
  return <div>{v()}</div>;
}
export function ComponentOwnerInHandler() {
  const [v, setV] = createSignal(0);
  const owner = getOwner();
  return (
    <button
      onClick={() => {
        runWithOwner(owner, () => {
          setV(1);
        });
      }}
    >
      {v()}
    </button>
  );
}
export function BoundOwnerDirect() {
  const [v, setV] = createSignal(0);
  const owner = createOwner();
  runWithOwner(owner, () => {
    setV(1);
  });
  return <div>{v()}</div>;
}
export function MaybeNullOwnerDirect() {
  const [v, setV] = createSignal(0);
  const owner = attached ? getOwner() : null;
  runWithOwner(owner, () => {
    setV(1);
  });
  return <div>{v()}</div>;
}

// `untrack` keeps the caller's owner: a write in a function it runs in place is
// as legal as the call itself.
export function UntrackIife() {
  const [v, setV] = createSignal(0);
  untrack(() => {
    (() => {
      setV(1);
    })();
  });
  return <div>{v()}</div>;
}
export function UntrackForEachInMemo() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    untrack(() => {
      [1].forEach(() => {
        setV(1);
      });
    });
    return 1;
  });
  return <div>{v()}{m()}</div>;
}
export function UntrackIifeInHandler() {
  const [v, setV] = createSignal(0);
  return (
    <button
      onClick={() => {
        untrack(() => {
          (() => {
            setV(1);
          })();
        });
      }}
    >
      {v()}
    </button>
  );
}

// A default value runs only when a call omits the argument, and nothing here
// proves the omission: uncertifiable under a proven owner, as in a root body
// (`write-default-in-root-closure`).
export function CreatedOwnerIifeDefaultOmitted() {
  const [v, setV] = createSignal(0);
  runWithOwner(createOwner(), () => {
    ((p: number = setV(1)) => p)();
  });
  return <div>{v()}</div>;
}
export function CreatedOwnerIifeDefaultSupplied() {
  const [v, setV] = createSignal(0);
  runWithOwner(createOwner(), () => {
    ((p: number = setV(1)) => p)(5);
  });
  return <div>{v()}</div>;
}

// A closure bound to a name inside an `untrack` callback is built there, not
// run: it runs where it is called, under the owner `untrack` keeps (ADR 0455).
// Called in the callback's own pass under an owned scope it is a violation,
// also through layers that run in place and in a memo; called only on a fresh
// stack, in a handler or under a detached owner it is legal; only returned, or
// stored or handed on in a literal, it is uncertifiable.
export function UntrackClosureCalled() {
  const [v, setV] = createSignal(0);
  untrack(() => {
    const bump = () => setV(1);
    bump();
  });
  return <div>{v()}</div>;
}
export function UntrackClosureViaMap() {
  const [v, setV] = createSignal(0);
  untrack(() => {
    const bump = () => {
      return setV(1);
    };
    [1].map(() => {
      return bump();
    });
  });
  return <div>{v()}</div>;
}
export function UntrackFunctionInMemo() {
  const [v, setV] = createSignal(0);
  const m = createMemo(() => {
    untrack(() => {
      function bump() {
        setV(1);
      }
      bump();
    });
    return 1;
  });
  return <div>{v()}{m()}</div>;
}
export function UntrackClosureInHandler() {
  const [v, setV] = createSignal(0);
  return (
    <button
      onClick={() => {
        untrack(() => {
          const bump = () => setV(1);
          bump();
        });
      }}
    >
      {v()}
    </button>
  );
}
export function UntrackClosureUnderNullOwner() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    untrack(() => {
      const bump = () => setV(1);
      bump();
    });
  });
  return <div>{v()}</div>;
}
export function UntrackClosureInTimeout() {
  const [v, setV] = createSignal(0);
  untrack(() => {
    const bump = () => setV(1);
    setTimeout(() => bump());
  });
  return <div>{v()}</div>;
}
export function UntrackClosureReturned() {
  const [v, setV] = createSignal(0);
  const run = untrack(() => {
    const bump = () => setV(1);
    return bump;
  });
  return <button onClick={() => run()}>{v()}</button>;
}
export function UntrackClosureDefault() {
  const [v, setV] = createSignal(0);
  untrack(() => {
    const bump = (p: number = setV(1)) => p;
    bump();
  });
  return <div>{v()}</div>;
}
export function UntrackClosureByName() {
  const [v, setV] = createSignal(0);
  untrack(() => {
    const bump = () => setV(1);
    [1].forEach(bump);
  });
  return <div>{v()}</div>;
}
export function UntrackLiteralStored() {
  const [v, setV] = createSignal(0);
  untrack(() => ({ go: () => setV(1) }));
  return <div>{v()}</div>;
}
function callNow(callback: () => void) {
  callback();
}
export function UntrackLiteralToHelper() {
  const [v, setV] = createSignal(0);
  untrack(() => {
    callNow(() => setV(1));
  });
  return <div>{v()}</div>;
}
