import { Show, createEffect, createMemo, createOwner, createRoot, createSignal, createTrackedEffect, refresh, runWithOwner } from "solid-js";

// ADR 0537: four residual families of the full-6 grid (2026-11-01), each an
// rc.17 misuse the checker certified clean or gave the wrong rule.

// (1) A write that reaches an inline carrier only through a helper call.
// rc.17 throws REACTIVE_WRITE_IN_OWNED_SCOPE on each obligation below; the
// direct write at the same place is an obligation too (ADR 0535).

// Obligation: the setter through `wrapSet`, `hf` handed to `forEach` inside
// `runWithOwner` in a handler.
export function WriteOwnerForEach() {
  const [, setW] = createSignal(0);
  const wrapSet = () => {
    setW(7);
  };
  const owner = createOwner();
  return (
    <button
      onClick={() => {
        runWithOwner(owner, () => {
          const hf = () => {
            wrapSet();
          };
          [1].forEach(hf);
        });
      }}
    >
      go
    </button>
  );
}

// Obligation: `refresh` through a helper, `hf` handed to project code.
export function RefreshOwnerProject() {
  const m = createMemo(() => 1);
  const wrapRefresh = () => {
    refresh(m);
  };
  const owner = createOwner();
  return (
    <button
      onClick={() => {
        runWithOwner(owner, () => {
          const hf = () => {
            wrapRefresh();
          };
          const run = (g: () => void) => {
            g();
          };
          run(hf);
        });
      }}
    >
      go
    </button>
  );
}

// Obligation: a member's function value in a root a handler creates.
export function WriteRootMember() {
  const [, setW] = createSignal(0);
  const wrapSet = () => {
    setW(7);
  };
  return (
    <button
      onClick={() => {
        createRoot(() => {
          const ho: { m?: () => void } = {};
          ho.m = () => {
            wrapSet();
          };
          ho.m?.();
        });
      }}
    >
      go
    </button>
  );
}

// Legal: the same helper handed to `forEach` in the handler itself.
export function WriteHandlerForEach() {
  const [, setW] = createSignal(0);
  const wrapSet = () => {
    setW(7);
  };
  return (
    <button
      onClick={() => {
        const hf = () => {
          wrapSet();
        };
        [1].forEach(hf);
      }}
    >
      go
    </button>
  );
}

// (2) A tagged template calls its tag where it is written (rc.17 throws
// PRIMITIVE_IN_FORBIDDEN_SCOPE in a leaf owner).

// Violation: the tag creates a memo inside `createTrackedEffect`.
export function LeafTagged() {
  createTrackedEffect(() => {
    const hf = (_s: TemplateStringsArray) => {
      createMemo(() => 1);
    };
    hf`x`;
  });
  return <p />;
}

// Obligation: a helper the callback calls tags the template; the template is
// in no completion census, so the memo is possible only.
export function LeafTaggedInHelper() {
  const hf = (_s: TemplateStringsArray) => {
    createMemo(() => 1);
  };
  const helper = () => {
    hf`x`;
  };
  createTrackedEffect(() => {
    helper();
  });
  return <p />;
}

// Legal: the tag creates nothing.
export function LeafTaggedPlain() {
  createTrackedEffect(() => {
    const hf = (s: TemplateStringsArray) => s.length;
    hf`x`;
  });
  return <p />;
}

// Legal: the inner `hf` shadows the one that creates a memo.
export function LeafTaggedShadowed() {
  const hf = (_s: TemplateStringsArray) => {
    createMemo(() => 1);
  };
  hf`y`;
  createTrackedEffect(() => {
    const hf = (s: TemplateStringsArray) => s.length;
    hf`x`;
  });
  return <p />;
}

// (3) A pending async read through a hook result, in a named function handed
// on inside a control-flow child: rc.17 raises PENDING_ASYNC_UNTRACKED_READ,
// never ASYNC_OUTSIDE_LOADING_BOUNDARY. Placed nowhere: an obligation.

// Obligation: handed to project code.
export function HookRunShow() {
  const a1 = createMemo(async () => 1);
  const useThing = () => ({ useIt: a1 });
  const { useIt } = useThing();
  return (
    <div>
      <Show when={1}>
        {(_u) => {
          const hf = () => {
            useIt();
          };
          const run = (g: () => void) => {
            g();
          };
          run(hf);
          return <p>x</p>;
        }}
      </Show>
    </div>
  );
}

// Obligation: a member's function value.
export function HookMemberShow() {
  const a1 = createMemo(async () => 1);
  const useThing = () => ({ useIt: a1 });
  const { useIt } = useThing();
  return (
    <div>
      <Show when={1}>
        {(_u) => {
          const ho: { m?: () => void } = {};
          ho.m = () => {
            useIt();
          };
          ho.m?.();
          return <p>x</p>;
        }}
      </Show>
    </div>
  );
}

// Violation (unchanged): the hook read rendered under `Show` with no
// `Loading` boundary.
export function HookJsxShow() {
  const a1 = createMemo(async () => 1);
  const useThing = () => ({ useIt: a1 });
  const { useIt } = useThing();
  return (
    <div>
      <Show when={1}>
        <p>{useIt()}</p>
      </Show>
    </div>
  );
}

// Obligation (unchanged): the direct binding's twin of HookRunShow.
export function DirectRunShow() {
  const a1 = createMemo(async () => 1);
  return (
    <div>
      <Show when={1}>
        {(_u) => {
          const hf = () => {
            a1();
          };
          const run = (g: () => void) => {
            g();
          };
          run(hf);
          return <p>x</p>;
        }}
      </Show>
    </div>
  );
}

// (4) Reads after an await in a tracked async computation (rc.17 warns
// UNTRACKED_READ_AFTER_AWAIT).

// Violation: 15 in-place layers, under ADR 0532's 16-layer bound.
export function AwaitDepth15() {
  const [r1] = createSignal(3);
  createEffect(async () => {
    await Promise.resolve();
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
                                  r1();
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
    return 1;
  }, () => {});
  return <p />;
}

// Violation: `Function.prototype.call` runs `hf`, which calls the helper.
export function AwaitWrapperCall() {
  const [r1] = createSignal(3);
  const wrapUse = () => {
    r1();
  };
  createEffect(async () => {
    await Promise.resolve();
    const hf = () => {
      wrapUse();
    };
    hf.call(null);
    return 1;
  }, () => {});
  return <p />;
}

// Violation: `apply`.
export function AwaitWrapperApply() {
  const [r1] = createSignal(3);
  const wrapUse = () => {
    r1();
  };
  createEffect(async () => {
    await Promise.resolve();
    const hf = () => {
      wrapUse();
    };
    hf.apply(null, []);
    return 1;
  }, () => {});
  return <p />;
}

// Violation: a tagged template.
export function AwaitWrapperTagged() {
  const [r1] = createSignal(3);
  const wrapUse = () => {
    r1();
  };
  createEffect(async () => {
    await Promise.resolve();
    const hf = (_s: TemplateStringsArray) => {
      wrapUse();
    };
    hf`x`;
    return 1;
  }, () => {});
  return <p />;
}

// Obligation: after a loop that awaits, whether the code runs is unproven.
export function AwaitLoopTagged() {
  const [r1] = createSignal(3);
  const wrapUse = () => {
    r1();
  };
  createEffect(async () => {
    for (const _ of [1]) {
      await Promise.resolve();
    }
    const hf = (_s: TemplateStringsArray) => {
      wrapUse();
    };
    hf`x`;
    return 1;
  }, () => {});
  return <p />;
}

// Legal: an async click handler is no tracked computation.
export function HandlerAwaitCall() {
  const [r1] = createSignal(3);
  const wrapUse = () => {
    r1();
  };
  return (
    <button
      onClick={async () => {
        await Promise.resolve();
        const hf = () => {
          wrapUse();
        };
        hf.call(null);
      }}
    >
      go
    </button>
  );
}
