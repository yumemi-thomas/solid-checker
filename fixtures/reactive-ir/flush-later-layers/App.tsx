import { action, createEffect, createMemo, createSignal, createTrackedEffect, flush } from "solid-js";

const [n] = createSignal(0);

// ADR 1206: the program steps an action, runs an effect's apply and opens a leaf scope, so no program-wide reason holds. A flush in exact in-place
// layers (ADR 1090) whose outermost call runs past a suspension of its own async function no await dominates (ADR 1201) runs there too.
export const step = action(function* () {
  yield 1;
});

export function Host() {
  createEffect(() => n(), () => {});
  createTrackedEffect(() => {
    n();
  });
  return <div />;
}

// Clean: a helper called once by a closure called once in a named compute's later pass (the effect-phase walk's unplaced row asks the climb's end).
function wrapUse() {
  flush();
}
export function WrapperNamed() {
  const cb1 = async () => {
    for (let i = 0; i < 2; i += 1) {
      if (i > 0) {
        const f1 = () => {
          wrapUse();
        };
        f1();
      }
      await Promise.resolve();
    }
    return 1;
  };
  const m1 = createMemo(cb1);
  createEffect(() => m1(), () => {});
  return <div />;
}

// Clean: a chain of five closures, each called once, in an inline compute's later pass (the census asks the climb's end).
export function Chain() {
  const m2 = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      if (i > 0) {
        const hc0 = () => {
          flush();
        };
        const hc1 = () => {
          hc0();
        };
        const hc2 = () => {
          hc1();
        };
        const hc3 = () => {
          hc2();
        };
        const hc4 = () => {
          hc3();
        };
        hc4();
      }
      await Promise.resolve();
    }
    return 1;
  });
  createEffect(() => m2(), () => {});
  return <div />;
}

// Clean: fifteen nested IIFEs in a `finally` behind the `try` head's await.
export function DeepFinally() {
  createEffect(
    async () => {
      try {
        await Promise.resolve();
      } finally {
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
      }
      return 1;
    },
    () => {},
  );
  return <div />;
}

// Clean: seventeen nested IIFEs. The climb's bound is sixteen layers; ADR 1199 takes the innermost IIFE's one call as its entry and climbs from
// there, so every layer is still crossed exactly.
export function Deep17() {
  createEffect(
    async () => {
      try {
        await Promise.resolve();
      } finally {
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
      }
      return 1;
    },
    () => {},
  );
  return <div />;
}

// Uncertifiable (fail closed): the closure is also handed on, so a run of it may be anywhere.
const kept: (() => void)[] = [];
export function HandedOn() {
  const m4 = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      if (i > 0) {
        const f3 = () => {
          flush();
        };
        kept.push(f3);
        f3();
      }
      await Promise.resolve();
    }
    return 1;
  });
  createEffect(() => m4(), () => {});
  return <div />;
}

// Uncertifiable (fail closed): the helper is also called on the first pass, before any await.
function twiceUse() {
  flush();
}
export function AlsoFirstPass() {
  const m3 = createMemo(async () => {
    for (let i = 0; i < 2; i += 1) {
      if (i > 0) {
        const f2 = () => {
          twiceUse();
        };
        f2();
      }
      twiceUse();
      await Promise.resolve();
    }
    return 1;
  });
  createEffect(() => m3(), () => {});
  return <div />;
}
