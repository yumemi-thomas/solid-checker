import { createSignal } from "solid-js";

function EvalHandler(props: { onPress: () => void }) {
  const handler = () => props.onPress();
  eval("handler()");
  return <button onClick={handler} />;
}

export function UsesEvalHandler() {
  const [count] = createSignal(0);
  return <EvalHandler onPress={() => { count(); }} />;
}
