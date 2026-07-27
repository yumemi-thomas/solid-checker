function Card(props: { hidden: boolean }) {
  if (props.hidden) return null;
  return <h1 />;
}

export { Card };
