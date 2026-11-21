import { Errored, createContext, createEffect, createMemo, createSignal, onCleanup, useContext } from "solid-js";

const Label = createContext("none");
let runs = 0;

export function Probe() {
  const [fail, setFail] = createSignal(false);
  const [tick, setTick] = createSignal(0);
  function Child() {
    const content = createMemo(() => {
      if (fail()) throw new Error("boom");
      return "content";
    });
    return <p>{content()}</p>;
  }
  return (
    <Label value="provided">
      <button type="button" id="fail" onClick={() => setFail(true)}>
        fail
      </button>
      <button type="button" id="tick" onClick={() => setTick((n) => n + 1)}>
        tick
      </button>
      <Errored
        fallback={(error) => {
          runs += 1;
          const seen = tick();
          const label = useContext(Label);
          onCleanup(() => {});
          const doubled = createMemo(() => seen * 2);
          createEffect(
            () => doubled(),
            () => {},
          );
          return <p id="fb">{`${label}:${runs}:${seen}:${doubled()}:${typeof error}`}</p>;
        }}
      >
        <Child />
      </Errored>
    </Label>
  );
}
