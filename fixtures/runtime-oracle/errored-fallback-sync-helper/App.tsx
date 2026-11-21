import { Errored, createMemo, createSignal, onCleanup } from "solid-js";

const helper = () => {
  onCleanup(() => {});
};

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
          helper();
          return <p id="fb">fallback</p>;
        }}
      >
        <Child />
      </Errored>
    </div>
  );
}
