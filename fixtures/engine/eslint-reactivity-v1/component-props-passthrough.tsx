declare function consume(value: object): void;

function Card(props: { title: string }) {
  consume(props);
  return <h1 />;
}

export { Card };
