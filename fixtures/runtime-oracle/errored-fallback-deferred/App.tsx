import { Errored, createMemo, createSignal, onCleanup } from "solid-js";

function release() {
  onCleanup(() => {});
}

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
          setTimeout(release, 0);
          const run = () => {
            onCleanup(() => {});
          };
          Promise.resolve().then(run);
          return <p id="fb">fallback</p>;
        }}
      >
        <Child />
      </Errored>
    </div>
  );
}
