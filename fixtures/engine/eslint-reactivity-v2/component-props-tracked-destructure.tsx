import { createMemo } from "solid-js";

function Card(props: { title: string }) {
  const title = createMemo(() => {
    const { title } = props;
    return title;
  });
  return <h1>{title()}</h1>;
}

export { Card };
