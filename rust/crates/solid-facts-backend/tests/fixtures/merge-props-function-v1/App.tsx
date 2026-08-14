import { createSignal, mergeProps } from "solid-js";

export function Panel(props: { label: string }) {
  const [count] = createSignal(0);
  const merged = mergeProps(props, () => ({
    summary: `${props.label}: ${count()}`,
  }));
  return <div>{merged.summary}</div>;
}
