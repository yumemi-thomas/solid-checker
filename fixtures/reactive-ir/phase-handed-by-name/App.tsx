import {
  action,
  createEffect,
  createMemo,
  createSignal,
  createTrackedEffect,
  flush,
  onSettled,
} from "solid-js";

// ADR 0538: a named function handed to a carrier, a member or project code
// inside a primitive's callback. The effect-phase walk (SC2008, SC6002), the
// action-step proof (SC2006) and the leaf-owner walk (SC3001) place the run
// at an inline carrier and leave an unplaced hand-off as an obligation.
// rc.17 fires each misuse here (full-6 grid, 2026-11-01).

// Violation (SC2008): `forEach` runs the function before it returns, on the
// apply's stack.
export function ApplyForEach() {
  createEffect(
    () => 1,
    () => {
      const hf = () => {
        flush();
      };
      [1].forEach(hf);
    },
  );
  return <p />;
}

// Violation: a template tag and `Reflect.apply` are carriers too.
export function ApplyTagged() {
  createEffect(
    () => 1,
    () => {
      const hf = (_s: TemplateStringsArray) => {
        flush();
      };
      hf`x`;
    },
  );
  return <p />;
}

export function ApplyReflect() {
  createEffect(
    () => 1,
    () => {
      const hf = () => {
        flush();
      };
      Reflect.apply(hf, null, []);
    },
  );
  return <p />;
}

// Obligation: a project helper may run its argument anywhere.
export function ApplyWrapper() {
  createEffect(
    () => 1,
    () => {
      const hf = () => {
        flush();
      };
      const run = (g: () => void) => {
        g();
      };
      run(hf);
    },
  );
  return <p />;
}

// Obligation: a member's value runs wherever the member is called.
export function ApplyThisMember() {
  createEffect(
    () => 1,
    () => {
      class Holder {
        m?: () => void;
        constructor() {
          this.m = () => {
            flush();
          };
        }
      }
      new Holder().m?.();
    },
  );
  return <p />;
}

export function ApplyReturnedMember() {
  createEffect(
    () => 1,
    () => {
      const make = () => {
        const o: { m?: () => void } = {};
        o.m = () => {
          flush();
        };
        return o;
      };
      make().m?.();
    },
  );
  return <p />;
}

// Obligation: an object slot read through its object.
export function ApplySlot() {
  createEffect(
    () => 1,
    () => {
      const box = {
        run: () => {
          flush();
        },
      };
      box.run();
    },
  );
  return <p />;
}

// Legal: a fresh stack and a kept listener run the function elsewhere.
export function ApplyTimeout() {
  createEffect(
    () => 1,
    () => {
      const hf = () => {
        flush();
      };
      setTimeout(hf);
    },
  );
  return <p />;
}

export function ApplyListener() {
  createEffect(
    () => 1,
    () => {
      const hf = () => {
        flush();
      };
      document.addEventListener("click", hf);
    },
  );
  return <p />;
}

// Legal: a module-level class is reached through no layer.
class ModuleHolder {
  m?: () => void;
  constructor() {
    this.m = () => {
      flush();
    };
  }
}
export function ApplyModuleClass() {
  createEffect(
    () => 1,
    () => {
      new ModuleHolder().m?.();
    },
  );
  return <p />;
}

// Violation (SC2008): a callback prop the child only calls in its apply, as
// a member of the props object and as a destructured binding.
export function PropApply() {
  return (
    <Kid
      run={() => {
        flush();
      }}
    />
  );
}
function Kid(props: { run: () => void }) {
  createEffect(
    () => 1,
    () => {
      props.run();
    },
  );
  return <p />;
}

export function PropApplyDestructured() {
  return (
    <KidDestructured
      run={() => {
        flush();
      }}
    />
  );
}
function KidDestructured({ run }: { run: () => void }) {
  createEffect(
    () => 1,
    () => {
      run();
    },
  );
  return <p />;
}

// Obligation: the child also calls the prop from a handler.
export function PropShared() {
  return (
    <KidShared
      run={() => {
        flush();
      }}
    />
  );
}
function KidShared(props: { run: () => void }) {
  createEffect(
    () => 1,
    () => {
      props.run();
    },
  );
  return <p onClick={() => props.run()} />;
}

// Legal: the child calls the prop only from a handler, or defers it.
export function PropHandler() {
  return (
    <KidHandler
      run={() => {
        flush();
      }}
    />
  );
}
function KidHandler(props: { run: () => void }) {
  return <p onClick={() => props.run()} />;
}

export function PropDeferred() {
  return (
    <KidDeferred
      run={() => {
        flush();
      }}
    />
  );
}
function KidDeferred(props: { run: () => void }) {
  createEffect(
    () => 1,
    () => {
      setTimeout(() => props.run());
    },
  );
  return <p />;
}

// Violation (SC6002): a computation created in a function a carrier runs on
// the apply's stack, with no owner set in between.
export function MemoForEach() {
  createEffect(
    () => 1,
    () => {
      const hf = () => {
        createMemo(() => 1);
      };
      [1].forEach(hf);
    },
  );
  return <p />;
}

// Violation (SC6002): through a callback prop.
export function MemoProp() {
  return (
    <KidMemo
      run={() => {
        createMemo(() => 1);
      }}
    />
  );
}
function KidMemo(props: { run: () => void }) {
  createEffect(
    () => 1,
    () => {
      props.run();
    },
  );
  return <p />;
}

// Obligation (SC6002): handed to a project helper.
export function MemoWrapper() {
  createEffect(
    () => 1,
    () => {
      const hf = () => {
        createMemo(() => 1);
      };
      const run = (g: () => void) => {
        g();
      };
      run(hf);
    },
  );
  return <p />;
}

// Violation (SC2006, 831c70635): a carrier runs the function inside the step.
export const actionForEach = action(function* () {
  const hf = () => {
    flush();
  };
  [1].forEach(hf);
  yield 1;
});

// Obligation (SC2006): a member's value, a helper's argument, and a chain of
// in-place layers past ADR 0532's bound.
export const actionMember = action(function* () {
  const hf = () => {
    flush();
  };
  const ho = { m: hf };
  ho.m();
  yield 1;
});

export const actionWrapper = action(function* () {
  const hf = () => {
    flush();
  };
  const run = (g: () => void) => {
    g();
  };
  run(hf);
  yield 1;
});

export const actionThisMember = action(function* () {
  class Holder {
    m?: () => void;
    constructor() {
      this.m = () => {
        flush();
      };
    }
  }
  new Holder().m?.();
  yield 1;
});

export const actionDeep = action(function* () {
  (() => {
    (() => {
      (() => {
        (() => {
          (() => {
            (() => {
              (() => {
                (() => {
                  (() => {
                    (() => {
                      (() => {
                        (() => {
                          (() => {
                            (() => {
                              (() => {
                                (() => {
                                  (() => {
                                    flush();
                                  })();
                                })();
                              })();
                            })();
                          })();
                        })();
                      })();
                    })();
                  })();
                })();
              })();
            })();
          })();
        })();
      })();
    })();
  })();
  yield 1;
});

// Legal: the function runs on a fresh stack.
export const actionTimeout = action(function* () {
  const hf = () => {
    flush();
  };
  setTimeout(hf);
  yield 1;
});

// Violation (SC3001): a template tag runs its function inside the leaf
// callback.
export function TrackedTagged() {
  const [s] = createSignal(1);
  createTrackedEffect(() => {
    s();
    const hf = (_s: TemplateStringsArray) => {
      flush();
    };
    hf`x`;
  });
  return <p />;
}

export function SettledTagged() {
  onSettled(() => {
    const hf = (_s: TemplateStringsArray) => {
      flush();
    };
    hf`x`;
  });
  return <p />;
}

// Legal: a tag that performs no forbidden operation.
export function SettledTaggedClean() {
  onSettled(() => {
    const hf = (_s: TemplateStringsArray) => 1;
    hf`x`;
  });
  return <p />;
}
