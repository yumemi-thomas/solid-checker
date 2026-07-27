import { createMemo } from "solid-js";

function Card(props: { title: string }) {
  createMemo(() => props.title);
  return <h1 />;
}

export { Card };
