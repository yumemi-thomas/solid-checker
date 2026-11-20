import { Errored, createEffect, createMemo, createSignal, onCleanup } from "solid-js";

let later: (() => void) | undefined;

export function Probe() {
  const [fail, setFail] = createSignal(false);
  const [go, setGo] = createSignal(0);
  function Child() {
    const content = createMemo(() => {
      if (fail()) throw new Error("boom");
      return "content";
    });
    return <p>{content()}</p>;
  }
  createEffect(
    () => go(),
    () => {
      later?.();
    },
  );
  return (
    <div>
      <button type="button" id="fail" onClick={() => setFail(true)}>
        fail
      </button>
      <button type="button" id="go" onClick={() => setGo((n) => n + 1)}>
        go
      </button>
      <Errored
        fallback={function show() {
          later = () => {
            onCleanup(() => {});
          };
          return <p id="fb">fallback</p>;
        }}
      >
        <Child />
      </Errored>
    </div>
  );
}
