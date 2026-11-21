import { Errored, createEffect, createMemo, createSignal } from "solid-js";

export function Probe() {
  const [fail, setFail] = createSignal(false);
  function Child() {
    const content = createMemo(() => {
      if (fail()) throw new Error("boom");
      return "content";
    });
    return <p>{content()}</p>;
  }
  return (
    <div>
      <button type="button" id="fail" onClick={() => setFail(true)}>
        fail
      </button>
      <Errored
        fallback={() => {
          createEffect(
            () => 0,
            () => {
              createMemo(() => 1);
            },
          );
          return <p id="fb">fallback</p>;
        }}
      >
        <Child />
      </Errored>
    </div>
  );
}
