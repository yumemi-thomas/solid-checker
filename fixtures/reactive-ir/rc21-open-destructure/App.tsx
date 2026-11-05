import { createSignal, createStore, merge } from "solid-js";

function Second({ a }: { a: number }, z = 1) {
  return <i>{a + z}</i>;
}

function Forms(props: { a: number }) {
  let a = 0;
  ({ a } = props);
  const [{ a: b }] = [props];
  const { a: c } = merge({ a: 2 }, props);
  for (const { a: d } of [props]) {
    void d;
  }
  return <i>{a + b + c}</i>;
}

function Default(props: { a: number }, { a } = props) {
  return <i>{a}</i>;
}

const useStore = () => createStore({ a: 1 })[0];
const useLiteral = () => ({ a: 1 });
function Hooks(props: { a: number }) {
  const { a } = useStore();
  const { a: b } = useLiteral();
  const [c] = createSignal(props.a);
  return <i>{a + b + c()}</i>;
}

export const App = () => [Second, Forms, Default, Hooks];
