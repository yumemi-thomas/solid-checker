import { createSignal, Show, For, Match, Switch, Repeat } from "solid-js";
function useBranch(render: () => any) {
  return () => <Show when={true}>{(_value) => render()}</Show>;
}
export function C() {
  const [count] = createSignal(0);
  const B = useBranch(() => { console.log(count()); return null; });
  return <B />;
}
