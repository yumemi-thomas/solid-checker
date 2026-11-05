import { createMemo, createSignal, Show } from "solid-js";
import { render } from "@solidjs/web";

const [on] = createSignal(false);

function Proven() {
  const memo = createMemo(async () => 1);
  return <p>{memo()}</p>;
}

function Later() {
  const memo = createMemo(async () => 1);
  return <p>{memo()}</p>;
}

function Arms(props: { flag: boolean }) {
  const memo = createMemo(async () => 1);
  return (
    <>
      <Show when={on()}><p>{memo()}</p></Show>
      {props.flag ? <p>{memo()}</p> : null}
      {props.flag && <p>{memo()}</p>}
    </>
  );
}

render(
  () => (
    <div>
      <Proven />
      <Show when={on()}><Later /></Show>
      <Arms flag={true} />
    </div>
  ),
  document.body,
);
