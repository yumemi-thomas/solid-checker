import { Errored, createMemo, createSignal } from "solid-js";

export function Probe() {
  const [fail, setFail] = createSignal(false);
  const [count, setCount] = createSignal(0);
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
      <Errored fallback={<p id="outer">outer fallback</p>}>
        <Errored
          fallback={() => {
            setCount(1);
            return <p id="fb">{`fallback ${count()}`}</p>;
          }}
        >
          <Child />
        </Errored>
      </Errored>
    </div>
  );
}
