import {
  createEffect,
  createMemo,
  createRenderEffect,
  createSignal,
  onMount,
  type Component,
} from "solid-js";

const [count, setCount] = createSignal(0);

function plainHelper() {
  setCount(6);
}

const App: Component = () => {
  // Component setup is owned but one-shot, not tracked in Solid 1.x.
  setCount(1);
  plainHelper();
  onMount(() => setCount(5));

  // Solid 1.x SC2001 is restricted to tracked execution. These three compute
  // callbacks and the JSX expression below can re-trigger the graph that is
  // currently running, so every setter call is a proven violation.
  createMemo(() => {
    setCount(2);
    return count();
  });
  createEffect(() => {
    setCount(3);
    count();
  });
  createRenderEffect(() => {
    setCount(4);
    count();
  });

  return (
    <button onClick={() => setCount(7)}>
      {setCount(8) && count()}
    </button>
  );
};

void <App />;
