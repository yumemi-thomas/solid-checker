function merge<A, B>(first: A, second: B): A & B {
  return { ...first, ...second };
}

function Card(props: { title?: string }) {
  const merged = merge({ title: "Untitled" }, props);
  const title = merged.title;
  return <h1>{title}</h1>;
}

export { Card };
