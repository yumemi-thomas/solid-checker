function Card(props: { title: string }) {
  setTimeout(() => console.log(props.title), 0);
  return <div />;
}
