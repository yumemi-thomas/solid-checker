// ADR 0459: a write in a function a `createRenderEffect` apply or a `For` row
// runs in place is placed where that code runs, never by the deferred role
// the callback's lexical scope would lend it.
import {
  For,
  createEffect,
  createMemo,
  createRenderEffect,
  createSignal,
  createStore,
  refresh,
  untrack,
} from "solid-js";
import * as S from "solid-js";

const { refresh: refreshD } = S;

function h1(cb: () => unknown) {
  cb();
  return 1;
}

// Uncertifiable: the apply's first run is synchronous under the component's
// owner (rc.17 throws there), but only when the compute settles on its first
// pass outside a lane, which is not proven; later runs are legal.
export function RenderApplyClosure() {
  const [, setStore] = createStore({ v: 0 });
  createRenderEffect(
    () => 1,
    () => {
      const f1 = () => {
        setStore((state) => {
          state.v = 7;
        });
      };
      f1();
    },
  );
  return <div />;
}

export function RenderApplyIife() {
  const m1 = createMemo(() => 1);
  createRenderEffect(
    () => 1,
    () => {
      (() => {
        refresh(m1);
      })();
    },
  );
  return <div />;
}

export function RenderApplyHelper() {
  const [, setW1] = createSignal(0);
  createRenderEffect(
    () => 1,
    () => {
      h1(() => {
        setW1(7);
      });
    },
  );
  return <div />;
}

export function RenderApplyDestructured() {
  const m1 = createMemo(() => 1);
  createRenderEffect(
    () => 1,
    () => {
      (() => {
        refreshD(m1);
      })();
    },
  );
  return <div />;
}

// Negative: a `createEffect` apply runs from the effect queue, with no owner.
export function EffectApplyClosure() {
  const [, setW1] = createSignal(0);
  createEffect(
    () => 1,
    () => {
      const f1 = () => {
        setW1(7);
      };
      f1();
    },
  );
  return <div />;
}

// Violations: the row's JSX child is a render computation (owned), and every
// layer between it and the write runs in place.
export function RowIife() {
  const m1 = createMemo(() => 1);
  return (
    <ul>
      <For each={[1]}>
        {() => (
          <li>
            {(() => {
              refresh(m1);
              return 1;
            })()}
          </li>
        )}
      </For>
    </ul>
  );
}

export function RowHelper() {
  const [, setW1] = createSignal(0);
  return (
    <ul>
      <For each={[1]}>
        {() => (
          <li>
            {(() => {
              h1(() => {
                setW1(7);
              });
              return 1;
            })()}
          </li>
        )}
      </For>
    </ul>
  );
}

export function RowNamespace() {
  const m1 = createMemo(() => 1);
  return (
    <ul>
      <For each={[1]}>
        {() => (
          <li>
            {(() => {
              S.refresh(m1);
              return 1;
            })()}
          </li>
        )}
      </For>
    </ul>
  );
}

export function RowDestructured() {
  const m1 = createMemo(() => 1);
  return (
    <ul>
      <For each={[1]}>
        {() => (
          <li>
            {(() => {
              refreshD(m1);
              return 1;
            })()}
          </li>
        )}
      </For>
    </ul>
  );
}

export function RowStore() {
  const [, setStore] = createStore({ v: 0 });
  return (
    <ul>
      <For each={[1]}>
        {() => (
          <li>
            {(() => {
              setStore((state) => {
                state.v = 7;
              });
              return 1;
            })()}
          </li>
        )}
      </For>
    </ul>
  );
}

// `untrack` keeps the render computation's owner.
export function RowUntrack() {
  const [, setW1] = createSignal(0);
  return (
    <ul>
      <For each={[1]}>
        {() => (
          <li>
            {(() => {
              untrack(() => setW1(7));
              return 1;
            })()}
          </li>
        )}
      </For>
    </ul>
  );
}

// A named closure the row body calls runs under the row's owner.
export function RowNamedClosure() {
  const [, setW1] = createSignal(0);
  return (
    <ul>
      <For each={[1]}>
        {() => {
          const f = () => {
            setW1(7);
          };
          f();
          return <li>x</li>;
        }}
      </For>
    </ul>
  );
}

// Negatives: an event handler, a timer and an effect apply in a row run with
// no owner.
export function RowHandler() {
  const [, setW1] = createSignal(0);
  return (
    <ul>
      <For each={[1]}>
        {() => (
          <li
            onClick={() => {
              (() => setW1(7))();
            }}
          >
            x
          </li>
        )}
      </For>
    </ul>
  );
}

export function RowTimer() {
  const [, setW1] = createSignal(0);
  return (
    <ul>
      <For each={[1]}>
        {() => (
          <li>
            {(() => {
              setTimeout(() => setW1(7), 0);
              return 1;
            })()}
          </li>
        )}
      </For>
    </ul>
  );
}

export function RowEffect() {
  const [, setW1] = createSignal(0);
  return (
    <ul>
      <For each={[1]}>
        {() => {
          createEffect(
            () => 1,
            () => {
              (() => setW1(7))();
            },
          );
          return <li>x</li>;
        }}
      </For>
    </ul>
  );
}
