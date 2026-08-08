const isServer = true;

function Card(props: { title: string }) {
  if (isServer) return null;
  return <h1 />;
}

export { Card };
