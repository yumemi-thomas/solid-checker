import { Errored, createMemo, createSignal } from "solid-js";

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
        fallback={(error, reset) => (
          <button
            type="button"
            id="retry"
            onClick={() => {
              setFail(false);
              reset();
            }}
          >
            {`retry ${String(error())}`}
          </button>
        )}
      >
        <Child />
      </Errored>
    </div>
  );
}
