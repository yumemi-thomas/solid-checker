import { createMemo, createSignal, onCleanup } from "solid-js";

function Errored(props: { fallback: () => unknown; children?: any }) {
  return (
    <div>
      {props.children}
      <button
        type="button"
        id="show"
        onClick={() => {
          props.fallback();
        }}
      >
        show
      </button>
    </div>
  );
}

export function Probe() {
  const [n] = createSignal(0);
  const label = createMemo(() => `n=${n()}`);
  return (
    <Errored
      fallback={() => {
        onCleanup(() => {});
        return <p id="fb">fallback</p>;
      }}
    >
      <p>{label()}</p>
    </Errored>
  );
}
