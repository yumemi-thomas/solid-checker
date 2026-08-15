import { createSignal } from "solid-js";

// Not exported: every call site is enumerable, so whether props.title is
// signal-backed is decided entirely by what Root passes.
function Card(props: { title: string }) {
  const title = props.title;
  return <div title={title} />;
}

export function Root() {
  const [label] = createSignal("live");
  console.log(label);
  return <Card title="static" />;
}
