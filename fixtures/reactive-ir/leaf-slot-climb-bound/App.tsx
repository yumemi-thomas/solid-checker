import { createTrackedEffect, flush, untrack } from "solid-js";

// The slot climb of the leaf-owner walk follows at most sixteen layers (ADR 0959): `untrack` runs its callback in place, so the call is on the stack of the leaf
// owner, and the walk crosses one layer per callback. A call under seventeen of them is a climb that ran out of layers before it reached the leaf callback: an
// obligation of SC3001 (`leaf-owner-unproven`), never silence and never a violation.
export function PastTheBound() {
  createTrackedEffect(() => {
    untrack(() => {
      untrack(() => {
        untrack(() => {
          untrack(() => {
            untrack(() => {
              untrack(() => {
                untrack(() => {
                  untrack(() => {
                    untrack(() => {
                      untrack(() => {
                        untrack(() => {
                          untrack(() => {
                            untrack(() => {
                              untrack(() => {
                                untrack(() => {
                                  untrack(() => {
                                    untrack(() => {
                                      flush();
                                    });
                                  });
                                });
                              });
                            });
                          });
                        });
                      });
                    });
                  });
                });
              });
            });
          });
        });
      });
    });
  });
  return <p>x</p>;
}

// Sixteen layers are still crossed: the walk's own proof (a violation).
export function AtTheBound() {
  createTrackedEffect(() => {
    untrack(() => {
      untrack(() => {
        untrack(() => {
          untrack(() => {
            untrack(() => {
              untrack(() => {
                untrack(() => {
                  untrack(() => {
                    untrack(() => {
                      untrack(() => {
                        untrack(() => {
                          untrack(() => {
                            untrack(() => {
                              untrack(() => {
                                untrack(() => {
                                  untrack(() => {
                                    flush();
                                  });
                                });
                              });
                            });
                          });
                        });
                      });
                    });
                  });
                });
              });
            });
          });
        });
      });
    });
  });
  return <p>x</p>;
}
