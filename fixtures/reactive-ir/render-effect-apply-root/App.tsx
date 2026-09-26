import * as Solid from "solid-js";
import {
  createEffect,
  createMemo,
  createRenderEffect,
  createRoot,
  createSignal,
  onCleanup,
} from "solid-js";

const [count] = createSignal(0);
const [shared] = createSignal(0);

// Ownership inside a root. The first apply runs during the call under the
// root; a later one, from the flush, has no owner, so its cleanup never runs
// on dispose. A later run needs `count` to change, which nothing here proves:
// uncertifiable, as under a component body.
createRoot(() => {
  createRenderEffect(
    () => count(),
    () => {
      onCleanup(() => {});
    },
  );
});

// An effect created there: on a later run it has no owner.
createRoot(() => {
  createRenderEffect(
    () => count(),
    () => {
      createEffect(
        () => shared(),
        () => {},
      );
    },
  );
});

// The namespace spelling resolves to the same primitives.
createRoot(() => {
  Solid.createRenderEffect(
    () => count(),
    () => {
      Solid.onCleanup(() => {});
    },
  );
});

// A root, then a memo compute: the apply's first run answers to the memo, a
// later one to nothing. Every enclosing owner-creating callback is the same
// shortcut.
createRoot(() => {
  createMemo(() => {
    createRenderEffect(
      () => count(),
      () => {
        onCleanup(() => {});
      },
    );
    return 1;
  });
});

// A helper declared inside the root and called from the apply runs in the
// apply's extent, later runs included. Judged through the owner graph, not
// by where the helper is written.
createRoot(() => {
  function register() {
    onCleanup(() => {});
  }
  createRenderEffect(
    () => count(),
    () => {
      register();
    },
  );
});

// Controls: each stays silent.
createRoot(() => {
  // In the root body itself.
  onCleanup(() => {});
  // In a render-effect compute, which the render effect owns.
  createRenderEffect(
    () => {
      onCleanup(() => {});
      return count();
    },
    () => {},
  );
  // In a helper the root body calls, and nothing else.
  function registerHere() {
    onCleanup(() => {});
  }
  registerHere();
});

// Not this fixture's claim, and deliberately unchanged: a `createEffect` apply
// has no owner on any run, so this cleanup never runs either, but every call
// lexically inside a root outside a render-effect apply is still treated as
// root-owned (see README.md).
createRoot(() => {
  createEffect(
    () => count(),
    () => {
      onCleanup(() => {});
    },
  );
});

// Read wording. Each read below is SC1001 at the read, and its message names
// the primitive whose apply the read runs in.
export function RenderApplyRead() {
  createRenderEffect(
    () => 1,
    () => {
      console.log(shared());
    },
  );
  return <div />;
}

export function NamespaceRenderApplyRead() {
  Solid.createRenderEffect(
    () => 1,
    () => {
      console.log(shared());
    },
  );
  return <div />;
}

export function NamedRenderApplyRead() {
  const onValue = () => {
    console.log(shared());
  };
  createRenderEffect(() => 1, onValue);
  return <div />;
}

export function EffectApplyRead() {
  createEffect(
    () => 1,
    () => {
      console.log(shared());
    },
  );
  return <div />;
}

// One function passed as the apply of both primitives: the message names
// neither.
export function SharedApplyRead() {
  const onEither = () => {
    console.log(shared());
  };
  createEffect(() => 1, onEither);
  createRenderEffect(() => 1, onEither);
  return <div />;
}
