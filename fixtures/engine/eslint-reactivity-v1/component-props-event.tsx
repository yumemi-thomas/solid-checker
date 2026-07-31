function Button(props: { label: string }) {
  return <button onClick={() => console.log(props.label)}>{props.label}</button>;
}
