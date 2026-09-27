import * as Solid from "solid-js";
import { createEffect, createRoot, createSignal, onCleanup, onSettled } from "solid-js";

const [count] = createSignal(0);

// A callback handed directly to a reviewed fresh-stack host scheduler runs
// from a task or microtask queue with no owner, wherever it was scheduled.

// Module scope.
setTimeout(() => {
  onCleanup(() => {});
}, 0);

// A `createRoot` callback owns only its synchronous extent.
createRoot(() => {
  setTimeout(() => {
    onCleanup(() => {});
  }, 0);
});

// A component body: every owner requirement, and every reviewed scheduler.
function Clock() {
  setTimeout(() => {
    onCleanup(() => {});
  }, 0);
  setTimeout(() => {
    createEffect(
      () => count(),
      () => {},
    );
  }, 0);
  setTimeout(() => {
    onSettled(() => () => {});
  }, 0);
  // The namespace spelling of the operation changes nothing.
  setTimeout(() => {
    Solid.onCleanup(() => {});
  }, 0);
  // A named function handed to the scheduler is the callback.
  function tick() {
    onCleanup(() => {});
  }
  setInterval(tick, 1000);
  queueMicrotask(() => {
    onCleanup(() => {});
  });
  requestAnimationFrame(() => {
    onCleanup(() => {});
  });
  requestIdleCallback(() => {
    onCleanup(() => {});
  });
  Promise.resolve(1).then(() => {
    onCleanup(() => {});
  });
  Promise.resolve(1).catch(() => {
    onCleanup(() => {});
  });
  Promise.resolve(1).finally(() => {
    onCleanup(() => {});
  });
  new IntersectionObserver(() => {
    onCleanup(() => {});
  });
  new ResizeObserver(() => {
    onCleanup(() => {});
  });
  new MutationObserver(() => {
    onCleanup(() => {});
  });
  new PerformanceObserver(() => {
    onCleanup(() => {});
  });
  new ReportingObserver(() => {
    onCleanup(() => {});
  });
  return <div />;
}

// Controls: each stays silent.
declare const thenable: PromiseLike<number>;

// A timer callback that only reads has no owner requirement. (Scheduled from a
// root rather than a component body: see README.md for the separate `SC1001`
// the same read draws in a component.)
createRoot(() => {
  setTimeout(() => {
    console.log(count());
  }, 0);
});

function Controls() {
  // A root created in the timer callback owns what it contains.
  setTimeout(() => {
    createRoot(() => {
      onCleanup(() => {});
    });
  }, 0);
  // The cleanup that clears a timer is registered in the component body.
  const id = setInterval(() => console.log("tick"), 1000);
  onCleanup(() => clearInterval(id));
  // Not a reviewed fresh-stack scheduler: a thenable may call back
  // synchronously, and a listener runs on a synchronous dispatcher's stack.
  thenable.then(() => {
    onCleanup(() => {});
  });
  window.addEventListener("resize", () => {
    onCleanup(() => {});
  });
  return <div />;
}

// A local function spelled like a scheduler is not the standard-library
// declaration: it runs its callback inline, under the component's owner.
function Shadowed() {
  const setTimeout = (fn: () => void, _delay: number) => fn();
  setTimeout(() => {
    onCleanup(() => {});
  }, 0);
  return <div />;
}

// Not claimed: the scheduler receives what the wrapper returns, and the arrow
// inside may run on the wrapper's own stack, so it gets no owner edge. Pinned
// as a known false negative (see README.md).
function wrap(fn: () => void): () => void {
  return fn;
}
function Wrapped() {
  setTimeout(
    wrap(() => {
      onCleanup(() => {});
    }),
    0,
  );
  return <div />;
}
