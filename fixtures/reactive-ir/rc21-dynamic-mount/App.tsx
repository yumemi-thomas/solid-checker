import { createMemo, createSignal, Loading } from "solid-js";
import { Dynamic, render } from "@solidjs/web";

const [on] = createSignal(true);

function Exclusive() {
  const memo = createMemo(async () => 1);
  const value = memo();
  return <p>{value}</p>;
}

function Aliased() {
  const memo = createMemo(async () => 1);
  const value = memo();
  return <p>{value}</p>;
}
const Alias = Aliased;

function Mixed() {
  const memo = createMemo(async () => 1);
  const value = memo();
  return <p>{value}</p>;
}

function Tag() {
  const memo = createMemo(async () => 1);
  const value = memo();
  return <p>{value}</p>;
}

render(
  () => (
    <Loading fallback="x">
      <Dynamic component={Exclusive} />
      <Dynamic component={Alias} />
      <Dynamic component={on() ? Mixed : "p"} />
      <Tag />
    </Loading>
  ),
  document.body,
);
