import { mergeProps } from "solid-js";

function Card(props: { title?: string }) {
  const merged = mergeProps({ title: "Untitled" }, props);
  const title = merged.title;
  return <h1>{title}</h1>;
}

export { Card };
