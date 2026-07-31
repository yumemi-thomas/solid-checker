function Card(props: { title: string }) {
  const alias = props;
  const title = alias.title;
  return <h1>{title}</h1>;
}

export { Card };
