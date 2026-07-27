import { merge } from "solid-js";

function Card(props: { title?: string }) {
  const merged = merge({ title: "Untitled" }, props);
  const title = merged.title;
  return <h1>{title}</h1>;
}

export { Card };
