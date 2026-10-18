import { createSignal } from "solid-js";

type P = { a?: () => void; b?: () => void; c?: () => void; depth: number };

function Branch(props: P) {
  const f = props.c;
  if (f) f();
  return (
    <div onClick={props.a}>
      {props.depth > 0 && <Branch b={props.a} c={props.b} depth={props.depth - 1} />}
    </div>
  );
}

export function App() {
  const [count] = createSignal(0);
  return <Branch a={() => console.log(count())} depth={3} />;
}
