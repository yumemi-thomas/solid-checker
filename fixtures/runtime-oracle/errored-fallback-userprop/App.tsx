import { createMemo, createSignal, onCleanup } from "solid-js";

function Guard(props: { fallback: () => unknown; children?: any }) {
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
    <Guard
      fallback={() => {
        onCleanup(() => {});
        return <p id="fb">fallback</p>;
      }}
    >
      <p>{label()}</p>
    </Guard>
  );
}
